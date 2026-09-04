# Installation

Morph ships as a single native binary (`morphc`) — no Python, no Node.js, no runtime to install separately.

## Prerequisites

You only need a C++ compiler with C++23 support. Morph's build system will download the C++ runtime automatically.

### Linux (Debian/Ubuntu)

```bash
# C++ toolchain
sudo apt install g++ cmake make pkg-config

# Graphics
sudo apt install libglfw3-dev libgl1-mesa-dev libx11-dev

# Text rendering
sudo apt install libfreetype-dev libharfbuzz-dev
```

### macOS

```bash
# C++ toolchain (Xcode includes clang++)
xcode-select --install

# Graphics + text
brew install glfw freetype harfbuzz cmake pkg-config
```

### Windows

Use MSVC (Visual Studio 2022+) or MinGW with C++23 support. GLFW, FreeType, and HarfBuzz are bundled — no manual installation needed.

## Install Morph

### Stable Release (Recommended)

```bash
cargo install morphc
```

This downloads and compiles the Rust binary. First run takes 1-2 minutes; subsequent runs are instant.

### From Source (Latest Features)

```bash
git clone https://github.com/Levizr/morph.git
cd morph
cargo install --path crates/morphc
```

## Verify Your System

```bash
morph doctor
```

Checks your toolchain (g++, cmake, pkg-config), graphics libs (GLFW, OpenGL), and text libs (FreeType, HarfBuzz).

```bash
morph doctor -y    # auto-install missing packages (Linux only)
morph doctor -v    # show detailed version info
```

## Supported Package Managers (for `morph doctor -y`)

| Manager | OS |
|---|---|
| apt | Debian, Ubuntu |
| dnf | Fedora |
| pacman | Arch |
| zypper | openSUSE |
| apk | Alpine |
| brew | macOS |
| winget | Windows |
| choco | Windows |

## What Gets Installed

- `morphc` binary (~10-15 MB) — the compiler and CLI
- No Python dependencies
- No Node.js / npm
- Runtime sources are downloaded on first `morph dev` or `morph build` and cached globally at `~/.morph/cache/runtimes/`