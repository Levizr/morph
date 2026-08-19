# Under Construction

What's actively being built right now. These features are functional in the current dev tree but not fully hardened — expect rough edges, and help is welcome.

## Keyed List Rendering

`{items.map(item => <JSX/>)}` is compiled into a keyed `ListContainer` (`morph/runtime/ui/morph_list.h`) with runtime reconciliation — unchanged keys are reused, new keys create nodes, missing keys are removed. This is the newest feature: wiring lives in `node_emitter.py` / `logic_emitter.py`, and the usage guide is in [List Rendering](../elements/lists.md).

**Status:** New / shipping in the current dev cycle. Conditions, text bindings, and item-level updates are in place; keyed reuse across insert/remove/reorder is the area most likely to still have edge cases.

## JS / TypeScript Runtime Coverage

The compile-time TS→C++ translator (`TSToCppTranslator`) is being extended to cover more of the language surface. Current work-in-progress:

- Array spread — `[...items, item]`, nested spreads (materialized via an IIFE so evaluation order matches JS)
- `Array.prototype.slice` — negative indices, `slice(0, -1)`, `slice(1)`
- `.length` semantics — cast to a JS number so string-concat and `JsValue` conversions stay unambiguous
- Extra `JsString`/`JsNumber` overloads to keep codegen output compiling cleanly

**Status:** Actively expanding. The runtime type layer (`JsValue`, `JsArray`, `JsString`, `JsNumber`, `JsObject`) grows alongside it in `morph/runtime/types/`.

## Forge Renderer (Retained Tile Compositor)

The `forge` renderer is **beta / buggy**. Damage tracking + retained FBO are shipped and can be toggled live in dev from the DevTools Rendering tab, but there are known bugs around:

- Damage-rect edges
- Scroll-shift
- Some compositor-animation paths

**Flash remains the recommended / default production renderer.** Planned follow-ups: per-tile LRU pool and scroll-shift tile remap (Phases 4 and 6 in `help/renderer-flash-forge.md`).

## CSS Cascade Resolver

The selector engine (`morph/style/selector.py`) parses descendant / child (`>`) / adjacent (`+`) / sibling (`~`) combinators, compounds, pseudo-classes, and specificity. Runtime `:hover` / `:active` and ancestor-hover rules work. The **full cascade** — merging all matched rules by specificity and origin into a final computed style — is still being built out.

**Status:** Partial. Parse + specificity exist; full rule-merging cascade in progress.

## `position: relative` / `fixed` / `sticky`

`position`, `left`, `right`, `top`, `bottom` parse and feature-gated runtime fields exist (`MORPH_FEATURE_POSITION`). Offset positioning is functional; **sticky is still in progress**.

**Status:** Partial.

## Multi-Window Navigation

`windowConfig` and `<morph-window>` already create windows, and `WindowManager` tracks them (`morph/runtime/core/window_manager.h`). The manager's `open()` and `navigate()` methods are still stubs — showing/hiding windows and page navigation across windows is not wired up yet.

**Status:** Scaffolded, not yet functional.

## `morph check` (Semantic Linting)

The latest release added semantic linting for `.mx` files via the `morph check` command. It's new and being hardened as part of the current cycle.

---

### How to Help

The most impactful areas right now are the **CSS cascade resolver**, **TS→C++ translator coverage**, and the **Forge tile pool**. See [Contributing](../../CONTRIBUTING.md) before starting.