# Border & Border-Radius Bug Fix Plan

## Root Cause

Investigation of the entire border/border-radius pipeline (CSS -> IR -> codegen -> C++ runtime -> GL rendering) revealed that CSS parsing, IR building, codegen, and C++ style structs are all correct. The bugs are in **OpenGL stencil buffer management**.

### Two Critical Stencil Bugs

**Bug A (`gl_renderer.cpp:269`):** `GLRenderer::clear()` calls `glClear(GL_STENCIL_BUFFER_BIT)` which resets the hardware stencil buffer to 0, but does **not** reset the tracking variable `m_stencilClipDepth`. If `m_stencilClipDepth` is non-zero at frame end, the next frame's `beginRoundedClip()` computes an unreachable stencil reference. All fragments of every element with `border-radius > 0` fail the stencil test — element becomes invisible.

**Bug B (`gl_renderer.cpp:292`):** `beginRoundedClip()` uses `glStencilFunc(GL_ALWAYS, ...)` instead of `glStencilFunc(GL_EQUAL, currentDepth, ...)`. For nested border-radius elements, this increments stencil values for pixels OUTSIDE the parent's clip region, corrupting the stencil buffer and causing the depth tracker to drift. Makes Bug A far more likely to trigger.

## Fixes

- [x] **Fix 1 (Critical)** — Reset `m_stencilClipDepth` in `clear()`
  - File: `morph/runtime/render/gl_renderer.cpp:269`
- [x] **Fix 2 (High)** — Use `GL_EQUAL` in `beginRoundedClip()`
  - File: `morph/runtime/render/gl_renderer.cpp:292`
- [x] **Fix 3 (Medium)** — Fix scissor nest handling (`endClip()` unconditionally disables scissor test, breaking nested `overflow: hidden`)
  - File: `morph/runtime/render/gl_renderer.cpp:279`
- [x] **Fix 4 (Medium)** — Add rounded clip to `ButtonNode::executeDisplayList()`
  - File: `morph/runtime/widgets/morph_button.h:39`
- [x] **Fix 5 (Medium)** — Set `PaintDirty` in `layout()` (children directly laid out by parent don't get `PaintDirty` set, leaving stale display lists)
  - File: `morph/runtime/core/node.cpp:1029`
- [x] **Fix 6 (Low)** — Remove hardcoded 100px radius clamp in shader
  - File: `morph/runtime/render/shader.h:51`
- [x] **Build & test** — `morph build` in my-app (198 KB, 108/109 tests pass)
