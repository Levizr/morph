# Calculator

A native calculator with dynamic layout and themed UI.

## Files

| File | Description |
|---|---|
| `src/App.mx` | JSX template with layout, state, buttons |
| `src/style.css` | Animations and component styles |
| `morph.config.json` | Window size (400×600), entry point |

## Features Demonstrated

- `morphState` for expression, result, and theme
- CSS `@keyframes` for animations (fade, slide, glow)
- Transition + keyframe style swap on theme toggle
- Ternary theme classes + inline reactive styles
- Grid-based button layout with Tailwind utilities
- Imported native C++ function (`mathFn`) from `native.cpp`
- Expression parser written in C++ called from JSX event handlers

## Run

```bash
morph run
```

See the [full README](../../examples/calculator/README.md) for more details.
