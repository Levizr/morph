# component-test

Runtime test project for the reusable `.mx` component system. Build it with
`morph build` (or lint it with `morph check`) from this directory.

## What it exercises

- `src/App.mx` — entry default export; imports a default component (`Hero`)
  and named components (`Counter`, `Badge`); passes literals (`label`,
  `step`), a state setter wrapped in an inline arrow (`onStep`), and a named
  function (`announce`); reads the imported shared `total` store; uses `import './style.css'`.
- `src/components/Hero.mx` — default export with a typed `title` prop and
  per-instance `morphState`.
- `src/components/Counter.mx` — named export; two instances in `App` must
  keep independent `count` state (`inst0_count` vs `inst1_count`); declares a
  function-typed prop `onStep`; reads/writes the shared `total` store exported
  by `Store.mx`; subscribes to its exported `resetEvent`.
- `src/components/Badge.mx` — reads the same shared store (no providers) and
  emits `resetEvent` to clear both counters.

## Expected behavior

- The two counters step independently (1s and 5s).
- `Total` (Badge) and `Shared total` (App) always agree — one shared store.
- `Reset` zeroes the total and both counters via the channel.
- `Last step` follows the "Ones" counter; stepping "Fives" logs to console.
