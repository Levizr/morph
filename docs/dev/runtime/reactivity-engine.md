# The Reactivity Engine: Signals That Actually Signal

**Part of:** [Dev Docs](../architecture/overview.md)

Every `morphState` counter, every `morphEffect` cleanup, every `await fetch()` in every Morph app runs on the same small engine in `runtime/cpp/reactivity/`: signals with auto-subscription, an effect pool with pending queues, eager C++20 coroutines for async, and a pub/sub channel registry for events. No virtual DOM, no diffing, no framework ceremony — just values that know who is watching them. This page explains each gear, the threading contract it lives under, and the two famous footguns (double-fire subscriptions and forgotten channel clears).

For how `morphState` / `morphShared` / `morphEvent` travel from source to C++, see [State & Event Internals](../state/state-events-internals.md). That page is the journey; this page is the engine room.

## Gear 1 — Signals (`reactivity/signal.h`)

`Signal<T>` is embarrassingly simple, which is the point:

- **`get()` auto-subscribes the current effect.** There is a `thread_local EffectContext g_effect_ctx` recording which effect (if any) is currently running. Read a signal inside an effect, and the effect is now subscribed. No decorator, no dependency array, no manual `subscribe()` — the dependency graph builds itself out of reads.
- **`set()` notifies.** Subscribers are stored on `SignalBase` (with a mutex — signals can be `set()` from worker threads, e.g. the fetch thread or a native-interop `std::thread` updating state behind a mutex).

```cpp
Signal<int> count;
create_effect([&]{ render(count.get()); });  // get() subscribes this effect
count.set(1);                                 // set() re-runs it
```

Helpers `str()` / `fmt_double` keep UI printing sane (`8` not `8.000000`, `2.5` not `2.500000`, `Error` for NaN/inf). Small function, enormous demo-day value.

**Concept to pocket:** dependency tracking by *observation*, not declaration. The framework never asks what you depend on — it watches what you touch. This is why effects can't lie about their dependencies: the subscription list is a transcript of actual reads.

## Gear 2 — Effects (`reactivity/effect.cpp`)

Effects live in a pool with a pending queue. `EffectNode` carries the function, its cleanup, and flags (`pending`, `dead`, `deps`). The protocol:

1. `create_effect(fn)` registers the effect.
2. `notify_all` (triggered by `set()`) enqueues — it does not run immediately.
3. `run_pending_effects()` flushes the queue **once per frame**, running each effect's cleanup first, then the effect.

Cleanup-first ordering is what makes `morphEffect`'s return-a-cleanup-function contract work: the old subscription/timer/listener dies before the new one is born. The once-per-frame flush is what keeps a burst of ten `set()` calls from running your effect ten times — it runs once, with the latest values. Batching isn't an optimization here; it's semantics.

## Gear 3 — Coroutines: `Task`, timers, and `Result` (`reactivity/task.h`, `task.cpp`, `promise.h`)

`async`/`await` in your `.mx` file becomes real C++20 coroutines:

- **`morph::Task`** is an *eager* coroutine (`suspend_never` initial suspension — it starts running immediately, no awkward "did you remember to start it" phase). A `next_frame` awaiter parks a coroutine until the next frame; `schedule_coroutine` enqueues; `process_tasks()` drains pending coroutines once per frame, from the same heartbeat that flushes effects.
- **Timers** (`set_timeout` / `set_interval` / `clear_timer`) fire from `process_tasks()` too. One pump drives coroutines and timers alike — the frame loop is the event loop.
- **`morph::Result<T>`** is the JS `Promise` / async-function return type: also eager, storing a `std::optional` value plus an `exception_ptr`. Awaitable via `operator co_await`; introspectable via `resolved()` / `pending()`; printable as `Promise { <pending> | <rejected> | value }` through a `std::formatter`.

The one-frame heartbeat (flush effects, drain coroutines, fire timers) is the engine's central rhythm. Everything reactive in Morph happens on this beat — which is also why everything reactive is deterministic enough to test with `--morph-self-test`.

## Gear 4 — Channels (`reactivity/channel.h`)

`morphEvent` channels are pub/sub with a registry:

- `emit` publishes a value to every subscriber; `on` subscribes; `off` unsubscribes.
- The registry is mutex-guarded (emits can come from any thread).
- **`clear_channels()` exists for hot reload.** `morph_logic_rewire` re-runs on every dev-mode reload and re-registers every subscription — without the clear, each reload stacks another copy of every handler and one emit fires N times after N saves. Startup registration has nothing to clear, so the call is guarded on `channel_subs` being non-empty.

That last paragraph has caused more déjà vu than any other bug in the codebase: if event handlers ever double-fire, check the rewire/clear boundary *first*. The contract (from [State & Event Internals](../state/state-events-internals.md)) is that the IR layer produces `{channel, body: lambda}` pairs and exactly one site — the emitter — adds the `.on(...)` wrap. A doubled `.on(` means somebody broke the contract, not that channels are haunted.

## The threading contract, in one table

| Who | Runs where | May do what |
|---|---|---|
| Main thread | Event/style/layout/paint, effect flush, coroutine drain | Everything UI |
| Compositor thread | Vsync, GL presentation, CPU-side animation interpolation | Write only `animOffsetX/Y`, `animOpacity`, color/radius fields; never touch the node tree |
| Net worker threads | Blocking HTTP per `fetch()` | Do the request, report to DevTools, then `resume()` the awaiting coroutine — never touch UI state directly |
| Your `std::thread` (native interop) | Wherever you spawned it | `set()` signals (mutex-protected); never touch nodes |

Signals are the thread-safe seam: anything may `set()` a signal, and the main-thread heartbeat turns it into UI. Nodes are *not* thread-safe — all node access stays on main. Respect the seam and every thread lives; cross it and you get the exciting kind of crash.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Change subscription semantics | `signal.h` (`SignalBase`, `Signal<T>::get/set`) |
| Change effect scheduling or cleanup order | `effect.cpp` (pool, pending queue, `run_pending_effects`) |
| Change coroutine/timer behavior | `task.h` / `task.cpp` (`schedule_coroutine`, `process_tasks`) |
| Change promise semantics | `promise.h` (`Result<T>`) |
| Change event delivery | `channel.h` — and keep the rewire `clear_channels()` contract intact |

## Verify by

```bash
cargo test --workspace
<binary> --morph-self-test     # must report 0 failures
./tests/runtime/run-selftests.sh
```

Reactivity changes deserve the `component-test` fixture (cross-file shared state + event subscribe/emit end to end) and the `list-test` fixture (keyed reconciliation + effects). If those pass with `0 failures`, the engine room is in order.
