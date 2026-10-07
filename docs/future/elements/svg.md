# SVG Engine — Own Renderer, Chrome-Look Parity, Pay-for-What-You-Use Math

**Status:** future · **Priority:** medium · **Depends on:** [Icons & SVG](icons-svg.md) (subset + icon system), [Graphics APIs](../rendering/graphics-apis.md) (GPU path choice), [Animations](../css/animations.md) (choreography for later JS-driven animation)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

We write this down as a team (Oct 2026): Morph renders raster images today and no SVG at all. For SVG support we decided to build our own engine instead of embedding a third-party one — for deep-level control over the render pipeline and for advanced dead-code elimination, so the final binary contains only the math of the SVG features the app actually uses. The rendering target is 1:1 with Chrome at best performance; JS-driven SVG animation arrives later, when animating with JS lands.

Companion product doc: [Icons & SVG](icons-svg.md) (the `<svg>` subset, `morph-icons`, `morph icons:add`). That page is the icon-system track; this page is the engine underneath it — Phase 0 here *is* the subset described there.

## 1. Summary

- Today Morph decodes raster images only (`stbi_load` in `gl_renderer.h:529-540`: PNG/JPEG/BMP/GIF/PSD/HDR — no SVG). Zero SVG parsing exists in `crates/` or `runtime/`. An `.svg` file cannot be displayed at all.
- We build our own SVG engine. No resvg / nanosvg / lunasvg / ThorVG. Reasons: control over tessellation, caching, and the compositor path; per-feature dead-code elimination no third-party engine offers; static linking without a foreign dependency tree.
- Rendering parity target is Chrome: same pixels for the supported subset, verified against Chrome screenshots on a fixed corpus.
- Performance strategy is AOT-shaped: parse and tessellate at build where the SVG is static, cache per (path, size) where it is dynamic — like the glyph cache, not like a browser's runtime raster.
- The binary contains only the math it uses: `MORPH_FEATURE_SVG_*` defines per capability, emitted from the actual element/attribute set found at build.
- Animation comes in layers: CSS `@keyframes` on SVG presentation attributes reuses the existing animation engine; JS-driven SVG animation waits for the imperative animation API (`animations.md` scope). SMIL is not planned.

### Why SVG at all

Every app needs resolution-independent graphics — icons, logos, illustrations — and raster formats fail that job structurally:

- **PNGs don't scale.** HiDPI turned fixed-resolution assets into a bug factory: one asset per density, blurriness on fractional scales, ballooning asset folders. A vector scales to any size and DPR from a single file, for free.
- **One asset, every size.** The same 2KB icon file renders a 16px toolbar glyph and a 256px empty-state illustration. No export matrix, no stale assets when a size changes.
- **Small where it counts.** Icons and logos are overwhelmingly paths and fills — kilobytes of math instead of hundreds of kilobytes of pixels. The binary and memory savings compound across an app's whole icon set.
- **Themable by construction.** `currentColor` lets icons inherit text styling — dark mode and accent changes flow through with zero asset variants. Raster assets need a full duplicate set per theme.
- **Designers already speak SVG.** Figma, Illustrator, and icon libraries export SVG natively. Supporting it removes an asset-conversion step (and its bugs) from every project's pipeline.
- **Motion-ready.** Vectors animate cleanly — transforms stay crisp mid-animation where scaled rasters smear. Today's icons become tomorrow's animated illustrations with no format migration.
- **The alternatives are worse.** PNG-per-density multiplies assets; icon fonts hijack text shaping, break with emoji, and can't do multicolor; drawing icons from divs doesn't scale past geometric toys. Each workaround this engine removes is complexity deleted from userland.

In short: SVG is the only format that is simultaneously scalable, small, themeable, designer-native, and animatable. That combination is why it gets an engine, not just a decoder.

## 2. Current state in Morph

| Piece | State |
|---|---|
| Raster image decode (`stbi_load` → texture upload on compositor) | ✅ Shipped (`render/gl_renderer.h:529`, `ui/image.h:20`) |
| SVG parsing, DOM, rendering | ❌ Nothing — no matches for svg/SVG anywhere in `crates/` or `runtime/` outside vendored HarfBuzz headers (SVG color-emoji tables, unrelated) |
| `<svg>` subset plan (path/rect/circle, `viewBox`, `currentColor`, CPU tessellation + cache) | ❌ Planned in [Icons & SVG](icons-svg.md) — becomes Phase 0 of this engine |
| `morph-icons` / `morph icons:add` | ❌ Planned in [Icons & SVG](icons-svg.md) |

