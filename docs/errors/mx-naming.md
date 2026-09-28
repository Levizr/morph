# mx-naming — Invalid Module File or Directory Name

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

A source file or directory name violates the module naming gates, or two
modules normalize to the same C++ namespace:

```
error : mx-naming : directory `Ledger` in `src/Ledger/list.mx` should be `ledger` (lowercase `[a-z0-9_]` only). Rename it.
  Learn more: https://morph.levizr.com/docs/errors/mx-naming
```

```
error : mx-naming : `src/Shop/item.mx` and `src/shop/item.mx` both normalize to `shop::item` — rename one.
  Learn more: https://morph.levizr.com/docs/errors/mx-naming
```

## Why Morph raises it

Module paths become C++ namespaces mechanically: entry-relative segments,
lowercased, `[a-z0-9_]` only (`src/components/shop/ShopStore.mx` →
`components::shop::shopstore`). The rules exist because C++ namespaces cannot
contain uppercase, dashes, or dots, and two files normalizing to one namespace
would emit colliding symbols — a linker error pages away from its cause.
Enforcing the gates at graph time keeps namespaces human-computable: you can
predict the generated namespace from the path. Digit-leading segments are
`_`-prefixed; anything outside the entry tree is rejected.

## Example that triggers it

```
src/
  Ledger/            # ❌ should be `ledger`
    list.mx
  shop/
    Item.mx          # ❌ should be `item.mx`
```

Routes share the same source root, so shared components are fine:

```
src/
  add/
    route.mx         # ✅ `import NavBar from "../components/NavBar.mx"`
  components/
    NavBar.mx        # ✅ same `src/` tree — one namespace `components::navbar`
```

A file genuinely outside `src/` is still rejected:

```
error : mx-naming : `shared/NavBar.mx` is outside the source tree `src` — move it under `src/`.
  Learn more: https://morph.levizr.com/docs/errors/mx-naming
```

## How to fix

```
src/
  shop/            # ✅ lowercase, unique segments
    item.mx
  checkout/
    cart.mx
```

Rules:

- Directories and stems: lowercase `[a-z0-9_]` only (`src/ledger`, not
  `src/Ledger`; `item.mx`, not `Item.mx`).
- Every module in one build must live under the source root (`src/`).
  Routes (`src/add/route.mx`) and the entry (`src/App.mx`) share it, so
  `../components/…` imports are in-tree.
- After lowercasing, every module path must be unique project-wide.
- Leading digits are `_`-prefixed automatically — avoid them where possible.

Steps:

1. Rename the flagged file/directory to lowercase with underscores.
2. Update every import that referenced the old path.
3. If the error is a collision, rename one of the two modules.
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. The gates protect namespace generation — violating
names cannot produce valid C++.

## See also

- [mx-import-type](mx-import-type.md) — unsupported import kinds
- [How a Morph Project Is Structured](../getting-started/project-structure.md)

