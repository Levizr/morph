# Runtime Layout

**Part of:** [Dev Docs](../architecture/overview.md)

How `runtime/cpp/` is organized, the conventions runtime code follows, and where a new helper, type, or module goes. This is the orientation page — the deep dives for each neighborhood are linked below, and this page tells you which one you need.

## Tree

```
runtime/cpp/
├── types/        JsValue + wrappers (JsNumber/JsString/JsArray/JsObject/JsBoolean),
│                 morph::str helpers, vector/optional println formatters
│                 → [JS Semantics](../morpher/js-semantics.md)
├── core/         window, renderer, compositor, event, node, mat4, spsc_queue, clipboard
│                 → [Rendering](rendering-deep-dive.md), [Layout & Style](layout-style-engine.md)
├── reactivity/   signals/state, effects, scheduler (coroutines: Task, Result<T>), channels
│                 → [Reactivity Engine](reactivity-engine.md)
├── net/          fetch/HTTP client used by lowered fetch() calls
│                 → [fetch()](networking.md)
├── ui/           widgets + layout consumers (MorphNode subclasses, list, inputs…)
├── widgets/      higher-level widget wrappers (morph_button, morph_text, …)
├── viewport/     embedded OpenGL canvas scaffolding (planned; parser wiring missing)
│                 → [Layout & Style](layout-style-engine.md) for ui/widgets/viewport
├── renderers/    flash (production default) / forge (beta tile compositor)
├── render/       shared GL backend: shaders, batching, fonts, textures
│                 → [Rendering(rendering-deep-dive.md) for renderers + render
├── style/        computed-style application + feature headers (base, flex, border, …)
│                 → [Layout & Style](layout-style-engine.md)
├── dev/          dev-runtime support (hot-reload IPC, JSON DOM parser, inspector, logs, net)
│                 → [Dev Mode](../architecture/dev-mode.md)
├── vendor/       third-party (glad, stb_image…), untouched
└── morph_api.h   the single public entry header — the front door with the welcome mat
```

Every optional subsystem is compile-time gated by a `MORPH_FEATURE_*` define (ANIMATION, POSITION, ZINDEX, TRANSFORM, DEV, DEV_RENDERER_SWITCH, DIRTY_RENDERING, INPUT, TEXT, IMAGE, CURSOR, SCROLL, FLEX, BORDER, BORDER_BOX, RADIUS, MIN_MAX, MARGIN_COLLAPSE, INLINE, DISPLAY_NONE, OPACITY, FORGE_RENDERER…). Dev mode enables all features; production builds enable only what `FeatureSet` detected ([crate-codegen](../crates/crate-codegen.md)). There are no runtime `if`s for features — unused code costs zero bytes because it never compiles in.

## Conventions

- **C++17, header-only** for runtime headers. No separate compilation for `types/` — morpher includes them directly into generated translation units, so keep them self-contained and minimal.
- **Generated file-morph output is C++23** (`g++-14 -std=c++23`) — a *different* standard from the runtime itself. Runtime headers must therefore stay C++17-clean; never use C++20/23 features in `runtime/cpp/`. Two standards, two rooms, no mud tracked between them.
- **Includes are absolute at emit time**: morpher rewrites `../../runtime/cpp/...` to the global cache path (`~/.morph/cache/runtimes/cpp/vX.Y.Z/...`) so generated files compile from any directory. New headers are picked up automatically as long as they're under `runtime/cpp/` — no registration step, no ceremony.
- **`morph::str` pattern for helpers**: JS-shaped behavior over native storage, free functions in a namespace, no classes unless state is required (`runtime/cpp/types/js_string_helpers.h` is the template to copy).
- **C++ style** (binding, per `CODING_STANDARDS.md`): Allman braces, `m_` member prefix (`s_` for statics), `#pragma once`, `inline constexpr` over `#define`, no raw `new`/`delete` in new code, feature code inside `#ifdef MORPH_FEATURE_*`, `clang-format` clean.
- **One class per header** (when over ~200 lines); large classes split into feature `.cpp` files under a subdirectory (`core/node/` is the exemplar: `layout.cpp`, `style.cpp`, `events.cpp`, `flatten.cpp`, `paint_order.cpp`, `animation.cpp`).

## Adding to the runtime

| "I want to…" | Do |
|---|---|
| Add a string/array helper | New function in the matching helper header → mapping in `crates/morpher/src/codegen/string_methods.rs` (or the array path) → fixture case proving Node-identical output |
| Add a `Js*` capability | Extend the wrapper header (`types/js_*.h`) → check `js_value.h` dispatch still covers it → `js_value_format.h` if it should print |
| Add a style feature | New `style/features/*.h` header + `MORPH_FEATURE_*` gate + `FeatureSet` rule in `morph-codegen` (+ reactive entry) + fixture exercising it statically and reactively |
| Add a new module (e.g. `node:fs` someday) | New directory + entry header → include-needs registration in morpher's `Ctx::need` flow → [Node.js Support](../../future/nodejs-support.md) tracks the plan |
| Add a widget | `ui/` subclass + `widgets/` wrapper + example usage |
| Touch renderers | Flash first (production); forge is beta with known damage-rect/scroll bugs — read `help/renderer-flash-forge.md` before changing compositor behavior |
| Touch the threads | Main owns the tree; compositor writes only `anim*` fields; workers never touch UI — see [Reactivity Engine](reactivity-engine.md) threading contract |

## Memory rules for runtime code

Runtime headers execute inside apps morpher generates, so they inherit the no-GC contract ([Intent-Based Codegen](../../guides/intent-based-codegen.md#memory-management-in-detail-no-gc), [Escape Analysis](../morpher/escape-analysis.md)): deterministic destruction, `shared_ptr` only for genuinely shared ownership (`JsArray.elements` and `JsObject.properties` are the canonical examples — JS reference semantics require shared ownership; copies alias, they don't clone), no hidden global allocators, no exceptions across generated-code boundaries unless the emitter generates the matching `try`/`catch`.

## Verify by

```bash
<binary> --morph-self-test     # 0 failures, no display needed
./tests/runtime/run-selftests.sh   # full fixture sweep from the repo root
```

The C++ compiler is the linter for this layer: rebuild affected fixtures and run them. Rendering or layout changes additionally deserve screenshots under a live X server — pixels are the contract, diff them with your eyes.
