# Tooling & Ecosystem — VSCode Extension, `morph-icons`, `morph-animate`

**Status:** future · **Priority:** low

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

The developer-experience and ecosystem layer. Nothing here is a prerequisite for the core framework — it all compounds on top of a stable core.

## VSCode extension

**Planned:** syntax highlighting, IntelliSense, and project tooling for `.mx` files in VSCode.

- `.mx` is JSX + TS + CSS in one file — highlighting needs a custom TextMate grammar or a grammar composition
- Autocomplete can reuse the shipped `node_modules/morph` `.d.ts` (already in every project for editor support)
- Nice-to-haves: `morph dev` task integration, error squiggles via `morph check` output, hover docs

## `morph-icons` (first-party package)

**Planned:** an icon package installable via `morph pkg install morph-icons` (see [Packages](packages.md)).

- Icon set rendered as text (icon font via FreeType) or as vector SDF paths
- SDF rendering fits Morph's shader stack perfectly (rounded rects already use SDF)
- Depends on the package JS→C++ bridge landing so icons ship as a real package

## `morph-animate` (animation library)

**Planned:** a higher-level animation library built on top of the CSS animation engine.

- The runtime already has: CSS `@keyframes` + `animation-*` properties, easing functions, property interpolation, `HoverTransition` interpolation, and a compositor that interpolates compositor-safe properties at vsync
- `morph-animate` would add: imperative tween API (`animate(el, { opacity: 0 }, { duration: 300 })`), sequenced/timeline animations, spring easing
- Could ship as a package (same dependency on [Packages](packages.md)) or as a built-in module

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
| VSCode extension / icons / animate | ❌ Not started |