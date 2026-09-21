# mx-import-type — Unsupported Import Type

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

An import targets a module kind Morph cannot resolve — anything other than a
`.css` stylesheet, a `.cpp` native module, the `'morph'` runtime, or another
source module (`.mx`, `.tsx`, `.ts`):

```
warning : mx-import-type : Unsupported import `./data.json` — only .css, .cpp, 'morph', and source modules can be imported
  hint: Move the data into a `.ts` module or fetch it at runtime
  Learn more: https://morph.levizr.com/docs/errors/mx-import-type
```

Notably, importing another `.mx` file as a *value* outside the component
import form is not supported yet — keep shared UI in components and shared
logic in `.ts`/`.cpp`.

## Why Morph raises it

Imports are resolved at compile time into the module graph (components, logic,
styles, native code). A `.json` file, an npm package, or a URL has no graph
node type — the compiler cannot turn it into C++. Rather than emitting a
broken reference, Morph warns at the import line. This is a warning (not an
error) because the import may be type-only or otherwise erasable — but if the
value is used, expect a follow-up [mx-undefined](mx-undefined.md) or a build
failure.

## Example that triggers it

```tsx
// ⚠️ JSON imports have no module kind — mx-import-type
import config from './config.json';

export default function App() {
  return <div>{config.title}</div>;
}
```

## How to fix

```tsx
// ✅ Option 1: move the data into a .ts module
// config.ts:  export const title = "My App";
import { title } from './config';

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

