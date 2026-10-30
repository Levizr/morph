# State & Event Internals

**Part of:** [Dev Docs](../architecture/overview.md)

How `morphState` / `morphShared` / `morphEvent` travel from `.mx` source to C++: identity model, per-module namespacing, the IR contract, and the emit/subscribe rewrite pipeline. For the *user-facing* API, see the [API reference](../../api/overview.md); this page is the contributor's map of the machinery underneath.

## Identity: no string keys anywhere

Identity is always **canonical module path + binding name**, computed in `crates/morph-ir/src/builder.rs`:

| Binding | Identity function | Example |
|---|---|---|
| Shared store | `binding_identity` → `{module}::{getter}` | `src/cart.ts::count` |
| Event channel | `event_channel_id` → `evt:{module}::{name}` | `evt:src/events.ts::cartUpdated` |

Consequences (all enforced, none conventional):

- Importing the same binding from the same module is the same signal/channel — no registration, no provider, just the module graph.
- The same getter name in a different file is a different store. There is no global key registry to collide in.
- Importing one local name from two modules is an **ambiguity error** at build time (`seed_module_bindings` tracks a `seeded` set of exports + imports and bails on the second claim).

## Per-module C++ namespaces

Each module gets a namespace segment so generated symbols can never collide across files:

```cpp
namespace app {
namespace store_cart_a1b2c3d4 {   // store_{stem}_{hash8}
  static Signal<int> __shared_count;      // backing signal
  Signal<int>& shared_count() { ... }     // accessor
}
}
```

- `module_namespace` (`builder.rs`): stem = sanitized file stem (non-alphanumeric → `_`, extension stripped), hash = low 32 bits of **FNV-1a** over `module.display()`. The stem is readability; the hash is uniqueness (same-named files in different dirs differ).
- `shared_signal_accessor`: `shared_{sanitized_getter}` — only getter names *within one file* must differ.
- Both codegen backends emit the same shape: `logic_emitter.rs` writes statics/accessors/wrappers inside the namespace block; `cpp/mod.rs` references them via `shared_expr(ns, accessor)` → `app::{ns}::{accessor}()`.

## Pipeline stage 1 — parse (`morph-parser`)

`js_walker.rs` collects, per module (`MxSource`):

- `shared_bindings`: exported top-level `const [getter, setter] = morphShared<T>(init)` — callee check + destructuring shape + mandatory init.
- `event_bindings`: exported top-level `const name = morphEvent<T>()`.
- `imports` with `MxImportKind::Component { path, specifiers }` — the same import form carries components, shared getters/setters, and event names; what a specifier *resolves to* is decided at build time, not parse time.

`resolve.rs` follows `.mx` / `.ts` / `.tsx` imports into the `ModuleGraph` (`module_imports`). A missing import target is a hard error — there is no silent `undefined`.

Scope linting (`linter.rs`): `mx-state-scope` (state inside components only), `mx-shared-scope` / `mx-event-scope` (shared/event at module scope, exported), `mx-api-removed` (old string-key forms).

## Pipeline stage 2 — IR build (`morph-ir`)

`seed_module_bindings` runs once per module per instance frame and fills two ambient maps:

- `frame.vars`: `getter` → `app::{ns}::{accessor}().get()`, `setter` → `app::{ns}::{accessor}().set`. This is the *only* place user-visible names become C++ expressions — every downstream consumer (`translate_js`, `translate_logic`, reactive text) just does string substitution through `state_map`/`frame.vars`.
- `frame.events`: `eventName` → channel id (`evt:…`).

It also appends to `ctx.shared_entries` via `register_binding`, deduped by the `key` field. Each entry is `{key, ns, accessor, type, init, getter, setter}` and becomes `WindowIR.shared_vars` (see `node.rs`, serialized in `serializer.rs`). Codegen trusts `shared_vars` as the complete list of signals to declare — if a binding is missing there, no C++ is emitted for it.

Type resolution (`shared_binding_type`): explicit `T` first, else infer from init (`string`→`std::string`, `boolean`→`bool`, `number`→ context-dependent numeric, else `auto`).

## Pipeline stage 3 — event rewrite (still `morph-ir`)

Two textual rewrites, both order-sensitive:

**Emit** — `rewrite_event_emits` scans logic/JSX sources for top-level `{name}.emit(args)` where `name` is visible in `frame.events`, and rewrites to a `morphEmit("<escaped_id>", args)` placeholder. Member expressions are never matched; string literals and comments are skipped; nested emits inside arguments are handled. The emitter later lowers the placeholder to `morph::channel("<id>").emit(args)`.

