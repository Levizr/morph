# Tooling & Ecosystem — VSCode Extension, `morph-icons`, `morph-animate`

**Status:** future · **Priority:** low

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

The developer-experience and ecosystem layer. Nothing here is a prerequisite for the core framework — it all compounds on top of a stable core.

## VSCode extension (full support)

**Planned:** the extension is the front door most devs walk through — it gets the full treatment, not just highlighting:

- **Grammar** — custom TextMate composition for `.mx` (JSX + TS + CSS regions, plus `windowConfig` and `morphShared`/`morphEvent` scope tinting so misplaced calls *look* wrong before the linter says so)
- **Diagnostics** — `morph check` output as squiggles with fixes (rename-on-ambiguous-import as a one-click code action, not a chore)
- **Hover docs** — registry-fed tooltips for every API, route ids with go-to-`route.mx`
- **Run integration** — `morph dev` / `morph build` / [`morph test`](test-runner.md) as tasks with problem matchers; failing tests jump to the exact line
- **Debugger attach** — breakpoints in `.mx` mapped through to generated C++ (the source-map story from the compiler pipeline, surfaced where devs live)
- **Timeline view** — the [time-travel](time-travel.md) strip embedded beside the editor, because scrubbing state without leaving your editor is how you stay in flow

## `morph-icons` (first-party package)

Full design now lives at [Icons & SVG](icons-svg.md): `<svg>` path subset first, then the tree-shaken `morph-icons` set, then `morph icons:add` for project sets. This section keeps the package-angle summary — the icon set ships via `morph pkg install morph-icons` (see [Packages](packages.md)) once the bridge lands.

## `morph-animate` (animation library)

Full design now lives at [Animations](animations.md): enter/exit transitions, springs, shared-element morphing, page-transition orchestration. `morph-animate` survives as the imperative escape hatch (`animate(el, { opacity: 0 }, { duration: 300 })`, timelines) for the 5% declarations can't express — same dependency on [Packages](packages.md).

## Why low priority

- VSCode extension: polish; users can write `.mx` in any editor today
- `morph-icons` / `morph-animate`: depend on the package bridge; the CSS animation engine must be stable first

## Current state

| Piece | State |
|---|---|
| Editor `.d.ts` (autocomplete) | ✅ Shipped |
| CSS animation engine | ✅ Shipped |
| Package CLI | ✅ Shipped |
| Package build bridge | ❌ (see [Packages](packages.md)) |
| VSCode extension (full spec above) | ❌ Not started |
| `morph test` runner | ❌ Spec at [Test Runner](test-runner.md) |
| Icons / animate (full designs linked above) | ❌ Not started |