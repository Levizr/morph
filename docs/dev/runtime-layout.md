# Runtime Layout

**Part of:** [Dev Docs](overview.md)

How `runtime/cpp/` is organized, the conventions runtime code follows, and where a new helper, type, or module goes.

## Tree

```
runtime/cpp/
├── types/        JsValue + wrappers (JsNumber/JsString/JsArray/JsObject/JsBoolean),
│                 morph::str helpers, vector/optional println formatters
├── core/         window, renderer, compositor, event, node, mat4, spsc_queue, clipboard
├── reactivity/   signals/state, effects, scheduler (coroutines: Task, Result<T>)
├── net/          fetch/HTTP client used by lowered fetch() calls
├── ui/           widgets + layout consumers (MorphNode, list, inputs…)
├── renderers/    flash (production default) / forge (beta tile compositor)
├── render/       shared render primitives
├── style/        computed-style application at runtime
├── viewport/     embedded OpenGL canvas
├── widgets/      higher-level widget set
├── dev/          dev-runtime support (hot-reload IPC, JSON DOM parser)
├── vendor/       third-party, untouched
└── morph_api.h   the single public entry header
```

## Conventions

- **C++17, header-only** for runtime headers. No separate compilation for `types/` — morpher includes them directly into generated translation units, so keep them self-contained and minimal.
- **Generated file-morph output is C++23** (`g++-14 -std=c++23`) — a *different* standard from the runtime itself. Runtime headers must therefore stay C++17-clean; never use C++20/23 features in `runtime/cpp/`.
- **Includes are absolute at emit time**: morpher rewrites `../../runtime/cpp/...` to the global cache path (`~/.morph/cache/runtimes/cpp/vX.Y.Z/...`) so generated files compile from any directory. New headers are picked up automatically as long as they're under `runtime/cpp/`.
- **`morph::str` pattern for helpers**: JS-shaped behavior over native storage, free functions in a namespace, no classes unless state is required (`runtime/cpp/types/js_string_helpers.h` is the template to copy).

## Adding to the runtime

| "I want to…" | Do |
|---|---|
| Add a string/array helper | New function in the matching helper header → mapping in `crates/morpher/src/codegen/string_methods.rs` (or the array path) → fixture case proving Node-identical output |
| Add a `Js*` capability | Extend the wrapper header (`types/js_*.h`) → check `js_value.h` dispatch still covers it → `js_value_format.h` if it should print |
| Add a new module (e.g. `node:fs` someday) | New directory + entry header → include-needs registration in morpher's `Ctx::need` flow → [Node.js Support](../future/nodejs-support.md) tracks the plan |
| Touch renderers | Flash first (production); forge is beta with known damage-rect/scroll bugs — read `help/renderer-flash-forge.md` before changing compositor behavior |

## Memory rules for runtime code

Runtime headers execute inside apps morpher generates, so they inherit the no-GC contract ([Intent-Based Codegen](../guides/intent-based-codegen.md#memory-management-in-detail-no-gc)): deterministic destruction, `shared_ptr` only for genuinely shared ownership (`JsArray.elements` and `JsObject.properties` are the canonical examples — JS reference semantics require them), no hidden global allocators, no exceptions across generated-code boundaries unless the emitter generates the matching `try/catch`.
