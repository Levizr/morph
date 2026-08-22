# Flash

`flash` is Morph's lightweight direct renderer — the production default. It redraws the full window every frame: clear, draw everything, present.

## How It Works

Every frame, flash takes the flattened `RenderFrame` and replays it from scratch:

1. Full-surface clear
2. All nodes drawn in paint order (rects, text, borders, images via the batched GL renderer)
3. Present

There is no retained state between frames — no surfaces cached on the GPU, nothing to invalidate. What you trade in redundant drawing, you get back in simplicity: no damage tracking bugs, no tile management, no stale-pixel failure modes. Flash is pixel-correct by construction.

## Characteristics

| | |
|---|---|
| Strategy | Direct, full clear each frame |
| Retained GPU memory | None beyond textures |
| Hello-world RAM @1080p | ~22 MB (design target) |
| Binary size impact | Smallest |
| Correctness risk | None — full redraw can't go stale |

## When to Use Flash

- **Default choice.** It's the recommended production renderer until forge matures
- Small apps and static UIs where the whole window fits in a few thousand nodes
- Low-footprint targets where RAM matters more than present bandwidth

The cost of full-clear drawing shows up as present bandwidth: on a large static UI, flash re-uploads unchanged pixels every frame. That's the problem [forge](forge.md) solves — but for most apps, flash's bandwidth is well within what any GPU handles at 60 Hz.

## In Dev Mode

Flash is always compiled into `morph_devrt`. Switch to it live from DevTools → Rendering → RENDERER (`Flash | Forge` toggle), and watch frame stats while it runs.
