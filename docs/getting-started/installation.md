# Installation

Morph ships as a single native binary (`morphc`) — no Python, no Node.js, no runtime to install separately.

## Prerequisites

You only need a C++ compiler with C++23 support. Morph's build system will download the C++ runtime automatically.

### Linux (Debian/Ubuntu)

```bash
# C++ toolchain (g++-14+ required for C++23)
sudo apt install g++-14 cmake make pkg-config

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

One-line installer (auto-detects your platform and installs the latest prebuilt binary):

```bash
curl -fsSL https://morph.levizr.com/install.sh | sh
```

- Detects Linux (x64/arm64), macOS (Apple Silicon), and Windows (x64).
- Installs `morph` to `~/.local/bin` and prints PATH setup if needed.
- Pin an exact version with `MORPH_VERSION=1.2.3` or change the install dir with `MORPH_INSTALL_DIR=/path/to/bin`.
- If no prebuilt binary exists for your platform (e.g. Intel Macs), the script prints Cargo/source instructions instead.

If you prefer Rust's package manager:

```bash
cargo install morphc
```

This downloads and compiles the Rust binary. First run takes 1-2 minutes; subsequent runs are instant.

### Prebuilt Binaries

Download `morph-<os>-<arch>.tar.gz` from the [Releases page](https://github.com/Levizr/morph/releases) and put the `morph` binary on your `PATH`. Assemblies: Linux (x64/arm64), macOS (arm64), Windows (x64). Intel Macs have no prebuilt binary — use `cargo install morphc` there.

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
morph doctor -v    # show detailed version info
```

> Note: `morph doctor -y` auto-installs missing system packages via your native package manager (no prompt). Without `-y`, doctor prompts before installing.

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