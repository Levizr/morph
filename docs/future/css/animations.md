# Animations — Transitions, Springs & Shared Elements

**Status:** future · **Priority:** medium · **Depends on:** [Forge Renderer](../rendering/forge-tile-pool.md) (damage-aware animation), [Performance](../rendering/performance.md) (compositor interpolation)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Making rectangles feel feelings. The runtime already animates correctly on the main thread (CSS `@keyframes`, easing, `HoverTransition`, vsync interpolation of compositor-safe properties) — what it lacks is the *choreography* layer: entering/leaving, shared-element continuity across navigation, and physics that feels alive instead of computed.

## Why it matters
z
- **Motion is meaning** — a list item that slides out says "deleted"; one that blinks out says "error". Static UIs make users re-read the screen after every action
- **Navigation without transitions feels broken** — a hard cut on `navigate()` reads as a glitch in 2026; users expect the old page to yield to the new one
- **Springs beat easings** — interruptible, velocity-preserving motion (drag a card, let go, watch it settle) is what makes software feel physical; cubic-bezier can't do it
- **Shared elements are the signature move** — a thumbnail that grows into its detail page is the single highest "how did they do that" per line of code in UI design

## How it will work

```tsx
// 1. Enter/exit as declarations — no imperative animation code
<div transition={{ enter: "slide-up", exit: "fade", duration: 200 }}>
  {isOpen && <Panel />}
</div>

// 2. Springs for anything draggable or interruptible
import { spring } from 'morph/animate'
const [x, setX] = spring(0, { stiffness: 170, damping: 26 })

// 3. Shared element across navigate() — thumbnail becomes hero
<img sharedId="cover-42" src={cover} />
{/* detail route: <img sharedId="cover-42" .../> — runtime morphs one into the other */}
```

- **Compositor-first** — transforms/opacity run on the compositor thread (already interpolated at vsync); layout-affecting properties fall back to main thread with a `morph check` nudge when you animate `width` and mean `transform`
- **Interrupted animations compose** — retargeting a running spring keeps velocity (no snap-back), because springs are state, not timelines
- **`morph check` as choreographer** — animating a non-compositor property warns with the cheaper alternative; shared IDs duplicated on one screen error at build time
- **`morph-animate` graduates here** — the package idea from [Tooling](../tooling/vscode.md) lands as this design's imperative escape hatch (`animate(el, {...})`, timelines) for the 5% declarations can't express

## Current state

| Piece | State |
|---|---|
| CSS `@keyframes` + `animation-*`, easing, property interpolation | ✅ Shipped |
| `HoverTransition` interpolation | ✅ Shipped |
| Compositor vsync interpolation (X/Y, opacity, colors, radius) | ✅ Shipped (fixed set) |
| Enter/exit transitions | ❌ Not built |
| Spring physics + interruptible retarget | ❌ Not built |
| Shared-element across `navigate()` | ❌ Not built |
| Page-transition orchestration | ❌ Not built |

## Open questions

- **Exit + unmount** — leaving elements must outlive their unmount (the page cache holds trees; can it hold *leaving* trees?). Who owns the extra frames?
- **Shared-element mismatch** — detail page missing the `sharedId` (deep link straight in): fade fallback or snap?
- **Reduced motion** — `prefers-reduced-motion` equivalent as a first-class media query from day one, not a patch later?
- **Testing** — animations are time; does [Test Runner](../tooling/test-runner.md) freeze the clock (`--freeze-animations`) for deterministic snapshots?

## Build steps (when picked up)

1. Enter/exit transitions on mount/unmount (main thread, compositor-fast path where possible)
2. Spring primitive (`morph/animate`) with velocity-preserving retarget
3. `morph check` animation lints (non-compositor property nudge, duplicate `sharedId`)
4. Page-transition orchestration around `navigate()`
5. Shared-element morphing (same-window first, cross-window later)
6. Reduced-motion support + frozen-clock test hooks
7. Validation app: master-detail with shared covers, springy drawer, animated navigate
