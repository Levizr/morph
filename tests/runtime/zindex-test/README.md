# Z-Index Test

Validates CSS 2.1 `z-index` paint-order stacking with overlapping absolutely positioned elements.

## What it tests

- Negative z-index (`z-index: -1`) paints below the static block-flow layer
- `auto` z-index paints in the auto layer
- `z-index: 0` paints above auto
- `z-index: 5` and `z-index: 10` stack correctly on top
- Static in-flow block (`position: static`) acts as the backdrop between negative and non-negative layers

## Run

```bash
cd tests/runtime/zindex-test
morph dev
# or
morph run
```
