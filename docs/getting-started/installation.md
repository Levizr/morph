# Installation

## Prerequisites

Morph requires Python 3.10+ and a C++ compiler with C++23 support.

### Linux (Debian/Ubuntu)

```bash
# Python
sudo apt install python3 python3-pip

# C++ toolchain
sudo apt install g++ cmake make pkg-config

# Graphics
sudo apt install libglfw3-dev libgl1-mesa-dev libx11-dev

# Text rendering
sudo apt install libfreetype-dev libharfbuzz-dev
```

### macOS

```bash
# Python
brew install python@3.12

# C++ toolchain (Xcode includes clang++)
xcode-select --install

# Graphics + text
brew install glfw freetype harfbuzz cmake pkg-config
```

### Windows

Use MSVC (Visual Studio) or MinGW with C++23 support. GLFW, FreeType, and HarfBuzz are bundled for Windows builds — no manual installation needed.

## Install Morph

```bash
pip install levizr-morph
```

## Verify Your System

```bash
morph doctor
```

This checks your toolchain (g++, cmake, make, pkg-config), graphics libs (GLFW, OpenGL), text libs (FreeType, HarfBuzz), and bundled vendor files. If anything is missing, it offers to install it automatically:

```bash
morph doctor -y    # auto-install missing packages
morph doctor -v    # show detailed version info
```

## Supported Package Managers

`morph doctor` detects your system package manager and uses the correct install commands:

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