**Subscribe** — `translate_event_sub` converts `name.on(handler)` into *only* a lambda: `[](const JsValue& __ch_N) { … }`. A plain handler param is renamed to a fresh `__ch_N` (avoids capture collisions), `p.field` reads become `p["field"]` (JsValue has no members), and the body is translated through `translate_logic` with the frame's maps. A destructured object/array param is kept as a pattern, lowered through a synthetic `const <pattern> = __ch_N;` declaration (seeded with `__ch_N: JsValue`), and its bound names are removed from the frame's ambient rename/type maps so payload fields shadow same-named state, props, and imports. Handler locals are translated at function scope so they never become module-level `static` variables. The caller pairs `{channel: id, body: lambda}` into `ctx.channels`; **codegen adds the single `.on(...)` wrap**. If you ever see a doubled `.on(`, the bug is that this function started emitting the wrap itself — the contract is "lambda only".

Constraints enforced here (not in the linter): unknown event name, empty/missing handler, non-inline handler, and more than one handler param — all hard errors naming the component and module. Defaults (including a whole-pattern `({ a } = {})`, whose default is unreachable for a real event), rest, and computed keys are lowered inside the pattern.

### Destructured callable parameters (normal functions and function props)

The same one-argument contract applies outside events. A destructured
parameter on a function declaration/expression/arrow/method, or on a
function-prop callback adapted by `synthesize_adapter`, keeps the declared
arity: the C++ signature receives one synthetic `__argN` (typed from the
annotation), and the function body opens with `const <pattern> = __argN;`.
Morpher's declarator destructuring then expands that into field/index
bindings (`auto a = __arg0["a"];`). Call sites therefore keep passing one
object/array, matching JavaScript.

Bound names are removed from the frame's ambient rename/type maps before the
body is translated, so `({ x }) => …` shadows same-named state/props/imports
instead of reading `__st_x`. The pattern itself is prepared with the ambient
prop/state/import rewrites, so defaults and computed keys can read outer
values.

Morpher lowers static patterns through its fast field/index path. Patterns
containing `AssignmentPattern`, rest elements, or computed keys go through a
recursive emitter instead: the source is staged once, defaults become a lazy
`is_undefined()` conditional (so they fire exactly for `undefined`, never for
`null` or another falsy value), object/array rest call the runtime
`js_object_rest` / `js_array_rest` helpers, and computed key expressions are
evaluated once into a `std::string` used for both the read and the rest
exclusion list. A whole-pattern default (`({ x } = {})`) is supported by
normal functions and stripped for JSX event payloads and function-prop
adapters, whose single payload argument is supplied by the call site.

## Pipeline stage 4 — C++ emission (`morph-codegen`)

- `logic_emitter.rs`: emits each `shared_vars` entry as static + accessor inside its `app::<ns>` block, plus `generate_state_header` / `emit_init` wiring. Subscriptions become `morph::channel("<id>").on(<lambda>)`.
- `cpp/mod.rs`: builds `state_map` (getter→`{read}.get()`, setter→`{read}.set`, event names→`{name}()`) used by `node_emitter.rs` for reactive text, conditions, and list expressions; serializes `shared_decls` (with `ns`) into `app_main.cpp.tera`, which wraps declarations in the conditional namespace block.
- Build and dev TUs emit the same function-local statics so hot-reload and AOT agree on identity.

## Component-body flow (`body_stmts`) → one setup effect

Statements between a component's declarations that no declarative
collector owns — `if`/`else`, `for`, `while`, `switch`, plain calls — are
split out at parse time (`MxWalker::split_body_flow`) into
`MxComponent::body_stmts`. Any `let` those statements (or a function in
the body) reassign moves out of `consts` into `mutable_locals`, and its
declaration becomes a seeding assignment at its original position — so
the block re-derives the local exactly like re-running the body would.

`morph-ir` lowers the whole block into **one setup effect** per component
(`IRWindow::body_effects`), emitted *before* node construction so JSX
reads seeded values on the first frame:

- Reads of a mutable local come from `.peek()`, and the signal is
  declared with `body_local = true`; `Signal::set` prunes the *running*
  effect's subscription before notifying. Together they stop the effect
  from re-enqueueing itself when it seeds its own locals — reads through
  helper functions included.
- Reads of state come through `.get()`, so the effect subscribes to them
  and a branch re-runs when they change (same contract as `morphEffect`).
- Writes use the setter lowering: `x = v` → `x.set(v)`, `x += v` →
  `x.set(x.get() + v)`, `x++` → `x.set(x.get() + 1)`. The head comes from
  `CppTranslator::signal_set_head`, which strips the `.get()` / `.peek()`
  suffix off any mapped signal read; unmapped names keep their plain C++
  form.

