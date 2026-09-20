# Testing: The Fixture Hammer and the Self-Test Gauntlet

**Part of:** [Dev Docs](../architecture/overview.md)

Morph trusts exactly two things: the C++ compiler and the test suite. Everything else — your reasoning, my reasoning, the reviewer's Friday-afternoon reasoning — is treated as a rumor until a test confirms it. This page tours the three layers of verification (unit tests, the translator fixture hammer, and the runtime self-test gauntlet), explains how the only Python left in the repo earned its keep, and gives you the recipe for adding a fixture.

## Layer 1 — Unit tests (`cargo test`)

```bash
cargo test --workspace          # everything: all crates
cargo test -p morph-ir -p morph-parser -p morph-codegen   # the pipeline trio
```

These cover data-structure behavior and IR-shape regressions: `shared_store_is_namespaced_per_file`-style tests asserting exact `shared_vars` entries and qualified accessor strings. If your change moves an expected `app::…` string, that diff means every existing store/event relinks — read it like a confession, not a nuisance.

## Layer 2 — The fixture hammer (`tests/translate/`)

The translator's real contract. Twenty-nine numbered fixtures (`01_…` through `29_…`) plus regression checks, driven by the only Python left in the repo — a test harness, not toolchain (it drives the built binary; it ships nothing).

### How a fixture run works (`test_rust_translate.py`, behavioral)

For each `fixtures/*.ts`:

1. Run `morph <file> --to cpp` (binary: `target/debug/morph`, else `morph` on PATH, else `cargo run -p morphc` fallback; `pytest.skip` if none exists).
2. `_patch_includes()` rewrites relative `../../runtime/cpp/...` includes to absolute repo paths so the file compiles from anywhere.
3. Compile with **`g++-14 -std=c++23`**, linking `runtime/cpp/net/net.cpp` + `reactivity/task.cpp`.
4. The binary must exit 0; stdout is diffed line-by-line against `npx tsx` ground truth (float tolerance < 0.001, case-insensitive bools).

`.ts`-only fixtures (no `.cpp` sibling) run via `npx tsx` ground truth: `07_operators`, `09_try_catch`, `10_console_log`, `14_inheritance`, `21_js_comparison`, `23_int_range`, `24_trust`, `26_ownership_return`, `27_escaping_closure`, `28_last_use_move`, `29_destructuring`. Extra regression checks pin down include hygiene (JsNumber+`println` needs `<print>` but no `<format>`; template-var initializers need `<format>`; plain vars need neither) and the absence of removed flags (`--optimize` must fail — it was retired with intent-mode-always-on).

### How intent coverage works (`test_intent.py`, structural)

No compilation — it asserts *emitted C++ shape* via `translate_source()` + `code_without_strings()` (string literals stripped so assertions can't accidentally match inside them): native `std::string` stays native, closures capture `shared_ptr` by value, returned scalars stay on the stack, `int32_t` refinement and forfeiture, trusted-annotation victories, last-use `std::move`, exotic-destructuring fallback to the runtime error. If [Escape Analysis](../morpher/escape-analysis.md) is the law, this file is the police blotter.

### Adding a fixture (the recipe)

1. Add `tests/translate/fixtures/2X_name.ts` — minimal, one behavior per fixture.
2. Run it: `python3 -m pytest tests/translate -v -k <name>`.
3. The generated `.cpp` is **gitignored** — only `.ts` sources and expected outputs are reviewed.
4. If your change alters an existing fixture's output, that diff *is* the review. Inspect it line by line before asking anyone else to.

Debugging table for the usual suspects: whitespace diffs (check trailing newlines), `g++-14` missing (install the toolchain, run `morph doctor`), `tsx` missing (Node side), include errors (new headers need `Ctx::need()` / `headers_for()` registration in `type_resolver.rs`), async hangs (re-read the `fetch_response()` race comment in [fetch()](../runtime/networking.md)).

## Layer 3 — The self-test gauntlet (`tests/runtime/`)

Full-app smoke projects (`.mx` apps with configs), each a visual/contract test for a subsystem:

| Fixture | Exercises |
|---|---|
| `component-test` | Reusable components, per-instance state, shared store + events across files (in self-tests) |
| `native-interop` | Bidirectional C++/JSX, `native` config block, `MID_*` constants, cross-thread state (in self-tests) |
| `animation-test`, `animation-test-2.0` | Keyframes, multi-animation, iteration counts, fill modes, hover swaps |
| `culltest` | Viewport culling stress: 870+ nodes (flash renderer) |
| `input-test` | Controlled inputs, validation, password, focus/blur, selection |
| `list-test` | Keyed reconciliation + effects |
| `opacity-test`, `test-hover`, `transform-test`, `ui-test`, `zindex-test` | The visual contract for their respective subsystems |

`run-selftests.sh` builds the CLI if missing, then for `component-test` and `native-interop`: deletes stale outputs, `morph build --no-upx`, runs the binary with `--morph-self-test`, and greps for `0 failures`. Headless check for any binary, no display needed:

```bash
<binary> --morph-self-test     # must report 0 failures
./tests/runtime/run-selftests.sh   # from the repo root
```

## The examples (`examples/`) are tests with better PR

`calculator`, `components`, `dynamic`, `ipchecker`, `login`, `budget` — each demonstrates a feature slice (reactive keypads, shared carts, theme toggling, async fetch, validated forms, native interop + keyed lists + timers). They double as manual verification: `cd examples/calculator && morph dev` should just work. If an example rots, users notice before CI does — treat example breakage as a P0.

## The quirks shelf (known, documented, not hidden)

- `transform-test`'s config says `"trnasform-test"` — a typo, preserved here so the next person to `grep` doesn't think they're hallucinating.
- `ui-test`'s config name is `test` — non-descriptive, same deal.
- `my-app/` mixes the old `CSS.load` API with the forge renderer and mismatched window sizes — it's a playground with history, not a reference.
- `tests/translate/fixtures/main` (a stray ELF) and `my-app/src/` leftovers (`app.cpp`, `test.cpp`, `problem.md`, …) are cleanup candidates, not fixtures.

## The contributor's pre-flight

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
python3 -m pytest tests/translate -v
./tests/runtime/run-selftests.sh
```

Five commands, zero excuses. Run them before every commit — `AGENTS.md` says so, `CODING_STANDARDS.md` agrees, and the fixture hammer does not accept apologies.
