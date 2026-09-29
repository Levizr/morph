# Virtualized Lists — 100k Rows Without Rendering 100k Rows

**Status:** future · **Priority:** medium · **Depends on:** [Forge Renderer](forge-tile-pool.md) (scroll-shift), [Performance](performance.md) (layout cost)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Every desktop app eventually shows a list that doesn't fit in memory — logs, files, messages, trades, tracks. Rendering all of them is how you turn a 30 MB app into a 3 GB app. Virtualization renders only the visible window plus a small overscan buffer, recycling nodes as you scroll. Every framework users love has this (virtualized lists are why your chat app doesn't die at 50k messages); Morph's list story today is "render everything and hope."

## Why it matters

- **Correctness by default** — a `<VirtualList>` that stays fast at any length means devs never have to learn windowing to ship a log viewer
- **Signals are the ideal substrate** — fine-grained reactivity already updates only what changed; virtualization extends that from "what changed" to "what's visible"
- **Scroll physics need a partner** — compositor scroll-shift (Forge) makes movement cheap; virtualization makes the *content* cheap. Either alone is half the story

## How it will work

```tsx
import { VirtualList } from 'morph'

<VirtualList
  count={messages.length}
  rowHeight={56}                       // fixed fast path
  overscan={4}                         // extra rows above/below the viewport
  renderRow={(i) => <MessageRow msg={messages[i]} />}
/>
```

- **Fixed-height fast path first** — arithmetic row lookup, zero measurement; variable heights later via a measured-cache (measure once, cache by item key)
- **Node recycling, not remounting** — scrolled-off rows are rebound to new indices (state slots follow the *item*, never the recycled node — per-instance `morphState` inside rows keys by item, or it will haunt you)
- **Scroll state survives** — prepend (new chat message above) keeps the viewport anchored instead of jumping; theajt classic virtualization bug, fixed by tracking anchor item + offset
- **`morph check` guidance** — `.map()` over 10k items without virtualization warns with the conversion; `key` misuse (the existing `mx-key-misuse` rule) becomes load-bearing here and errors instead of warning inside virtual lists

## Current state

| Piece | State |
|---|---|
| List rendering (`.map()` templates, keyed reconciliation) | ✅ Shipped |
| `mx-key-misuse` lint | ✅ Shipped |
| Dirty-flag layout skipping | ✅ Shipped |
| `<VirtualList>` element + recycling | ❌ Not built |
| Variable-height measured cache | ❌ Not built |
| Prepend anchoring | ❌ Not built |

## Open questions

- **Row state identity** — `morphState` inside a recycled row: key by item id automatically, or require explicit keys? (Automatic is friendly; explicit is honest. Probably automatic with an escape hatch.)
- **Variable heights** — measure-on-first-render cache vs developer-declared `estimateHeight`? (Cache first; estimates lie.)
- **Horizontal + grid** — rows first, or design the 2D API up front? (Rows first — grids are rows wearing a trench coat. Mostly.)
- **Sticky headers/footers** — part of v1 or a follow-up? (Follow-up; sticky positioning has its own layout implications.)

## Build steps (when picked up)

1. `<VirtualList>` fixed-height with recycling + overscan
2. Item-keyed row state (no recycled-state ghosts)
3. Prepend anchoring (chat-case correctness)
4. `morph check`: large-`.map()` nudge + strict keys inside virtual lists
5. Variable-height measured cache
6. Validation app: 100k-row log viewer holding 60 fps + a chat that prepends without jumping
