# Transform Test

Validates CSS `transform` property parsing and runtime application.

## What it tests

- `transform: translateY(20px)` on hover
- `transition: all 0.5s ease` — smooth transform interpolation
- Vertex-shader applied transform matrix
- `MORPH_FEATURE_TRANSFORM` compile-time gate

## Run

```bash
cd tests/runtime/transform-test
morph dev
# or
morph run
```
