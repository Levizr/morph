# Under Construction

What's actively being built right now. These features are functional in the current dev tree but not fully hardened — expect rough edges, and help is welcome.

## Rust CLI Parity (Python CLI Still Present)

The Rust `morph` binary owns direct file morphing (`morph app.ts --to cpp`, `--type`), but the Python CLI is deliberately still in the repo: GUI builds need its layout engine, dev hot-reload, and full JS translation (see [What Still Needs the Python CLI](../guides/migration.md#what-still-needs-the-python-cli)). The removal sequence is: wire `morpher` into `morph-codegen` in strict mode → parity harness diffing both translators → port the layout engine → delete Python. Until then, file morphing → Rust; `.mx` projects → keep Python installed.

## Keyed List Rendering

`{items.map(item => <JSX/>)}` is compiled into a keyed `ListContainer` (`runtime/cpp/ui/morph_list.h`) with runtime reconciliation — unchanged keys are reused, new keys create nodes, missing keys are removed. This is the newest feature: wiring lives in `crates/morph-codegen/` (`node_emitter.rs` / `logic_emitter.rs`), and the usage guide is in [List Rendering](../elements/lists.md).

**Status:** New / shipping in the current dev cycle. Conditions, text bindings, and item-level updates are in place; keyed reuse across insert/remove/reorder is the area most likely to still have edge cases.

## JS / TypeScript Runtime Coverage

The compile-time TS→C++ translator (`TSToCppTranslator`) is being extended to cover more of the language surface. Current work-in-progress:

- Array spread — `[...items, item]`, nested spreads (materialized via an IIFE so evaluation order matches JS)
- `Array.prototype.slice` — negative indices, `slice(0, -1)`, `slice(1)`
- `.length` semantics — cast to a JS number so string-concat and `JsValue` conversions stay unambiguous
- Extra `JsString`/`JsNumber` overloads to keep codegen output compiling cleanly

**Status:** Actively expanding. The runtime type layer (`JsValue`, `JsArray`, `JsString`, `JsNumber`, `JsObject`) grows alongside it in `runtime/cpp/types/`.

## Forge Renderer (Retained Tile Compositor)

The `forge` renderer is **complete and opt-in**. Damage tracking + retained FBO ship per-window, with scroll-shift reuse (intra-FBO blit, exposed strip + old/new thumb damage), retained-layer promotion (transitions/color anims, 8-layer / 4 MB budget), tile-pool residency (16 MB LRU), idle present-skip, and a >32-anim fullscreen degradation guard. The dev Flash↔Forge toggle forces fullscreen on switch-back so the retained surface never goes stale. Verification is landed: `tests/runtime/check-forge-math.sh` (51 checks), `tests/runtime/bench-forge.sh` + `help/forge-benchmarks.md` (scrub 158.8×, scroll 20.1×), `examples/flash` / `examples/forge`, and the Rendering-tab FORGE card + damage overlay.

**`flash` remains the default renderer; `forge` is selected per app via `"renderer": "forge"`.** X/Y animations now flow through damage (old box at commit, new box present-side); only transformed movers escalate to fullscreen.

## CSS Selector Coverage & Cascade Resolver

Build output matches tag/class/id/universal selectors, descendant/child combinators, comma groups, trailing `:hover`/`:active`, and specificity. Sibling (`+`/`~`), attribute, structural pseudo-class, and ancestor-hover rules are accepted by the parser but do not match in the IR builder or emit in build output yet. The **full cascade** — merging all matched rules by specificity and origin into a final computed style — is still being built out.

**Status:** Partial. The basic selector subset and specificity are shipped; broader selectors and full rule merging are tracked in [CSS Finalization for Basic Apps](../future/css/finalization.md).

## `position: relative` / `fixed` / `sticky`

`position`, `left`, `right`, `top`, `bottom`, `fixed`, and `sticky` parse and run behind `MORPH_FEATURE_POSITION`. Relative offsets and sticky clamping are covered by `tests/runtime/position` and the position layout tests.

**Status:** Shipped for the supported positioning subset.

## Multi-Window Navigation

`windowConfig` and `<morph-window>` already create windows, and `WindowManager` tracks them (`runtime/cpp/core/window_manager.h`). The manager's `open()` and `navigate()` methods are still stubs — showing/hiding windows and page navigation across windows is not wired up yet.

**Status:** Scaffolded, not yet functional.

## `morph check` (Semantic Linting)

The latest release added semantic linting for `.mx` files via the `morph check` command. It's new and being hardened as part of the current cycle.

---

### How to Help

The most impactful areas right now are the **CSS cascade resolver**, **TS→C++ translator coverage**, and the **Forge tile pool**. See [Contributing](../../CONTRIBUTING.md) before starting.