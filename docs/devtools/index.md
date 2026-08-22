# DevTools

Morph ships a browser-style DevTools panel built into the dev runtime (`morph_devrt`). Press **F12** in a running `morph dev` window to open it — inspect elements, profile the render pipeline, watch network traffic, and read app output without leaving your app.

DevTools only exists in dev mode. Production binaries built with `morph build` contain no DevTools code at all — zero size, zero runtime cost.

## Opening

| Key | Action |
|---|---|
| `F12` | Toggle the DevTools panel |
| `F2` | Toggle element inspect mode |

You can also click the **Inspect Element** button in the panel header instead of pressing `F2`.

## Panel Layout

The panel docks to the right side of the window. Your app's layout is constrained to the remaining content area, so the panel never covers your elements — it behaves like a docked browser panel, not an overlay.

- Drag the panel's left edge to resize it (minimum 240px; your app always keeps at least 360px)
- On window resize, the panel width is preserved and the content area clamps so the app never collapses

## Tabs

| Tab | What it shows | Docs |
|---|---|---|
| **Elements** | Inspect mode, box-model overlay, layout/display/style info | [Elements](elements.md) |
| **Rendering** | Frame stats, layout/paint diagnostics, live renderer switch | [Rendering](rendering.md) |
| **Network** | Every `fetch()` request with status, timing, headers, body | [Network](network.md) |
| **Logs** | App output with levels and timestamps | [Logs](logs.md) |

## Hot Reload Behavior

DevTools state survives hot reloads — open/closed, active tab, and inspect mode are all preserved. The hovered element reference is cleared because the node tree is rebuilt, so re-hover after a reload.

## Availability

| Mode | DevTools |
|---|---|
| `morph dev` | Full panel |
| `morph build` binary | Compiled out entirely |
