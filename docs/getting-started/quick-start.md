# Quick Start

Build and run a native desktop app in three steps.

## 1. Create a Project

```bash
morph init my-app
cd my-app
```

This launches an interactive wizard where you set the project name, window size, title, and entry file. Or pass everything upfront:

```bash
morph init my-app --width 1024 --height 768 --title "My App"
```

Use `morph init .` to scaffold into the current directory.

## 2. Start Development

```bash
morph dev
```

A native window opens. Edit `src/App.mx` — the window updates instantly without restarting. On every save, Morph re-runs the pipeline, recompiles your JS logic to a shared library, and pushes the updated IR to the running window.

## 3. Ship

```bash
morph run
```

Builds an optimized native binary and runs it. `morph run` compiles first, then launches the binary. Press `Ctrl+C` to stop.

You can also build without running:

```bash
morph build          # compile only
morph build --static # single self-contained binary (bundles GLFW/FreeType)
```

## What Just Happened?

```
src/App.mx  ──morph──►  native binary (OpenGL + GLFW)
```

Morph compiles your `.mx` file (JSX + CSS + TypeScript) directly to C++ via tree-sitter, then compiles that to a native binary with g++. There is no browser, no Node.js, and no interpreter in the final output.

## Next Steps

- [Project Structure](project-structure.md) — Understand what `morph init` generated
- [Configuration](configuration.md) — All `morph.config.json` options
- [CSS Properties](../css/properties.md) — What CSS you can use
- [State & Effects](../javascript/state.md) — Reactivity with `morphState`
