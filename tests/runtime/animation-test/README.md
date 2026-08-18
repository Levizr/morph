# Animation Test

Validates CSS `@keyframes` animations with various shorthand/longhand properties, fill modes, and iteration counts.

## What it tests

- `animation` shorthand — `pulse infinite`, `3s ease 1s 2 alt both slide-in`
- Longhand properties — `animation-name`, `animation-duration`, `animation-timing-function`, `animation-delay`, `animation-iteration-count`, `animation-direction`, `animation-fill-mode`
- Multiple animations on one element (`combo`, `multi`)
- Duplicate keyframe offsets
- `!important` dropped from animation values
- Paused animations
- Layout-affecting animations (`grow`)

## Run

```bash
cd tests/runtime/animation-test
morph dev
# or
morph run
```
