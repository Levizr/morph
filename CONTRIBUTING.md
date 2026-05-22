# Contributing to Morph

Morph is in early development. All contributions — code, docs, bug reports, ideas — are welcome.

## Quick Start

```bash
git clone https://github.com/levizr/morph
cd morph
pip install -e ".[dev]"
morph doctor
```

## Where to Start

The most impactful areas right now:

| Area | Difficulty | Impact | Guide |
|---|---|---|---|---|
| **C++ Node Emitter** (`morph/codegen/node_emitter.py`) | Medium | Generates actual C++ from IR | [Pipeline Deep-Dive](docs/pipeline.md) |
| **CSS Style Resolver** (`morph/style/resolver.py`) | Medium | CSS cascade + specificity | [Development Guide](docs/development.md) |
| **JS Interpreter** (`morph/js/interpreter.py`) | Hard | JS expressions → C++ lambdas | [Development Guide](docs/development.md) |
| **`morph_devrt` binary** (`runtime/`) | Hard | Dev mode renderer | — |
| **`flex-wrap` / `position: relative` / `border`** | Medium | Layout and rendering features | [Development Guide](docs/development.md) |
| **Tests** | Easy | Increase coverage | [Testing Guide](docs/testing.md) |
| **Bug fixes** | Varies | Any open issue | — |

## Code Conventions

- **Python**: 3.10+, type hints everywhere, `from __future__ import annotations` in new files
- **C++**: C++17, header-only for runtime headers
- **Imports**: stdlib → third-party → morph modules (blank line between groups)
- **Naming**: `snake_case` for Python, `PascalCase` for classes, `UPPER_CASE` for constants
- **Errors**: Custom exception classes in `morph/parser/errors.py` pattern

## Pull Request Process

1. Open an issue first for significant changes so we can align on design
2. Keep PRs focused — one feature/fix per PR
3. Add or update tests in `tests/`
4. Run tests: `python -m pytest tests/ -v`
5. Update docs if changing public API or pipeline behavior
6. Mention in PR description what's working and what's still TODO

## Project Board

See the [Current State](./README.md#current-state-early-development) section in the README for a full breakdown of what's implemented, stubbed, and missing.
