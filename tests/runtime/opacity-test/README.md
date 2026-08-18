# Opacity Test

Validates the `opacity` CSS property across various scenarios — individual elements, grouped containers, Tailwind utilities, and images.

## What it tests

- Explicit opacity values: `1.0`, `0.7`, `0.45`, `0.2`
- Group opacity — container at `0.6` multiplies every child's paint color
- Tailwind `opacity-50` utility class
- Image opacity (faded avatar)
- Hover-triggered opacity transition (`transition: all 0.3s ease`)

## Run

```bash
cd tests/runtime/opacity-test
morph dev
# or
morph run
```
