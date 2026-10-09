# CSS Properties

Morph supports a subset of CSS properties that map to native OpenGL rendering. Properties can be set via inline styles, CSS rules, or Tailwind utilities.

## Units

Lengths accept: `px`, bare numbers (treated as px), `%`, `em`, `rem` (1rem = 16px), `vh`, `vw`, and absolute units `pt`, `pc`, `cm`, `mm`, `in` (converted at 96 dpi). `%`, `vh`, and `vw` resolve at layout time against the parent/viewport.

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
| `color` | hex, rgb, named | `#000000` | Text color. Inherits from parent. |
| `border-radius` | px | `0` | Clamped to `[0, 100]` in the SDF shader. |
| `opacity` | 0–1 | `1` | Multiplies with parent opacity. |

## Border

| Property | Values | Default | Notes |
|---|---|---|---|
| `border` | shorthand | — | `border: 2px solid #fff` expands to width/style/color. |
| `border-width` | px | `0` | |
| `border-color` | hex, rgb | `#000000` | |
| `border-style` | `none`, `solid`, `dashed`, `dotted`, `double`, `groove`, `ridge`, `inset`, `outset`, `hidden` | `none` | Only `solid` renders a border — other values are accepted by the linter but draw nothing. |

## Typography

| Property | Values | Default | Notes |
|---|---|---|---|
| `font-size` | px, %, em, rem, bare number | `16px` | Inherits from parent. |
| `font-weight` | `normal`, `bold`, `lighter`, `bolder`, 100–900 | `normal` | Bold uses `DejaVuSans-Bold.ttf`. |
| `text-align` | `left`, `center`, `right`, `justify` | `left` | Inherits from parent. `justify` behaves like `left`. |

Style inheritance: `color`, `font-size`, `font-weight`, and `text-align` cascade from parent to children.

## Layout

| Property | Values | Default | Notes |
|---|---|---|---|
| `display` | `block`, `inline`, `inline-block`, `flex`, `hidden`, `none` | `block` | `none` removes the element from layout and rendering. |
| `overflow` | `visible`, `hidden`, `scroll`, `auto` | `visible` | `auto`/`scroll` enable scroll containers. |
| `position` | `static`, `absolute`, `relative`, `fixed` | `static` | `relative`/`fixed` in progress. |
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
| `gap` | px | `0` | |

See [Flexbox](flexbox.md) for a deep-dive.

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

| Property | Values | Default |
|---|---|---|
| `scrollbar-width` | px | `8` |
| `scrollbar-track-color` | hex, rgb | semi-transparent gray |
| `scrollbar-thumb-color` | hex, rgb | semi-transparent gray |
| `scrollbar-border-radius` | px | `4` |

## Transition

| Property | Values | Default | Notes |
|---|---|---|---|
| `transition` | shorthand | — | `all 0.3s ease-in-out` |
| `transition-duration` | seconds | `0` | |
| `transition-timing-function` | `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` | `ease-in-out` | `ease` is an alias for `ease-in-out`. |

See [Transitions](transitions.md) for details.

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
