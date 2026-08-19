# More CSS — `box-shadow`, `outline`, Margin Collapse

**Status:** future · **Priority:** low · **Depends on:** [CSS Cascade](css-cascade.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Three CSS features with existing scaffolding or a long-standing spot on the roadmap.

## `box-shadow`

**Scaffold exists:** `morph/style/features/shadow.h` declares `BoxShadow` / `ShadowStyle` structs, guarded by `MORPH_FEATURE_SHADOW` — but the header is **never included** by `style.h` and the define is **never emitted** by `feature_set.py`. Purely dormant.

```cpp
// style/features/shadow.h (future wiring)
struct BoxShadow {
    float offsetX, offsetY, blur, spread;
    Color color;
    bool inset;
};
```

**Planned behavior:**

- `box-shadow: 2px 2px 8px rgba(0,0,0,0.3)` and the inset variant
- Rendering approach: SDF shader extension — the same rounded-rect SDF that renders borders can compute shadow blurs cheaply
- Tailwind utilities (`shadow-sm`, `shadow-md`, …) as a bonus once the property lands

## `outline`

**Scaffold exists:** `morph/style/features/outline.h` declares `OutlineStyle`, guarded by `MORPH_FEATURE_OUTLINE` — same dormancy: not included, not emitted.

**Planned behavior:**

- `outline` / `outline-color` / `outline-width` / `outline-offset`
- Unlike `border`, outline does **not** affect layout (drawn outside the box)
- Natural companion to the [Text Input](text-input.md) focus model — a focus ring via `outline` on `:focus`

## Margin collapse

Already on the v0.1.0 list. **Planned behavior (CSS 2.1 §8.3.1):**

- Adjacent vertical margins collapse to the larger of the two
- Empty elements' top/bottom margins collapse together
- Collapsing suppressed by padding/border/clearance, or by `overflow: hidden` ancestors
- Layout-engine change in `morph/layout/` (Python) + `runtime/core/layout.cpp` (C++) — must stay pixel-identical between dev and build modes

## Why low priority

- Shadows and outlines are polish — no API or architecture impact
- Margin collapse changes layout math for every existing app and needs a browser-parity test suite (flexbox margins must *not* collapse per spec — the two systems interact)

## Current state

| Feature | State |
|---|---|
| `BoxShadow` / `ShadowStyle` structs | ✅ Declared, dormant |
| `OutlineStyle` struct | ✅ Declared, dormant |
| Margin collapse | ❌ Not started (roadmap item) |
| SDF shadow rendering | ❌ Not started |

## Build steps (when picked up)

1. `box-shadow`: include `shadow.h` in `style.h`, emit `MORPH_FEATURE_SHADOW` in `feature_set.py`, add parser + SDF shader path
2. `outline`: same wiring + draw outside the box
3. Margin collapse: implement in the Python layout engine, mirror in C++, add collapse suppression cases to the flexbox tests