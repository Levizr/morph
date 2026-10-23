# mx-import-type — Unsupported Import Type

**Severity:** error for bound imports; warning for bare imports | **Blocks `morph build`:** yes when bound

> **Status:** enforced by `morph check`. The rule reports at the source site
> before codegen, so unsupported code cannot reach the C++ compiler.

## What this error means

An import targets a module kind Morph cannot resolve — anything other than a
`.css` stylesheet, a `.cpp` native module, the `'morph'` runtime, or another
source module (`.mx`, `.tsx`, `.ts`):

```
error : mx-import-type : Unsupported import `./data.json` — only .css, .cpp, 'morph', and source modules can be imported
  hint: Move the data into a `.ts` module or fetch it at runtime
  Learn more: https://morph.levizr.com/docs/errors/mx-import-type
```

Notably, importing another `.mx` file as a *value* outside the component
import form is not supported yet — keep shared UI in components and shared
logic in `.ts`/`.cpp`.

## Why Morph raises it

Imports are resolved at compile time into the module graph (components, logic,
styles, native code). A `.json` file, an npm package, or a URL used as a value
has no graph node type — the compiler cannot turn it into C++. Rather than
emitting a broken reference, Morph reports at the import line. The rule is an
error when the import binds a name (`import config from …`), because codegen
must resolve that name; a bare side-effect import (`import "./data.json"`) is
only a warning because it can be erased. A side-effect stylesheet URL
(`import "https://…/app.css"`) is still supported and does not warn.

## Example that triggers it

```tsx
// ❌ Bound name from a JSON module — mx-import-type (error)
import config from './config.json';

export default function App() {
  return <div>{config.title}</div>;
}
```

```tsx
// ⚠️ Bare side-effect import — mx-import-type (warning)
import './config.json';
```

## How to fix

```tsx
// ✅ Option 1: move the data into a .ts module
// config.ts:  export const title = "My App";
import { title } from './config.ts';

export default function App() {
  return <div>{title}</div>;
}
```

```tsx
// ✅ Option 2: fetch it at runtime (async + fetch are supported)
import { morphState, morphEffect } from 'morph';

export default function App() {
  const [title, setTitle] = morphState("");
  morphEffect(() => {
    fetch("https://example.com/config.json").then((r) => setTitle("loaded"));
  }, []);
  return <div>{title}</div>;
}
```

```tsx
// ✅ Supported import kinds (no warning)
import "./app.css";          // stylesheet
import { add } from "./math.cpp"; // native code
import { morphState } from 'morph'; // runtime API
import Hero from './Hero.mx';       // component module
```

Steps:

1. Identify the import kind Morph cannot handle (JSON, npm package, URL).
2. Convert static data to a `.ts` module; load remote data with `fetch`;
   implement native helpers in `.cpp`.
3. Re-run `morph check`.

## Tuning this rule

```json
{
  "lint": {
    "disable": ["mx-import-type"]
  }
}
```

## See also

- [mx-css-file-missing](mx-css-file-missing.md) — CSS import target missing
- [mx-undefined](mx-undefined.md) — using the unimportable name
- [How to Call C++ from Morph](../guides/native-cpp.md)
- [Async and Fetch](../javascript/async.md)

