# More CSS — `box-shadow`, `outline`, Margin Collapse

**Status:** shipped · **Priority:** done · **Depends on:** [CSS Cascade](cascade.md)

All three proposals in this record have landed. The notes below are retained as the design record; user-facing syntax lives in [CSS Properties](../../css/properties.md).

## `box-shadow`

**Shipped.** Parsed into `IRStyle.box_shadows` (up to four layers, `inset` supported) and painted through the runtime shadow path behind `MORPH_FEATURE_SHADOW`.

- `box-shadow: 2px 2px 8px rgba(0,0,0,0.3)` and the inset variant
- Rounded-rect SDF rendering, verified against Chrome within 2/255 per channel
- Tailwind shadow utilities resolve through the same IR path

## `outline`

**Shipped.** `outline`, `outline-color`, `outline-width`, `outline-style`, and `outline-offset` are parsed into `IRStyle` and painted as a ring outside the border edge behind `MORPH_FEATURE_OUTLINE` (which implies the border paint path).

- Unlike `border`, outline does **not** affect layout
- Supports `none`, `solid`, `dashed`, `dotted`, and `double`
- `outline-offset` moves the ring outward (positive) or inward (negative)
- Natural companion to the focus model for [Text Input](../elements/text-input.md), though `:focus` state wiring is still future work

## Margin collapse

**Shipped** behind `MORPH_FEATURE_MARGIN_COLLAPSE` in `runtime/cpp/core/node/layout.cpp` (CSS 2.1 §8.3.1):

- Adjacent vertical margins collapse to the larger of the two
- Parent–child collapse-through via `m_computedMargin`
- Collapsing suppressed by padding/border, inline content before/after, and flex containers
- Provisional-then-final child layout keeps the first pass correct

## Current state

| Feature | State |
|---|---|
| `box-shadow` parsing + painting | ✅ Shipped |
| `outline` parsing + ring painting | ✅ Shipped |
| Margin collapse | ✅ Shipped |
| Focus rings via outline | ❌ Not started (`:focus` state not modeled; tracked in [CSS Finalization](finalization.md#1-state-and-selector-coverage)) |

## Remaining work

1. `:focus` / `:focus-visible` selectors so outline can serve as a focus ring — tracked in [CSS Finalization](finalization.md#1-state-and-selector-coverage)
2. Optional shader-accelerated text-shadow blur refinement (currently a soft multi-pass approximation)
