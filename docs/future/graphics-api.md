# Multiple Graphics APIs — Vulkan, Metal, DirectX

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Morph renders through **OpenGL 3.3** today. The future is a pluggable graphics layer: **Vulkan**, **Metal** (macOS), and **DirectX 11/12** (Windows) backends behind one internal abstraction — with the API selected per app or auto-detected per platform.

## Why it matters

- **macOS** — OpenGL is deprecated on Apple platforms; a Metal backend is effectively required for first-class macOS support (see [Platforms](platform.md))
- **Windows** — DirectX is the native API; Vulkan/D3D backends make Windows a first-class target
- **Vulkan** — the only truly cross-platform modern API (Linux/Windows/macOS/Android); explicit GPU control maps naturally to Morph's retained [Forge](forge-renderer.md) tile architecture
- **Performance headroom** — low-overhead APIs (explicit barriers, command buffers, async compute) for the large-UI cases Forge targets
- **Rust synergy** — the [Rust runtime](rust.md) can use `wgpu`, which wraps Vulkan/Metal/D3D12/GL in one crate — nearly all backends for free on the Rust side

## How it will work

### Selection

```jsonc
// morph.config.json
{ "graphics": "auto" }        // default: pick per platform (metal on macOS, d3d12 on Windows, opengl/vulkan on Linux)
{ "graphics": "vulkan" }      // force a specific backend
{ "graphics": "opengl" }      // today's behavior, unchanged
```

- Selection is **compile-time** like the renderer choice today (`constexpr` dispatch, unselected backends eliminated — zero dead code, no runtime branch)
- Dev mode can compile multiple backends and switch at runtime (same pattern as the Flash/Forge dev toggle)
- `"graphics": "auto"` resolves at build time based on the target platform

### The abstraction

The low-level GPU layer (`runtime/render/` — `GLRenderer`: rects/text/borders, batching + flush, clips, stencil) is already the natural seam. It becomes an interface with one backend per API:

```
runtime/render/
├── gpu.h                  # GpuDevice / GpuTexture / GpuBuffer / GpuPipeline / GpuSwapchain
├── gpu.cpp
├── backends/
│   ├── gl/                # today's GLRenderer, moved behind the interface
│   ├── vulkan/            # swapchain, command buffers, descriptor pools
│   ├── metal/             # MTLDevice, render pass encoders (macOS)
│   └── d3d12/             # command lists, root signatures (Windows)
```

```cpp
// gpu.h (sketch — the exact surface is free-form)
struct GpuDevice;
struct GpuTexture;
struct GpuSwapchain;

struct GpuDevice {
    virtual GpuTexture* createTexture(int w, int h, const void* pixels) = 0;
    virtual GpuBuffer* createVertexBuffer(const void* data, size_t size) = 0;
    virtual void beginFrame(GpuSwapchain&) = 0;
    virtual void draw(const Batch&, const GpuPipeline&) = 0;
    virtual void present(GpuSwapchain&) = 0;
};
```

Everything above the interface is unchanged: node tree, layout, style, the batched draw list, texture atlas, SDF math, stencil clipping.

### Shaders — one source, every API

The fragment shaders (rounded-rect SDF, borders, text, transforms) are the shared core:

1. GLSL sources compiled once to **SPIR-V** (glslang)
2. SPIR-V cross-compiled per backend: GLSL (OpenGL), MSL (Metal), DXIL (DirectX)
3. Backends load their native shader form; the SDF math is identical everywhere

This guarantees **pixel-identical rendering across all backends** — the same layout math, the same shader math.

### What maps naturally

| Concept | OpenGL | Vulkan | Metal | D3D12 |
|---|---|---|---|---|
| Frame buffer | FBO | swapchain + render pass | CAMetalLayer + pass | swapchain + command list |
| Batched quads | one VAO/VBO | one vertex buffer + descriptor | one buffer | one buffer + root sig |
| Texture atlas | GL texture | image view + sampler | MTLTexture | SRV heap |
| Clipping | stencil ops | stencil attachment | stencil in pass | stencil in PSO |
| Compositor thread | GL context owner | command pool + queue | render pass queue | command queue |

The compositor-thread model stays: each backend owns its device context on the compositor thread and presents at vsync.

## Current state

| Building block | State |
|---|---|
| OpenGL 3.3 renderer (`runtime/render/`) | ✅ Shipped |
| Renderer seam pattern (Flash/Forge compile-time + dev toggle) | ✅ Shipped — the blueprint to copy |
| SDF shader stack (rounded rects, borders, text) | ✅ Shipped |
| `graphics` config key | ❌ Not built |
| GPU abstraction (`gpu.h`) | ❌ Not built |
| Vulkan / Metal / D3D backends | ❌ Not started |

## Suggested phases

1. **Seam refactor** — extract `GLRenderer` behind `gpu.h`; zero behavior change (the Flash/Forge Phase-1 pattern, reused)
2. **SPIR-V pipeline** — GLSL → SPIR-V → per-API shader; render an identity test image on all backends
3. **Vulkan backend** — init, swapchain, command buffers, batched draws, stencil clipping
4. **Metal backend** — macOS target (unblocks first-class [macOS support](platform.md))
5. **D3D11/12 backend** — Windows target (unblocks [Windows support](platform.md))
6. **Auto-selection** — `"graphics": "auto"` per platform + forced overrides + dev runtime switch
7. **Forge on new backends** — damage/tiles/scroll-shift map to retained resources (Vulkan and D3D12 are the natural homes for Forge's tile pool)
8. **Parity matrix** — pixel-identical rendering across all backends + the C++/Rust runtimes

## Open questions

- **Which first?** Metal (unblocks macOS) and D3D (unblocks Windows) are platform-critical; Vulkan is the ambitious cross-platform one. Order depends on whether [Platforms](platform.md) or raw Vulkan matters more
- **Vulkan boilerplate** — full explicit Vulkan is a huge surface; do we use a helper layer (volk/glslang) or write it raw?
- **Rust runtime** — if the Rust side uses `wgpu`, the abstraction is inherited; should the C++ side mirror wgpu's surface (hal-like) for symmetry?
- **Dev-mode multi-backend** — compile all backends in dev for live switching, or one backend per dev build?

## Build steps (when picked up)

1. `gpu.h` seam + GLRenderer extraction, smoke-tested pixel-identical
2. SPIR-V shader pipeline (one GLSL source → GLSL/MSL/DXIL)
3. Vulkan backend with `examples/calculator` running pixel-identical
4. Metal backend → macOS CI; D3D backend → Windows CI
5. `"graphics": "auto"` resolution + config override + docs