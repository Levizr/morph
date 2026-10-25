# Transitions

CSS transitions animate style changes when a property value changes — typically triggered by `:hover`.

Morph applies **one duration and one easing to the whole changed style delta**. It does not model a separate timeline per property.

## Basic Usage

```css
.btn {
  transition: all 0.3s ease-in-out;
}
```

This makes every property change on `.btn` animate smoothly over 0.3 seconds.

## Longhand Properties

```css
.card {
  transition-duration: 0.2s;
  transition-timing-function: ease;
}
```

## Transition Shorthand

```css
/* all changed properties, 0.3s, ease-in-out */
transition: all 0.3s ease-in-out;

/* the property name is parsed for compatibility, but timing still applies to the whole delta */
transition: background-color 0.2s ease;
```

Per-property durations and multiple independently timed transitions are parsed but not represented separately: the first duration and easing token win for the whole delta. Use one transition timing for the card/button rather than expecting `color` and `background-color` to run on different clocks.

## Easing Functions

| Value | Behavior |
|---|---|
| `linear` | Constant speed |
| `ease` | Alias for `ease-in-out` |
| `ease-in` | Slow start |
| `ease-out` | Slow end |
| `ease-in-out` | Slow start and end (default) |

`cubic-bezier()` and `steps()` are not supported yet — they fall back to the default easing.

## What Animates

These properties interpolate smoothly (numbers lerp, colors blend):

- `background-color`, `color` — RGBA interpolation
- `margin`, `padding` — per-side numeric interpolation
- `border-width`, `border-radius` — numeric interpolation
- `border-color` — RGBA interpolation
- `font-size`, `gap` — numeric interpolation
- `width`, `height` — interpolated when both values are explicit
- `opacity` — numeric interpolation
- `left`, `right`, `top`, `bottom` — numeric interpolation
- `background-image` (gradient-to-gradient), `border-image`, `box-shadow` — interpolated layer-by-layer when both sides share structure (same kind / same stop or shadow count, same units); mismatched structures flip discretely at the midpoint, like browsers

These properties **snap instantly** (no interpolation):

- `display`, `position`, `flex-direction`, `font-weight`
- `overflow`, `text-align`, `box-sizing`, `border-style`
- `cursor`, `margin-auto`
- grid tracks, placement and alignment (`grid-template-*`, `grid-auto-*`, `grid-column`/`grid-row`, `grid-area`, `align-content`, `justify-items`, `justify-self`, `row-gap`, `column-gap`) — flip discretely at the midpoint in `@keyframes`, like browsers

## Example: Hover Button

```css
.btn {
  background-color: #6366f1;
  color: #ffffff;
  transition: all 0.3s ease;
}

.btn:hover {
  background-color: #4f46e5;
  transform: translateY(-2px);
}
```

## Example: Card Hover

```css
.card {
  border: 1px solid #334155;
  transition: all 0.3s ease;
}

.card:hover {
  border-color: #6366f1;
  transform: translateY(-4px);
}
```

## Hover Scope

Hover transitions work on the element being hovered:

```css
.card-title {
  transition: color 0.3s ease;
}

.card-title:hover {
  color: #6366f1;
}
```

Ancestor/group hover (`.card:hover .card-title`) and hover/active rules inside `@media` are **not emitted in build output yet**; they currently do nothing without a warning. The runtime has ancestor-hover machinery, but the build path does not generate the rules. Track this in [CSS Finalization for Basic Apps](../future/css/finalization.md#1-state-and-selector-coverage).

## Limitations

- `transition-property` and `transition-delay` are informational only; the runtime interpolates the whole state delta with no delay.
- `cubic-bezier()` and `steps()` fall back to the default easing.
- `:hover`/`:active` only apply to the final matched element, not an ancestor or sibling.
- Grid tracks, placement, and alignment do not interpolate; keyframes flip them discretely at the midpoint.

## How It Works Internally

1. On hover enter, the runtime captures the current style as the start state and the hover style as the target
2. Each frame, it interpolates between start and target using the easing function
3. On hover leave, the current interpolated style becomes the new start and the base style becomes the target
4. Mid-transition direction changes capture the current interpolated position for smooth reversal

Transitions run per-node — parent and child can transition independently.

## Duration 0

Setting `transition-duration: 0` (or omitting the transition) disables animation — style changes snap instantly. This is the default behavior and is fully backward-compatible.
