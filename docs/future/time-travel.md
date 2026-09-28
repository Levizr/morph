# Time-Travel Debugger — Rewind Your Whole App

**Status:** future · **Priority:** medium · **Depends on:** [State/Events/Native](state-events-native-interop.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

An undo button for your entire app — every window included. Because all state transitions in Morph flow through exactly two typed choke points (signal `set()` and channel `emit()`), the dev runtime can record them with window IDs and replay any session deterministically. Other frameworks record pixel video of your bug; this records the bug itself.

## Why it matters

- **Multi-window bugs are un-reproducible by hand** — "click here in window A, then quickly there in window B" is not a bug report, it's a choreography. A recording replays it exactly, every time
- **Late subscribers stop being a mystery** — scrub back and watch *which* emit a component missed and *what* the store held at that moment
- **Bisect by scrubbing** — drag the timeline until the value goes wrong; the frame under your cursor names the setter call that did it
- **Recordings are shareable** — attach a 50 KB event log to a bug report instead of a 50 MB screen recording (your issue tracker will send flowers)

## How it will work

Dev mode keeps a ring buffer of transitions (bounded, e.g. last 10,000):

```
t=1042  W2  set  shop/cart.ts::count      3 → 4
t=1043  W2  emit evt:toast.ts::toastEvent  {message: "Added"}
t=1044  W1  set  shop/cart.ts::count      4 → 4   (synced copy catches up)
```

The DevTools timeline renders this as a scrubbable strip per window, with:

- **`morph dev --record session.mlog`** — save the log to disk; `morph replay session.mlog` replays headlessly and asserts the final state matches (instant regression test from a real session)
- **Breakpoints on identity** — pause when `cart.ts::count` is set, or when *any* emit crosses windows
- **State snapshots** — diff the whole app (all windows) between two ticks; the diff names the one setter responsible
- **Privacy default** — recording is dev-mode only, never shipped; the buffer holds values, so it never leaves the machine unless you export it

## Current state

| Piece | State |
|---|---|
| Dev-mode `SignalStore` (signals survive hot reload by name) | ✅ Shipped |
| Hot-reload rewire (`morph_logic_rewire`, channel clear + re-register) | ✅ Shipped |
| DevTools shell (logs, network tabs) | ✅ Shipped |
| Transition ring buffer + timeline UI | ❌ Not built |
| `morph replay` headless assertions | ❌ Not built |
| Identity breakpoints + snapshot diff | ❌ Not built |

## Open questions

- **Nondeterminism** — `fetch()` resolutions, timers, and random values break pure replay; record-and-stub them (like a VCR) or mark replay "best effort" past I/O?
- **Buffer bounds** — fixed 10k transitions, time-based, or memory-based? (Somebody's cart *will* have 100k rapid sets.)
- **Snapshot cost** — full-app snapshots every tick vs snapshots on demand + event-sourced rewind between them?
- **Replay vs real backend** — replaying against a live server double-submits orders; does replay mode auto-stub `fetch()`?

## Build steps (when picked up)

1. Transition ring buffer in dev runtime (`set`/`emit` tap points — two call sites, that's the whole hook surface)
2. Timeline strip in DevTools (per-window lanes, scrub + inspect)
3. Snapshot + diff on demand
4. `morph replay` with final-state assertions (feeds [Test Runner](test-runner.md))
5. Identity breakpoints
6. Validation: reproduce a real two-window sync bug from a recording alone
