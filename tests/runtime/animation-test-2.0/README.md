# Animation Test 2.0

Advanced CSS animation tests — multi-stop keyframes, multiple simultaneous animations, fractional iteration counts, hover-triggered swaps, and percentage-based property values.

## What it tests

- Multi-stop keyframes (5 properties animated through multiple stops)
- Triple animations (3 `animation-name` values on one element)
- Fractional iterations (`animation-iteration-count: 2.5`)
- `alternate` and `alternate-reverse` directions
- Hover-triggered animation swap
- `fill-mode: both` / `backwards` with delay
- Percentage-based `left`/`width` and `translate` in keyframes

## Run

```bash
cd tests/runtime/animation-test-2.0
morph dev
# or
morph run
```
