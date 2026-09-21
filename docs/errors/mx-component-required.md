# mx-component-required — Missing Required Prop

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

A component declares a **required** prop (not marked optional with `?`), but a
call site does not pass it:

```
error : mx-component-required : `<Hero>` is missing required prop `title`
  hint: `Hero` declares: title, onPress
  Learn more: https://morph.levizr.com/docs/errors/mx-component-required
```

## Why Morph raises it

Required props become mandatory C++ constructor parameters. There is no `null`
or `undefined` to fall back on at runtime — the generated code reads the prop
unconditionally. Allowing a missing required prop would generate code that
reads garbage or fails to compile deep inside the C++ backend. Failing at the
JSX call site keeps the error where you can act on it.

## Example that triggers it

```tsx
export function Hero(props: { title: string, onPress: () => void }) {
  return <button onClick={props.onPress}>{props.title}</button>;
}

export default function App() {
  // ❌ `title` is required but not passed — mx-component-required
  return <Hero onPress={() => console.log("hi")} />;
}
```

## How to fix

Pass the missing prop, or make it optional if callers legitimately omit it:

```tsx
// ✅ Fix 1: pass the required prop
export default function App() {
  return <Hero title="hi" onPress={() => console.log("hi")} />;
}
```

```tsx
// ✅ Fix 2: the prop is genuinely optional — mark it with `?`
export function Hero(props: { title: string, subtitle?: string }) {
  return (
    <div>
      {props.title} {props.subtitle}
    </div>
  );
}

export default function App() {
  return <Hero title="hi" />;
}
```

Steps:

1. Read the hint for the full declared prop list.
2. Pass the missing prop at every flagged call site (`morph check` lists each
   one), or add `?` to the declaration if the component handles its absence.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. If a component genuinely accepts many call shapes,
declare the varying props optional instead of silencing the check.

## See also

- [mx-component-prop](mx-component-prop.md) — passing a prop that is not
  declared
- [Reusable Components](../elements/components.md)

