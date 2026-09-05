# How Do I Install Morph?

Morph is distributed as a single native binary called `morph`. No Python, no Node.js, no runtime to install separately.

## Quick Install

```bash
cargo install morphc
```

This installs the `morphc` package, which provides the `morph` binary.

That's it. First run downloads and compiles the Rust toolchain (~1-2 minutes). Subsequent runs are instant.

## System Requirements

You only need a C++ compiler with C++23 support. Morph's build system downloads the C++ runtime automatically.

### Linux (Debian/Ubuntu)

```bash
sudo apt install g++-14 cmake make pkg-config libglfw3-dev libgl1-mesa-dev libx11-dev libfreetype-dev libharfbuzz-dev
```

### macOS

```bash
xcode-select --install
brew install glfw freetype harfbuzz cmake pkg-config
```

### Windows

Use MSVC (Visual Studio 2022+) or MinGW with C++23 support. GLFW, FreeType, and HarfBuzz are bundled — no manual installation needed.

## Verify Installation

```bash
morph --version
morph doctor
```

`morph doctor` checks your toolchain (g++, cmake, pkg-config), graphics libs (GLFW, OpenGL), and text libs (FreeType, HarfBuzz).

```bash
morph doctor -y    # auto-install missing packages (Linux only)
morph doctor -v    # show detailed version info
```

## From Source (Latest Features)

```bash
git clone https://github.com/Levizr/morph.git
cd morph
cargo install --path crates/morphc
```

## What Gets Installed

- `morph` binary (~10-15 MB) — the compiler and CLI
- No Python dependencies
- No Node.js / npm
- Runtime sources are downloaded on first `morph dev` or `morph build` and cached globally at `~/.morph/cache/runtimes/`