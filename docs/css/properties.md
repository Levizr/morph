# CSS Properties

Morph supports a subset of CSS properties that map to native OpenGL rendering. Properties can be set via inline styles, CSS rules, or Tailwind utilities.

## Units

Lengths accept: `px`, bare numbers (treated as px), `%`, `em`, `rem` (1rem = 16px), `vh`, `vw`, and absolute units `pt`, `pc`, `cm`, `mm`, `in` (converted at 96 dpi). `%`, `vh`, and `vw` resolve at layout time against the parent/viewport.

`calc()`, general `min()`/`max()`/`clamp()`, and CSS custom properties are not evaluated yet. Unsupported values on a known property are dropped without a warning today; check the [CSS finalization plan](../future/css/finalization.md) for planned diagnostics.

## Sizing

| Property | Values | Default | Notes |
|---|---|---|---|
| `width` | `auto`, px, % | `auto` | |
| `height` | `auto`, px, % | `auto` | |
| `min-width` | px | `0` | |
| `min-height` | px | `0` | |
| `max-width` | px | — | Constrains child width in layout |
| `max-height` | px | — | |

## Box Model

| Property | Values | Default | Notes |
|---|---|---|---|
| `margin` | px, `auto` | `0` | Shorthand for all sides. `auto` = horizontal centering. |
| `margin-top` | px | `0` | |
| `margin-right` | px | `0` | |
| `margin-bottom` | px | `0` | |
| `margin-left` | px | `0` | |
| `padding` | px | `0` | Shorthand for all sides. |
| `padding-top` | px | `0` | |
| `padding-right` | px | `0` | |
| `padding-bottom` | px | `0` | |
| `padding-left` | px | `0` | |
| `box-sizing` | `content-box`, `border-box` | `content-box` | |

`margin: auto` is re-resolved dynamically on window resize, so centered elements stay centered.

## Visual

| Property | Values | Default | Notes |
|---|---|---|---|
| `background-color` | hex, rgb, named | `transparent` | Transparent by default (no white background). |
| `background`, `background-image` | gradients | — | `linear-gradient()`, `radial-gradient()`, `conic-gradient()` and their `repeating-*` variants, with full stop/hint/position syntax. Verified against Chrome (≤5/255 per channel). URL images and `background-size`/`position`/`repeat` are not supported yet; use an absolutely positioned `<img>` as the current workaround. |
| `border-image` | gradient + slice | — | Gradient border-images evaluate directly in border space; the slice value is accepted but does not change rendering. Linear gradients match Chrome; radial/conic ring evaluation is Morph's simplification of slice-and-stretch. |
| `box-shadow` | shadow list | `none` | `offset-x offset-y blur spread color` + `inset`, comma-separated, up to 4 layers. Verified against Chrome (≤2/255 per channel). |
| `outline` | shorthand | — | Non-layout ring painted outside the border edge: `outline: 2px dashed #4cc2ff`. |
| `outline-width` | px, `thin`, `medium`, `thick` | `3px` | Initial `medium`; only paints with a non-`none` style. |
| `outline-color` | hex, rgb, named, `currentcolor` | `currentcolor` | Resolved from the element's text color at paint time. |
| `outline-style` | `none`, `solid`, `dashed`, `dotted`, `double` | `none` | |
| `outline-offset` | px | `0` | Positive values push the ring outward; negative values pull it inward. |
| `color` | hex, rgb, named | `#000000` | Text color. Inherits from parent. |
| `border-radius` | px | `0` | Clamped to `[0, 100]` in the SDF shader. |
| `opacity` | 0–1 | `1` | Multiplies with parent opacity. |

## Border

| Property | Values | Default | Notes |
|---|---|---|---|
| `border` | shorthand | — | `border: 2px solid #fff` expands to width/style/color. |
| `border-top`, `border-right`, `border-bottom`, `border-left` | shorthand | — | `border-top: 1px dashed red` sets one side's width/style/color. |
| `border-width` | px | `0` | |
| `border-color` | hex, rgb | `#000000` | |
| `border-style` | `none`, `solid`, `dashed`, `dotted`, `double`, `groove`, `ridge`, `inset`, `outset`, `hidden` | `none` | `solid` and `dashed` render (dashed verified pixel-level); other values are accepted but may draw as solid. |

## Typography

