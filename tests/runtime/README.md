# Runtime fixtures

Full-app `.mx` projects used as visual/contract tests for the runtime. Every
directory here is a real Morph project — build it with
`morph build --no-upx` and run the resulting binary with `--morph-self-test`
(no display needed; assertions run before GLFW init).

## Layout

Fixtures are grouped by the subsystem they hold accountable:

| Group | Holds |
|---|---|
| `app/` | The app spine — components + cross-file state, C++ interop, windows, routing |
| `render/` | Paint order and visual properties — culling, opacity, transform, z-index, hover |
| `border/` | Border rendering — styles, per-side widths/colors, radius, Chrome parity |
| `layout/` | Flex and full-UI layout breadth |
| `reactivity/` | Keyed reconciliation and effects |
| `widgets/` | Element behavior — `<input>`, the DOM event surface |
| `animation/` | CSS keyframes |
| `size/` | Lean-binary budget fixture (gated by `check-size.sh`) |
| `scratch/` | Throwaway repros. Not gated, not reviewed |

Put a new fixture in the group matching what broke. A one-off repro goes in
`scratch/`; if it earns its keep, promote it into a real group.

## Scripts

```bash
./tests/runtime/run-selftests.sh   # build + run --morph-self-test on app/ fixtures
./tests/runtime/check-size.sh      # assert size/hello-size stays under the byte budget
./tests/runtime/check-forge-math.sh  # forge DamageSet/TilePool/scroll-shift unit tests (g++-14, headless)
./tests/runtime/bench-forge.sh     # forge damage-model benchmarks + TU-exclusion nm checks
./tests/runtime/check-forge-pixels.sh  # headless EGL raster verification: real renderNode culling, scroll-shift, exclusion (skips green without EGL)
```

Run both from the repo root. `run-selftests.sh` greps for `0 failures`; any
fixture that stops reporting that is a P0.

## Adding a fixture

1. `mkdir -p tests/runtime/<group>/<name>` with a `morph.config.json` and
   `src/App.mx`.
2. Name the config after the directory. Two deliberate exceptions exist
   (`render/transform-test` → `"trnasform-test"`, `layout/ui-test` → `"test"`)
   — leave them, they are load-bearing for existing grep muscle memory.
3. Put `--morph-self-test` assertions in the app if the fixture should be
   gated, and add it to `run-selftests.sh` only if it is a true app-spine
   regression. Most fixtures are visual and verified by screenshot instead.
4. `.morph/` and `dist/` are gitignored build output. Stale fingerprints skip
   recompiles when only the compiler changed — delete the binary if a rebuild
   does not pick up your change.

Fixtures move as whole directories. Nothing inside one may reference a path
that escapes the fixture directory, because the runtime include path is
resolved relative to the project's depth (see `find_runtime_dir` in
`crates/morph-build/src/lib.rs`, which searches up to four levels). Sibling
references go in as repo-root-relative `tests/runtime/<group>/<name>`.
