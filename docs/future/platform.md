# Platform Support — Windows & macOS

**Status:** future · **Priority:** medium

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Morph currently targets **Linux** (X11, optionally Wayland). The system-requirements table already lists macOS and Windows as targets — this page is about actually making them work.

## Why it matters

- The largest desktop app markets are Windows + macOS
- GLFW is already cross-platform — the windowing layer is portable in principle
- Everything above the windowing layer (layout, renderer, compiler, JS runtime) is platform-agnostic C++/Python

## What's platform-specific today

| Layer | Linux | macOS | Windows |
|---|---|---|---|
| Windowing | GLFW (X11/Wayland) | GLFW (Cocoa) | GLFW (Win32) |
| OpenGL | 3.3 core | 3.3 core (deprecated but works) | 3.3 core |
| Text | FreeType + HarfBuzz (system) | FreeType/HarfBuzz (brew) | **bundled** (per requirements table) |
| Compiler | g++ 11+ | clang++ 13+ | MSVC / MinGW |
| Build | CMake + make | CMake + make | CMake + MSVC |

The table in the README claims FreeType/HarfBuzz are bundled on Windows already — the dependency story needs to be proven end-to-end (`morph doctor` has per-package-manager install maps for apt/dnf/pacman/zypper/apk/brew/**winget/choco**).

## What needs work

1. **CI** — no Windows/macOS runners exist; the runtime has never compiled there
2. **`morph_devrt`** — the pre-compiled dev renderer is a Linux binary; dev mode needs per-platform builds
3. **Graphics APIs** — OpenGL is deprecated on macOS and a foreign API on Windows; first-class platform support needs the Metal/DirectX backends (see [Graphics APIs](graphics-api.md))
4. **Native modules** — [Menu / Tray / Dialog](native-modules.md) are impossible without a platform abstraction layer over OS APIs (tray icons, native dialogs)
5. **Signing/packaging** — `.msi`/`.dmg` packaging is a downstream concern, but `morph build` should eventually produce distributable artifacts
6. **Compositor/GL specifics** — context creation flags, vsync (`glfwSwapInterval`), and HiDPI (`glfwGetFramebufferSize`) need per-platform verification

## Suggested order

1. **Windows first** (biggest market): MSVC/MinGW toolchain in `morph doctor`, dev runtime build via CMake, CI job
2. **macOS second**: clang++ toolchain, brew deps, CI job
3. Platform abstraction for native modules once both run

## Current state

| Piece | State |
|---|---|
| Linux (X11 + Wayland toggle) | ✅ Shipped |
| GLFW cross-platform windowing | ✅ (library level — unverified on Win/macOS) |
| `morph doctor` platform checks | ✅ Has winget/choco/brew maps |
| Windows / macOS builds | ❌ Never built in CI |
| Platform abstraction for native modules | ❌ Not started |