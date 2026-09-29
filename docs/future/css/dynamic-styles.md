# Dynamic Styles & Classes (Runtime Path)

**Status:** `future` · **Priority:** medium · **Shipped parts:** template-literal `className`, state-driven float/color/inline styles ([user docs](../../guides/dynamic-styles.md), proven by `examples/dynamic`)

> State-driven `:style` values on **keyword (enum) fields** and fully-dynamic `:class` bindings. Everything else dynamic already resolves — this page tracks only the runtime path that remains. No performance emergency: see §3.

---

## 1. What Works Today (Shipped)

- **Template-literal classes** (`` className={`header ${theme == "light" ? "bg-white" : "bg-gray-900"}} ``) — each branch becomes a reactive style effect ([user docs](../../guides/dynamic-styles.md), `examples/dynamic`).
- **State-driven float/color/inline styles** (`style={{ width: bodyWidth, backgroundColor: accent }}`) — the `float`/`color` emitter arms handle any expression.
- **Ternary classes** (`:class="cond ? 'a' : 'b'"`) resolve each branch to static styles at build time (`analyze_dynamic_class` in `morph-ir`). No runtime selector matching exists — in either flow.
- **Static style values** (stylesheet rules, hover/active diffs, conditional-class branches) emit enum literals after Kill-Strings batches 1–2.
- **Runtime `parseX` helpers exist** (`CSS::parseDisplay`, `parseOverflow`, … in `runtime/cpp/style/css_enums.h`) — the emitter side below is the only missing piece.
- **Fully-dynamic `:class="someVar"`** never applied styles at all — `setClassName` only *tracks* the string for the inspector. Nothing re-matches selectors at runtime.

## 2. The Gap

**State-driven `:style` values on converted keyword fields.** `:style="{display: myDisplay}"` translates the *expression* (`__st_myDisplay.get()`), but both emitters only know the string path — `node_emitter.rs` (`"string"` arm) and `logic_emitter.rs::emit_style_effects` (`"string"` arm) emit `style.display = morph::str(expr)`, and `display` is now a `CSS::Display` enum, so compilation fails. Pre-Kill-Strings this "worked" via runtime string compares (the exact cost that project kills).

## 3. Performance Analysis (Why This Can Wait)

No string *performance* problem exists here — only the compile error above:

| Cost per effect run | Scale | Notes |
|---|---|---|
| `markDirty` + layout + paint of the subtree | Thousands of ops | Inherent to reactivity; untouched by strings work |
| `morph::str(expr)` conversions | One small alloc | Pre-existing, inherent (text/attrs *are* strings) |
| Planned `parseX` wrapper (below) | ~5 integer compares, no alloc | `<<1%` of the effect — noise next to the relayout |

Dynamic classes cost nothing (compile-time resolved; the fully-dynamic form is a no-op). Remaining runtime strings live elsewhere: text content, attribute values, event payload *lookups* (Kill-Strings batch 3), dev-mode JSON parsing (cold).

## 4. Plan (When Scheduled)

1. **Shared helper** in `morph-codegen/src/node_emitter.rs`:
   `keyword_value_expr(field, cpp) -> Option<String>` — static string literal → enum literal at compile time (garbage → behavioral default, same rule as static emission); anything else → `CSS::parseX(morph::str(expr))` per effect run. `None` for still-string fields (`flexBasis`), which keep today's path.
2. **Both reactive-style `"string"` arms** use it: `node_emitter.rs` (build TU) and `logic_emitter.rs::emit_style_effects` (dev TU). `css_val_to_cpp` untouched — its callers only pass static class values.
3. **Tests**: static literal → enum; dynamic expr → parse-wrap; unconverted field → `None`; garbage literal → default literal.
4. **Probe** (throwaway): state-driven `display`/`overflow` via `:style` under both `morph dev` and `morph build`.
5. **Docs**: `native-cpp.md` dynamic-styles paragraph; full verify as usual.

## 5. Explicitly Out of Scope Here

- **Fully-dynamic `:class="var"` applying styles.** Would need a runtime stylesheet + selector matcher — a feature, not a strings fix, and it never worked before. Separate proposal if demanded.
- **List-template `reactive_class` via the builder.** Item-param mapping exists for codegen-translated paths (`translate_js`); builder-verbatim paths (`reactive_class`, conditional conditions) do not map the item variable yet. Same class of gap, same fix shape (thread item scope through the builder frame).

---

## 6. Decision Log

| Date | Decision | Rationale |
|---|---|---|
| 2026-09-19 | Defer to a scheduled pass; no perf emergency | Only cost is the compile error on state-driven keyword styles; runtime cost after fix is negligible |
| 2026-09-19 | `parseX` wrapper over `string_view`, not pre-parsed units | Effect-run frequency (state changes) doesn't justify schema churn; matches convert-at-boundary rule loosely (boundary = effect run, not frame) |

---

*Related: [Kill All Runtime Strings](../shipped/kill-runtime-strings.md) · [C++ / JSX Interop Guide](../../guides/native-cpp.md)*
