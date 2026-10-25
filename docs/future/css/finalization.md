# CSS Finalization for Basic Apps

**Status:** future · **Priority:** high · **Depends on:** [CSS Cascade](cascade.md), [Animation Engine](fixing-animation.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Morph can already build basic apps. The included calculator, login, budget, and IP-checker examples use the existing CSS surface without a browser or WebView. This record is about the remaining browser basics that make a *normal* app fight the stylesheet — not about full web parity.

The target is a basic CRUD/dashboard/chat/settings app that can use focus and disabled form states, readable multi-line text, truncation, image backgrounds, `calc()`/`aspect-ratio`, and flex reordering without JavaScript workarounds. CSS custom properties, `::before`/`::after`, the full cascade, and animation choreography remain separate tracks.

## Definition of done

A basic app can rely on all of these without an unstated workaround:

- **Forms:** `:focus`, `:focus-visible`, `:disabled`, `:checked`, attribute selectors, and sibling selectors work in built apps.
- **Text:** `line-height`, `font-style`, `text-decoration`, `text-transform`, `white-space`, `text-overflow`, and basic wrapping controls work; semantic text tags render as documented.
- **Images:** `background-image: url(...)`, `background-size`/`position`/`repeat`, and `object-fit` work for local assets.
- **Sizing:** simple `calc()` expressions, `aspect-ratio`, and the `inset` shorthand work for common layouts.
- **Flex:** `inline-flex`, `order`, and one- and two-axis gaps work.
- **Diagnostics:** unsupported values and selectors warn instead of disappearing silently.
- **Docs:** the supported-property, transition, flexbox, animation, Tailwind, and element pages describe what the build actually does.

## What already ships

| Area | Shipped today |
|---|---|
| Box model | `width`/`height`, min/max, margins (including horizontal auto), padding, `box-sizing` |
| Flexbox | direction, grow/shrink/basis, wrap, justify/align, one-value `gap` |
| Grid | template/auto tracks, areas, line placement, `repeat()`, `minmax()`, `fit-content()`, per-axis gaps |
| Positioning | static, relative, absolute, fixed, sticky; `z-index`; per-axis overflow and scrolling |
| Visuals | background colors, linear/radial/conic/repeating gradients, borders, per-corner radius, `box-shadow`, `outline`, `opacity`, transforms, `text-shadow` |
| Typography | `font-size`, `font-weight` (normal/bold only), `color`, `text-align`, `letter-spacing`, text-shadow, inheritance for that subset |
| Interaction | `:hover`/`:active` on the matched element, transitions, keyframe animations |
| Responsive | width-based `@media` variants resolved into breakpoint styles |
| Colors | named colors, hex, `rgb()`/`rgba()`, and `hsl()` (normalized by LightningCSS before the IR) |

The missing pieces below are the part that still surprises browser-trained authors.

## 1. State and selector coverage

Only `:hover` and `:active` on the final compound are represented in build output. Supported structural matching is limited to tag/class/id/universal, descendant/child combinators, and comma groups.

Missing:

- `:focus` and `:focus-visible`
- `:disabled` and `:checked`
- `:first-child`, `:last-child`, `:nth-child()`, `:not()`
- attribute selectors such as `[type="checkbox"]` and `[disabled]`
- sibling combinators `+` and `~`
- ancestor/group hover such as `.card:hover .card-title`
- `:hover` and `:active` rules inside `@media`

This blocks CSS-only focus rings, disabled buttons, custom checkboxes, list separators, zebra striping, and parent-hover cards. Focus/blur events already work, so a JS state/class workaround is possible; it should not be required for basic styling.

Design notes:

- Extend the selector matcher to carry attributes and supported pseudo-classes, not just a structural string.
- Emit state deltas for focus/disabled/checked alongside the existing hover/active deltas.
- Keep specificity for new selector forms and decide how state deltas compose.
- The runtime already has hover/active and ancestor-hover machinery; the missing build-path work is selector metadata and IR/codegen emission. Do not count runtime-only support as shipped.
- Hover/active inside width media variants should either become responsive state variants or produce a clear warning. Do not silently skip them.

Acceptance: login/settings fixture with focus ring, disabled button, custom checkbox, first/last row, sibling label/input, and parent-hover card; build and dev output must match.

## 2. Typography and text layout

Missing properties:

- `line-height`
- intermediate `font-weight` values (only normal/bold faces exist; 100–500 render regular, 600–900 bold)
- `font-family` and, later, `@font-face` / custom font loading
- `font-style`
- `text-decoration` (underline, line-through, overline)
- `text-transform`
- `white-space` (`nowrap`, `pre`, `pre-wrap`, `pre-line`)
- `text-overflow: ellipsis`
- `word-break` / `overflow-wrap`

Impact: multi-line spacing, truncating list items, underlined links, italic emphasis, uppercase labels, and code/monospace text. These are ordinary app screens, not advanced effects.

Related gaps:

- `<em>`/`<i>`, `<u>`/`<ins>`, `<del>`/`<s>`, `<code>`/`<kbd>`/`<samp>`, and `<sub>`/`<sup>` currently render essentially plain; only `<strong>`/`<b>`, `<small>`, and `<mark>` have their documented effect.
- Tailwind `text-xs` through `text-4xl` emits a companion `line-height` that the IR drops. Fixing `line-height` fixes the utility too.
- `font-family` needs a font-selection model before `@font-face`; start with bundled/system families, then asset loading.
- `font-style: italic` needs an italic face or a deliberate synthesized fallback; do not fake it in text measurement without checking against the renderer.

Acceptance: a text-heavy fixture with multi-line body copy, a truncated one-line title, an underlined link, italic text, a code span, and a `<sub>`/`<sup>` example. Compare wrapping and baselines against Chrome.

## 3. Background images and image fitting

Today `background-image` accepts gradients only. Missing:

- `url(...)` image sources
- `background-size` (`cover`, `contain`, explicit lengths)
- `background-position`
- `background-repeat`
- `object-fit` on `<img>` (`cover`, `contain`, `fill`, `none`, `scale-down`)
- `object-position`

Impact: hero/card image backgrounds require an absolutely positioned `<img>` and overlay content; avatars and cards distort when both dimensions are set. The workaround is usable but not CSS.

Design notes:

- Reuse the existing `<img>` texture cache and asset resolver; do not create a second image pipeline.
- Start with one background image layer. Multiple background layers and image-plus-gradient stacking can follow.
- Define texture memory/eviction behavior and remote URL policy alongside the feature.
- Keep `border-radius` clipping behavior consistent with `<img>`.

Acceptance: a local hero image with `cover` and center positioning, a rounded avatar with `object-fit: cover`, and a card that falls back visibly when the asset is missing. Verify the binary size impact stays within budget.

## 4. Sizing and layout conveniences

Missing or incomplete:

- `calc()` for simple length expressions, plus general `min()`/`max()`/`clamp()`
- `aspect-ratio`
- the `inset` shorthand
- `display: inline-flex`
- `order`
- flex `row-gap`/`column-gap` and two-value `gap`

Important current sharp edge: one-value `gap: 12px` works in flex, but `gap: 8px 16px`, `row-gap`, and `column-gap` only reach the grid path. Flex still reads the single `gap` field, so a two-value gap silently removes flex spacing. Fix this before documenting two-axis flex gaps.

Design notes:

- `calc()` should start with layout lengths (`%`, px, and unitless combinations) and resolve at layout time alongside existing percentage units. Do not attempt a full CSS math engine in the first pass.
- `aspect-ratio` interacts with auto width/height, flex shrink, and images; define the definite-size precedence before implementation.
- `order` should be a stable sort by order then DOM order for both flex and grid.
- Extend the flex style fields to carry row/column gaps, or teach flex to consume the existing grid gap fields without changing grid behavior.
- `inline-flex` can follow the existing inline-block/flex layout split; Tailwind already emits the name but the style emitter has no mapping.

Acceptance: a sidebar `width: calc(100% - 280px)`, a 16:9 card, a reordered toolbar, and a wrapped flex grid with different row/column gaps. Add layout tests and a broken-value lint test.

## 5. Form controls and form styling

`:focus` styling is covered by the state work above. Two non-CSS dependencies still block normal forms:

- `<select>` and `<textarea>` are registered stubs; `morph check` flags them.
- Input placeholder/selection styling is not available beyond the built-in rendering; pseudo-elements remain out of scope.

`<input>` supports text/password, caret, selection, undo/redo, clipboard, `disabled`, `maxLength`, and `minLength`. Focus and blur events are wired, but focus is only visible through the caret today. A focus ring should be expressible with the existing outline support once `:focus` lands.

Acceptance: a settings form with text input, password input, select, textarea, disabled submit button, and visible keyboard focus. This closes the “basic app” form story.

## 6. Polish and cross-cutting gaps

These are common in polished apps but are not required to call the basic target done:

- `filter`, `backdrop-filter`, `clip-path`
- general `currentColor` (currently special-cased for outline and text-shadow) and CSS-wide keywords (`inherit`, `initial`, `unset`, `revert`) on ordinary properties
- per-property transitions, transition delays, `cubic-bezier()`, and `steps()`; transitions currently use one duration/easing for the entire style delta
- media breadth: `prefers-color-scheme`, orientation, pointer, `not`/`or`, and comma-separated query lists (width remains the only fully supported axis)
- cursor values beyond `default`, `pointer`, and `text`; `user-select` (the Tailwind resolver still contains `select-*` utilities that the IR drops)
- per-keyframe `animation-timing-function`; the animation engine doc tracks the shared timing-function work
- Tailwind no-op names: `inline-flex`, `select-*`, and `cursor-not-allowed` currently resolve in the Tailwind map but do not produce the expected runtime style (`text-*` line heights are covered by the typography work)

Animations already cover transforms, opacity, colors, radius, shadows, gradients, and several layout values. The remaining animation work belongs to [Animation Engine](fixing-animation.md), not this document.

## 7. Diagnostics must stop lying

The CSS linter currently validates property *names*. Unsupported values and selectors are silently dropped:

- `background-image: url(...)` is a known property, so it produces no `mx-css-prop` warning even though the value is ignored.
- `input:focus`, `.parent:hover .child`, and sibling selectors produce no warning and no style.
- Tailwind utilities whose target property is unsupported are skipped silently.

This makes the surface look more complete than it is. Add value- and selector-level diagnostics:

- `mx-css-value` for a known property with an unsupported value; include the supported forms.
- `mx-css-selector` for unsupported pseudo-classes, attribute selectors, sibling combinators, and state selectors that cannot be emitted.
- A warning when a hover/active rule inside `@media` is skipped.
- A warning when a Tailwind utility resolves to a property the IR does not accept, with the utility name and the reason.
- Keep suggestions and source spans consistent with `mx-css-prop`.

These checks should be warn-by-default, with a strict mode available for teams that want CI failures.

## 8. Implementation order

| Phase | Scope | Why first |
|---|---|---|
| 0 | Diagnostics + docs truth | Prevents false “supported” impressions and keeps the supported list honest |
| 1 | Typography and text layout | Independent of selector architecture and immediately improves every app |
| 2 | Focus/disabled/checked + attributes/siblings/ancestor hover | Unblocks forms, lists, navigation, and card interactions |
| 3 | Background images + `object-fit` | Reuses the image pipeline; unlocks the most visible missing basic |
| 4 | `calc()`/`aspect-ratio`/`inset`, `inline-flex`, `order`, flex gaps | Removes common layout workarounds without an engine rewrite |
| 5 | `select`/`textarea`, cursor/user-select, media breadth, filters/clip-path | Form completeness, accessibility, and polish |
| 6 | Per-property transitions and timing functions | Shared with the animation-engine timing work; not a basic-app blocker |

## Acceptance and verification

- New fixture: `tests/runtime/css/finalization/` covering text truncation, focus/disabled/checked, sibling/attribute selectors, parent hover, URL background, `object-fit`, `calc()`, `aspect-ratio`, `inline-flex`, `order`, and two-axis flex gaps.
- Rust tests for selector matching, specificity, property parsing, and diagnostics (line/column spans, suggestions).
- Codegen assertions for each new IR field and state delta; dev/build parity.
- C++ layout tests for flex gap axes, `order`, `aspect-ratio`, and `object-fit`; screenshot parity against Chrome for text and images.
- `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --all-targets --all-features -- -D warnings`, `./tests/runtime/run-selftests.sh`, and the binary-size budget.
- Update user docs and both registries in the same change as each shipped phase.

## Explicit non-goals

- CSS custom properties/`var()`
- `::before` / `::after` and other pseudo-elements
- Full cascade/`!important`/runtime stylesheet re-resolution — tracked in [CSS Cascade](cascade.md)
- Animation choreography, springs, enter/exit, and shared elements — tracked in [Animations](animations.md)
- Full browser parity, table layout, SVG, and advanced at-rules (`@supports`, `@layer`, `@container`)

Custom font loading (`@font-face`) is a later typography extension, not part of the first `font-family` pass.

## Decision log

| Date | Decision | Rationale |
|---|---|---|
| 2026-10-25 | Treat basic apps as viable today; finalize the listed gaps before adding advanced CSS | Existing examples prove the core; the remaining pain is predictable |
| 2026-10-25 | Fix flex two-axis gaps before documenting them | One-value gap works; two-value gap currently drops flex spacing silently |
| 2026-10-25 | Do not advertise runtime-only ancestor hover as shipped | Built apps must emit the state rules, not just the dev runtime |
| 2026-10-25 | Add selector/value diagnostics before or with the selectors | Silent drops caused the “everything works” false signal |
| 2026-10-25 | Keep typography ahead of image backgrounds | Text gaps affect every screen; image backgrounds are usually workaroundable with `<img>` |

## Related

- [CSS Properties](../../css/properties.md)
- [Transitions](../../css/transitions.md)
- [Flexbox](../../css/flexbox.md)
- [Tailwind](../../css/tailwind.md)
- [Animations](../../css/animations.md)
- [Text Input](../elements/text-input.md)
- [Full CSS Cascade](cascade.md)
