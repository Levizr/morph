# Calculator

A fully functional calculator built with Morph. Demonstrates reactive state, conditional JSX rendering, typed functions, and flexbox keypad layout.

## What it shows

- **`morphState`** — reactive state for current value, accumulator, operator, and display mode
- **Conditional JSX** — `{op !== 0 && <span>{acc} {opSym}</span>}` hides the expression when no operator is active
- **Typed functions** — `:double` and `:int` type annotations on `compute()`, `pressDigit()`, etc.
- **Flexbox layout** — grid keypad with `flex-wrap`, gap, and `justify-content`
- **Window constraints** — `minWidth`/`maxWidth`/`minHeight`/`maxHeight` lock the window to fixed dimensions

## Run

```bash
cd examples/calculator
morph dev          # live window with hot reload
# or
morph run          # build + run optimized binary
```
