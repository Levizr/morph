# `morph-parser`: The Crate That Reads

**Part of:** [Dev Docs](../architecture/overview.md)

`morph-parser` (`crates/morph-parser/src/`) is where text stops being text. It takes `.mx` source (which is TSX with opinions) and external CSS, and produces *facts*: an `MxSource` per module, a `ModuleGraph` for the project, and a pile of precisely-coded complaints when something is off. It never decides what anything *means* — that is the IR builder's job ([crate-ir](crate-ir.md)). It just writes down everything it saw, accurately, like a court stenographer with no opinions about the trial.

## The entry points (`lib.rs`)

Two public functions, no ceremony:

| Function | Does |
|---|---|
| `parse_mx_str(source, filename)` | Parses `.mx` text: Oxc parse → `MxWalker` visit → `MxSource` |
| `parse_mx_file(path)` | Reads the file, then calls `parse_mx_str` |
| `parse_css(source)` | Parses a stylesheet into `CssData` (delegates to `css_parser`) |

The Oxc invocation is worth noting: `SourceType::from_path("file.tsx")` — every `.mx` file is parsed as TSX, because that is what it is. A panicked parser or any diagnostic is a hard `bail!` naming the file. There is no "parse what you can and hope" mode; a file either parses or it doesn't, and you find out immediately.

## The vocabulary (`ast_types.rs`): `MxSource` and friends

`MxSource` is the crate's central product — one per module, carrying everything downstream needs:

| Field | Contents |
|---|---|
| `imports` | Every import, classified (`MxImportKind`, below) |
| `window_config` | The `windowConfig` object, if declared |
| `components` | `MxComponent`s: name, props, JSX tree, state, effects, handlers |
| `shared_bindings` / `event_bindings` | Exported top-level `morphShared` / `morphEvent` declarations, with `line`/`col` |
| `state_vars`, `effects`, `inner_functions`, `function_declarations`, `class_declarations` | Module- and component-level logic inventory |
| `exported_vars`, `named_exports`, `default_export`, `re_exports` | The full export surface, including `export *` (`star`) and `export * as ns` (`star_as`) |
| `global_vars`, `console_logs`, `extra_headers`, `cpp_imports` | Globals, log sites, and native-import edges |

Two types deserve a closer look:

- **`MxImportKind`** classifies every import at parse time: `CssLocal` / `CssUrl` (stylesheets), `CppLocal` (native imports with specifiers), and `Component` (everything else — with `default` local name plus `(local, imported)` specifier pairs, so `import { a as b }` records `("b", "a")`). Helpers answer the only questions downstream asks: `is_mx()`, `is_ts()`, `is_tsx()`, `is_morph_module()` (the `'morph'` runtime import), and `is_module_source()` (anything that joins the module graph).
- **`JsxNode`** is the JSX tree: `Element` (tag + props + children), `Fragment`, `Text` (whitespace-normalized at parse), `Expression` (raw source kept verbatim for later translation), `Conditional` (condition + then/else branches), and `List` (`array_expr`, `item_param`, `index_param`, `key_expr`, `item_template` — an `array.map(...)` call recognized structurally, not by name matching). Props are `JsxPropValue`: `String` / `Expr` / `Ref` / `Fn` / `Template` / `Style` (with `Static` vs `Expr` values per property) / `Bool`.

`ComponentProp.is_function()` is a small delight: it detects callable prop types by scanning for a top-level `=>` outside all brackets (tracking `<>`/`()`/`[]`/`{}` depth, skipping quoted strings) — so `onAdd: (id: number) => void` is a function prop and `label: string` is not, without a full type parser.

## The walker (`js_walker.rs`): `MxWalker` visits everything

`MxWalker` implements Oxc's `Visit` trait and extracts, method by method: `extract_import`, `extract_window_config_from_decl`, `extract_component` (plus `extract_arrow_component` / `try_extract_arrow_declarator` for `const X = (props) => …` components), `extract_binding_declarator` (the `morphShared` / `morphEvent` / `morphState` shapes), `component_props` (identifier params vs destructured params merged with the type annotation via `parse_props_annotation` / `split_top_level_props`), `state_vars_from_body`, `effects_from_body`, `inner_funcs_from_body`, `event_subs_from_body` (statement-level `name.on(handler)`), `consts_from_body`, `logs_from_body`, and the legacy `CSS.load()` detector (`check_css_load` / `walk_css_load_stmts`).

Text handling has opinions: `normalize_jsx_text` collapses JSX whitespace the way a browser would, `camel_to_kebab` bridges `className`-style props to CSS, and line/col attribution runs through byte-offset tables (`build_line_offsets`, `offset_to_line_col_fast`) so every diagnostic points at the right character.

The walker's test module is a contract written as examples: default vs named imports, aliased specifiers, typed/destructured/untyped props, arrow components, top-level shared/event bindings, `state_var_type_arg` — read the tests when the code confuses you; they are shorter than the implementation and just as precise.

## The CSS side (`css_parser.rs`): lightningcss does the parsing

