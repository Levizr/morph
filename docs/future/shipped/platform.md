# Platform Support — Linux, Windows & macOS (Shipped)

**Status:** production · **Priority:** — (shipped)

> **Note:** This page is a shipped record, not a proposal. Windows and macOS targets build and release from CI; what remains is tracked in the linked pages.

Morph ships on **Linux** (X11, optionally Wayland), **Windows** (x64, MSVC), and **macOS** (arm64, clang). The release pipeline cross-compiles all three (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`) with per-platform packaging, and `morph doctor` maps dependencies per package manager (apt/dnf/pacman/zypper/apk/brew/**winget/choco**).

## What shipped

| Layer | Linux | macOS | Windows |
|---|---|---|---|
| Windowing | GLFW (X11/Wayland) | GLFW (Cocoa) | GLFW (Win32) |
| OpenGL | 3.3 core | 3.3 core (deprecated but works) | 3.3 core |
| Text | FreeType + HarfBuzz (system) | FreeType/HarfBuzz (brew) | Bundled |
| Compiler | g++ 11+ | clang++ 13+ | MSVC |
| CI release builds | ✅ | ✅ | ✅ (+ packaging step) |

## What remains (elsewhere)

- **Modern graphics backends** — Metal (macOS) and DirectX (Windows) still go through [Graphics APIs](../graphics-api.md). OpenGL works on both today; it's deprecated on macOS, not dead.
- **OS-level modules** — tray icons, native dialogs, and notifications need APIs beyond GLFW — see [Native Modules](../native-modules.md).
- **Signing & stores** — built on top of these targets in [Security & Commercial Release](../security.md).

## History

Shipped step by step: Linux first, then the CI matrix grew `aarch64-apple-darwin` (macOS-arm64) and `x86_64-pc-windows-msvc` (windows-x64) release jobs with a Windows packaging step. The old "never compiled there" checklist (per-platform dev runtime, toolchain maps, context/HiDPI verification) closed out as the release workflow landed.
