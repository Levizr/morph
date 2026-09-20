# The Compiler Pipeline: Your `.mx` File's Hero Journey

**Part of:** [Dev Docs](overview.md)

Every `.mx` file you write goes on the same hero journey: it gets *read*, *understood*, *redrawn as a plan*, and *reborn as C++*. No interpreters, no runtime babysitters — by the time your app runs, your JavaScript is machine code and your CSS is structs. This page follows one tiny component through all three acts so you can see exactly where each transformation lives and where to cut when something looks wrong.

The short version lives in [GUI Pipeline](gui-pipeline.md). This is the long version, with names, files, and opinions.

## The cast

Three crates do the compiling. One drives, two specialize:

| Crate | Source | Job in one sentence |
|---|---|---|
| `morph-parser` | `crates/morph-parser/src/` | Reads text, produces facts (`MxSource`, `ModuleGraph`) |
| `morph-ir` | `crates/morph-ir/src/` | Turns facts into a plan (`IRWindow` / `IRNode` trees) |
| `morph-codegen` | `crates/morph-codegen/src/` | Turns the plan into C++ (or Rust, experimentally) |

Data flows one way: `morph-parser` → `morph-ir` → `morph-codegen`. Nothing downstream ever edits an upstream structure. If you are debugging, that means the bug is always in the *earliest* stage whose output looks wrong — check the parse output before blaming codegen.

## Act 1 — Parse: from text to facts (`morph-parser`)

The parser's job is gloriously unglamorous: turn characters into data structures and complain early about nonsense. It uses two battle-tested libraries instead of hand-rolled parsing:

- **Oxc** for all TypeScript/JSX parsing (`oxc_parser`, `oxc_ast`, friends in the workspace `Cargo.toml`). Your `.mx` file is JSX with opinions, so a real JS parser is the only sane choice.
- **lightningcss** for CSS parsing. Your stylesheets are parsed as CSS, full stop — not regexed, not guessed at.

### The files, in the order your code meets them

| File | Owns |
|---|---|
| `lib.rs` | Public surface: `parse_mx_str`, `parse_css` |
| `ast_types.rs` | The shared vocabulary: what a component, prop, or state hook looks like once parsed |
| `js_walker.rs` | The JSX walker: walks the Oxc AST and fills an `MxSource` per module (imports, components, `morphState` / `morphShared` / `morphEvent` / `morphEffect` bindings, event handlers) |
| `resolve.rs` | The module graph: follows `.mx` / `.ts` / `.tsx` imports into a `ModuleGraph`. A missing import target is a hard error — there is no silent `undefined` here |
| `css_parser.rs` | CSS side: rules, keyframes, and friends, via lightningcss |
| `linter.rs` | Scope rules with `mx-*` codes: `mx-state-scope` (state lives inside components, nowhere else), `mx-shared-scope` / `mx-event-scope` (shared stores and events live at exported module scope), `mx-api-removed` (the old string-key APIs, may they rest in peace) |

### Worked example: what parse produces

Say you write this (the traditional offering to the demo gods):

```tsx
import { morphState } from 'morph'
import './style.css'

export default function App() {
  const [count, setCount] = morphState(0)
  return (
    <body>
      <button className="btn" onClick={() => setCount(count + 1)}>
        Clicked {count} times
      </button>
    </body>
  )
}
```

After Act 1, there is no more "code" in any meaningful sense. There is an `MxSource` that says, roughly: *module imports `morphState` from `morph` and `./style.css`; defines component `App`; `App` owns state binding `count` initialized to `0` with setter `setCount`; renders a `button` with class `btn`, an `onClick` handler, and reactive text reading `count`.* The JSX walker does not care what your button *means*. It just writes down everything it saw, accurately, like a court stenographer with no opinions about the trial.

**Concept to pocket:** parsing collects *facts*, never *decisions*. "There is a class called `btn`" is a fact. "That means 12px of padding" is a decision, and it belongs to Act 2.

## Act 2 — IR: from facts to a plan (`morph-ir`)

The IR (intermediate representation) is where Morph stops transcribing and starts *thinking*. `IRBuilder` (`builder.rs`) takes every `MxSource` plus the parsed CSS and builds `IRWindow` / `IRNode` trees (`node.rs`): a full description of the window and every node in it — structure, resolved styles, reactive wiring, animations — everything codegen needs, and nothing it doesn't.

### The files

| File | Owns |
|---|---|
| `builder.rs` | `IRBuilder` itself, plus `seed_module_bindings` (the state/event identity machinery — see [State & Event Internals](../state/state-events-internals.md)) and the `rewrite_event_emits` / `translate_event_sub` event rewrites |
| `node.rs` | `IRNode`, `WindowIR` (including `shared_vars`, the complete list of signals codegen must declare), plus serialization support in `serializer.rs` |
| `style.rs` | Style resolution: matching rules to nodes, folding in inline styles |
| `css_registry.rs` | The CSS registry: every rule and keyframe, indexed for lookup |
| `tailwind.rs` | The Tailwind resolver: 500+ utility classes lowered to style values at compile time — no Node.js, no Tailwind install, just a lookup table with ambition |
| `transforms.rs` | CSS transform lowering (`translate` / `rotate` / `scale` / `skew`, 2D and 3D, `deg` / `rad` / `grad` / `turn`) |
| `serializer.rs` | IR serialization — the same bytes feed both the ahead-of-time compiler and the dev-mode hot-reload pipe (more in [Dev Mode](dev-mode.md)) |

