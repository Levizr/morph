# mx-windowconfig-key — Invalid windowConfig Key

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

The `windowConfig` export contains a key Morph does not recognize:

```
error : mx-windowconfig-key : Unknown windowConfig key `titel`
  hint: Did you mean `title`? Valid keys: title, width, height, minWidth, ...
  Learn more: https://morph.levizr.com/docs/errors/mx-windowconfig-key
```

## Why Morph raises it

`windowConfig` is compiled directly into native window-creation parameters. An
unknown key has nowhere to go — silently ignoring it would mean your setting
(`titel`, `widht`) does nothing while looking correct. Morph fails with a
did-you-mean suggestion instead. (Wrong *value types* are a separate check:
[mx-windowconfig-type](mx-windowconfig-type.md).)

Valid keys include `title`, `width`, `height`, `minWidth`, `minHeight`,
`maxWidth`, `maxHeight`, `visible`, and `modal` — see
[configuration](../getting-started/configuration.md) for the full list.

## Example that triggers it

```tsx
// ❌ `titel` is not a windowConfig key — mx-windowconfig-key
export const windowConfig = { titel: "App", width: 800, height: 600 };

export default function App() {
  return <div>Hello</div>;
}
```

## How to fix

```tsx
// ✅ Correct key spelling
export const windowConfig = { title: "App", width: 800, height: 600 };

export default function App() {
  return <div>Hello</div>;
}
```

Steps:

1. Read the hint for the valid key list and the closest match.
2. Fix the spelling, or remove the key if no equivalent exists.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unknown keys are always typos or unsupported
options — neither is fixed by silencing the diagnostic.

## See also

- [mx-windowconfig-type](mx-windowconfig-type.md) — right key, wrong value type
- [How to Configure a Morph Project](../getting-started/configuration.md)

