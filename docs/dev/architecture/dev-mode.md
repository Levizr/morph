# Dev Mode: Hot Reload Without the Heart Attack

**Part of:** [Dev Docs](overview.md)

`morph dev` is the command you will run a thousand times, so it had better feel like magic and behave like engineering. It does both: the same in-process pipeline from [The Compiler Pipeline](compiler-pipeline.md) runs on every save, but instead of producing a standalone binary, it hot-swaps fresh logic into a living window over a loopback TCP connection. The window, the GL context, and the layout tree stay alive. Only your mistakes get replaced.

## The five moving parts

| Part | Lives in | Job |
|---|---|---|
| The watcher | `crates/morph-build/src/dev.rs`, driven by `crates/morphc/src/commands/dev.rs` | Notices you saved, debounces, kicks the pipeline |
| The dev renderer | `morph_devrt`, built from `runtime/cpp/dev/` via CMake in `crates/morph-build/src/devrt.rs` | A prebuilt native window that knows how to receive new brains |
| The logic library | `crates/morph-build/src/logic.rs` | Your app logic compiled per-change to a shared object (`g++ -shared`) |
| The pipe | `crates/morph-build/src/ipc.rs` + `runtime/cpp/dev/dev_socket.h` | Loopback TCP (`127.0.0.1:39573`, ephemeral fallback on collision), buffered protocol |
| The new brain format | `crates/morph-ir/src/serializer.rs` + `runtime/cpp/dev/ir_deserializer.h` + `json_parser.h` | Serialized IR document the dev runtime parses and installs |

## The sequence, slowly

**1. Boot.** `morph dev` ensures the runtime is installed, then builds `morph_devrt` via CMake — but only if its source hash changed, so the second launch is fast. The dev renderer launches and announces its IPC address. This is the last restart you will see for a while.

**2. Watch.** Source directories are watched with `notify`, debounced at 100ms. The debounce exists because editors love saving files three times in a row and version control loves touching everything at once. One calm rebuild beats three panicked ones.

**3. Recompile.** On change: parse → CSS → IR → emit, exactly like a build. Then the logic is compiled to a shared library (`g++ -shared`) instead of linked into a standalone binary. Compiling only the logic is the whole trick — it is small, so it is fast.

**4. Push.** The serialized IR document travels over loopback TCP to `morph_devrt`. Loopback means it never leaves your machine; the ephemeral-port fallback means two `morph dev` sessions on one box do not fight over `39573`.

**5. Swap.** The running window installs the new IR document and rewires logic without restarting. State signals are preserved through the swap via the signal store (`runtime/cpp/dev/signal_store.h`), nodes are matched through the node registry (`node_registry.h`), and `morph_logic_rewire` re-registers every subscription. Because re-registration runs on *every* reload, it calls `clear_channels()` first (see [The Reactivity Engine](../runtime/reactivity-engine.md)) — otherwise one emit would fire N handlers after N reloads, and your counter would develop a caffeine problem.

## The dev-runtime supporting cast

The `runtime/cpp/dev/` directory is the part of the runtime that production binaries never see:

| Header | Role |
|---|---|
| `dev_socket.h` | The buffered socket protocol — framing, partial reads, the unglamorous plumbing that keeps the pipe from starving |
| `json_parser.h` | Parses the incoming IR document |
| `ir_deserializer.h` | Turns the parsed document into live nodes |
| `node_registry.h` | Matches new nodes to existing ones so state and focus survive the swap |
| `signal_store.h` | Preserves signal values across rewires |
| `logic_prelude.h` | What the hot-loaded logic expects to find on arrival |
| `inspector.h`, `dev_log.h`, `dev_net.h` | DevTools support: element inspection, log ring buffers, network request logging |

`morph_api.h` is the single public entry header the logic plugins build against — the front door with the welcome mat.

## What survives a reload, and what doesn't

- **Survives:** the window, the GL context, the layout tree, signal values (via the signal store), and your dignity.
- **Rebuilt:** the logic shared object, the IR document, every event subscription (cleared, then re-registered).
- **Reset:** anything you only set up in startup-only code paths. If a value mysteriously returns to its initial state on every save, it lives somewhere the rewire path does not replay — that is your bug, and now you know where to look.

## Failure modes, translated

| Symptom | Likely cause | Where to look |
|---|---|---|
| Change never appears | Watcher missed it, or the debounce ate a multi-save burst | `dev.rs` watch roots and debounce; check the CLI log for a rebuild line |
| Window restarts instead of swapping | Dev renderer rebuilt (source hash changed) or IPC connect failed | `devrt.rs` hash check; `ipc.rs` address announcement |
| Counter jumps by N after N saves | Subscriptions stacking — the `clear_channels()` guard regressed | Rewire path; see the FAQ in [State & Event Internals](../state/state-events-internals.md) |
| Crash right after swap, referencing focus/capture | Use-after-free of `s_focusedNode` / `s_mouseCapture` across the swap | `core/node.h` statics; the dtor clearing logic |
| Styles update but logic doesn't (or vice versa) | IR pushed but logic `.so` failed to compile, or the reverse | `logic.rs` compile output — read the actual `g++` error, it is usually honest |

## Verify by

```bash
# From any fixture project: delete the stale binary first so fingerprinting
# doesn't skip the rebuild, then build without UPX for speed.
rm -f .morph/output/<name>*
<repo>/target/debug/morph build --no-upx
<binary> --morph-self-test     # must report 0 failures
./tests/runtime/run-selftests.sh   # the full fixture sweep, from the repo root
```

Dev-mode changes deserve the same sweep: edit, save, watch the window update, then run the self-tests. Hot reload that only works when nobody checks is not hot reload — it is a demo.
