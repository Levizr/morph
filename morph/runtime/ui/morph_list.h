#pragma once
#include "../core/node.h"
#include "../ui/rect.h"
#include "../types/js_array.h"
#include "../types/js_number.h"
#include "../types/js_string.h"
#include "../types/js_value.h"
#include <functional>
#include <memory>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace morph {

// ── Per-item binding ────────────────────────────────────────────
// Heap-allocated (shared_ptr) so effects created by the item factory can
// capture `JsValue& item` / `int& index` by reference: the binding outlives
// any single reconcile pass and is only freed when the item node is deleted.
struct ListItemBinding {
    JsValue item;   // current item value (object identity may change per run)
    int index = 0;  // current position in the array
};

// ── Stable key computation ──────────────────────────────────────
// "n:" / "s:" / "i:" prefixes prevent collisions between number, string and
// index fallback keys (e.g. the number 12 vs the string "12" vs index 12).
inline std::string list_key(const JsValue& v, int index) {
    if (v.is_number()) {
        const JsNumber& n = std::get<JsNumber>(v.inner);
        if (n.is_int())
            return "n:" + std::to_string(std::get<int64_t>(n.value));
        return "n:" + std::to_string(n.as_double());
    }
    if (v.is_string())
        return "s:" + std::get<JsString>(v.inner).value;
    return "i:" + std::to_string(index);
}

// ── Keyed list container ────────────────────────────────────────
// Renders `{arr.map(item => <JSX/>)}`. The codegen wires:
//   arrayFn     — evaluates the array source (reads signals → reactive)
//   itemFactory — builds one item node from a binding (runs per item)
//   keyFn       — optional; defaults to index keys (React semantics)
class ListContainer : public RectNode {
public:
    using ArrayFn = std::function<JsArray()>;
    using ItemFactory = std::function<MorphNode*(ListItemBinding&)>;
    using KeyFn = std::function<std::string(const JsValue&, int)>;

    ArrayFn arrayFn;
    ItemFactory itemFactory;
    KeyFn keyFn;

    ListContainer(float x, float y, float w, float h)
        : RectNode(x, y, w, h) {}

    // One reconcile pass: create/reuse/remove item nodes to match `arr`.
    // O(n) keyed diff via unordered_map. After reconcile, children order
    // equals array order and m_bindings[i] is the binding of children[i].
    void reconcile(const JsArray& arr) {
        const size_t n = arr.length();

        // ── Index existing children by key ──
        std::unordered_map<std::string, MorphNode*> byKey;
        std::unordered_map<MorphNode*, std::shared_ptr<ListItemBinding>> bindingOf;
        byKey.reserve(children.size());
        for (size_t i = 0; i < children.size(); ++i) {
            MorphNode* c = children[i];
            std::string k = keyOf(i);
            byKey[k] = c;
            if (i < m_bindings.size())
                bindingOf[c] = m_bindings[i];
        }

        // ── Pass 1: compute the new order (reuse or create) ──
        std::vector<MorphNode*> next;
        std::vector<std::shared_ptr<ListItemBinding>> nextBindings;
        next.reserve(n);
        nextBindings.reserve(n);
        std::unordered_set<std::string> usedKeys;
        usedKeys.reserve(n);

        for (size_t i = 0; i < n; ++i) {
            const JsValue item = arr[i];
            std::string k = key(item, (int)i);
            // Duplicate user keys can't both be reused — fall back to the
            // index key for the second occurrence (matches React's hazard).
            if (usedKeys.count(k))
                k = "i:" + std::to_string(i);
            usedKeys.insert(k);

            auto it = byKey.find(k);
            if (it != byKey.end()) {
                // Reuse: rebind the item value, refresh effects if changed
                MorphNode* node = it->second;
                byKey.erase(it);
                auto bit = bindingOf.find(node);
                std::shared_ptr<ListItemBinding> b =
                    (bit != bindingOf.end()) ? bit->second
                                             : std::make_shared<ListItemBinding>();
                const bool changed = !(b->item == item);
                b->item = item;
                b->index = (int)i;
                if (changed)
                    node->runAssociatedEffects();
                next.push_back(node);
                nextBindings.push_back(b);
            } else {
                auto b = std::make_shared<ListItemBinding>();
                b->item = item;
                b->index = (int)i;
                MorphNode* node = itemFactory ? itemFactory(*b) : nullptr;
                if (!node)
                    continue;  // factory failed — item not rendered
                next.push_back(node);
                nextBindings.push_back(b);
            }
        }

        // ── Pass 2: drop stale nodes ──
        for (auto& kv : byKey)
            delete kv.second;

        // ── Commit new order + bindings ──
        // Bypassing addChild() (for an O(n) swap) means we must wire the
        // parent link ourselves: effects inside the item call markDirty and
        // rely on parent propagation to reach the window and trigger repaint.
        for (auto* c : next) {
            c->parent = this;
            c->markDirty(LayoutDirty);
            c->markDirty(PaintDirty);
        }
        if (next != children) {
            children = std::move(next);
            m_bindings = std::move(nextBindings);
            invalidatePaintOrder();
            markDirty(SubtreeDirty);
        }
    }

private:
    std::vector<std::shared_ptr<ListItemBinding>> m_bindings;

    std::string key(const JsValue& item, int index) const {
        if (keyFn)
            return keyFn(item, index);
        return "i:" + std::to_string(index);
    }

    std::string keyOf(size_t i) const {
        if (i < m_bindings.size() && m_bindings[i]) {
            const ListItemBinding& b = *m_bindings[i];
            if (keyFn)
                return keyFn(b.item, b.index);
        }
        return "i:" + std::to_string(i);
    }
};

} // namespace morph