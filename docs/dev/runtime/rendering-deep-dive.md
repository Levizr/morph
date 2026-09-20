# Pixels on Screen: Flash, Forge, and the Compositor

**Part of:** [Dev Docs](../architecture/overview.md)

Morph renders with OpenGL 3.3 and no browser — but "renders" hides a two-thread architecture, a pointer-free frame snapshot format, two competing renderer personalities, and a font pipeline involving HarfBuzz shaping and dual texture atlases. This page is the full tour: how a frame gets from your node tree to photons, who runs on which thread, and why there are two renderers when one would have been so much simpler.

The user-facing choice ("which renderer should I pick?") lives in the main rendering docs. Short answer: **flash for production, forge is beta.** This page explains what that means mechanically.

## The two threads and the sacred snapshot

The architecture splits work across two threads with a lock-free handoff:

| Thread | Does | Must never |
|---|---|---|
| Main | Events, style, layout, paint (flatten), effect flush, coroutine drain | Block on the network; touch GL presentation state |
| Compositor | Vsync wait, GL presentation, CPU-side animation interpolation | Touch the node tree |

They communicate through pointer-free snapshots, defined in `core/render_frame.h`:

- **`FlatRenderNode`** — the node, flattened: `dlOffset`/`dlCount` into the frame's POD `DrawOp` array, `animOffsetX/Y` + `animOpacity` (compositor-written), baked `opacity` (the accumulated product — transparency is multiplied down the tree at flatten time), transform fields, screen-space `cullX/Y/W/H`, input overlay fields (`selX0/selX1/caretX` with NaN sentinels), `textOpOffset/textOpCount`, children indices. No pointers anywhere — the frame can cross the thread boundary without anyone chasing references.
- **`FlatTextOp`** — a text run, carrying `centerInk` (shared-line em baseline vs optical per-run centering).
- **The queues** — two lock-free SPSC queues: `g_eventQueue<MorphEvent, 64>` (GLFW input → main) and `g_feedbackQueue<AnimCompletionEvent, 64>` (compositor → main, "that animation finished"). Plus the frame triple: `g_frontFrame` (atomic), `g_backFrames[2]`, `g_framePending`, `g_frameInterpolated`.

The compositor (`core/compositor.cpp`) sleeps 1ms waiting on `g_framePending`, consumes it, applies easing **on CPU** (writing the `anim*` fields of the live front frame — no GL on this thread), stores `g_frameInterpolated`, and pushes completion events. Interpolating on CPU against an immutable snapshot is the trick that makes animations smooth without locking the node tree: the main thread can already be laying out the *next* frame while the compositor eases the *current* one.

**Concept to pocket:** only the compositor-safe subset animates off-main (`CompositorAnimProperty`: X, Y, colors, border radius, opacity — see `Easing`: Linear/EaseIn/EaseOut/EaseInOut). Layout-affecting properties stay on main, because moving layout off-thread would require locking the tree, and locking the tree would defeat the entire design.

## Flash: the reliable workhorse (`renderers/flash/flash.cpp`)

Flash is 55 lines long, and every line is honest:

```
flashCommit: layoutIfNeeded → recordPaintTree → flatten to back frame
             → atomic swap → g_framePending → clearPendingRender
flashPresent (= win.renderFrame): wait interpolated → clear body color
             → recursive renderNode → flush → swap buffers
```

Full clear, full redraw, every frame. No retained surfaces, no damage tracking, no cleverness — and that is precisely why it is the production default. There is almost nothing to go stale, tear, or ghost. The `culltest` fixture (870+ nodes, 600 scrollable rows) exists to prove flash stays fast by culling off-screen work rather than by caching on-screen work.

## Forge: the ambitious understudy (`renderers/forge/`)

Forge is the retained-surface compositor, currently beta with known damage-rect and scroll quirks. Phase 3 (shipped) keeps a **persistent FBO + RGBA8 color texture + `DEPTH24_STENCIL8` RBO** and repaints only what changed:

```
forgeCommit: snapshot pre-layout PaintDirty nodes (genuine content changes,
             BEFORE layoutIfNeeded stamps them)
             → fullscreen damage if first frame / compositor anim running / node count changed
             → else per-node box-diff vs g_prevRects (old + new rects, killing ghosts),
                1px rounded-clip margin, clipTo(view); prune stale prev-rects on rebuild
forgePresent: wait interpolated → ensureSurface
             → fullscreen? clear + drawFrameNodes()
             → damaged? reset depth/stencil, scissor-clear each damage rect with body
                color, re-raster only nodes touching damage, then blit the WHOLE FBO
                (the swapchain is undefined after swap — savings come from retained
                raster, not the blit)
```

Supporting cast: `damage.h`/`damage.cpp` (`DamageRect`, `DamageSet` with greedy bounding-union merging, `totalArea`, `intersects`), `tile.h`/`tile_pool.h` (`TilePool`, 16MB budget, LRU deque) and `layer.h` (`RetainedLayer`) as Phase-2 tile scaffolding for the planned tile pool (see the forge-renderer future doc). Stats (`damageArea`/`presentBytes`) feed the DevTools Rendering tab.

