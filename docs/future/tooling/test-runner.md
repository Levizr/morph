# `morph test` — Headless E2E Runner

**Status:** future · **Priority:** medium · **Depends on:** [Time-Travel Debugger](time-travel.md) (replay), [Platforms](../shipped/platform.md) (shipped — CI images exist)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Robots clicking buttons so humans don't have to. Today verification is `<binary> --morph-self-test` plus screenshot-by-hand (`import -window <id>`); there is no assertion-based runner a CI pipeline can gate on. `morph test` closes that gap: drive windows, assert state, screenshot on failure.

## Why it matters

- **CI needs a red/green gate** — without a runner, every PR is "works on my machine" with extra steps
- **Multi-window flows are untestable by hand at scale** — open two windows, sync via event, assert both copies agree; nobody does this manually twice
- **Screenshots catch what assertions can't** — a layout regression that keeps every value correct still looks broken; pixel-diff the fixtures
- **Recordings become tests** — a [time-travel](time-travel.md) session log *is* a regression test; the runner just replays it with assertions attached

## How it will work

```tsx
// tests/checkout.test.mx
import { test, expect } from 'morph/test'

test("two windows stay in sync", async () => {
  const a = await Window.open("/shop")
  const b = await Window.open("/shop")

  await a.click("Add to cart")
  expect(await b.text("Cart:")).toBe("Cart: 1")   // cross-window sync asserted

  await expect(a.screenshot()).toMatchSnapshot()  // pixel gate
})
```

- **Headless by default** — runs under Xvfb (or headless EGL) in CI; `--headed` for local debugging, because watching the robot work never gets old
- **Addressing by test-id** — `data-testid` attributes (stripped from release binaries, zero cost in prod)
- **Three assertion layers** — signal values (fast, semantic), DOM-ish tree queries (text/role), screenshots (slow, visual) — pick per test
- **Failure artifacts** — on red: screenshot diff, last-100 transition log, and the window's signal dump. Debugging starts *after* the evidence is collected, not before
- **Quarantine mode** — flaky timing tests get retried with backoff and flagged, not silently green

## Current state

| Piece | State |
|---|---|
| `<binary> --morph-self-test` (headless runtime check) | ✅ Shipped |
| `tests/runtime/run-selftests.sh` fixture sweep | ✅ Shipped |
| Manual screenshots via `import -window` | ✅ Shipped (manual) |
| `morph test` runner + `morph/test` assertions | ❌ Not built |
| `data-testid` stripping in release builds | ❌ Not built |
| Snapshot store + diff + failure artifacts | ❌ Not built |

## Open questions

- **Timing model** — await signal quiescence (deterministic, Morph-native) vs fixed sleeps (flaky, universal)? Quiescence first, sleeps as escape hatch.
- **Snapshot storage** — in-repo PNGs (simple, noisy diffs) vs content-hash store (clean, needs tooling)?
- **Web parity** — reuse Playwright-style API names (`click`, `toBe`, `toMatchSnapshot`) for zero learning cost, or Morph-native naming?
- **Who owns the windows** — one runner process driving N app windows, or tests compiled *into* the app binary?

## Build steps (when picked up)

1. Headless harness: Xvfb/CI recipe + `--headed` flag
2. `morph/test` minimal API (`test`, `expect`, window handles, click/text queries)
3. `data-testid` pipeline (parse → runtime lookup → release strip)
4. Screenshot + snapshot-diff store
5. Replay integration (session log + assertions = generated regression test)
6. Failure artifacts bundle + quarantine retries
7. Validation: `morph test` gates the fixture sweep in CI
