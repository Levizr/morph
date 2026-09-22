# Lean binaries: only what you use, budgeted at 150KB

**Status:** active · **Priority:** high

> Core promise: a Morph binary contains exactly what the app uses —
> nothing else. A hello-world app (single `h1`) must never exceed
> **150KB without UPX** (a 120KB no-UPX binary was achieved before, so
> the budget is proven, not aspirational). UPX size is reported
> separately as a bonus, never counted toward the budget.

## Budget enforcement

- `tests/runtime/hello-size/` — single-`h1` fixture, the measuring stick.
- `tests/runtime/check-size.sh` — builds it *without* `--self-test`,
  asserts ≤153600 bytes, prints file bytes + UPX size. Last gate before
  every lean commit.
- Every lean commit message carries a per-step file-byte report
  (file-manager bytes, not sections).

## Per-app optimization level (`-Os` vs `-O2`)

No global default — the feature scan already knows whether the app
needs performance:

- **`-Os`**: no `animation`, `transform`, `forge`, or `list` anywhere.
  Static content pays nothing to be made "faster".
- **`-O2`**: any of those present (unrolling/inlining pay off there).

Derived in `morph-build` from the defines list it already receives
(zero new plumbing). Static mode keeps its `-Os` path. No manual knob
(a knob users set wrong is worse than a scan that never misclassifies).

## Flag catalog (all derived from scan, never user-set)

| Flag | Detected from | Gates |
|---|---|---|
| `MORPH_FEATURE_REACTIVITY` | state/shared/event/effect/channel/mid decls, reactive bindings, conditionals, lists, animations | `effect.cpp` TU skip + whole-file guard; template omits `run_pending_effects()` / `destroy_all_effects()` |
| `MORPH_FEATURE_TASKS` | timers, `async`/`await`/`Promise`, `fetch` (implied) | `task.cpp` TU skip + guard; template omits `process_tasks()` |
| `MORPH_FEATURE_NET` | `fetch(` (implies `TASKS`) | `net.cpp` TU skip + whole-file guard |
| `MORPH_FEATURE_OWNERSHIP` | `parent`/`modal`/`role` in opts, route `windowConfig`, `[window]` defaults | Follow chain (gated pos-callback registration lets GC drop it); close/create plain paths when off (zero popup bytes) |
| `MORPH_FEATURE_PAGECACHE` | `navigation.cache != 0` (config, no scan) | `navigate` plain destroy path when `0` |
| `MORPH_FEATURE_EVENT` (new) | node `events` non-empty | `events.cpp` TU skip; mouse-button callback registration |
| existing CSS flags | unchanged | unchanged |

Deliberately **not** gated: window system (every app opens a window),
`JsValue`/types (props flow through them), header-only code (zero bytes
when unused), dev builds (all-on rule stands).

## Mechanics (all four layers, like CSS)

1. **Detect** — `FeatureSet::scan` (IR decls, reactive markers, body
   markers) + `note_*` for routes/config + `build.rs` additions.
2. **Emit** — `required_defines()` arms; template `{% if %}` for loop
   calls, `channel.h`, `setColor`; codegen plain-path branches
   (`navigate`, `close`, `create`); mount `MountScope`/teardown gating.
3. **Build** — `runtime_sources_with_features` skips TUs by defines;
   `strip --strip-unneeded` every release link (~77KB of symtab on
   hello-size); `morph build --self-test` opts the test body back in
   (default lean; `run-selftests.sh` passes the flag).
4. **Runtime** — whole-file `#ifdef`s (animation.cpp precedent);
   unconditional-trivial API stays (3-line setters cost nothing and
   keep generated code compilable); window.cpp callback registrations
   follow `input`/`event`/hover features so bodies GC-drop.

## Measurement log

| Step | hello-size file bytes |
|---|---|
| Baseline (self-test in binary, symtab) | 371,100 |
| `--self-test` opt-in (default lean) | 305,100 |
| `strip` on every release link | 227,700 |
| Template branches + TU skips + header gating | 162,100 |
| Callback registration gating | 153,900 |
| Forge backend skip | 145,700 |
| `-Oz` + `-flto` + `-fno-exceptions/-rtti/-unwind` + regex-header split | 112,800 |
| Static: HarfBuzz skipped (no shaping content) | 1,049,800 → 560,500 |
| Static: GLFW gamepad DB stubbed + unwind-free dep builds + static `-Oz` | → **397,500** |
| Static UPX | **~177,000** |

## Open experiments (decide with data, in order)

1. `-Os`/`-O2` conditional rule + `-fno-exceptions` try (keep iff the
   hello link succeeds and tests stay green).
2. Unused-backend TU skip (`forge.cpp` unless chosen; verify flash
   isn't load-bearing under forge).
3. `logic_prelude.h` only with `startup_logs`/native code;
   `windowConfig` JsObject only with native code.
4. `glad.c` slimming — almost certainly not worth it (correctness
   minefield for ~8KB); listed so nobody re-investigates.
