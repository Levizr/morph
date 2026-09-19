# Kill All Runtime Strings

**Status:** `development` · **Priority:** high · **Shipped parts:** batch 1 (`NodeType` + `CSS::Display` + `CSS::Position`), batch 2 (remaining style enums + text path + converter deletion)

> Every `std::string` compare, lookup, and allocation on a hot path — layout, paint, text, events, property access — replaced with integers, enums, or hashes. Strings survive only where they belong: build-time parsing, debug printing, and genuinely freeform values (font names, custom text). Companion to [State, Events & Native C++ Interop](state-events-native-interop.md) and [Universal Module Bindings](universal-module-bindings.md), which killed string *identity*; this page kills string *comparison*.

---

## 1. Why

Measured patterns in the current runtime (all verified in source):

| Site | Cost today |
|---|---|
| `style.display == "flex"` et al (~15 distinct literals in `layout.cpp` + more in `node.cpp`, `paint_order.cpp`) | `std::string ==` per node per layout/paint pass |
| `node->type == "__text__"` etc. | Every child, every container, every pass — plus a heap `std::string` (`"div"` default) per node |
| `fontWeight`/`textAlign` strings in `TextOp` + 4-compare conversion in `flattenExtra` | Per text node per flatten |
| `flatten.cpp` `*ToEnum` converters | Re-conversion per flatten (half-done enums) |
| `JsObject`: `std::map<string,JsValue>` tree | O(log n) string compares per `e["value"]`, per row field, per `has`+`[]` double lookup |
| `JsObject::operator[]` (non-const) inserts on miss | Reads silently grow the map |
| `JsValue[string]` on arrays: `std::stoll(key)` per access, **throws** on `"length"` | Parse cost + crash hazard |

---

## 2. Target State

```cpp
// Before (generated + runtime today)
node->style.display = "flex";
if (c->style.display == "inline" || c->type == "__text__") { ... }
__st_user.set(e["value"]);            // tree lookup per keystroke
arr["length"];                        // throws std::invalid_argument

// After
node->style.display = CSS::Display::Flex;
if (c->style.display == CSS::Display::Inline || c->type == NodeType::Text) { ... }
__st_user.set(e["value"]);            // hash lookup, same result
arr["length"];                        // JsNumber(length) — no throw, ever
```

```cpp
namespace CSS {
enum class Display : uint8_t { Block, Flex, None, Inline, InlineBlock, Hidden };
enum class Position : uint8_t { Static, Absolute, Relative, Fixed, Sticky };
enum class TextAlign : uint8_t { Left, Center, Right, Justify };
// … one scoped enum per keyword property (see §4)
}

enum class NodeType : uint8_t { Div, Button, Input, Img, Text, Expr, List, ... };
```

`CSS::` prefix (not bare `Display::`): bare `Display` collides with X11/GLFW types already in the TU.

---

## 3. Rules

1. **Convert once at the boundary.** CSS parser (build time) → enums; codegen emits enum literals; dev deserializer maps on load. Hot paths never see a string.
2. **IR keeps strings.** `IRStyle`/`IRNode.type` stay `String` — zero churn in parser/IR/builder matching code. The mapping lives in codegen emission + runtime parse functions.
3. **Clean break, no shims.** `app.cpp` regenerates; hand-written user C++ assigning `style.display = "flex"` fails at compile time pointing at the line. Never silent.
4. **One parse function per property** (`parseDisplay()` etc., constexpr tables) replicating today's exact match semantics (case handling, defaults, invalid → same fallback). Round-trip unit tests per property.
5. **Inspector/devtools keep strings.** `enum→string` printing on the debug path only.

---

## 4. Property Inventory (string-valued today)

Layout-critical first, then the rest. Numeric properties (`opacity`, `border-radius`, `float[4]` colors) are untouched.

