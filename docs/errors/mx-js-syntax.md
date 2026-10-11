# mx-js-syntax — Unhandleable Syntax Construct

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** reported by `morph check`. Only probe-verified breakage is
> rejected — `?.`, destructuring, `for...in/of`, array spread and getters
> lower correctly and stay allowed.

## What this error means

Your code uses a syntax construct the translator cannot handle:
generators / `yield`, the `with` statement, labeled `break`/`continue`,
`import.meta`, dynamic `import()`, object spread, `namespace` blocks, or
decorators:

```
error : mx-js-syntax : generator function cannot be translated to C++
  hint: Use an explicit state machine or an array you index through
  Learn more: https://morph.levizr.com/docs/errors/mx-js-syntax
```

Stays allowed because it lowers correctly: `?.` / `??`, destructuring,
`for...in` / `for...of`, array spread, `super`, getters/setters, enums.

## Why Morph raises it

Each rejected construct needs hidden runtime machinery Morph omits on
purpose: generators need resumable frames; `with` needs dynamic scope;
labeled jumps have no C++ equivalent; `import.meta` / dynamic `import()`
need a module loader; object spread needs reflective copies; namespaces
and decorators vanish in translation. Supporting any of them approximately
would change program meaning — so each is rejected with its explicit
rewrite. See
[translator coverage](../future/javascript/coverage.md) for what is planned.

## Examples that trigger it

```tsx
// ❌ Optional chaining — mx-js-syntax
export default function App() {
  const user = { name: "Ada" };
  return <div>{user?.name}</div>;
}
```

```tsx
// ❌ Object spread — mx-js-syntax
export default function App() {
  const base = { a: 1 };
  const ext = { ...base, b: 2 };
  return <div>Hello</div>;
}
```

```tsx
// ❌ Labeled break — mx-js-syntax
export default function App() {
  let total = 0;
  top: {
    total = 1;
    break top;
  }
  return <div>{total}</div>;
}
```

## How to fix

| Instead of | Write |
|---|---|
| `{...o, k: v}` | explicit object literal |
| generator / `yield` | state machine with `morphState` |
| `with (o) { ... }` | explicit `o.` qualification |
| labeled `break`/`continue` | flag variable or early return |
| `import.meta` / `import()` | static imports and constants |
| `namespace N { ... }` | exported functions and constants |
| decorators | inline the decorator's effect |

```tsx
// ✅ State machine instead of a generator
export default function App() {
  const [step, setStep] = morphState(0);
  const next = step + 1;
  return <div onClick={() => setStep(next)}>{next}</div>;
}
```

Steps:

1. Match the flagged construct to the table rewrite.
2. For object-heavy logic, consider moving it to `.cpp` (see
   [native C++](../guides/native-cpp.md)).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. These constructs have no native representation.

## See also

- [mx-js-op](mx-js-op.md) — unsupported operators
- [mx-transpile](mx-transpile.md) — the general rule
- [How Morph Compiles JavaScript](../javascript/overview.md)

