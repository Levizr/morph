# Layout & Style: Flexbox Without a Browser

**Part of:** [Dev Docs](../architecture/overview.md)

Morph implements a real CSS box engine — flexbox with wrapping, grow/shrink, justification and alignment, inline flow with whitespace collapsing, margin collapse, absolute/fixed/relative/sticky positioning, z-index paint order, hover/active transitions, and keyframe animations — all in native code with no browser. Styles are resolved at compile time ([The Compiler Pipeline](../architecture/compiler-pipeline.md)); this page is about what happens afterwards: dirty flags, layout passes, style interpolation, flattening, and culling in `runtime/cpp/core/node*` and `runtime/cpp/style/`.

## The node: one class, many files

`MorphNode` (`core/node.h`) is the scene-graph node: layout box, style, children, dirty flags, and a constellation of statics (`s_lastHoveredNode`, `s_focusedNode` for keyboard ownership, `s_mouseCapture` for drag capture past box/window edges, `s_activePressNode` for the `:active` chain — whose dtor clears it so releasing after the pressed subtree was deleted never walks freed memory). The class is big, so its implementation is split by concern:

| File | Owns |
|---|---|
| `core/node/layout.cpp` | The box engine: sizing, flexbox, inline flow, positioning, margin collapse, sticky |
| `core/node/style.cpp` | Hover/active transitions, ancestor-hover rules, style interpolation |
| `core/node/events.cpp` | Transform-aware `hitTest` (children first, reverse paint order) |
| `core/node/flatten.cpp` | Flattening to `FlatRenderNode` snapshots: accumulated transforms, opacity products, culling |
| `core/node/paint_order.cpp` | CSS 2.1 Appendix-E paint buckets under `MORPH_FEATURE_ZINDEX` |
| `core/node/animation.cpp` | CSS `animation` + `@keyframes` driver (compiles to nothing without `MORPH_FEATURE_ANIMATION`) |
| `core/node.cpp` | `markDirty` and the core wiring |

## Dirty flags: laziness as architecture