| Property | Values | Batch |
|---|---|---|
| `node->type` | div/button/input/img/text/expr/list/… | 1 |
| `display` | block/flex/none/inline/inline-block/hidden | 1 |
| `position` | static/absolute/relative/fixed/sticky | 1 |
| `textAlign` | left/center/right/justify | 2 |
| `fontWeight` | normal/bold + 100–900 (exact set: `bold\|700\|800\|900` → bold) | 2 |
| `flexDirection` | row/column/row-reverse/column-reverse | 2 |
| `justifyContent` | flex-start/center/flex-end/space-between/space-around | 2 |
| `alignItems` | flex-start/center/flex-end/stretch | 2 |
| `flexWrap` | nowrap/wrap/wrap-reverse | 2 |
| `cursor` | default/pointer/text | 2 |
| `borderStyle` | none/solid/… (only `solid` renders) | 2 |
| `overflow` | visible/hidden/scroll/auto | 2 |
| `flexBasis` + remaining keyword fields in feature headers | per-field audit at implementation | 2 |

---

## 5. Per-Layer Changes (kiske liye kya badlega)

### `morph-parser` (`css_parser`, `js_walker`, `linter`, `resolve`)
- **Almost nothing.** CSS stays strings in IR (rule 2). `mx-naming`-style gates not needed — invalid values already fall through to defaults; enum parse replicates that.
- Only addition: none planned. (If a keyword set later needs build-time validation, it lands in the linter — not this pass.)

### `morph-ir` (builder, `IRStyle`, `IRNode`, serializer)
- **Nothing.** String fields and matching code untouched by design.

