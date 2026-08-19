# Full CSS Cascade

**Status:** development → future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

A complete runtime CSS cascade: merging every matched rule — across CSS files, Tailwind utilities, inline styles, and defaults — by **specificity and origin** into one final computed style per element. The selector engine exists; the cascade engine (`morph/style/resolver.py`) is a stub.

## Why it matters

Today styles resolve mostly at IR-build time (inline > Tailwind > CSS rules > UA defaults per property). A real cascade gives:

- Predictable CSS semantics — `!important`, specificity battles, source order — matching what web developers expect
- Runtime re-resolution when CSS changes (hot reload / dynamic styles)
- `morph check` parity — warn on rules that would never win

## Current state

| Piece | State |
|---|---|
| Selector engine (`style/selector.py`) — descendant/child/adjacent/sibling combinators, compounds, pseudo-classes, specificity | ✅ Shipped |
| Runtime `:hover` / `:active` / ancestor-hover rules | ✅ Shipped |
| Builder cascade (inline > Tailwind > CSS rules > defaults) | ✅ Shipped |
| Cascade resolver (`style/resolver.py`) | ❌ Stub — `# TODO: selector matching + cascade + specificity` |

## Planned behavior

```python
# morph/style/resolver.py (future)
resolve(element, rules) -> computed_style
```

1. Collect all matching rules for the element (selector engine)
2. Sort by origin + `!important` + specificity + source order
3. Merge declarations per property into a computed style
4. Apply inheritance for inherited properties (`color`, `font-size`, `font-weight`, `text-align`)
5. Output the same `IRStyle` / runtime style structs the renderer already consumes

The C++ side then deserializes or emits the final computed style — **pixel-identical dev/build rendering is a hard requirement** (already enforced by the existing dev deserializer).

## Open questions

- **Where it runs** — Python at IR-build time (today) vs. C++ at runtime (for dynamic styles). The runtime path is what makes the resolver meaningful
- **`!important`** — does it belong in `.mx` CSS? (Trivial to support once the cascade sorts by origin)
- **CSS variables** (`--custom-props`) — natural extension once the cascade exists, currently unsupported

## Build steps (when picked up)

1. Implement rule collection + specificity sort in `style/resolver.py`
2. Merge declarations with inheritance
3. Wire `!important` and source-order semantics
4. Extend `morph check` cascade diagnostics
5. Fuzz-test against browser cascade behavior on the existing examples