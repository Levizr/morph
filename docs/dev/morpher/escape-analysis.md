# Escape Analysis: The Compiler Reads Your Variable's Diary

**Part of:** [Dev Docs](../architecture/overview.md)

JavaScript has a garbage collector. Morph does not — there is no GC in the generated C++, by design, forever. So how does a `let` in TypeScript become memory-safe C++ with no collector sweeping up after it? Answer: the compiler reads your variable's diary before deciding where it gets to live. That diary-reading is *escape analysis*, and this page is its biography.

For the cutting guide (which file to edit for which change), read [Morpher Internals](morpher-internals.md) first. For the *why* behind the whole intent model, read [Intent-Based Codegen](../../guides/intent-based-codegen.md). Here we go deep on the analysis itself.

## The three questions

For every variable, morpher asks three questions. The answers jointly determine the emitted C++:

1. **Declared intent** — what did the programmer *say*? The TypeScript annotation (`: number`, `: string`, `: int`). In `Strict` type mode this is law; in `Infer` mode it is a strong hint.
2. **Observed intent** — what does the code *do* with it? Collected as `UsageKind`: arithmetic, string ops, comparisons, array methods, passed to functions, and so on.
3. **Lifetime intent** — how long does it *live*, and who else holds it? Collected as `EscapeKind`: does it stay on the stack, get returned, go global, get captured by a closure, cross an async boundary, or get aliased beyond counting?

**Widening** reconciles the three: if you declared `number` but demonstrably need JS string coercion somewhere, the type widens (`ToJsNumber` / `ToJsString` / `ToJsValue` / `ToJsArray`) to the smallest type that survives all observed uses. Except — and this is the fun part — a *trusted native-number annotation* beats `ToJsNumber`/`ToJsValue`, because the programmer pinky-swore and the compiler believes them. Proven-small integer literals refine down to `int32_t`; big ones stay `int64_t`; computed reassignment forfeits the dainty `int32_t` and goes back to wide.

## The machinery (`crates/morpher/src/codegen/`)

| File | Role in the diary-reading |
|---|---|
| `analyzer.rs` | `EscapeAnalyzer`: collects escape / widen / usage / comparison facts, then runs both fixpoints to closure |
| `cpp.rs` | `CppTranslator`: the emitter (~3.5k lines — navigate by function, never by reading top to bottom) |
| `context.rs` | `Ctx`: the notebook — `var_types`, `async_fns`, `escape_hints`, `shared_ptr_vars`, include-needs, indentation |
| `type_resolver.rs` | Annotation → C++ maps, plus `Promise` → `Result` denormalization |

The iron rule, repeated here because it saves careers: **analysis in `analyzer.rs`, emission in `cpp.rs`, never mixed.** If your change needs a new fact about the code, collect it in the analyzer and read it in the emitter. Smuggling analysis into the emitter produces bugs that only manifest three fixtures away.

## The decision flow, with examples

`emit_typed_variable_declarator` (`codegen/cpp.rs`) decides everything about a `let`/`const`, in order. Watch it work:

**Step 1 — Base type.**

```ts
const name = "Ada"          // literal → std::string, stays native, no JsString
let count = 0               // proven-small literal → int32_t
let big = 9999999999        // too big for int32 → int64_t
const xs = [1, 2, 3]        // homogeneous array → vector<int64_t>-ish, recursively
const p = new Promise<int>()// → morph::Result<int> (denormalized by type_resolver)
const w = new Widget()      // class instantiation → shared_ptr<Widget>
const mystery = frob(x)     // no clue → JsValue, or auto for deducible expressions
```

The philosophy: stay native whenever provably safe. `std::string` instead of `JsString` when the value never needs JS semantics; `int32_t` instead of `int64_t` when the literal fits. Every native choice is binary size and compile time saved (a `JsValue` baseline drags in `<format>` and roughly half a megabyte of overhead — the variant tax).

**Step 2 — Widening.** Apply the analyzer's `widens` map. Mixed-type `==` later emits `morph::js_cmp::loose_eq`; same-type stays a direct comparison (the full coercion matrix lives in [JS Semantics in C++](js-semantics.md)).

**Step 3 — Sync strip.** `Result<T>` → `T` for plain sync call initializers (`strip_result_for_sync_call`). Async-ness is a property of the call, not the variable — don't make the variable pay for it.

**Step 4 — Storage.** `EscapeKind` picks the pointer story:

| Escape | Storage | Example |
|---|---|---|
| `None` | Stack value, no pointer at all | `int32_t count = 0;` — the dream |
| `Return` / `Global` | `unique_ptr` + `make_unique` for class/vector; stack copy for scalars | Returned class → `unique_ptr` (wrapped in `shared_ptr` on second use) |
| `ClosureCapture` / `MultipleRefs` / `AsyncBoundary` | `shared_ptr` + `make_shared` (scalar use sites dereference) | Closure-captured counter → `std::shared_ptr<int64_t>`, captured by value (`[&, count]`) |
| Last use | `std::move` on the final call arg / assignment source | Move semantics fall out of use-counting — the compiler noticed you were done |

Returned scalars stay on the stack — no `unique_ptr` for an `int` going home to its caller. That restraint is the whole aesthetic: smart pointers only where semantically required, never as decoration.

**Step 5 — Linkage/decoration.** `static` at file scope, `const`-folding for `Js*` scalars, and the verdict recorded in `ctx.var_types` so later references stay consistent.

## The fixpoints, demystified

"Runs both fixpoints" sounds arcane; it means the analyzer loops over its facts until nothing changes. Why? Because facts beget facts: discovering that `x` escapes into a closure means the closure's captures must be re-examined, which may reveal that `y` escapes too, which… you get it. The loop runs until a full pass learns nothing new. Termination is guaranteed because each pass only *adds* facts from a finite set — the diary has a last page.

## Testing an analysis change

Analysis changes are high-blast-radius: one widening-rule tweak can alter every declaration in every fixture. The two suites that guard you:

```bash
cargo test --workspace
python -m pytest tests/translate/ -v
```

- `test_intent.py` asserts emitted C++ *shape* (with string literals stripped so assertions can't match inside them): native `std::string` stays native, closures capture `shared_ptr` by value, returned scalars stay stack-allocated, `int32_t` refinement and forfeiture, trusted-annotation victories, last-use moves, exotic-destructuring fallback.
- `test_rust_translate.py` is behavioral: each fixture `.ts` is translated, compiled with `g++-14 -std=c++23`, run, and diffed against Node.js (`npx tsx`), with float tolerance under 0.001.

If your change alters any existing fixture's `.cpp`, inspect that diff line by line — the diff *is* the review. Fixture `.cpp` files are gitignored; only the `.ts` sources and expected outputs are reviewed.
