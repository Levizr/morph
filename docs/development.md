# Development Guide

## Setup

```bash
# Clone and install
git clone https://github.com/levizr/morph
cd morph
pip install -e ".[dev]"

# Verify dependencies
morph doctor

# Run tests
python -m pytest tests/ -v
```

## Project Layout

```
morph/                        # Python toolchain
├── cli/                      # Command-line interface
│   ├── main.py               #   argparse dispatch
│   ├── cmd_init.py           #   morph init
│   ├── cmd_dev.py            #   morph dev
│   ├── cmd_build.py          #   morph build (TODO)
│   ├── cmd_doctor.py         #   morph doctor
│   ├── cmd_pkg.py            #   morph pkg
│   └── cmd_cache.py          #   morph cache
├── parser/                   # .mx file parsing
│   ├── morph_parser.py       #   tree-sitter AST builder
│   ├── jsx_walker.py         #   AST → imports + components
│   └── errors.py             #   custom exceptions
├── style/                    # CSS pipeline
│   ├── css_parser.py         #   tree-sitter CSS parser
│   ├── css_fetcher.py        #   remote CSS download + cache
│   ├── tailwind.py           #   Tailwind class resolver
│   ├── resolver.py           #   cascade + specificity (STUB)
│   ├── properties.py         #   known CSS property defaults
│   ├── sheet.py              #   StyleRule/StyleSheet dataclasses
│   └── units.py              #   unit conversion (px, %, em)
├── ir/                       # Intermediate Representation
│   ├── builder.py            #   walked AST → IR (implemented)
│   ├── node.py               #   IRNode/IRWindow/IRPage/IRViewport
│   ├── style.py              #   IRStyle dataclass
│   ├── event.py              #   IREvent dataclass
│   └── serializer.py         #   IR → JSON dict
├── layout/                   # Layout engine
│   ├── engine.py             #   vertical stacking (basic)
│   ├── flex.py               #   flexbox (STUB)
│   ├── box.py                #   box model (partial)
│   └── units.py              #   layout unit resolution (minimal)
├── dom/                      # DOM model
│   ├── node.py               #   DOMNode/TextNode/ElementNode
│   ├── tree.py               #   DOMTree with walk()
│   ├── attributes.py         #   morph-open/close/navigate parsing
│   └── query.py              #   DOM query (partial)
├── js/                       # JavaScript interpretation
│   ├── walker.py             #   generic AST visitor
│   ├── interpreter.py        #   JS semantics → intents (STUB)
│   ├── bridge.py             #   intents → IREvent
│   └── builtins.py           #   morph JS builtins map
├── codegen/                  # C++ code generation
│   ├── emitter.py            #   Jinja2 rendering engine
│   ├── node_emitter.py       #   IR node → C++ (STUB)
│   ├── event_emitter.py      #   event handlers → C++ (partial)
│   ├── feature_set.py        #   scan IR for required C++ headers
│   └── templates/            #   Jinja2 templates
├── dev/                      # Dev mode
│   ├── pipeline.py           #   full .mx → IR pipeline
│   ├── devrt.py              #   dev runtime launcher
│   ├── watcher.py            #   file watcher
│   └── server.py             #   Unix socket IPC
├── pkg/                      # Package system
│   ├── manager.py            #   CLI operations (partial)
│   ├── installer.py          #   tarball download + extract
│   ├── registry.py           #   package registry client
│   ├── manifest.py           #   morph.pkg.json parser
│   └── resolver.py           #   dependency resolution (STUB)
├── config/                   # Project config
│   ├── schema.py             #   MorphConfig dataclass
│   └── loader.py             #   morph.config.json reader
├── build/                    # (DOES NOT EXIST YET)
│   ├── compiler.py           #   g++ invocation
│   └── optimizer.py          #   binary optimization
└── utils/                    # Shared utilities
    ├── logger.py             #   colored terminal output
    ├── platform.py           #   OS detection
    ├── fs.py                 #   file helpers
    └── color.py              #   hex/rgb color parsing

tests/                        # Test suite
├── unit/                     #   unit tests
└── integration/              #   integration tests

runtime/                      # C++ runtime headers
├── core/                     #   MorphNode, Renderer, Event, WindowManager
├── viewport/                 #   ViewportDriver interface
├── dev/                      #   dev mode IPC (empty)
├── shaders/                  #   GL shaders (empty)
├── vendor/                   #   third-party (empty)
└── widgets/                  #   built-in widgets (empty)

templates/default/            # morph init scaffolding
my-app/                       # sample project
```

## Key Design Decisions

### .mx Files Instead of Separate HTML/CSS/JS

The original design used separate `.html`, `.css`, `.js` files. The current codebase uses single `.mx` files with JSX-like syntax parsed by tree-sitter. CSS and JS can be imported within the `.mx` file or loaded from external files.

### Tree-sitter Over Hand-written Parsers

Both the `.mx` parser and CSS parser use tree-sitter grammars instead of hand-written lexers/parsers. This gives us robust syntax error handling and support for evolving the language syntax without rewriting parsing logic. The `morph/lexer/` package referenced in old tests no longer exists.

### No Runtime Dependencies

The final binary has zero Python, zero Node.js, and zero browser engine. Python is build-time only. The C++ runtime uses GLFW + OpenGL directly.

### Two-pass Style Resolution

Style resolution happens in two phases:
1. **Build-time** — CSS files are parsed and Tailwind classes are resolved into CSS property dicts
2. **IR-time** — The resolved CSS is matched against elements (selector matching, cascade, specificity) to produce final `IRStyle` objects

The second phase is currently stubbed in `morph/style/resolver.py`.

## Common Tasks

### Adding a New CSS Property

1. Add default value to `morph/style/properties.py`
2. Add typed field to `morph/ir/style.py` (`IRStyle`)
3. Add conversion logic in `morph/ir/builder.py` (when implemented)
4. Add to Jinja2 template in `morph/codegen/templates/` if needed for C++ output

### Adding a New HTML Element

1. No parser changes needed — tree-sitter handles arbitrary tag names
2. Add rendering logic to the relevant C++ template or node emitter
3. If it's a special morph element (like `morph-window`), handle it in `IRBuilder.build()`

### Adding a Tailwind Class

Add the class to the `STATIC_MAP` dict in `morph/style/tailwind.py`. Follow the existing pattern:

```python
"bg-red-500": {"background-color": "#ef4444"},
```

## Debugging

- **`morph dev`** — runs the full pipeline; check terminal output for errors
- **`morph doctor`** — verify system dependencies
- **`python -m pytest tests/ -v -k <test_name>`** — run specific test
- **Pipeline logs** — errors are printed via `morph/utils/logger.py` with colored output
- **No IR output?** — Check `morph/ir/builder.py` for errors in the style resolution or node building logic
