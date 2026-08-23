# Transitions

CSS transitions animate style changes when a property value changes — typically triggered by `:hover`.

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
/* all properties, 0.3s, ease-in-out */
transition: all 0.3s ease-in-out;

/* specific property */
transition: background-color 0.2s ease;

/* multiple transitions */
transition: color 0.3s ease, background-color 0.5s ease-in-out;
```

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

These properties **snap instantly** (no interpolation):

- `display`, `position`, `flex-direction`, `font-weight`
- `overflow`, `text-align`, `box-sizing`, `border-style`
- `cursor`, `margin-auto`

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
  transition: border-color 0.3s ease, transform 0.3s ease;
}

.card:hover {
  border-color: #6366f1;
  transform: translateY(-4px);
}
```

## Ancestor Hover Transitions

`.parent:hover .child` transitions work — when the parent is hovered, the child's style changes animate:

```css
.card:hover .card-title {
  color: #6366f1;
}

.card-title {
  transition: color 0.3s ease;
}
```

## How It Works Internally

1. On hover enter, the runtime captures the current style as the start state and the hover style as the target
2. Each frame, it interpolates between start and target using the easing function
3. On hover leave, the current interpolated style becomes the new start and the base style becomes the target
4. Mid-transition direction changes capture the current interpolated position for smooth reversal

Transitions run per-node — parent and child can transition independently.

## Duration 0

Setting `transition-duration: 0` (or omitting the transition) disables animation — style changes snap instantly. This is the default behavior and is fully backward-compatible.