No regexes, no hand-rolled tokenizer — `parse_css` hands the source to lightningcss (leaked to `&'static str` to satisfy the stylesheet lifetime) and converts the result into `CssData`: rules in source order (equal specificity resolves last-write-wins, like a browser) plus a `@keyframes` registry keyed by animation name.

Keyframe handling fans out grouped selectors (`0%, 100% { … }` becomes two entries) and merges duplicate offsets with later blocks winning per-property. Both behaviors are pinned by unit tests (`grouped_keyframe_selectors_fan_out`, `duplicate_keyframe_offsets_merge_later_wins`). If animation output looks wrong downstream, check here first: the IR and the renderer can only animate the keyframes they were given.

## The graph (`resolve.rs`): BFS with hard errors

`resolve_graph(entry, cwd)` parses the entry and every transitively imported `.mx` / `.ts` / `.tsx` module, breadth-first, entry first:

- Each module becomes a `ResolvedModule` (canonical path, directory, `MxSource`, and `module_imports`: raw-path → canonical-target pairs).
- **Missing imports are hard errors** — `Component import not found: ./Nope.mx (imported from …)`. There is no silent `undefined`, ever.
- Re-export targets join the graph too (equally fatal when missing).
- **Import cycles terminate** without re-parsing (already-seen paths are skipped); render-time cycles are the IR builder's problem, not the resolver's.
- Non-module imports (`'morph'`, `'./style.css'`) are never followed.

`module_ns_segments` / `module_ns_path` compute each module's C++ namespace from its position relative to the entry directory (`src/components/shop/ShopStore.mx` → `components::shop::shopstore`), enforcing the `mx-naming` gates: lowercase `[a-z0-9_]` only, digit-leading segments `_`-prefixed, nothing outside the entry tree. Name your files accordingly or hear about it at build time.

## The bouncer (`linter.rs`): `check`, `lint`, `lint_graph`

Three levels, cheapest first:

1. **`check(source, file_path)`** — parse plus semantic lints for one file. Oxc diagnostics become `parse-error` errors with line/col; a panicked parser becomes `parse-panic` ("check for unmatched braces or truncated file"). Then it walks the (possibly partial) program and runs `lint`.
2. **`lint(source, content, file_path)`** — the rulebook over one `MxSource`: `mx-export` (exactly one default export, unless it's a logic/store module), window-config presence, `lint_jsx` (tags against `SUPPORTED_TAGS` — 32 of them, plus `select`/`textarea` stubs; props against `GLOBAL_PROPS` + 12 `EVENT_PROPS` + per-tag tables, with `data-*`/`aria-*` always allowed and typo suggestions via `suggest_prop_for_tag`), `lint_component_usages` (unknown components, prop mismatches against declarations), `lint_state_event_scope` (the `mx-state-scope` / `mx-shared-scope` / `mx-event-scope` / `mx-api-removed` codes from the [Compiler Pipeline](../architecture/compiler-pipeline.md)), and `UNSUPPORTED_GLOBALS` (`document`, `window`, `localStorage`, …, `requestAnimationFrame` — the browser APIs that don't exist here, rejected with directions, not silence).
3. **`lint_graph(graph)`** — cross-file rules: props resolved through imports and re-exports, including TypeScript component sources.

The scope checker (`scope_stmt` through `scope_expr`, ~800 lines) walks bodies tracking where each API may legally appear. It is the reason "state outside a component" is a precise build error instead of a silent behavior change — see the FAQ in [State & Event Internals](../state/state-events-internals.md) for why that strictness exists.

## Worked example: `morph check` on a broken file

```tsx
export default function App() {
  const [count, setCount] = morphState(0)   // mx-state-scope is fine here (inside component)
  return <div><Frob onClik={1} /></div>     // unknown component + typo'd prop
}
```

`check` returns diagnostics with `code`, `message`, `suggestion`, `file_path`, `line`, `col` — machine-readable (`LintError` serializes to JSON) and human-actionable. `morph check` prints them; `morph build` refuses to proceed while errors remain. The linter is not advisory. It is a gate.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Support a new JSX shape or binding form | `js_walker.rs` extractor + `ast_types.rs` variant + walker test |
| Support a new tag or prop | `SUPPORTED_TAGS` / `TAG_PROPS` in `linter.rs` (+ typo-suggestion tables) |
| Allow a new browser-ish global | Remove from `UNSUPPORTED_GLOBALS` *and* implement it in the runtime — never just the first half |
| Add a scope rule | `linter.rs` scope walk + new `mx-*` code + user-docs lint table |
| Change module identity/namespaces | `resolve.rs` `module_ns_segments` — and the builder's `module_namespace`, which must agree |
| Change CSS/keyframe parsing | `css_parser.rs` + its two unit tests |

## Verify by

```bash
cargo test -p morph-parser
cargo test --workspace
target/debug/morph check   # inside any fixture project
```

Parser changes are upstream of everything — run the full workspace suite, not just the crate's tests. A walker tweak that passes `morph-parser` tests can still break IR-shape expectations downstream.
