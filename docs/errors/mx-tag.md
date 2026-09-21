# mx-tag — Unknown Element Tag

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

You used a lowercase tag that is not a supported Morph element and not a
component (components are capitalized):

```
error : mx-tag : Unknown tag <vidio>
  hint: Did you mean <video>?
  Learn more: https://morph.levizr.com/docs/errors/mx-tag
```

## Why Morph raises it

Lowercase tags compile to native `MorphNode` subclasses — each supported tag
maps to a real C++ renderer node. An unknown tag has no node class, no layout
rule, and no paint path, so there is nothing to generate. Morph rejects it with
a similarity-based suggestion instead of emitting an empty node that would
silently swallow your UI.

See [which elements Morph supports](../elements/overview.md) for the full list
(`div`, `span`, `button`, `input`, headings, and more).

## Example that triggers it

```tsx
// ❌ <vidio> is not a Morph element — mx-tag
export default function App() {
  return (
    <div>
      <vidio src="clip.mp4" />
    </div>
  );
}
```

## How to fix

```tsx
// ✅ Correct the tag name
export default function App() {
  return (
    <div>
      <video src="clip.mp4" />
    </div>
  );
}
```

If no supported tag matches what you need:

- Build the UI from supported primitives (`div` + styles + events).
- For fully custom rendering, create a
  [custom C++ node](../guides/custom-cpp-nodes.md).
- For drawing canvases, watch the planned `viewport` element
  ([roadmap](../future/viewport.md)) — do not invent a tag and hope.

Steps:

1. Read the `Did you mean <...> ?` hint — it is usually right.
2. Check the [supported elements list](../elements/overview.md).
3. If you meant a **component**, capitalize it (`<Hero />`) and define or
   import it — see [mx-component-unknown](mx-component-unknown.md).
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unknown tags can never render — the fix is a real
tag or a component, not a silenced check.

## See also

- [mx-tag-stub](mx-tag-stub.md) — tag exists but is not implemented yet
- [mx-component-unknown](mx-component-unknown.md) — the capitalized-tag
  equivalent
- [mx-prop](mx-prop.md) — the tag is fine, the prop is not
- [Which HTML Elements Morph Supports](../elements/overview.md)

