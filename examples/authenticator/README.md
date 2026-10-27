# Authenticator

A Material-style 2FA authenticator built with Morph. Replicates an authenticator phone UI: search bar, account list with one-time codes, copy buttons, and a FAB — with a **dedicated `/settings` route** that switches a full **dark/light Material theme** and code size live across windows.

## What it shows

- **File-based routing** — `src/settings/route.mx` becomes `/settings`; Home opens it with `new Window("/settings", { data: { theme, codeSize } })`, and Settings closes itself with `useWindow().close()`
- **Route props** — `theme` and `codeSize` arrive as Settings' `props`, so a freshly opened Settings window starts on the active selection; the page keeps local `pickedTheme`/`pickedCodeSize` state while it is open
- **Cross-window preferences** — `morphShared` is per-window, so Settings broadcasts each pick over the `morphEvent` bus (`src/store.mx`); Home subscribes with `.on()` and updates its own shared copies
- **Reactive inline styles** — colors, selected-row backgrounds, and code font sizes are inline expressions on static layout classes; changing theme or code size updates only the affected style fields
- **Search filter** — controlled `<input>` + case-sensitive `indexOf` match; non-matches hide via `display: none` (no list rebuild)
- **Copy feedback** — tapping a copy icon swaps the outlined copy box for a green ✓ for 1.5 seconds
- **CSS and image icons** — the hamburger is three CSS bars, the FAB's multicolor `+` is five plain divs, and the search-bar cloud is a small generated PNG (`src/assets/cloud.png`); no icon font is used
- **Scrollable list** — `overflow: auto` list with a floating `position: absolute` FAB

## Run

```bash
cd examples/authenticator
morph dev          # live window with hot reload
# or
morph run          # build + run optimized binary
```

## Try it

1. Type in the search bar — non-matching accounts disappear (match is case-sensitive).
2. Click a copy icon — it flips to a green ✓ and resets after 1.5 seconds.
3. Click **≡** — a Settings window opens showing the current theme/size selected.
4. Pick **Light theme** — both the Settings window and Home switch palettes immediately.
5. Pick **Large** code size — Home codes grow from 32px to 40px.
6. Pick another theme or size to confirm the initial selection follows the current props when Settings is reopened.
7. Click **←** — Settings closes; Home keeps the last preferences.

## Notes

- The FAB is decorative (no add-account flow in the example) but carries `:active` press feedback.
- The hamburger opens Settings as a shortcut; on mobile this would be a navigation drawer.
- TOTP codes are static strings — no timer/crypto in scope; the layout is the point.
