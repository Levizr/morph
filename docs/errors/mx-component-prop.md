# mx-component-prop — Unknown Prop on a Component

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

You passed a prop to a component that does not declare it:

```
error : mx-component-prop : Unknown prop `titel` on `<Hero>`
  hint: `Hero` declares: title, onPress
  Learn more: https://morph.levizr.com/docs/errors/mx-component-prop
```

A special case is a component that declares **no props at all** but receives
one: `` `<Hero>` takes no props but got `title` ``.

## Why Morph raises it

Component props are bound at compile time to typed C++ parameters. An unknown
prop has no parameter to bind to — silently dropping it would hide typos
(`titel` vs `title`) and make refactors (renaming a prop) silently break every
call site. Morph rejects the call so renames fail loudly and exactly where the
stale prop is passed.

`key` is never treated as a prop: it is consumed by list reconciliation and is
excluded from this check.

## Example that triggers it

```tsx
export function Hero(props: { title: string }) {
  return <div>{props.title}</div>;
}

export default function App() {
  // ❌ `titel` is not declared on Hero — mx-component-prop
  return <Hero titel="hi" />;
}
```

```tsx
export function Badge() {
  return <div>new</div>;
}

export default function App() {
  // ❌ Badge takes no props — mx-component-prop
  return <Badge label="new" />;
}
```

## How to fix

Either fix the call site or declare the prop — decide which side is wrong:

```tsx
// ✅ Fix 1: correct the typo at the call site
export default function App() {
  return <Hero title="hi" />;
}
```

```tsx
// ✅ Fix 2: declare the prop the call site needs
export function Hero(props: { title: string, subtitle?: string }) {
  return (
    <div>
      {props.title} {props.subtitle}
    </div>
  );
}
```

Steps:

1. Read the hint — it lists exactly what the component declares.
2. If the prop name is misspelled at the call site, fix the spelling.
3. If the call site is right, add the prop to the component's props type (mark
   it optional with `?` when not every caller passes it).
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. It is the main safety net for component refactors —
disabling it turns every prop rename into silent dead UI.

## See also

- [mx-component-required](mx-component-required.md) — the reverse: declared
  prop not passed
- [mx-component-unknown](mx-component-unknown.md) — the component itself is
  missing
- [mx-prop](mx-prop.md) — the native-element equivalent
- [Reusable Components](../elements/components.md)