"Image support" in Morph therefore means raster formats only. SVG support starts from zero — which is exactly why owning the engine is affordable now: there is no legacy SVG path to migrate.

## 3. How Qt handles SVG

Qt's SVG story is widgets-era, frozen at a subset:

- **QtSvg module**: `QSvgRenderer` + `QSvgWidget` (plus `QGraphicsSvgItem`, and `QSvgGenerator` for *writing* SVG). Renders onto any `QPaintDevice` through `QPainter` — i.e. CPU raster through Qt's paint engine.
- **Subset: static SVG 1.2 Tiny.** No ECMA scripts, no DOM manipulation, no SMIL animation. Animated or scripted SVGs render as their first frame.
- **Qt Quick has no native SVG element.** Practice is one of: load `.svg` through the SVG image-format plugin (raster-at-load → texture at one size, vector crispness lost on scale), render via `QSvgRenderer` into a `QQuickImageProvider`, or hand-author vectors with `QtQuick.Shapes` (a different, QML-native path API — not SVG).
- Internally QtSvg carries third-party XSVG code under a permissive license — the "vendored parser" pattern this doc rejects for Morph (§7).

Net: Qt treats SVG as *static pictures* that get rasterized early. Good enough for icons; wrong for resolution-independent, animatable vector content.

## 4. How Chrome handles SVG

Chrome treats SVG as a *document*, fully integrated into the browser pipeline:

- **Full DOM**: `SVGElement` classes, `SVGLength`, path-segment lists, `<use>` shadow trees — scriptable through V8 like HTML.
- **Style/layout/paint integration**: SVG primitives get layout objects (`LayoutSVGShape` family) and participate in style recalc, layout, and paint. Presentation attributes (`fill`, `stroke-width`, `transform`) are CSS-styled and therefore animatable by the same engines as HTML.
- **Three animation sources**: SMIL declarative animation (still supported; SVG2 steers authors toward CSS), CSS animations/transitions on geometry properties, and Web Animations / JS-driven changes — all sampling against the same timeline model.
- **Two contexts with different rules**: inline `<svg>` (full document: DOM + style + script + interaction) vs SVG-as-image (`<img src="a.svg">`, CSS `background-image`) — rasterized, no scripts, no interaction, no external subresources.
- **Backend is Skia** (paths, gradients, filters, masks all lower to Skia primitives), shared with the rest of page rendering.
- **Known perf cliffs**: large SVGs with thousands of paths are slow to render and to zoom/pan (open Chromium issues on record) — browsers mitigate with raster caching, at the cost of memory and invalidation complexity.

Net: maximum fidelity and dynamism, paid for with a DOM, a style engine pass, script bindings, and megabytes of always-on machinery — costs that only make sense when the input is an arbitrary stranger's document.

## 5. Morph vs Qt vs Chrome

| | Qt | Chrome | Morph (this plan) |
|---|---|---|---|
| Input model | static files, known subset | arbitrary documents from strangers | static files, known at build |
| DOM / scripting | none (renders first frame) | full SVG DOM + V8 | none in v1; JS-driven animation later via the imperative animation API, not a live SVG DOM |
| Animation | none (static 1.2 Tiny) | SMIL + CSS + Web Animations | CSS `@keyframes` on presentation attributes (existing engine); JS later; SMIL never |
| Rendering backend | `QPainter` CPU raster | Skia (shared with page) | Morph's own GL pipeline (SDF shaders, batch renderer, Flash/Forge) |
| Vector crispness on scale | lost (raster-at-load in Quick) | kept (re-raster / re-record) | kept (re-tessellate + per-size cache) |
| Subset policy | fixed 1.2 Tiny, forever | everything (plus compat quirks) | grows by build-gated capabilities; `morph check` names unsupported elements |
| Binary cost of SVG | QtSvg module always linked when used | whole browser | only the math of features used (`MORPH_FEATURE_SVG_*`) |
| 1:1 look target | no (subset rendering) | self (is the reference) | Chrome screenshots on a fixed corpus |