| Property | Values | Default | Notes |
|---|---|---|---|
| `font-size` | px, %, em, rem, bare number | `16px` | Inherits from parent. |
| `font-weight` | `normal`, `bold`, `lighter`, `bolder`, 100–900 | `normal` | Only two faces render: `bold` and 600–900 use `DejaVuSans-Bold.ttf`; `normal`, 100–500, `lighter`, and `bolder` render regular. |
| `text-align` | `left`, `center`, `right`, `justify` | `left` | Inherits from parent. `justify` behaves like `left`. |
| `letter-spacing` | px, em, rem, `normal` | `normal` (`0`) | Inherits. Applied to measurement, wrapping, alignment, and glyph advances. |
| `text-shadow` | shadow list | `none` | `offset-x offset-y [blur] [color]`, comma-separated, up to 4 layers. Omitted color means `currentcolor`; blur is approximated by a soft multi-pass draw. |

Style inheritance: `color`, `font-size`, `font-weight`, `text-align`, `letter-spacing`, and `text-shadow` cascade from parent to children.

Not yet supported: `line-height`, `font-family`/`@font-face`, `font-style`, `text-decoration`, `text-transform`, `white-space`, `text-overflow`, `word-break`, and `overflow-wrap`. Multi-line spacing and no-wrap/ellipsis layouts therefore need workarounds; see the [CSS finalization plan](../future/css/finalization.md#2-typography-and-text-layout). Related HTML tags are listed in [Elements Overview](../elements/overview.md#text-elements).

## Responsive Media Queries

Width-based `@media` blocks are resolved at build time into complete style variants. Each node stores one variant per integer breakpoint interval, and the runtime reselects the matching variant when the window is resized, so styles react live rather than only at startup.

```css
.card {
  width: 420px;
  background-color: #1b2433;
}

@media (max-width: 600px) {
  .card {
    width: 160px;
    background-color: #3a1b24;
  }
}

@media screen and (min-width: 601px) {
  .card { background-color: #163a2a; }
}
```

Supported conditions:

- `min-width` and `max-width` (inclusive bounds)
- `width:` / range syntax such as `(600px <= width <= 900px)`
- multiple width conditions joined with `and`
- `screen` and `all` media types, with an optional `only` qualifier

Conditions outside this subset — `orientation`, `pointer`, `prefers-*`, `not`, `or`, or comma-separated query lists — are treated as non-matching rather than applied unconditionally. Responsive variants cover base-element declarations; `:hover` and `:active` rules inside `@media` are currently not represented in breakpoint variants.

## Layout

| Property | Values | Default | Notes |
|---|---|---|---|
| `display` | `block`, `inline`, `inline-block`, `flex`, `grid`, `none` | `block` | `none` removes the element from layout and rendering. `inline-flex` is not implemented yet. |
| `overflow` | `visible`, `hidden`, `scroll`, `auto` | `visible` | `auto`/`scroll` enable scroll containers. |
| `position` | `static`, `absolute`, `relative`, `fixed`, `sticky` | `static` | All five values are implemented; `fixed` and `sticky` are verified by runtime fixtures. |
| `left` | px | — | |
| `right` | px | — | |
| `top` | px | — | |
| `bottom` | px | — | |
| `z-index` | int | — | Negative/block/inline/auto/positive stacking layers. |

## Flexbox

| Property | Values | Default | Notes |
|---|---|---|---|
| `flex-direction` | `row`, `column`, `row-reverse`, `column-reverse` | `row` | |
| `flex` | shorthand | `initial` | `flex: 1` = `1 1 0%`, `flex: none` = `0 0 auto`, `flex: auto` = `1 1 auto` |
| `flex-grow` | number | `0` | |
| `flex-shrink` | number | `1` | |
| `flex-basis` | `auto`, px | `auto` | |
| `flex-wrap` | `nowrap`, `wrap`, `wrap-reverse` | `nowrap` | |
| `justify-content` | `flex-start`, `center`, `flex-end`, `space-between`, `space-around` | `flex-start` | |
| `align-items` | `flex-start`, `center`, `flex-end`, `stretch` | `stretch` | |
| `gap` | px | `0` | One value only in flex. Two-value `gap`, `row-gap`, and `column-gap` currently apply to grid, not flex. |

`order` is not implemented; DOM order is used for flex and grid. `display: inline-flex` is also not implemented yet. See [Flexbox](flexbox.md#limitations) for the current limits.

See [Flexbox](flexbox.md) for a deep-dive.

## Grid

| Property | Values | Default | Notes |
|---|---|---|---|
| `grid-template-columns` | track list | `none` | `100px 1fr 2fr`, `repeat(3, 1fr)`, `repeat(auto-fill, minmax(100px, 1fr))`, `[a] 100px [b]` line names |
| `grid-template-rows` | track list | `none` | same track syntax as columns |
| `grid-auto-columns` | track list | `auto` | implicit column size |
| `grid-auto-rows` | track list | `auto` | implicit row size |
| `grid-auto-flow` | `row`, `column`, `dense`, `row dense`, `column dense` | `row` | `dense` backfills holes |
| `grid-template-areas` | `"a a" "b c"` | `none` | named rectangles; non-rectangular areas are dropped |
| `grid-area` | name or `row / col / row / col` | `auto` | places by named area |
| `grid-column` | `start / end` | `auto` | line numbers (negative counts back), `span n`, named lines |
| `grid-row` | `start / end` | `auto` | same line syntax as columns |
| `grid-column-start`, `grid-column-end` | line | `auto` | single-side placement |
| `grid-row-start`, `grid-row-end` | line | `auto` | single-side placement |
| `align-content` | `start`, `center`, `end`, `stretch`, `space-between`, `space-around`, `space-evenly`, `normal` | `normal` | distributes free space; `stretch` grows `auto` tracks |
| `justify-content` | grid values + flex values | `flex-start` | same distribution on the inline axis |
| `justify-items` | `start`, `center`, `end`, `stretch`, `normal` | `normal` | default item alignment (`stretch` fills the area) |
| `justify-self` | `auto`, `start`, `center`, `end`, `stretch` | `auto` | per-item override (`auto` inherits) |
| `align-items`, `align-self` | shared with flexbox | — | `start`/`end` map onto the flex values |
| `row-gap`, `column-gap` | length | `0` | per-axis gaps; `gap` shorthand applies to both |
| `minmax()` | `minmax(min, max)` | — | min: fixed/`auto`/content keywords; max adds `fr` |
| `fit-content()` | `fit-content(limit)` | — | caps content sizing at the limit |

Track sizes: fixed lengths, `%`, `fr`, `auto`, `min-content`, `max-content`. Stretch items fill their area even with an explicit size (verified against Chrome); definite sizes overflow smaller tracks instead of shrinking. `subgrid` is not supported (declaration dropped). `order` has no effect (DOM order). `justify-content`/`align-items`/`align-self` in `@keyframes` are not supported — use `justify-items`/`justify-self`/`align-content`.

## Cursor

| Property | Values | Default |
|---|---|---|
| `cursor` | `default`, `pointer`, `text` | `default` |

## Scrolling

Scrolling works like a browser. The page scrolls by default whenever content is taller than the window — no CSS needed. A fixed-size box needs `overflow: auto` (or `scroll`); with the default `visible`, overflowing content simply paints outside the box and the wheel passes through to the page underneath:

```css
.panel {
  height: 400px;
  overflow: auto;   /* or overflow-y: auto for vertical only */
}
```

| Value | Behavior |
|---|---|
| `visible` (default) | No scrolling. Overflowing content paints outside the box. |
| `hidden` | Clips overflowing content. Still no scrolling — this is the manual off switch, including on `body` (locks page scroll). |
| `clip` | Like `hidden`, but also forbids programmatic scrolling. |
| `auto` | Scrolls **only when** the content is taller/wider than the box, independently per axis. |
| `scroll` | Always a scroll container. |

`overflow-x` and `overflow-y` are independent: `overflow-x: auto; overflow-y: hidden` gives a horizontal strip that never scrolls vertically. No explicit height is required on scrollers: a box sizes from its children and scrolls as soon as they exceed the available space. An explicit `height`/`max-height` works too and is the common case for fixed panels.

Rules that bite:

- **Mouse wheel scrolls the nearest scroll container** under the cursor; nested scrollers work inside-out, and a spent scroller hands the remainder to its parent (scroll chaining).
- **Shift+wheel scrolls horizontally**, and trackpads scroll both axes.
- **`auto`/`scroll`/`hidden`/`clip` always clip** their children to the box. Don't put them on a box whose children intentionally paint outside it (tooltips, dropdowns, shadows) — leave those `visible` and let the page scroll instead.

Tailwind equivalents: `overflow-auto`, `overflow-scroll`, `overflow-hidden`, `overflow-visible`, plus `overflow-x-auto`, `overflow-x-hidden`, `overflow-x-scroll`, `overflow-y-auto`, `overflow-y-hidden`, `overflow-y-scroll`. Scrollbar appearance is styled with the [`scrollbar-*` properties](#scrollbar) below.

Wheel scrolling jumps one step per notch by default. `scroll-behavior: smooth` (Tailwind: `scroll-smooth`, back to instant with `scroll-auto`) makes the wheel glide toward its target instead — notches accumulate mid-glide, and a spent scroller still chains the remainder to its parent:

```css
.feed {
  height: 400px;
  overflow-y: auto;
  scroll-behavior: smooth;
}
```

Thumb dragging and track paging stay instant (the thumb tracks the cursor 1:1).

To lock page scroll (e.g. behind a modal), put `overflow: hidden` on `body` — it propagates to the viewport like in browsers:

```css
body.modal-open { overflow: hidden; }   /* page holds still */
```

The `*` selector matches every element, so broad rules are one line — but remember the clipping rule above: `* { overflow: auto }` clips *everything*, so prefer scoping to the containers that actually scroll:

```css
* { box-sizing: border-box; }  /* fine: no clipping involved */
```

### Open questions

Ideas and edge cases welcome at [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com). The `overflow`-as-scroll-switch debate is settled: standard `overflow` with per-axis longhands stays — no Morph-specific property.

## Scrollbar

Minimal overlay style like modern browsers: no track background by default, just a slim rounded thumb floating over content. Set `scrollbar-track-color` to bring back a classic track.

| Property | Values | Default |
|---|---|---|
| `scrollbar-width` | px | `8` |
| `scrollbar-track-color` | hex, rgb | transparent (track paints only when set) |
| `scrollbar-thumb-color` | hex, rgb | semi-transparent gray |
| `scrollbar-border-radius` | px | `4` |

## Transition

| Property | Values | Default | Notes |
|---|---|---|---|
| `transition` | shorthand | — | `all 0.3s ease-in-out`. The first duration and easing token are used for the whole style delta. |
| `transition-duration` | seconds | `0` | One duration for every changed property. |
| `transition-timing-function` | `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` | `ease-in-out` | `ease` is an alias for `ease-in-out`. `cubic-bezier()` and `steps()` fall back to the default. |

`transition-property`, `transition-delay`, per-property durations, and multiple independently timed transitions are not represented yet. See [Transitions](transitions.md#limitations) for details.

## Transform

| Property | Values | Default | Notes |
|---|---|---|---|
| `transform` | functions | `none` | `translate()`, `rotate()`, `scale()`, `skew()`, `matrix()` + 3D variants (see [Transforms](transforms.md)) |
| `transform-origin` | px/% | `50% 50%` | |

See [Transforms](transforms.md) for details.

## Animation

| Property | Values | Default | Notes |
|---|---|---|---|
| `animation` | shorthand | — | `name duration timing-function delay iteration-count direction fill-mode` |
| `animation-name` | keyframe name | — | |
| `animation-duration` | seconds | `0` | |
| `animation-timing-function` | easing | `ease-in-out` | `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` (`ease` = alias). `cubic-bezier()`/`steps()` fall back to the default. |
| `animation-delay` | seconds | `0` | |
| `animation-iteration-count` | number, `infinite` | `1` | Fractional values supported (e.g. `2.5`). |
| `animation-direction` | `normal`, `reverse`, `alternate`, `alternate-reverse` | `normal` | |
| `animation-fill-mode` | `none`, `forwards`, `backwards`, `both` | `none` | |

See [Animations](animations.md) for details.

## Not yet supported for basic apps

The following gaps are tracked in the [CSS finalization plan](../future/css/finalization.md):

- **Selectors and states:** `:focus`, `:focus-visible`, `:disabled`, `:checked`, standalone structural pseudo-classes, attribute selectors, sibling combinators, and ancestor/group hover. Only `:hover`/`:active` on the matched element are emitted.
- **Text:** `line-height`, custom fonts, italic, underline/strikethrough, text transform, no-wrap, and ellipsis.
- **Images:** URL backgrounds, background sizing/position/repeat, and `object-fit`.
- **Sizing/layout:** `calc()`, general `min()`/`max()`/`clamp()`, `aspect-ratio`, the `inset` shorthand, `inline-flex`, `order`, and two-axis flex gaps.
- **Polish:** `filter`/`backdrop-filter`/`clip-path`, general `currentColor`, per-property transitions, dark-mode media queries, and cursor values beyond `default`/`pointer`/`text`.

For the app-level triage and implementation order, see [CSS Finalization for Basic Apps](../future/css/finalization.md).