`DirtyFlag`: Clean / StyleDirty / LayoutDirty / PaintDirty / ScrollDirty / SubtreeDirty. `markDirty` propagates Style/Layout/Subtree *up* the tree; **PaintDirty does not propagate** — paint is a local affair. `layoutIfNeeded` then does only the work the flags demand, collecting `DirtyStats` (layoutCount, paintCount, fullTreeCount, skippedCount, culledCount, plus forge's `damageArea`/`presentBytes`) for the DevTools Rendering tab.

The mental model: the tree is lazy by default and precise about its laziness. A color change never re-runs flexbox; a text edit never repaints the sidebar. When profiling shows full-tree counts climbing, some `markDirty` is over-propagating — the flags are the first place to look.

## The box engine (`layout.cpp`), greatest hits

- **Border-box awareness** via `hBonus`/`vBonus` — padding and border fold into sizing correctly.
- **Containing blocks**: absolute/fixed position against `m_absCb*`, not against vibes.
- **`relative` offsets and `sticky` clamping** (`applySticky()` clamps against the nearest scroll container's scrollport via `m_flowX/m_flowY` + `shiftStickySubtree`, with `updateStickySubtree` after the scrollport finalizes).
- **Auto margins**, `display:none` collapse, button vertical centering, min/max clamps.
- **Flexbox**: line wrapping, grow/shrink distribution, `justifyContent`/`alignItems`, auto-margin distribution across lines.
- **Inline flow** with whitespace collapsing and shared-line baselines (`m_centerInk`).
- **Margin collapse** via a provisional first pass (learn the collapsed-through `m_computedMargin`) followed by the real pass — collapse requires knowing the answer before computing it, hence two passes.
- **`MORPH_LAYOUT_DEBUG`** env traces for when you need to watch the engine think.

Known quirks (owned honestly in the story docs): flex double margins, flex-grow centering shift, wrap gap bug, `margin:auto` clamp, column-flex width, text padding ignored, single-level font inheritance. If you're fixing one, the layout `.cpp` is the arena and the `ui-test` / `culltest` fixtures are the spectators.

## Style dynamics (`style.cpp`): transitions that don't fight

Hover and `active` transitions share **one** `HoverTransition` (pre/press snapshots per state) so the two can't fight mid-interpolation. The details that prevent entire bug categories:

- `buildReleaseStyle` reverts only fields still equal to the press value — **reactive-effect writes are never clobbered** by a transition ending.
- `interruptStateTransitions()` snaps to target before effects write, so effects always start from a sane baseline.
- Ancestor-hover rules (`AncestorHoverRule` + `AncestorHoverTransition`) let parents react to descendants' hover.
- `interpolateStyles` lerps scalars and matrix transforms (decompose → slerp → recompose, via `core/mat4.h`'s W3C css-transforms-2 §6.1 implementation with 2D QR + full 3D quaternion slerp).
- `update()` recomputes `m_isTransitioning`/`m_hasLayoutTransition` — the culling guard that keeps animating nodes in the frame (see below).

The changelog's "How CSS Transitions Work" section (IR fields → `m_transitionDuration`/`m_transitionEasing` → heap `HoverTransition` → `interpolateStyles`, with snap-vs-lerp rules) remains the authoritative mechanics reference.

## Flatten and cull (`flatten.cpp`)

`flatten`/`flattenImpl` walks the laid-out tree with accumulated transform `A(node) = A(parent) × T(rel) × M(node)`, multiplies opacity down the tree into the baked product, and culls via screen-space AABB — off-screen nodes never reach the frame. `subtreeMayMove()` guards culling so compositor-animated nodes can't be frozen out of the frame they need to move through. `recordDisplayList`/`executeDisplayList` is the legacy path, kept for the single-threaded `render()` under `MORPH_FEATURE_DIRTY_RENDERING`.

## Paint order (`paint_order.cpp`)

Under `MORPH_FEATURE_ZINDEX`, lazy `ensurePaintOrder()` builds CSS 2.1 Appendix-E buckets (negative z / block flow / inline flow / auto / positioned), invalidated on child-list change or runtime z-index change. Hit-testing walks children in reverse paint order, transform-aware. The `zindex-test` fixture (negative/auto/0/5/10 vs a static backdrop) is the visual contract.

## The style headers (`style/`)

`css_enums.h` + `style.h` define the computed-style value space; `style/features/` splits it by concern (`base`, `flex`, `border`, `outline`, `shadow`, `position`, `scroll`, `cursor`, `opacity`, `transform`, `animation`, `zindex` — note `shadow.h`/`outline.h` are dormant scaffolding for planned work). This is where a new CSS property lands at runtime; the compile-time side lands in `morph-ir`'s `style.rs` / `tailwind.rs` / `transforms.rs` ([The Compiler Pipeline](../architecture/compiler-pipeline.md)).

## UI and widgets (`ui/`, `widgets/`, `viewport/`)

Node subclasses live here: Button, Input, Text, Image, List (`MorphNode` consumers + `morph_list.h` keyed reconciliation), View (`rect.h`, `radius.h`), and the viewport scaffold (`viewport_node.h`, `viewport_driver.h` — embedded OpenGL canvas work is planned, parser/builder wiring still missing). Higher-level widget wrappers sit in `widgets/`. Custom C++ nodes from user projects inherit `MorphNode` with their own rendering and behavior — the extension story documented in the main custom-nodes guide.

## Input model, briefly

Click focuses `<input>`, click-elsewhere blurs; keys go to `s_focusedNode->onKeyEvent` first (consume stops propagation), chars to the focused node only; drag capture (`s_mouseCapture`) keeps MouseMove/MouseUp flowing past edges; release-outside-capture ends drags. Text input boxes exist; caret/focus/selection/keyboard full behavior is planned work (see the text-input future doc) — today's `input-test` fixture (controlled `value`+`onChange`, `maxLength` in UTF-16 units, validation, password masking, `onFocus`/`onBlur`, mouse selection) is the honest boundary of what works.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Fix sizing/positioning/flex/inline/collapse | `core/node/layout.cpp` — reproduce with a minimal `.mx` first |
| Fix hover/active transition fights or effect clobbering | `core/node/style.cpp` — check the shared-`HoverTransition` and revert-only-press-values logic |
| Fix paint/z-order | `core/node/paint_order.cpp` + `zindex-test` fixture |
| Fix disappearing animated nodes | `subtreeMayMove()` / culling guards in `flatten.cpp` |
| Add a CSS property | `style/features/` header + IR-side resolution + `feature_set.rs` + fixture |
| Add a widget | `ui/` subclass + `widgets/` wrapper + example usage |

## Verify by

```bash
<binary> --morph-self-test     # 0 failures
./tests/runtime/run-selftests.sh
# Visual sweep under :0: ui-test, culltest, transform-test, opacity-test,
# zindex-test, animation-test 1+2.0, test-hover, input-test, list-test
```

Layout changes are guilty until proven innocent by screenshots. Run the visual sweep, compare against the pre-change screenshots, and be suspicious of any pixel that moved without an explanation.
