# Reusable Components

Split your UI into `.mx` files and reuse them like React components. Each
instance gets its own local state — two `<Counter />` on one page never share
a `count`.

## Importing a component

```tsx
import Hero from './components/Hero.mx';
import { Card } from './components/ui.mx';

export default function App() {
  return (
    <body>
      <Hero title="hi" />
      <Card t="x" />
    </body>
  );
}
```

Rules:

- Default import (`import Hero from ...`) binds the file's default export.
- Named imports (`import { Card } from ...`) bind exported components by name.
- Only the entry file's default export renders as a window. Every other
  component renders solely where it is instantiated.
- Missing imports are hard errors at build time, and `morph check` flags
  unknown components (`mx-component-unknown`) before you compile.

## Props

Declare props with a typed parameter:

```tsx
export function Hero(props: { title: string }) {
  return <div>{props.title}</div>;
}
```

You can also destructure. At each call site:

- Every required prop must be passed (`mx-component-required`).
- Unknown props are rejected (`mx-component-prop`).
- `key` is reserved for list rendering and is never passed as a prop.

### Function props

Props typed as functions accept a named function or an inline arrow:

```tsx
export function Button(props: { label: string, onPress: () => void }) {
  return <button onClick={props.onPress}>{props.label}</button>;
}

export default function App() {
  return (
    <body>
      <Button label="Save" onPress={() => console.log("saved")} />
    </body>
  );
}
```

Inline arrows on function-typed props are compiled into typed adapters, so
they behave like normal callbacks.

## Local state is per-instance

`morphState` inside a component belongs to that instance. Rendering `<Hero />`
twice gives two independent states, even though both run the same code:

```tsx
import { morphState } from 'morph';

export function Hero(props: { title: string }) {
  const [count, setCount] = morphState(0);
  return <div>{props.title}: {count}</div>;
}
```

Limitation: components rendered inside a `.map()` list template currently
share one state slot per template (per-instance state in lists is planned).

## Sharing state without providers: morphShared

Refer to the [`morphShared` API documentation](../api/morphShared.md) for details
on creating module-scoped shared state.

## Messaging with morphEvent

Refer to the [`morphEvent` API documentation](../api/morphEvent.md) for details
on creating typed event channels between components.

## Styles

Use standard CSS imports. `CSS.load()` is deprecated:

```tsx
import "./hero.css";
```

`morph check` warns with `mx-css-load-deprecated` on `CSS.load()` calls.

## Lint codes

| Code | Meaning |
|---|---|
| `mx-component-unknown` | Component tag is neither defined nor imported |
| `mx-component-prop` | Unknown prop passed to a component |
| `mx-component-required` | Required prop missing at a call site |
| `mx-state-scope` | `morphState` is called inside a component body |
| `mx-shared-scope` | `morphShared` is exported at module scope |
| `mx-event-scope` | `morphEvent` is exported at module scope |
| `mx-api-removed` | Removed string-key/shared/event APIs are not used |
| `mx-css-load-deprecated` | `CSS.load()` — use `import "./x.css"` |
