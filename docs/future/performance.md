# Performance — Hidden Classes & Compositor-Safe Properties

**Status:** future · **Priority:** low

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Performance optimizations with concrete designs already written down. None are urgent — the current design is correct and sufficient until profiling shows they matter.

## Hybrid object shapes (hidden classes)

**Problem:** every `app["config"]["theme"]["darkMode"]` does 3 string hash lookups through `std::map` plus 3 `shared_ptr` atomic refcount bumps. For deeply nested chains this dominates runtime.

**Planned solution:** keep `shared_ptr<map>` for correctness (JS reference semantics) but add an optional **shape fast path** for objects whose shape is known at translate time:

```cpp
struct ShapeDescriptor {
    std::vector<std::string> names;        // slot index → name
    std::map<std::string, size_t> lookup;  // name → slot index
};

struct JsObject {
    struct Storage {
        std::map<std::string, JsValue> dynamic;       // slow path (always available)
        std::shared_ptr<std::vector<JsValue>> slots;  // fast path (nullptr if unshaped)
        std::shared_ptr<const ShapeDescriptor> shape; // null if unshaped
    };
    std::shared_ptr<Storage> _inner;

    JsValue& fast_at(size_t idx);         // codegen emits this when index is known
    JsValue& operator[](const std::string& key);  // shape lookup, else dynamic
};
```

**Access path costs:**

| Pattern | Path | Cost |
|---|---|---|
| `obj.foo` (known key) | `fast_at(index)` | 1 vector load, no hashing, no atomic |
| `obj[key]` (computed, key in shape) | `shape->lookup.find` → `fast_at` | 1 map lookup |
| `obj[key]` (computed, key NOT in shape) | lookup miss → `dynamic[key]` | 2 map lookups |
| `obj.newProp = x` | `dynamic["newProp"] = x` | 1 map insert |

**Implementation steps:** ① runtime `Storage` refactor (backward-compatible `operator[]`) → ② shape registry at translate time (dedupe identical shapes) → ③ `_object_literal` emits `from_shape(...)` → ④ `_member_expression` emits `fast_at(i)` when shape is tracked → ⑤ propagate shape info through `_var_types`.

**When to implement:** object count > 10,000 or property accesses > 100,000 per frame, and profiling shows `std::map`/`shared_ptr` as the bottleneck.

## Compositor-safe property expansion

The compositor currently interpolates a fixed set of properties at vsync: **X/Y offset, opacity, background color, text color, border-radius**. The compositing-thread design doc explicitly marks this set as *extensible* — Chrome's compositor additionally animates filters and scroll.

**Candidates:**

- **Filter effects** — blur/grayscale/brightness as compositor-interpolated properties (needs the SDF shader stack to grow filter support)
- **Scroll** — compositor-driven scroll offset (scroll-shift in the [Forge](forge-renderer.md) work is the first step)
- **Transforms** — transform matrices are already applied in the vertex shader, so transform interpolation is close to free on the compositor

**Why low priority:** compositor interpolation is a vsync-fidelity win, not a correctness fix — the main thread already animates everything correctly today.

## Other known hot paths

- **Text measurement** — `estimate_text_width` runs per layout pass; caching measured widths per (string, font, size) is a low-risk win for text-heavy UIs
- **Layout** — the dirty-flag system already skips clean nodes; the remaining cost is deep-tree re-layout after single-node changes (subtree dirty propagation could be smarter)

## Current state

| Piece | State |
|---|---|
| Map-backed `JsObject` (correct, sufficient) | ✅ Shipped |
| Shape fast path | ❌ Deferred (design complete, see above) |
| Compositor-safe property set | ✅ Shipped (fixed set, marked extensible) |
| Text-measure caching | ❌ Not started |