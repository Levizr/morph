# Morph API Reference

This section documents all core Morph APIs available from the `'morph'` package.

## Core APIs

| API | Purpose | Scope |
|-----|---------|-------|
| [`morphState`](morphState.md) | Local component instance state | Inside component function |
| [`morphShared`](morphShared.md) | Global shared module-scoped state | Module scope (top level) |
| [`morphEvent`](morphEvent.md) | Fire-and-forget event channels | Module scope (top level) |
| [`morphEffect`](morphEffect.md) | Side effects with cleanup | Inside component function |
| [`Window` / `useWindow`](windows.md) | Runtime windows: open, control, navigate | Component body / event handlers (handles are WID ids) |

## Importing

All APIs are imported from the `'morph'` package:

```tsx
import { morphState, morphShared, morphEvent, morphEffect } from 'morph'
import { Window, useWindow } from 'morph'
```

## Scope Rules (Enforced by Linter)

| API | Valid Location | Invalid Location | Lint Code |
|-----|----------------|------------------|-----------|
| `morphState` | Inside component function | Module scope, outside component | `mx-state-scope` |
| `morphShared` | Module scope (top level) | Inside component function | `mx-shared-scope` |
| `morphEvent` | Module scope (top level) | Inside component function | `mx-event-scope` |
| `morphEffect` | Inside component function | — (convention) | `mx-effect-cb`, `mx-effect-deps` |

## Identity Model

All three declarative APIs (`morphShared`, `morphEvent`) use a **module-path + binding-name** identity:

```
Identity = "absolute/module/path.ts::exportedBindingName"
```

- Importing the same binding from the same module → **same identity** (same signal/channel)
- Same binding name in a different module → **different identity**
- Importing two bindings with the same local name from different modules → **ambiguity error** (must rename on import)

This eliminates string keys entirely — the TypeScript module system provides the namespacing.


## FAQ

### Why do all APIs start with `morph`? Why not `useState`, `useEffect` like React?

Because the same name would promise the same behavior — and the behavior is not the same. The `morph` prefix is a constant, unmissable reminder that these only *look* like React hooks. They differ in three ways that matter:

**1. Different performance model.** React hooks run on a VDOM reconciler: a `useState` setter schedules a re-render of the component subtree, which diffs a virtual tree to find what changed. Morph APIs compile to native signals and channels: a setter marks one signal dirty, and only its subscribers update — no tree walk, no diffing. Same call shape, completely different machine underneath. Calling it `useState` would invite VDOM-era performance instincts (memo everything, split components to isolate renders) that the signal model doesn't need.

**2. Different semantics in the details.** React's `useEffect` deps feed the reconciler's change detection *and* the closure captures values, giving you the stale-closure problem and the exhaustive-deps lint. Morph's deps build a change-signature guard over natively tracked signals, and body reads never subscribe — as [`faq/morphEffect.md`](faq/morphEffect.md) details. Named `useEffect`, users would import React mental models (stale closures, cleanup timing relative to paint commits) that simply don't apply, and debug phantoms.

**3. Honest migration.** When porting React code, `useState` → `morphState` renames force you to re-read every call instead of assuming identical behavior. Same-name APIs would be a trap: the code compiles, looks right, and behaves subtly differently in exactly the cases you stopped checking. The prefix makes "similar but not the same" impossible to miss.

### Then why will future APIs like `useWindow` NOT use the `morph` prefix?

Because there is nothing to distinguish them from. The prefix exists to separate Morph's versions of *React concepts* from React's versions. `useWindow` — managing native windows — has no React equivalent; React doesn't do windows at all. No user will bring React expectations to it, so no prefix is needed to warn them off. Its plain name already says what it is: made for Morph, explicitly.

The codebase already follows this rule: `export const windowConfig = { title, width, height }` in every app entry carries no prefix, because window configuration is a Morph-native concept with no React counterpart to confuse it with. Prefixing *everything* would dilute the signal. The `morph` prefix is reserved for exactly one message: "looks like React, isn't" — and it is spent only where that confusion is possible.


## FAQ - API specific

Per-API frequently asked questions live in [`faq/`](faq/):

- [`faq/morphState.md`](faq/morphState.md) — local state questions
- [`faq/morphShared.md`](faq/morphShared.md) — shared store questions
- [`faq/morphEvent.md`](faq/morphEvent.md) — event channel questions
- [`faq/morphEffect.md`](faq/morphEffect.md) — effect questions
- [`faq/choosing.md`](faq/choosing.md) — which API do I need?