**Why forge types never leak:** forge structs stay inside `renderers/forge/` and never appear in `node.h` — zero ABI risk for `logic.so` hot reload. Production selection is a build-time `constexpr` (`MORPH_RENDERER_FORGE`); dev mode can hot-switch via the atomic + `g_renderMode` toggle (`MORPH_FEATURE_DEV_RENDERER_SWITCH`). Read `help/renderer-flash-forge.md` before changing compositor behavior — it is the design record with the scars.

## The GL backend (`render/`): batches, SDF edges, and atlases

`gl_renderer.h`/`.cpp` turns draw ops into pixels with instanced batching: one unit quad, per-instance VBOs, `flush()` order fixed as quad batch → per-atlas text batches → per-texture image batches → **border batch last** (borders always on top).

- **Shaders** (`render/shader.h`): GLSL spliced with `#ifdef` macros (per-instance `mat4 aModel`). The quad fragment shader is an exact SDF rounded box with explicit directional derivatives (`dFdx`/`dFdy`) for a true 1px anti-aliased edge, nested fill+border blending, stencil mode. Text (R8 + RGBA atlas), image, and helpers round it out.
- **Clips**: scissor with flush-on-both-edges for the simple case; **rounded clips via stencil `GL_INCR` nesting** for intersecting masks.
- **Depth**: `GL_LEQUAL` under `MORPH_FEATURE_TRANSFORM` so 3D layers occlude while flat content keeps painter's order.
- **Fonts**: resolved at runtime from candidate path lists (DejaVu/Liberation/Arial/Helvetica/segoeui + Noto Color Emoji/Apple Color Emoji/seguiemj). `FontAtlas` keeps R8 (grayscale) and RGBA (color emoji) textures with row-packing cursors, incremental `glTexSubImage2D` uploads, and doubling `growAtlas`. **HarfBuzz shaping** produces advances/offsets/cluster→codepoint mapping; optical ink centering with shared-line em-baseline fallback; non-ASCII routes to the emoji atlas with `emojiScale` for strike mismatch. (Known quirk, documented in the story docs: emoji measure/draw font mismatch. Fonts are a journey.)
- **Images**: `stbi_load` decoding with a refcounted texture cache (dims packed in one int).

## Window plumbing (`core/window.*`, `event.h`, `clipboard.h`, `window_manager.h`)

`window.cpp` owns the event→UI seam: 0.3s double-click threshold; `_fireHoverDelta` firing hover/leave only on set-membership change (moving into a child doesn't re-trigger the parent); X11 pointer grab for scrollbar thumb drags; `:active` chain on press/release; click-to-focus for `<input>`, click-elsewhere blurs; key events route to `s_focusedNode->onKeyEvent` first (consume = stop), char events to the focused node only; window focus loss clears capture + blur. `MorphWindow` also hosts the **docked DevTools strip** (`contentWidth() = width − devtoolsWidth`, clamped ≥120). `renderNode` implements rounding suppression during layout transitions, viewport culling, damage-limited re-raster, the anchor-cancellation transform model path, clip stack (scissor vs stencil under transform), scroll push/pop, input selection/caret overlay, and scrollbar math. Clipboard is GLFW-backed; `WindowManager` is a singleton whose dtor is the *only* place `glfwTerminate()` runs (after all windows, handles, and cursors are freed — shutdown order is a feature).

## Every subsystem is compile-time gated

Nearly everything above is wrapped in `MORPH_FEATURE_*` defines (ANIMATION, POSITION, ZINDEX, TRANSFORM, DEV, DIRTY_RENDERING, INPUT, TEXT, IMAGE, CURSOR, SCROLL, FLEX, BORDER, RADIUS, MIN_MAX, MARGIN_COLLAPSE, INLINE, DISPLAY_NONE, OPACITY, FORGE_RENDERER…). Dev mode enables all features; production builds enable only what `feature_set` detected. The animation driver (`core/node/animation.cpp`) compiles to *nothing* without `MORPH_FEATURE_ANIMATION` — struct layout unchanged in prod builds. Feature flags live behind `#ifdef`, never runtime `if`s, so unused features cost zero bytes.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Change what's compositor-safe | `render_frame.h` (`CompositorAnimProperty`) + `compositor.cpp` interpolation — and prove the property never affects layout |
| Fix a flash bug | `renderers/flash/flash.cpp` + `window.cpp` `renderNode` — small surface, read it all |
| Fix forge damage/scroll ghosts | `renderers/forge/forge.cpp` + `damage.*` — check `g_prevRects` pruning and old+new rect union first |
| Change batching/shaders | `render/gl_renderer.*`, `render/shader.h` — flush order matters, borders last |
| Change font behavior | Atlas + shaping in `gl_renderer.cpp` — test emoji, bold, and CJK, not just ASCII |
| Add a render feature | New `MORPH_FEATURE_*` + `feature_set.rs` detection + CMake wiring |

## Verify by

```bash
<binary> --morph-self-test     # 0 failures
./tests/runtime/run-selftests.sh
# Visual: run culltest (flash stress), animation-test 1+2.0, transform-test,
# opacity-test, zindex-test under a live X server (:0) and screenshot the window
```

Rendering changes deserve screenshots, not just green tests. Pixels are the contract — diff them with your eyes.
