# mx-js-op — Unemittable Operator

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** reported by `morph check`. `in`, `**`/`**=`, and `&&=`/`||=`/`??=` lower to runtime helpers and stay allowed; the operators below error.

## What this error means

Your code uses an operator the C++ translator cannot emit: `typeof`,
`instanceof`, `delete`, `>>>`, `>>>=`, or `void`:

```
error : mx-js-op : Operator `typeof` cannot be emitted to C++
  hint: Track types explicitly with state or TypeScript annotations
  Learn more: https://morph.levizr.com/docs/errors/mx-js-op
```

## Why Morph raises it

Each operator needs a native lowering with **identical** semantics, and these
do not have one:

- `typeof` and `instanceof` need runtime type reflection Morph's
  zero-overhead types deliberately omit (see
  [intent-based codegen](../guides/intent-based-codegen.md)). `in`
  lowers to a key check and is allowed.
- `delete` assumes garbage-collected object graphs; Morph uses explicit
  ownership.
- `>>>` and `>>>=` operate on unsigned 32-bit values. Morph's `>>` is
  signed, so the same expression would produce different results for
  negative inputs.

Refusing is safer than approximating: numeric and identity bugs from
approximated operators would be nearly impossible to trace.

## Example that triggers it

```tsx
// ❌ typeof has no native lowering — mx-js-op
export default function App() {
  const kind = typeof 42;
  return <div>{kind}</div>;
}
```

## How to fix

| Instead of | Write |
|---|---|
| `typeof x` | track the type explicitly (state, union flag, TS annotation) |
| `x instanceof Y` | a discriminant field / explicit kind flag |
| `delete obj.k` | set to a default / restructure state |
| `>>>` / `>>>=` | explicit unsigned arithmetic in `.cpp` |
| `void expr` | drop the `void` — expression statements need no coercion |

```tsx
// ✅ Discriminant instead of instanceof
type Shape = { kind: string; radius: number; side: number };

export default function App({ shape }: { shape: Shape }) {
  const area = shape.kind === "circle" ? 3.14159 * shape.radius ** 2 : shape.side ** 2;
  return <div>{area}</div>;
}
```

Steps:

1. Find the flagged operator and pick the rewrite from the table.
2. For unsigned bit tricks, put the operation in a `.cpp` helper.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. These operators have no correct native emission.

## See also

- [mx-js-syntax](mx-js-syntax.md) — unsupported syntax constructs
- [mx-transpile](mx-transpile.md) — the general rule
- [Intent-Based Codegen](../guides/intent-based-codegen.md)
