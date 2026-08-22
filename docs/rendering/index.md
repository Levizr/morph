# Rendering

Morph has two renderers — **flash** and **forge**. They share everything above the paint step: the same node tree, layout engine, style system, dirty-flag tracking, and reactivity. The renderer is only the **paint + present** back half, so switching renderers never changes how your app behaves — only how pixels get to screen.

| Renderer | One-liner | Best for |
|---|---|---|
| [**flash**](flash.md) | Lightweight direct renderer — full clear each frame | Small apps, static UIs, lowest RAM and size |
| [**forge**](forge.md) | Hybrid retained compositor — retained surface + damage tracking | Large stable UIs: editors, dashboards |

## Choosing a Renderer

Set the `renderer` key in `morph.config.json` (see [Configuration](../getting-started/configuration.md)):

```json
{
  "renderer": "flash"
}
```

- `"flash"` is the default and the recommended production renderer today
- `"forge"` is beta — try it from the DevTools Rendering tab before committing

## How Selection Works

Selection is per-app and resolved differently in production vs dev:

| Mode | Mechanism | Result |
|---|---|---|
| Production (`morph build`) | Compile time (`constexpr` dispatch) | The unselected renderer is fully eliminated — zero runtime branch, zero dead code |
| Dev (`morph dev`) | Runtime toggle (relaxed atomic read) | Both compiled; hot-switch live from the DevTools Rendering tab |

The per-frame cost of the dev toggle is one relaxed atomic load (~2 ns). In production there is no branch at all.

## Shared Pipeline

Both renderers sit on the same foundation:

- `runtime/render/` — the batched GL primitives both use (rects, text, borders, clips)
- `runtime/core/window.cpp` — the dispatch host: calls `activeRenderMode()`, then the chosen backend's commit/present
- Compositor thread — owns the GL context exclusively; the main thread flattens a lock-free `RenderFrame` and atomically swaps pointers

Renderer code lives isolated under `runtime/renderers/flash/` and `runtime/renderers/forge/` — they never share implementation files.
