# Testing Guide

## Running Tests

```bash
# All tests
python -m pytest tests/ -v

# Specific test file
python -m pytest tests/unit/test_jsx_walker.py -v

# Specific test
python -m pytest tests/ -v -k "test_tailwind"

# With coverage
python -m pytest tests/ -v --cov=morph
```

## Test Structure

```
tests/
├── unit/
│   ├── test_morph_parser.py     # .mx parsing
│   ├── test_jsx_walker.py       # AST walking
│   ├── test_css_parser.py       # CSS parsing
│   ├── test_tailwind.py         # Tailwind resolution
│   ├── test_ir_builder.py       # IR building (trivial)
│   ├── test_layout.py           # layout engine (trivial)
│   └── test_color_utils.py      # color parsing
├── integration/
│   └── test_full_pipeline.py    # end-to-end (expects None currently)
├── test_html_lexer.py           # BROKEN — depends on non-existent morph.lexer
└── test_css_lexer.py            # BROKEN — depends on non-existent morph.lexer
```

## What's Tested vs What's Not

### ✅ Well Tested

| Module | Test File | Coverage |
|---|---|---|
| `MorphParser` | `test_morph_parser.py` | Valid syntax, syntax errors, edge cases |
| `JSXWalker` | `test_jsx_walker.py` | Imports, components, JSX structure, props, styles, camelCase→kebab |
| `CSSParser` | `test_css_parser.py` | Empty input, single rule, multiple rules, file vs string |
| `TailwindResolver` | `test_tailwind.py` | Static classes, arbitrary values, unknown classes skipped |
| `ColorUtils` | `test_color_utils.py` | Hex, shorthand, named colors, parse_color |

### ⚠️ Needs Better Tests

| Module | Test File | What It Actually Checks |
|---|---|---|
| `IRBuilder` | `test_ir_builder.py` | Empty input returns `[]` (needs tests for inline styles, Tailwind, events, etc.) |
| `LayoutEngine` | `test_layout.py` | Empty windows doesn't crash (needs position/size assertions) |

### ❌ Not Tested

| Module | Reason |
|---|---|
| `CSSFetcher` | No tests |
| `StyleResolver` | Stub — nothing to test |
| `FlexLayout` | Stub — nothing to test |
| `JSInterpreter` | Stub — nothing to test |
| `NodeEmitter` | Stub — nothing to test |
| `EventEmitter` | Stub — nothing to test |
| `Dev Pipeline` | No tests for the orchestration |
| `DevRT` | No tests, binary doesn't exist yet |
| `IPC Server` | No tests |
| `Package Manager` | Only `add` works, no tests |
| `Package Installer` | No tests |
| `Package Resolver` | Stub — nothing to test |
| `CLI commands` | No tests for argparse dispatch |

## Writing Tests

### Style Guidelines

Tests use plain `pytest` (no unittest). Follow the existing patterns:

```python
from morph.parser.morph_parser import MorphParser


def test_valid_mx_parses_without_error():
    source = """
    <morph-window title="Test" width="800" height="600">
        <div>Hello</div>
    </morph-window>
    """
    result = MorphParser().parse(source)
    assert result is not None
    assert result.root_node is not None
```

### What to Write Tests For

The most valuable tests right now:

1. **`IRBuilder.build()`** — Now implemented, needs tests for:
   - Empty component tree → empty list
   - Single div → single IRNode with correct type
   - Inline styles → correct IRStyle values (color parsing, unit conversion)
   - Tailwind classes → resolved to CSS properties
   - `morph-open` attribute → IREvent created
   - `morph-window` → IRWindow with config
   - CSS cascade order (inline > Tailwind > tag rules)
   - Style shorthand parsing (padding: 4 values → 4 sides)

2. **`LayoutEngine`** — Test:
   - Single node at origin
   - Vertical stacking with gap
   - Margin/padding affects position
   - Flex layout (when implemented)

3. **`StyleResolver`** — When implemented, test:
   - Tag selector matching (`h1` matches `<h1>`)
   - Class selector matching (`.foo` matches `<div class="foo">`)
   - Specificity ordering
   - Inline style override

4. **Pipeline integration** — Test:
   - Full .mx → IR dict end-to-end
   - CSS import resolution
   - Error handling for malformed input

### Running Broken Tests

Two test files import the non-existent `morph.lexer` module and will fail:

- `tests/test_html_lexer.py`
- `tests/test_css_lexer.py`

These are remnants of an older architecture. They should either be removed or rewritten for the current tree-sitter-based approach.
