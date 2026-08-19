# Interactive JS Interpreter

**Status:** future · **Priority:** low

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

An interactive JavaScript interpreter for scripting use cases, alongside the compile-time TS→C++ translator (not a replacement). Today all JS is compiled to C++ at build time — an interpreter adds a dynamic layer for runtime-loaded scripts, plugins, or a REPL.

## Why it matters

- **Plugins / themes** — load JS at runtime without recompiling the app
- **REPL / dev console** — evaluate expressions live against app state (browser DevTools-style)
- **Hot script iteration** — script-only changes skip the C++ toolchain entirely
- The original design planned `interpreter.py` under `morph/js/` for exactly this

## Current state

| Piece | State |
|---|---|
| Compile-time translator (`TSToCppTranslator`) | ✅ Shipped (native performance, zero interpreter overhead) |
| `morph/js/interpreter.py` | ❌ Scaffold with `# TODO: extract component instantiation` / `# TODO: extract .mount(), .on(), .call() etc.` |

## Planned approach

- Reuse the existing tree-sitter AST + `JsValue` runtime types (`JsNumber`, `JsString`, `JsArray`, `JsObject`) — the interpreter is a tree-walking evaluator over the same AST the translator consumes
- `JsValue` already implements JS semantics: truthiness, `==`/`!=`, `typeof`, property access, string coercion
- Interpreter-backed JSX would run through the same IR pipeline — component logic evaluated instead of compiled

## Scope considerations

- **Performance** — interpreted logic is slower than compiled C++; fine for scripts, wrong for hot paths (keep the translator as the default)
- **Sandboxing** — runtime-loaded code needs access control (which APIs: `fetch`? filesystem? C++ imports?)
- **Dev vs production** — an interpreter only in dev mode is far simpler than shipping one in production binaries

## Open questions

- Does the REPL need to mutate live state (re-wiring signals like `logic.so` hot reload does), or is it read-only evaluation?
- Do plugins run on the main thread or a worker (coroutine scheduler)?

## Build steps (when picked up)

1. Tree-walking evaluator over the TS AST (`morph/js/interpreter.py`)
2. Bind the existing runtime types (`JsValue` etc.) as the value model
3. Dev REPL surface (DevTools console tab is the natural home)
4. Optional: script-file loading API for plugins