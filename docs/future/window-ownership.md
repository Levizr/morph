# Window ownership: `parent` + `modal` + `role`

**Status:** design (approved 2026-09-21) · **Priority:** high ·
**Depends on:** [Window API](window-api.md), [File-Based Windows](file-routing.md)

> **Note:** This is a design record, not a commitment. It exists so window
> kinds get built on answered questions instead of silent guesses.

## Why not a `type` enum

No major framework drives window behavior with a single `type` string.
They all split it into three orthogonal axes, and Morph follows them:

| Axis | Electron | Qt | GTK | Morph (this design) |
|---|---|---|---|---|
| Relationship | `parent` option | `transientParent` | `transient_for` | `parent` (WID, interned) |
| Blocking | `modal: true` (needs parent) | `modality` (None/Window/App) | `modal` bool | `modal` bool (needs parent) |
| Presentation | `frame`, `alwaysOnTop` | `Qt::WindowFlags` bitmask | type hints | `role` (interned int, hints only) |

A single enum explodes combinatorially the moment someone wants
"centered but non-blocking". Relationships + flags compose; roles stay
presentation-only so a wrong role degrades to decorations, never to
broken behavior.

## Surfaces

`parent` / `modal` / `role` are declared everywhere geometry is, with the
same fallback chain (`new Window` opts → route `windowConfig` →
`[window]` app defaults):

```tsx
const main = useWindow()
const popup = new Window("/settings", { parent: main, modal: true, role: "popup" })
const win1 = new Window("/a", { id: "win1" })
const win2 = new Window("/b", { parent: win1 })   // handles chain; win1 owns win2
const solo = new Window("/c")                     // no parent → independent (browser-style)
```

`parent` accepts a handle (preferred — already a WID int, zero lookup),
an explicit `id` (alias lookup), or a route (most-recently-focused
window on that route, same rule as `useWindow("/route")`). `_blank`
links get `parent: auto` (focused window at click time); C++ passes
`parent: WID` (`kInvalidWid` = none). `modal: true` with no resolvable
parent is a hard error (Electron rule). Unknown `role` literals are hard
errors at build; `role` never gates behavior.

## Semantics

- **Cascade:** closing an owner destroys its owned subtree recursively,
  then itself. The app quits when the registry empties (existing
  `allClosed` loop) — independent windows keep it alive, browser-style.
- **Blocking:** a modal refuses `close()`/X on every *other* window
  (`false` / sweep skips **and clears** the pending `shouldClose` so no
  surprise death — the user re-closes explicitly). Modals themselves are
  always closable. Scope is the owner subtree; independent windows are
  unaffected (`ApplicationModal` remains a possible later addition).
- **Modal geometry:** centered on the parent at open; follows parent
  moves and vice versa (locked offset, re-entrancy guarded).
- **Parent death:** closing a window also closes its attached modals
  (no dangling parents).
- `closed` / `on_close` fire only on actual destroy. New props still
  remount fresh; the page cache is per-window and unaffected except that
  cached pages die with their window's close (existing flush).

## Role catalog

v1 roles: `default` (neutral — zero visual change; the default),
`dialog` (taskbar-grouped with parent), `popup` (borderless, no taskbar
entry — purely presentational, implies no behavior). Future ideas (not
this design): `sheet`, `drawer`, `splash`, `tooltip`, `tool`/`palette`,
`toast`, `fullscreen`/`kiosk`, `dialog` input-blocking (`dialog` +
`modal` covers most of it).

## Lowering (no runtime strings)

- `WindowRole` enum in `morph-config` (`Default=0, Dialog=1, Popup=2`,
  room to grow); `modal` is a bool (no table); `parent` is already an
  int at every call site (`__wid`, focused WID, alias/route lookup).
- `WindowConfig += parent/modal/role`; `RouteEntry += parent/modal/role`
  (literals only — dynamic values are hard errors); `OpenConfig +=
  parent/modal/role`; `.d.ts` gains the three fields.
- Generated `__morph_create_window` resolves each key through the
  fallback chain and calls `wm.setWindowParent/Modal/Role(wid, …)`.

## Open questions (answered at build time unless noted)

- Default `role`: `"default"` (recommended — zero visual change).
- `role: "popup"` stays purely presentational (recommended).
- Omitted `parent`: `none`/independent (recommended — Electron/Qt
  consensus) vs `auto`-attach. **Default to `none`; revisit if popups
  orphan too easily in practice.**