### Decisions made here (and only here)

1. **Which styles apply to which node.** Class matching, specificity-ish ordering, inline-style precedence — all resolved now, at compile time. The runtime applies computed styles; it does not negotiate them.
2. **Tailwind → concrete values.** `className="flex px-4"` becomes actual layout and padding values in the IR. If Tailwind output looks wrong, `tailwind.rs` is your crime scene, not the renderer.
3. **Transforms → matrices-ish.** `transforms.rs` lowers the CSS transform list into the numeric form the runtime consumes.
4. **State/event identity.** `seed_module_bindings` turns `count` into `app::<namespace>::count().get()` and event names into channel ids. The full machinery is documented in [State & Event Internals](../state/state-events-internals.md) — this page just notes that the IR is where your friendly names die and qualified C++ expressions are born.
5. **The shared-vars contract.** `WindowIR.shared_vars` is the complete list of signals to declare. Codegen trusts it blindly: if a binding is missing there, no C++ is emitted for it, and nobody apologizes.

Continuing the example: after Act 2, the button node knows its computed style (whatever `.btn` means in `style.css`), its text is marked reactive on `count`, and `count`/`setCount` have become qualified signal expressions. The plan is complete. All that remains is writing it down in a language `g++` understands.

## Act 3 — Codegen: from plan to C++ (`morph-codegen`)

Codegen is a faithful scribe with strong opinions about types. It emits `app.cpp` plus the state headers, pulling in *only the runtime features your app actually uses* (that is the `feature_set` below — your hello-world does not pay for the 3D transform pipeline).

### The files

| File | Owns |
|---|---|
| `node_emitter.rs` | Node emission: IR nodes → C++ node construction, reactive text, conditions, list expressions |
| `logic_emitter.rs` | Logic emission: state signals, accessors, event subscriptions, `_morph_state.h` |
| `feature_set.rs` | Feature detection: scans the IR and selects the `MORPH_FEATURE_*` defines so the compiler strips everything unused |
| `cpp/mod.rs` | The C++ backend: `state_map` construction, `shared_expr` namespacing, wiring `shared_decls` into the `app_main.cpp.tera` template |
| `rust/mod.rs` | Experimental TS→Rust emission. Not production. Admire from a distance |
| `lib.rs` | Public surface, backend selection |

The `app_main.cpp.tera` template is the skeleton every app hangs on; the emitters fill in the organs. Generated per-project headers form the entire native contract (documented from the user side in the native-interop guides): `_morph_state.h` (window state, `__st_` signals, per-module wrappers) and `morph_api.h` (shared-store accessors, event channel wrappers, `MID_*` constants).

Continuing the example one last time: the `onClick` arrow function is translated to C++ by the same intent-based machinery that powers direct file morphing (see [Morpher Internals](../morpher/morpher-internals.md) and [Escape Analysis](../morpher/escape-analysis.md)), the reactive text becomes a subscription on the `count` signal, and the whole thing lands in `app.cpp` ready for `g++`.

## How the three acts share work with `morpher`

Sharp-eyed readers will notice two translators in this repo: the GUI pipeline described here, and the `morpher` crate that morphs standalone `.ts` files (`morph foo.ts --to cpp`). They are not rivals; they are the same brain in two hats. The GUI logic emitter leans on morpher's analysis-and-emit machinery for translating component logic, while morpher on its own translates whole files. The cutting guide for that machinery is [Morpher Internals](../morpher/morpher-internals.md); the diary-reading part is [Escape Analysis](../morpher/escape-analysis.md); the costume department is [JS Semantics in C++](../morpher/js-semantics.md).

## Debugging the pipeline: follow the smell upstream

| Symptom | First suspect | Verify with |
|---|---|---|
| "Unknown import" / missing component | `resolve.rs` module graph | Check the import path spelling and extension; missing targets are hard errors by design |
| State/event misuse compiles but misbehaves | `linter.rs` scope rules | `morph check` should flag it — if it doesn't, the lint has a hole |
| Wrong styles, right structure | `style.rs` / `tailwind.rs` / `css_parser.rs` | Inspect the IR: if the IR style is wrong, the renderer is innocent |
| Reactive text never updates | `seed_module_bindings` / `state_map` | Check the qualified accessor string in the IR-shape tests |
| Event emits nothing / fires twice | `rewrite_event_emits` / `translate_event_sub` | Re-read the "lambda only" contract in [State & Event Internals](../state/state-events-internals.md) |
| Unused-feature bloat or missing define | `feature_set.rs` | Check which `MORPH_FEATURE_*` the build passed |

The golden rule: **reproduce at the earliest stage**. If the `MxSource` is wrong, no amount of staring at generated C++ will save you. The pipeline is a one-way street, and bugs drive the same direction as data.

## Verify by

```bash
cargo test -p morph-parser -p morph-ir -p morph-codegen   # unit + IR-shape regression tests
cargo test --workspace                                     # the full orchestra
```

The IR-shape tests assert exact structures (accessor strings, `shared_vars` entries). If your change moves any expected string, read that diff like a confession — it tells you exactly what relinked.
