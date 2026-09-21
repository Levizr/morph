# mx-transpile — JavaScript That Cannot Compile to C++

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

A JavaScript expression or statement in your component logic, handler, effect,
or helper cannot be translated to C++:

```
error : mx-transpile : Cannot translate to C++: `?.` optional chaining in `user?.name`
  hint: Use an explicit check: `user ? user.name : ""`
  Learn more: https://morph.levizr.com/docs/errors/mx-transpile
```

The flagged location may be a JSX expression, an event/effect body, a
component const, an inner function, a global, or a top-level function —
anything the translator must emit as native code.

## Why Morph raises it

Morph has no JavaScript engine in the binary: your logic is transpiled to C++
ahead of time (see [how JavaScript compiles](../javascript/overview.md) and
[intent-based codegen](../guides/intent-based-codegen.md)). Every construct
must have a native lowering. When none exists, generating *something* would
mean generating wrong code — so the compiler stops and tells you exactly which
expression, and usually the rewrite. Coverage grows over time (see
[translator coverage plans](../future/js-coverage.md)); the specific
unsupported-feature codes below are sub-cases of this general rule.

## Example that triggers it

```tsx
// ❌ Depends on the case: optional chaining, object spread, etc.
export default function App() {
  const user = { name: "Ada" };
  return <div>{user?.name}</div>;
}
```

## How to fix

Rewrite with supported constructs. Common rewrites:

| Instead of | Write |
|---|---|
| `user?.name` | `user ? user.name : ""` |
| `{...obj, x: 1}` | explicit object construction |
| `arr?.map(...)` | guard then map |
| unsupported method | equivalent supported method or helper |

```tsx
// ✅ Explicit check instead of `?.`
export default function App() {
  const user = { name: "Ada" };
  return <div>{user ? user.name : ""}</div>;
}
```

Steps:

1. Read the flagged expression and the hint's suggested rewrite.
2. If it matches a specific sub-code (`mx-js-op`, `mx-js-syntax`,
   `mx-js-method`, ...), open that page for the precise fix.
3. For complex logic, move it into a `.cpp` native function and import it
   (see [native C++](../guides/native-cpp.md)).
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Untranslatable code has no native representation.

## See also

- [mx-js-op](mx-js-op.md) / [mx-js-syntax](mx-js-syntax.md) /
  [mx-js-method](mx-js-method.md) / [mx-js-global](mx-js-global.md) /
  [mx-js-member](mx-js-member.md) — specific sub-cases
- [How Morph Compiles JavaScript](../javascript/overview.md)
- [Intent-Based Codegen](../guides/intent-based-codegen.md)

