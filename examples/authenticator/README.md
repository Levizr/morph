# Authenticator

A Material-style 2FA authenticator built with Morph. Replicates an authenticator phone UI: search bar, account list with one-time codes, copy buttons, and a FAB — with a **dedicated `/settings` route** that switches a full **dark/light Material theme** live across windows.

## What it shows

- **File-based routing** — `src/settings/route.mx` becomes `/settings`; the home page opens it with `new Window("/settings", { data: {...} })`, and Settings closes itself with `useWindow().close()`
- **Cross-window state** — `morphShared` is per-window, so theme prefs travel over the `morphEvent` bus (`src/store.mx`): Settings emits, Home listens with `.on()` and re-renders
- **Route props** — `data: { theme, codeSize }` arrives as the Settings page's `props`, so it opens showing the active selection
- **Theme as ancestor classes** — the root div swaps `theme-dark`/`theme-light` (+ `code-normal`/`code-large`); every themed value hangs off descendant selectors, so switching theme is one className swap with zero style writes
- **Search filter** — controlled `<input>` + `indexOf` match; non-matches hide via `display: none` (no list rebuild)
- **Copy feedback** — per-row tap state swaps the copy glyph for a ✓
- **CSS-only icons** — the FAB's multicolor `+` is 5 plain divs (no icon font needed); `:active` press feedback via `transform: scale()`
- **Scrollable list** — `overflow: auto` container with a floating `position: absolute` FAB

## Run

```bash
cd examples/authenticator
morph dev          # live window with hot reload
# or
morph run          # build + run optimized binary
```

## Try it

1. Type in the search bar — non-matching accounts disappear (match is case-sensitive).
2. Tap a copy icon — it flips to a green ✓ for that row.
3. Tap **≡** — a Settings window opens showing the current theme/size selected.
4. Pick **Light theme** — both windows re-skin instantly over the event bus.
5. Pick **Large** code size — home codes grow from 32px to 40px.
6. Tap **←** — the Settings window closes; Home keeps the last prefs.

## Notes

- The FAB is decorative (no add-account flow in the example) but carries `:active` press feedback.
- The hamburger opens Settings as a shortcut; on mobile this would be a navigation drawer.
- TOTP codes are static strings — no timer/crypto in scope; the layout is the point.