Emitters: the entry path emits each lambda with `morph::create_effect` in
`app.cpp` / `morph_logic_rewire`; route mounts emit
`morph::create_effect_scoped` inside the mount body (which is why a body
effect alone is enough to pull in the mount scope). `morphEffect` has no
lift point inside this block — a nested call is rejected by
`mx-effect-nested` (see that error page).

## Common contributions, by file

| "I want to…" | Touch |
|---|---|
| Change the namespace scheme | `module_namespace` in `builder.rs` **and** `shared_expr` in `cpp/mod.rs` **and** the template wrap — all three must agree or linkage breaks |
| Support a new init-literal → C++ type | `shared_binding_type` in `builder.rs`; check both emitters render it |
| Change emit/on lowering | `rewrite_event_emits` / `translate_event_sub` in `builder.rs`; keep the "lambda only" contract |
| Add a new scope rule | `linter.rs` scope checks + a `mx-*` code + docs in `elements/components.md` lint table |
| Debug "unknown event" / "ambiguous import" | Print `frame.events` / the `seeded` set in `seed_module_bindings` — the answer is always which module claimed the name first |

## Testing a state/event change

```bash
cargo test -p morph-ir -p morph-parser -p morph-codegen   # unit + IR-shape regression tests
cargo test --workspace                                     # full suite
```

The IR-shape tests are the review: `shared_store_is_namespaced_per_file`-style tests assert the exact `shared_vars` entries and qualified accessor strings. If your change alters any expected `app::…` string, inspect the diff — a changed namespace or accessor means every existing store/event relinks. Also run `morph check` + `morph build` on `tests/runtime/app/component-test` and `examples/components`, which exercise cross-file shared state and event subscribe/emit end to end.

## FAQ (why it is built this way)

### Why two names (`morphState` vs `morphShared`) instead of one API usable anywhere?

Because one name would make intent unrecoverable at lint time. If `morphState` were legal at module scope, the linter could not distinguish "shared on purpose" from "local accidentally left outside its component" — the most common state bug would become legal code. Two syntactic markers split the world cleanly: `morphState` is only valid inside components (`mx-state-scope`), `morphShared` only at exported module scope (`mx-shared-scope`), so each misuse is a precise build error instead of a silent behavior change. The user-facing rationale lives in the [`morphShared` FAQ](../../api/faq/morphShared.md).

### Why module-path identity instead of string keys?

A string-key registry is global mutable state with no owner: two libraries pick `'cart'`, they collide, and neither the compiler nor the IDE can help. Module path + binding name reuses the identity the module system already maintains — imports are the registry, renames are explicit, and "go to definition" lands on the store declaration. The one real cost: renaming a store file creates a new identity, so persisted state does not follow the rename.

### Why `store_{stem}_{hash}` instead of just a hash?

A bare hash is unique but unreadable — every build error and every line of generated C++ would point at `store_9f3ac201` with no hint of which file it came from. The stem is a human affordance; the FNV-1a low-32 over the full path is the actual uniqueness (same-named files in different directories differ). Keep both halves when changing the scheme.

### Why does `translate_event_sub` return only a lambda?

Because there was a double-`.on()` bug: the builder emitted the wrap *and* codegen wrapped again, so every handler fired twice. The contract now is that the IR layer produces `{channel, body: lambda}` pairs and exactly one site — the emitter — adds `.on(...)`. If subscriptions ever double-fire again, check that this boundary held before looking anywhere else.

### Why a `morphEmit` placeholder instead of lowering `channel().emit()` directly in the rewrite?

The rewrite runs on source text shared by two lowering contexts (logic vs. JSX/JS expressions), and the channel id needs escaping at the C++ boundary. The placeholder lets one scanner handle matching (identifiers only, strings/comments skipped, nested emits in args) while `rewrite_emit_for_cpp` / `rewrite_emit_for_js` each handle their own quoting. Direct lowering would duplicate the scanner or leak escaping bugs into one context.

### Why dedupe `shared_entries` by `key`?

One store imported by ten modules is seeded ten times (once per importing frame) but must be *declared* once in C++. `register_binding` dedupes on the identity key so emission is per-store, not per-import. Removing the dedupe would emit duplicate statics and fail linkage.

### Why does rewire call `clear_channels()` first?

`morph_logic_rewire` re-runs on every hot reload and re-registers every subscription. Without the clear, each reload would stack another copy of every handler and one emit would fire N times after N reloads. Startup registration has nothing to clear, so the call is guarded on `channel_subs` being non-empty.
