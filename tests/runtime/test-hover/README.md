# Hover Test

Minimal test for CSS `:hover` and `:active` pseudo-classes with transitions.

## What it tests

- `:hover` — background color change on mouse enter/leave
- `:active` — darker background on mouse down
- `transition: all 0.3s ease` — smooth interpolation between states
- `cursor: pointer` — hand cursor on hover

## Run

```bash
cd tests/runtime/test-hover
morph dev
# or
morph run
```
