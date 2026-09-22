# Window ownership: `parent` + `modal` + `role`

**Status:** shipped · **Priority:** high ·
**Depends on:** [Window API](window-api.md), [File-Based Windows](file-routing.md)

> **Shipped → main docs.** Ownership is implemented and documented for
> users in [Windows & Routes](../guides/windows-and-routing.md) (Ownership
> section) and [`Window` / `useWindow`](../api/windows.md) (config table,
> ownership, links). This page keeps the design history, the invariants
> every future change must preserve, and what remains below.

## Why not a `type` enum (design history)

No major framework drives window behavior with a single `type` string.
They all split it into three orthogonal axes, and Morph follows them:

| Axis | Electron | Qt | GTK | Morph (shipped) |
|---|---|---|---|---|
| Relationship | `parent` option | `transientParent` | `transient_for` | `parent` (WID, interned) |
| Blocking | `modal: true` (needs parent) | `modality` (None/Window/App) | `modal` bool | `modal` bool (needs parent) |
| Presentation | `frame`, `alwaysOnTop` | `Qt::WindowFlags` bitmask | type hints | `role` (interned int, hints only) |

A single enum explodes combinatorially the moment someone wants
"centered but non-blocking". Relationships + flags compose; roles stay
presentation-only so a wrong role degrades to decorations, never to
broken behavior.

## Shipped semantics (invariants)

- **Cascade:** closing an owner destroys its owned subtree recursively,
  then itself. The app quits when the registry empties (existing
  `allClosed` loop) — independent windows keep it alive, browser-style.
- **Blocking (strict scope):** a live modal refuses every *other*
  window's close — independents included. `close()` returns `false`; the
  sweep skips **and clears** the pending `shouldClose` so no surprise
  death (the user re-closes explicitly). Modals themselves always close.
  (`ApplicationModal`-style scoping was considered; strict won because
  the owner explicitly asked for it. Revisit if independents freezing
  proves annoying in practice.)
- **Modal geometry:** centered on the parent at open; owned windows
  float above their parent; follower and parent move rigidly in both
  directions (locked offset, re-entrancy guarded, parent target clamped
  to the monitor work area so a clamped parent stops the follower too).
  Async X11 moves are never read back for immediate action — requested
  positions are stored, live positions only for storage.
- **Refusals, not crashes:** `modal: true` with no resolvable parent
  refuses the open (invalid handle + stderr), it is not a build error —
  focus may legitimately be nowhere at open time.
- `closed` / `on_close` fire only on actual destroy. `role` literals are
  a closed set (`default`/`dialog`/`popup`) — unknown roles fail the
  build; `role` never gates behavior.

## Role catalog

v1 roles: `default` (neutral — zero visual change; the default),
`dialog` (taskbar-grouped with parent), `popup` (borderless, no taskbar
entry — purely presentational, implies no behavior). Future ideas (not
built): `sheet`, `drawer`, `splash`, `tooltip`, `tool`/`palette`,
`toast`, `fullscreen`/`kiosk`, real input-blocking for `dialog`.

## Remains (not built)

- `.d.ts` for `parent`/`modal`/`role` (config + `useWindow` handle
  forms) — `morph-routes.d.ts` covers routes only.
- `mx-window-type-entry` lint (entry window declaring `popup` — runtime
  resolves it as cascade-wins today, silently).
- Owner-subtree-scoped blocking as an alternative to strict (see above).
- Monitor-picking for centering (primary monitor today).
