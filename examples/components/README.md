# Components Demo

A small shop built from reusable `.mx` components. Demonstrates everything in
the component system: imports, typed props (including function props),
per-instance state, the `morphShared` global store, and `morphEmit`/`morphOn`
messaging.

## What it shows

- **Reusable components** — `Hero` (default import), `ProductCard` and
  `CartBar` (named imports) from `src/components/`
- **Typed props** — `ProductCard` declares `{ name: string, price: number,
  onAdd: (name: string) => void }`; call sites pass literals plus an inline
  arrow wrapping a state setter
- **Per-instance state** — three `ProductCard` instances keep independent
  `qty` state from the same code
- **`morphShared`** — `shop.cart` total shared between every card and the
  cart bar with no providers
- **`morphEmit` / `morphOn`** — `Clear cart` emits `shop:clear`, resetting
  all card quantities
- **Standard CSS imports** — `import './style.css'` (no `CSS.load`)

## Run

```bash
cd examples/components
morph dev          # live window with hot reload
# or
morph run          # build + run optimized binary
```