### `morph-codegen` (`node_emitter` — build TU)
- Emit enum literals instead of string assignments (`->display = CSS::Display::Flex`).
- One shared keyword→enum mapping table used by all emission sites (no per-site string switches).
- `for-in` over objects: emit **sorted iteration** (preserves today's exact `std::map` order; see §7).

### `morpher` (TS→C++ translator)
- `for k in obj` over objects: route through the same sorted-iteration helper (today emits raw `.keys()`).
- No change to comparison/operator lowering.

### `morph-codegen` (`logic_emitter` — dev TU)
- Dev TU consumes the same runtime headers; no emitter logic change. The dev deserializer (`ir_deserializer.h`, runtime side) maps strings→enums on load.

### `runtime/cpp` (the bulk of the work)
- `style/features/*.h`: keyword `std::string` fields → `CSS::` enums (defaults = today's defaults).
- `core/node/layout.cpp`, `node.cpp`, `paint_order.cpp`, `flatten.cpp`: every `== "literal"` → enum compare; **delete** `*ToEnum` converters once fields are enums.
- `core/node.h`: `type: std::string` → `NodeType`; update the 6 `type ==` sites + codegen `->type =` emission (already emits `__text__`/`__expr__` literals — switch to `NodeType::Text` etc.).
- `ui/text.h`, `core/render_frame.h`, `core/window.cpp`: `TextOp`/`FlatTextOp` `fontWeight`/`textAlign` → enums end-to-end; delete the 4-compare conversion in `flattenExtra`.
- `types/js_object.h`, `types/js_value.h`: `std::map` → `unordered_map`; read paths use find-once `get()` (no insert-on-read); `JsValue[string]` on arrays: digit-check fast path, `"length"` → length, else `undefined` (never throw).
- `net/net.h`: sort at the two `keys()` iteration sites (order preservation, outside codegen's reach).
- `dev/inspector.h`: `enum→string` pretty-printing for the debug surface.

### Docs & registries
- This page + `docs.registry.json` + `docs/future/index.md` (this commit).
- `native-cpp.md`: note on enum assignment for user C++ touching styles (after Batch 1).

---

## 6. Behavior Contract (parity proof obligations)

1. **Sorted `for-in` preserved.** `std::map` yields sorted order today; generated loops sort explicitly. Point lookups unaffected.
2. **No-insert-on-read.** `has(k)` after a mere read flips `true`→`false` — matches JS `in` semantics (today's `true` is the bug).
3. **Array string keys.** `"length"` → length, all-digits → index, garbage → `undefined`. Today garbage throws (crash) — strictly a fix.
4. **Enum parse tables** replicate exact current semantics (case, defaults, invalid fallbacks); round-trip unit tests per property.
5. **No silent changes.** Anything hand-written against string APIs breaks at compile time with a pointer to the line.

---

## 7. Batches & Verification

| Batch | Content | Proof |
|---|---|---|
| 1 | `NodeType` + `CSS::Display` + `CSS::Position` | ✅ Shipped — fixture rebuilds, screenshots, self-tests |
| 2 | Remaining style enums + text-path unification + converter deletion | ✅ Shipped — same + converter absence (`grep ToEnum` empty outside dead `widgets/`) |
| 3 | Map swap + sorted `for-in` + `stoll`-free indexing + `undefined`/`length` semantics + regression tests | Workspace tests incl. garbage-key/cycle tests, self-tests |

Each batch: `cargo test --workspace`, fixture rebuilds, `--morph-self-test`, `run-selftests.sh`, screenshots for visual batches. Deferred to measurement: hot-key interning (`"value"`, `"id"`) — the map swap removes tree compares; intern only if profiling justifies it.

---

## 8. Decision Log

| Date | Decision | Rationale |
|---|---|---|
| 2026-09-17 | `CSS::` prefix over bare enums | Bare `Display` collides with X11/GLFW types in the TU |
| 2026-09-17 | IR keeps strings; convert at codegen/runtime boundary | Zero churn in parser/IR/builder matching; mapping in one place per layer |
| 2026-09-17 | Clean break, no string shims | `app.cpp` regenerates; hand-written misuse fails loud at compile time |
| 2026-09-17 | Plain `unordered_map` (not insertion-ordered) + sorted codegen iteration | Max point-lookup speed; today's exact sorted-iteration behavior preserved without order-dependence analysis |
| 2026-09-17 | `undefined` (not throw) on garbage array keys; `"length"` → length | JS-correct; fixes a crash hazard |
| 2026-09-17 | Hot-key interning deferred to measurement | Map swap already removes the dominant cost |
| 2026-09-19 | Batch 1: `display:hidden` parses to `Block` | Replicates today's effective behavior (never matched; fell through to block paths) |
| 2026-09-19 | Batch 1: unknown tags parse to `NodeType::Custom` | Nothing compares against them; inspector prints `custom` |
| 2026-09-19 | Batch 1: ancestor-hover tag match via `toString` | Rule pipeline has no producer (dev IR only); reachable behavior unchanged |
| 2026-09-19 | Batch 1: generated refs use leading `::app::` | Bare `app::X` inside namespace blocks resolves through `app::app` (pre-existing trap, broke self-referencing modules in both flows) |
| 2026-09-19 | Batch 1: `fn.display`/`fn.position` keep uint8 via string-free mapping | Flatten discriminants are write-only; values bit-identical; removal in batch 2 |
| 2026-09-19 | Excluded from scope: transform fn names, net response types, signal `-0` | Parse-time/cold paths (verified in source) |
| 2026-09-19 | Batch 2: `alignItems` default `Stretch`, garbage → `FlexStart` | `stretch` is explicitly branched; garbage took the else-branches — exact parity needs the split |
| 2026-09-19 | Batch 2: `flexDirection` garbage → `Column` | Only `row` sets `isRow`; garbage took the column path in both flows |
| 2026-09-19 | Batch 2: cursor walk-stop quirk preserved | Garbage stopped the ancestor walk (resolving nullptr); browsers would continue — kept exact, noted here |
| 2026-09-19 | Batch 2: `FlatRenderNode` keyword fields converted to enums, all `*ToEnum` deleted | Discriminants were write-only except `overflow` (one reader updated); proof: no `ToEnum` outside dead `widgets/` |
| 2026-09-19 | Batch 2: atlas key is `{size, weight}` struct, no heap string per lookup | `atlasKey()` string concat ran per text measurement |
| 2026-09-19 | Batch 2: `resolvePct` inspects the unit suffix in place | Same acceptance, no per-tick heap string; full unit pre-parsing deferred to measurement |
| 2026-09-19 | Batch 2: `flexBasis` stays a string, dead `widgets/` untouched | Zero value-compares on `flexBasis`; zero references to `widgets/` repo-wide (deletion is separate cleanup) |
| 2026-09-19 | Batch 2: list `item`/`index` params thread through IR to factories | `item.name` → `__it["name"]`, effect captures `&__it`; custom param names included (pre-existing gap, fixed to unblock verification) |
| 2026-09-19 | Batch 2: `flexBasis` stays a string | Zero value-compares (write-only); nothing to kill |
| 2026-09-19 | Batch 2: dead `runtime/cpp/widgets/` untouched | Zero references repo-wide; deletion is a separate cleanup call |

---

*Related: [State, Events & Native C++ Interop](state-events-native-interop.md) · [Universal Module Bindings](universal-module-bindings.md) · [C++ / JSX Interop Guide](../guides/native-cpp.md)*