## 6. Why Morph is different (and why that decides the design)

The same AOT argument from the animation work applies,_move for move:

- **Closed input set.** `morph build` sees every `.svg` and every inline `<svg>` in the app. Chrome must support everything because page N+1 is unknown; Qt froze a subset for the same reason (can't negotiate per app). We can support *exactly what the app uses* — and prove it at build.
- **Bake, don't rasterize at runtime.** Static SVG geometry can be parsed and tessellated at compile time: ship triangles and paint parameters, not path bytecode plus a tessellator warm-up. What stays dynamic (sizes from layout, `currentColor` from theme, animated attributes) takes the cache path. Neither Qt (raster-at-load) nor Chrome (raster-always-late) can precompute per app.
- **Pay-for-what-you-use is enforceable.** A third-party engine links its whole feature matrix (resvg: full static SVG1.1 always). Our engine compiles per capability behind defines — an icon-only app never links gradient-mesh, filter-graph, or text-on-path math.
- **No compat burden.** Chrome carries decades of SVG quirk-compat; Qt carries its frozen subset forever. We define the supported subset in `morph check` diagnostics and reject the rest by name at build — silent misrendering is a build error, not a runtime surprise.

## 7. Why our own engine instead of a third-party one

We evaluated the usual candidates — resvg (most complete static renderer), nanosvg/lunasvg (small parsers/rasterizers), ThorVG (retained-mode vector scene graph with its own animation and threading). We decided against all of them, for reasons that compound:

1. **Pipeline control.** Our renderer is not Skia and not a generic canvas: SDF shader stack, batch renderer, Flash full-clear vs Forge damage-tracked present, per-size caches like the glyph cache. A foreign engine rasterizes to its own bitmap on its own schedule — we would lose damage tracking, pay a texture upload per frame for animated content, and re-raster on every scale instead of reusing tessellation.
2. **Dead-code elimination no vendor offers.** Third-party engines are feature-complete libraries: linking resvg links *all* of static SVG 1.1 even for an app with twelve monochrome icons. Our "final app contains only the math of what you use" requirement needs build-driven feature defines over our own code — impossible to retrofit onto someone else's crate without forking it, at which point it is our engine anyway.
3. **Static linking and dependency hygiene.** Vendoring a Rust SVG stack (resvg pulls `tiny-skia`, font stacks, and more) or a C++ one (ThorVG with its own build) into every Morph binary contradicts the lean-binary story (162KB hello-world). Our engine is a few thousand lines of path math plus reuse of FreeType/HarfBuzz already shipped.
4. **Subset honesty.** nanosvg-class parsers win size by silently approximating (no full arc handling, weak gradients). We prefer the opposite contract: a declared subset, enforced by `morph check`, rendering 1:1 with Chrome inside the subset — rather than a broad subset rendering approximately.

Our decision as a team: we own the parser, the tessellator, and the paint lowering; we reuse FreeType/HarfBuzz for `<text>` shaping and the existing GL backend for raster. No new runtime dependency.

## 8. What we will build

### Phase 0 — static subset (powers the [Icons & SVG](icons-svg.md) track)

Elements: `svg`, `path`, `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `g`, `use` (same-document), `symbol`, `defs`. Attributes: `viewBox`, `fill` (incl. `currentColor` inheriting text color and `none`), basic `stroke` + `stroke-width`, `transform` (translate/scale/rotate), `opacity`, `fill-rule`. Path commands: `M L H V C S Q T A Z` both cases. Tessellate to triangles on CPU at build for static SVGs; cache per (path, size) at runtime for layout-sized ones — the glyph-cache pattern from `icons-svg.md`.

### Phase 1 — full-fidelity static rendering (the 1:1 push)

`linearGradient` / `radialGradient` (both `objectBoundingBox` and `userSpaceOnUse`), `clipPath`, `mask` (luminance + alpha), `pattern` (static tiling), extended stroke (`stroke-dasharray`, `linecap`, `linejoin`, `miterlimit`), `fill-opacity` / `stroke-opacity`, `<text>` (shaped with the shipped HarfBuzz/FreeType pipeline — no new text engine), `preserveAspectRatio` full align/meet-or-slice semantics, nested `svg`, `image` inside SVG (raster only, same formats as `<img>`).

Explicitly deferred past 1:1: `filter` primitives (feGaussianBlur first, full filter graph later — each primitive is its own gated capability), `foreignObject`, color fonts inside SVG, SVG-as-image external subresources (matches Chrome's no-external-resources rule for image context).

### Phase 2 — CSS animation on SVG (reuses the animation engine)

Presentation attributes are style: `fill`, `stroke`, `opacity`, `transform` on SVG nodes animate with the existing `@keyframes` / `transition` machinery (`fixing-animation.md` §3). `transform-box` / `transform-origin` semantics per SVG2 for correct rotation centers. No new timing code — Phase 2 of the animation plan (spec-exact easings, shared solver) covers SVG ticks too.

### Phase 3 — JS-driven SVG animation (far future)

When the imperative animation API lands (`animations.md` scope), SVG attributes become animatable targets through it — attribute writes flow through the same invalidation → tessellation-refresh → damage path as CSS animation. This is *not* a live SVG DOM with scriptable `SVGElement` objects (Chrome's model): it is JS writing attributes on Morph nodes, exactly like JS writing styles on HTML nodes. SMIL (`<animate>`, `<animateTransform>`) is never planned — CSS animations already cover declarative motion, and one declarative system is enough.

## 9. Chrome-look parity strategy (what 1:1 actually means)

1:1 means pixels, not architecture: for every file in the parity corpus, Morph's render must match a Chrome screenshot within tolerance at the same viewport/DPR. Method:

1. **Corpus:** a fixed set of SVGs covering every supported element/attribute/edge case (arcs with large-arc/sweep flags, degenerate paths, gradient edge stops, `meet` vs `slice`, `currentColor` chains). Corpus lives in-repo; additions ship with the feature that needs them.
2. **Reference:** headless Chrome screenshots at 1x and 2x DPR, checked in or regenerable via script.
3. **Spec source:** SVG2 for boxes and layout semantics, SVG 1.1 for rendering model details where SVG2 is silent — same specs Chrome implements, minus legacy quirks (no quirk-compat: where Chrome renders a historical bug, we render the spec and record the divergence).
4. **Anti-aliasing contract:** coverage-based AA matching Chrome's edges; `shape-rendering` honored (`auto` default, `crispEdges` disables AA) since icon work needs it.
5. **Divergence log:** every known Chrome-vs-Morph pixel difference is a filed entry with corpus case — never silent.

## 10. Performance design

- **Build-time tessellation for static SVGs.** File-referenced SVGs with fixed geometry are parsed and flattened (arc → beziers → line segments at build-chosen tolerance) during `morph build`; the binary carries triangles + paint records. Runtime tessellation runs only for layout-dependent sizes and animated attributes.
- **Per-(path, size) tessellation cache** at runtime (glyph-cache pattern): HiDPI and resize land in cache, not in recompute.
- **Compositor routing for SVG transforms/opacity.** SVG nodes are `MorphNode`s — `transform`/`opacity` animation on them takes the same compositor path as HTML (see `fixing-animation.md` §3.4 and the planned compositor clock). Re-tessellation never runs for a pure transform/opacity animation.
- **Raster caching for expensive subtrees.** Filtered/masked groups cache to FBO tiles and re-composite while inputs are unchanged — the Forge damage model extended to SVG groups, not a parallel cache.
- **No per-frame parsing, ever.** Parsed SVG becomes IR once (build) or a retained node subtree (dev hot-reload); the tick path touches only floats.

## 11. Dead-code elimination ("only the math of what you use")

Mirrors the existing `feature_set.rs` + `#ifdef MORPH_FEATURE_*` pattern:

- Capabilities map 1:1 to defines: `MORPH_FEATURE_SVG_PATH`, `_SVG_ARC`, `_SVG_GRADIENT`, `_SVG_CLIP`, `_SVG_MASK`, `_SVG_PATTERN`, `_SVG_FILTER_*` (per primitive), `_SVG_TEXT`, `_SVG_ANIMATE_CSS` (animation hooks on SVG nodes).
- `morph build` scans the app's SVG corpus (files + inline `<svg>`) for used elements/attributes/commands and emits exactly the defines needed. An icon-only app (paths + fills) links no gradient, filter, mask, or text math.
- `morph check` reports per-file feature usage (`svg-gradient: linearGradient used by 3 files → +gradient math`) so size growth is visible and attributable — the same philosophy as the animation drop warnings.
- Estimate: path/fill/stroke core ≈ single-digit KB `.text`; each capability adds KB-scale increments, never a step function. Full static SVG 1.1 stays well under resvg-equivalent weight because tessellation and text shaping reuse shipped code.

## 12. Edge cases (locked behavior — do not regress once shipped)

- **`viewBox` + `preserveAspectRatio`:** default `xMidYMid meet` letterboxes; `slice` crops; `none` stretches. Wrong default here misrenders every icon — test the full align matrix.
- **`currentColor` chains:** `fill="currentColor"` inherits the node's text color, including through `<use>` and theme changes at runtime (no bake-through of the resolved color).
- **`<use>` / `<symbol>`:** same-document references only in v1; style inheritance crosses the use boundary per spec (inheritable properties take the `<use>` context, not the `<defs>` context).
- **Gradient units:** `objectBoundingBox` (default, relative, the common case) vs `userSpaceOnUse` (absolute, must survive ancestor transforms) — different math, different bugs.
- **Arc flags:** `A` with large-arc/sweep combinations plus endpoint/center parameterization edge cases (zero radii → line, identical endpoints) per spec, not per intuition.
- **Degenerate paths:** empty `d`, lone `M`, unclosed subpaths with `Z` semantics for fill vs stroke.
- **`%` lengths and user units:** resolve against viewport/`viewBox` per attribute (x/y/width/height vs stroke-width rules differ).
- **`<text>` shaping:** reuse HarfBuzz/FreeType; `textLength` + `lengthAdjust` spacing/xAdvance adjustments; font fallback identical to UI text.
- **External references:** no network, no external files in image context (Chrome parity); same-document `url(#id)` only in v1.
- **Invalid input:** malformed path data or unknown elements warn in `morph check` naming file + element + line; runtime renders the valid remainder (never a blank box without a diagnostic).
- **HiDPI:** tessellation tolerance scales with DPR; crispEdges honored; 1x/2x corpus screenshots gate both.
- **Animated attributes:** animating `d` (path morphing) is *not* in scope — only paint/transform/opacity attributes animate; `d` changes re-tessellate discretely (Chrome behavior for non-interpolable values).

## 13. FAQ

**Why not just embed resvg / ThorVG?**
Three compounding reasons (§7): our GL pipeline (damage tracking, SDF shaders, batching) can't consume a foreign rasterizer without losing frame-level optimizations; no vendor offers build-driven per-feature elimination, so every app would link all of static SVG 1.1; and vendoring a second SVG stack contradicts the lean-binary story. Forking one to retrofit DCE would cost more than owning the math.

**Will SVG support bloat my binary?**
Only by what is used (§11). No SVG at all → zero SVG code (feature defines stay off, like `MORPH_FEATURE_ANIMATION` today). Twelve monochrome icons → path/fill math, KB-scale. Gradients/filters/text link if and when the corpus uses them, and `morph check` shows the attribution.

**Will my SVG look exactly like in Chrome?**
Inside the supported subset, yes — that is the §9 contract, gated by corpus screenshots at 1x/2x. Outside the subset, `morph check` rejects the file's offending elements at build instead of misrendering. Chrome-quirk rendering (historical bugs) is intentionally not replicated; divergences are logged.

**Can SVG be animated?**
CSS `@keyframes` / `transition` on paint and transform attributes (Phase 2, reusing the animation engine — no new timing code). JS-driven animation arrives with the imperative animation API (Phase 3). SMIL (`<animate>`) will not be supported — one declarative system is enough. Path-morphing (`d` interpolation) is out of scope.

**`<img src="app.svg">` vs inline `<svg>`?**
Both are supported, with Chrome's context split: inline SVG is styleable (`currentColor`, CSS animation) and interactive; SVG-as-image is rasterized without scripts, interaction, or external subresources. Same file, different context rules — same as browsers.

**Scripts inside SVG?**
Never executed — same as Chrome's SVG-as-image context, applied everywhere. There is no SVG script engine on any roadmap.

**Filters? Text? Masks?**
Gradients, clips, masks, patterns, and shaped `<text>` are Phase 1 (static, gated per capability). Filter primitives start with `feGaussianBlur` and grow per-primitive after 1:1 lands. `foreignObject` is not planned.

**How does this relate to `morph-icons`?**
The icon system ([Icons & SVG](icons-svg.md)) becomes the first consumer of Phase 0: `morph-icons` ships SVGs that compile through this engine, tree-shaken as before. Icons stay monochrome-path-first; the engine is what lets them (and later full illustrations) render 1:1.

**Why is Qt's subset (SVG 1.2 Tiny) not enough?**
It freezes out gradients-as-designed, masks, and animation — the features real-world icon sets and illustrations use. And Qt rasterizes at load, losing vector crispness on scale. Our subset is build-determined per app instead of frozen for everyone.

**Why a custom engine — what exactly is wrong with using resvg or ThorVG?**
Four concrete mismatches, not philosophy (§7): neither engine targets a damage-tracked GL renderer, so animated content would cost a full texture upload per frame; neither offers build-driven per-feature elimination, so every app links all of static SVG 1.1; pulling resvg drags a Rust-side dependency tree (tiny-skia, font stack) into a C++ link, and ThorVG brings its own threading and scene-graph model that fights the `MorphNode` tree; and tracking upstream releases for security fixes becomes a permanent tax. Owning a few thousand lines of path math over the already-shipped FreeType/HarfBuzz/GL stack is cheaper over the project's lifetime.

**Will SVGs exported from Figma or Illustrator work?**
Static vector content generally yes: groups, transforms, clip paths, gradients, and expanded strokes are all in scope. Typical export cruft to watch: filter-based drop shadows (need `feGaussianBlur`, phased), text left as `<text>` (needs the referenced fonts at runtime — outlining text to paths avoids that), and base64-embedded raster images (supported as raster, same formats as `<img>`). `morph check` names anything unsupported with file, element, and line instead of misrendering.

**What happens to unsupported features — a silent hole in the picture?**
Never silent. Unsupported elements fail the build diagnostic naming file + element + line; at runtime the valid remainder renders. A blank box without a diagnostic is treated as a bug in the engine, not author error.

**Will SVG stay crisp on HiDPI and window resize?**
Yes — unlike raster-at-load approaches, geometry re-tessellates per size into the per-(path, size) cache, and the parity corpus gates 1x and 2x screenshots. `shape-rendering="crispEdges"` is honored for pixel-art-style icons.

**Can SVG follow dark mode / theming?**
Inline `<svg>` inherits `currentColor` from surrounding text styling, so themed icons work with zero props — same mechanism as `morph-icons`. SVG loaded as an image cannot inherit page styles (Chrome parity: image context is sealed).

**A designer handed over a SMIL-animated SVG — will it play?**
It renders as its first frame. `<animate>` / `<animateTransform>` elements are flagged by `morph check` with guidance to convert the motion to CSS `@keyframes`, which the engine supports on the same attributes. Self-animating image-context SVG is not on any roadmap.

**Text with custom fonts or emoji inside SVG?**
`<text>` shapes through the same HarfBuzz/FreeType stack as UI text, with the same font fallback — custom fonts work if the app ships them. COLR/CPAL color-emoji glyphs are out of scope initially (same position as `icons-svg.md`).

**Do `<title>` and `<desc>` do anything?**
No visual effect (Chrome parity). They are parsed and retained so the future screen-reader work (`platform/accessibility.md`) can expose them as accessible names — decorative SVGs should still carry `aria-hidden` equivalents when that lands.

**Filters, drop shadows, blurs?**
`feGaussianBlur` ships first (covers glows and soft shadows); the remaining primitives grow per-capability afterward, each behind its own `MORPH_FEATURE_SVG_FILTER_*` define. For UI shadows, the CSS `box-shadow` path (see `more-properties.md`) is preferred where it applies — no SVG needed.

**How much binary weight does each icon add?**
Roughly: the path/fill core is amortized once (KB-scale), then each icon adds its tessellated geometry — tens to hundreds of bytes for typical icon paths. `morph-icons` tree-shakes unused icons before the engine ever sees them, and `morph check` attributes feature cost per file, so growth is always traceable to its cause.

## 14. Current state

| Piece | State |
|---|---|
| Raster image decode (`stbi_load`, no SVG) | ✅ Shipped |
| SVG parsing / DOM / rendering | ❌ Not built (this page) |
| `<svg>` subset + tessellation + cache (Phase 0) | ❌ Planned — also tracked in [Icons & SVG](icons-svg.md) |
| Full-fidelity static set: gradients, clip, mask, pattern, `<text>`, `preserveAspectRatio` (Phase 1) | ❌ Planned |
| Parity corpus + Chrome screenshot gating (§9) | ❌ Planned |
| Per-capability `MORPH_FEATURE_SVG_*` elimination (§11) | ❌ Planned |
| CSS animation on SVG attributes (Phase 2) | ❌ Planned (reuses animation engine) |
| JS-driven SVG animation (Phase 3) | ❌ Far future (needs imperative animation API) |
| SMIL / scripts in SVG | ❌ Explicit non-goal |

## 15. Open questions

- **Subset vs 1:1 tension with [Icons & SVG](icons-svg.md)** — that page says full SVG "is a browser, and we are not building a browser." This page scopes 1:1 to *rendering pixels within the supported subset*, not to DOM/scripting — confirm both pages carry that sentence so the positions never read as contradictory.
- **`<text>` subset** — full shaping via HarfBuzz is reuse, but `textPath`, `tspan` positioning, and bidirectional edge cases each add surface. Which ships in Phase 1 vs later?
- **Filter subset order** — `feGaussianBlur` first is assumed (shadows/glows cover most UI); confirm against real icon/illustration corpora before locking.
- **Authoring model** — file-referenced `.svg` only, or also inline `<svg>` in `.mx`? Inline enables `currentColor`/theming ergonomics but needs JSX-namespace handling in the parser. (Recommendation: files first, inline second.)
- **`morph icons:add` pipeline** — project icon sets compile through Phase 0; who owns validation diagnostics shared with `morph check`?
- **`<use>` cross-document references** — same-document only in v1; is cross-file `use href="icons.svg#id"` ever needed, or does tree-shaking make it redundant?
- **Animated SVG-as-image** — SMIL-free means no self-animating image files; CSS-animated inline SVG covers motion. Confirm no author demand for animated image-context SVG before closing this.

## 16. Build steps (when picked up)

1. SVG parser (XML subset → retained scene: elements, attributes, path-data tokenizer with arc handling) + `morph check` validation diagnostics naming file/element/line.
2. Phase-0 tessellator (flatten beziers/arcs at tolerance, triangulate fills, stroke expansion) + per-(path, size) cache + `<svg>` node type in layout/paint/flatten.
3. `currentColor`, `viewBox` + `preserveAspectRatio`, transforms — wire into the existing style/layout system.
4. Parity corpus harness: corpus files + headless-Chrome screenshot script + tolerance gate in CI.
5. Phase-1 capabilities in order: gradients → clip → mask/pattern → extended strokes → `<text>` → nested svg/image; one `MORPH_FEATURE_SVG_*` each with size attribution in `morph check`.
6. Phase-2: presentation-attribute animation through the existing engine (no new timing code) + `transform-box` semantics.
7. Validation app: icon/illustration browser at 1x/2x DPR against Chrome screenshots; `--morph-self-test` 0 failures; binary-size report per capability.

## 17. References

- Product track: [Icons & SVG](icons-svg.md) (subset, `morph-icons`, `morph icons:add`), [Tooling](../tooling/vscode.md) (package angle).
- Engine-adjacent: [Animations](../css/animations.md) (choreography; Phase 3 hook), [Animation Engine](fixing-animation.md) (timing engine Phases 0–2 reused by SVG animation), [Performance](../rendering/performance.md), [Forge Tile Pool](../rendering/forge-tile-pool.md) (damage model for SVG groups).
- Specs: SVG 2 (boxes, `transform-box`, geometry properties), SVG 1.1 2nd ed. (rendering model, arc parameterization, gradients/masks), CSS `color` (`currentColor`), `shape-rendering`.
- Qt: `QSvgRenderer` / `QSvgWidget` docs (`doc.qt.io/qt-6/svgrendering.html`) — static SVG 1.2 Tiny, no scripts/DOM/animation; QtSvg carries third-party XSVG code.
- Chrome: Blink SVG (`LayoutSVGShape` family, SVG DOM + V8 bindings), Skia backend, SMIL/CSS/Web-Animations sources, inline-vs-image context split; WPE/WebKit SVG-engine rewrite notes for layout-architecture reference.
