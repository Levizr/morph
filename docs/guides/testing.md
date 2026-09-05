# Testing Morph Apps

How we test Morph itself, and how you can test apps built with it.

## Testing the Translator (24/24 Fixture Suite)

The JS/TS → C++ translator is covered by fixture tests in `tests/translate/`:

```bash
# Full suite (translates each .ts, compiles with g++-14, runs, diffs vs Node)
python3 -m pytest tests/translate -v
```

How each fixture works (`tests/translate/test_rust_translate.py`):

1. `morph <fixture>.ts --to cpp` generates `<fixture>.cpp`
2. Test patches `"../../runtime/cpp/..."` includes to absolute paths
3. Compiles: `g++-14 -std=c++23 <gen>.cpp runtime/cpp/net/net.cpp runtime/cpp/reactivity/task.cpp -I runtime/cpp`
4. Runs the binary (must exit 0)
5. Compares stdout line-by-line against `npx tsx <fixture>.ts` (numeric tolerance, case-insensitive bools)

Fixtures (`tests/translate/fixtures/`):

| Fixture | Covers |
|---|---|
| `01_variables` | `let/const/var`, primitives, inference |
| `02_functions` | functions, arrows, defaults, recursion, generics |
| `03_classes` | classes, ctors, `this`, `static`, `new` |
| `04_arrays` | literals, `.length`, index, `push`, spread, nesting |
| `05_objects` | literals, shorthand, computed, nesting |
| `06_control_flow` | `if/switch/for/while/do-while`, `break/continue` |
| `07_operators` | arithmetic, comparison, `===/!==`, `??`, `+=` |
| `08_template_literals` | `` `Hi ${x}` ``, nesting, expressions |
| `09_try_catch` | `throw`, `try/catch/finally`, nesting |
| `10_console_log` | multi-type/multi-arg `console.log` |
| `11_complex` | classes + arrays + templates together |
| `12_string_methods` | `length`, `toUpperCase`, `split`, `slice`, ... |
| `13_number_methods` | arithmetic, `toString()`, templates |
| `14_inheritance` | `extends`, `super`, interfaces, enums |
| `15_chaining` | `s.split().length`, method chains |
| `16_loops` | loop variants, `break/continue`, `while(true)` |
| `17_async` | `async/await`, `fetch`, async arrows/methods |
| `18_promises` | `Promise<T>`, `new Promise`, `Promise<void>`, generics |
| `19_fetch` | top-level `await fetch`, loops, dynamic URLs |
| `20_cpp_types` | `int/int32/uint/float/char/size_t`, `std::vector`, `std::optional` |

Extra regression checks:

- `JsNumber` + `println` uses `<print>` but no explicit `<format>`
- Template vars (`` `a${x},b${y}` ``) require `<format>` + `std::format`
- Plain vars require neither `<format>` nor `<print>` extras

## Testing the Compiler Crates

```bash
# All Rust unit tests
cargo test --workspace

# Single crate
cargo test -p morph-ir
cargo test -p morph-js
cargo test -p morph-parser
```

## Testing Your App

### 1. Lint First (Fastest Feedback)

```bash
morph check
morph check src/App.mx
```

Catches unsupported elements, props, events, CSS, and JS before compiling.

### 2. Dev Mode Smoke Test

```bash
morph dev
```

- Window opens, edits hot-reload
- Press **F12** → Logs tab shows `console.log` output
- Network tab shows `fetch()` traffic

### 3. Logic Unit Tests (Recommended Pattern)

Keep pure logic in functions that don't touch JSX, then test the generated C++ directly:

```tsx
// src/App.mx
export function add(a: number, b: number): number {
  return a + b;
}
```

```bash
morph logic.ts --to cpp   # extract logic to a file, or copy the function
g++-14 -std=c++23 -I runtime/cpp test_main.cpp runtime/cpp/net/net.cpp -o test_app
./test_app
```

Or test the TypeScript directly with Node before compiling:

```bash
npx tsx src/logic.ts
```

### 4. Golden-Output Tests

Same pattern the translator suite uses — compare app output vs expected:

```bash
morph build --output bin/
./bin/my-app --headless --dump-state > actual.txt
diff expected.txt actual.txt
```

### 5. CI Example

```yaml
# .github/workflows/test.yml
name: Test
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo apt update && sudo apt install -y g++-14 cmake libglfw3-dev libfreetype-dev libharfbuzz-dev
      - run: cargo test --workspace
      - run: python3 -m pytest tests/translate -x -q
      - run: cargo install --path crates/morphc
      - run: morph check
      - run: morph build
```

## Debugging Failing Tests

| Symptom | What To Check |
|---|---|
| Fixture output differs by whitespace | Tests compare line-by-line; check trailing spaces, `println` vs `print` |
| `g++-14: command not found` | Install GCC 14 (`apt install g++-14`) or set `build.cxx` |
| `npx tsx` fails | `npm i -D tsx`, Node 18+ required |
| `.cpp` has wrong includes | Check `Ctx::need()` / `headers_for()` in `type_resolver.rs` |
| Async test hangs | Missing `process_tasks()` loop, un-awaited `Result` |

## Writing New Fixture Tests

1. Add `tests/translate/fixtures/21_my_feature.ts` (must run clean under `npx tsx`)
2. Run `python3 -m pytest tests/translate -k 21_my_feature -v`
3. Generated `.cpp` is gitignored (`tests/translate/fixtures/*.cpp`) — only commit the `.ts`

## Related

- [Debugging & Profiling](debugging.md) — GDB, sanitizers, DevTools
- [Intent-Based Codegen](intent-based-codegen.md) — what `--optimize` changes
- [Runtime Internals](runtime-internals.md) — signals, coroutines, scene graph