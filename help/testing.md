# Testing Guide

## Running Tests

```bash
# All tests
python -m pytest tests/ -v

# Specific test file
python -m pytest tests/unit/test_jsx_walker.py -v

# Specific test
python -m pytest tests/ -v -k "tailwind"

# With coverage
python -m pytest tests/ -v --cov=morph
```

## Test Structure

```
tests/
├── unit/
│   ├── test_morph_parser.py     # .mx parsing (tree-sitter)
│   ├── test_jsx_walker.py       # AST walking (imports, components, JSX, styles)
│   ├── test_css_parser.py       # CSS parsing (rules, selectors, properties)
│   ├── test_tailwind.py         # Tailwind resolution (+ z-index utilities)
│   ├── test_selector.py         # selector parsing, combinators, specificity, ancestor-hover
│   ├── test_ir_builder.py       # IR building (styles, events, hover/active rules, z-index, flex)
│   ├── test_layout.py           # layout engine (box model, flex)
│   ├── test_layout_units.py     # unit conversion in layout
│   ├── test_color_utils.py      # color parsing
│   ├── test_feature_set.py      # MORPH_FEATURE_* detection
│   └── test_js_codegen.py       # TS → C++ translation (types, control flow, expressions)
├── integration/
│   └── test_full_pipeline.py    # end-to-end pipeline
└── fixtures/                    # HTML fixtures for pipeline tests
```

## What's Tested vs What's Not

### ✅ Well Tested

| Module | Test File | Coverage |
|---|---|---|
| `MorphParser` | `test_morph_parser.py` | Valid syntax, syntax errors, edge cases |
| `JSXWalker` | `test_jsx_walker.py` | Imports, components, JSX structure, props, styles, camelCase→kebab |
| `CSSParser` | `test_css_parser.py` | Empty input, single rule, multiple rules, file vs string |
| `TailwindResolver` | `test_tailwind.py` | Static classes, arbitrary values, unknown classes skipped, `z-*` |
| `SelectorEngine` | `test_selector.py` | Combinators (descendant/child/adjacent/sibling), specificity, ancestor-hover syntax |
| `ColorUtils` | `test_color_utils.py` | Hex, shorthand, named colors, parse_color |
| `IRBuilder` | `test_ir_builder.py` | Inline styles, Tailwind, hover/active rules, ancestor-hover, z-index, flex props, events |
| `TS→C++` | `test_js_codegen.py` | Variable/const/let, functions, arrow functions, classes, control flow, template literals, type annotations, JS semantics |
| `FeatureSet` | `test_feature_set.py` | Feature detection across IR trees |

### ⚠️ Needs Better Tests

| Module | Test File | What It Actually Checks |
|---|---|---|
| `LayoutEngine` | `test_layout.py` | Box model basics — flex-wrap / flex shorthand / inline measure paths need more assertions |

### ❌ Not Covered

| Module | Reason |
|---|---|
| `CSSFetcher` | No tests (network path) |
| `Dev Pipeline` | Orchestration tested only via integration test |
| `DevRT binary` | Requires a GLFW/OpenGL environment; not unit-tested |
| `IPC / socket protocol` | No tests |
| `C++ runtime` | `tests/test_slice_ts.cpp` compiles a sample translation manually — the C++ runtime itself isn't in CI |
| `Package manager` | No tests |

## Writing Tests

Tests use plain `pytest` (no unittest). Follow the existing patterns:

```python
from morph.parser.morph_parser import MorphParser


def test_valid_mx_parses_without_error():
    source = """
    <body>
      <div>Hello</div>
    </body>
    """
    result = MorphParser().parse(source)
    assert result is not None
    assert result.root_node is not None
```

### What to Write Tests For

The most valuable tests right now:

1. **`IRBuilder.build()`** — Already has a good suite; extend for:
   - `windowConfig` export parsing
   - `morphState`/`morphEffect` usage detection
   - Conditional JSX (`{cond && <JSX>}`, `{cond ? <A/> : <B/>}`)
   - Flex shorthand (`flex: 1` → `1 1 0%`)

2. **`LayoutEngine`** — Extend `test_layout.py`:
   - Multi-line flex wrap with per-line justify
   - Flex grow/shrink distribution
   - Inline measure pass and whitespace handling
   - `max-width`/`min-width` constraints

3. **`TS→C++`** — Extend `test_js_codegen.py`:
   - `await fetch()` → coroutine/resume codegen
   - `morphEffect` cleanup functions
   - Error rethrow from `morph::Result<T>`

4. **C++ runtime** — The most impactful gap: exercise `signal.h`, `task.h`, `net.h`, and the renderer dispatch against mocked GLFW.
