# mx-api-removed — Removed String-Key API

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

You use an API form Morph removed: string-key shared/event calls, or the
`morphOn` / `morphEmit` helpers:

```
error : mx-api-removed : `morphShared` now takes a single initial value (string keys removed)
  hint: Use `export const [value, setValue] = morphShared<T>(initialValue)` at module scope
  Learn more: https://morph.levizr.com/docs/errors/mx-api-removed
```

```
error : mx-api-removed : `morphOn` was removed — use `eventName.on(handler)` on an exported event
  hint: Substitute `morphOn(key, fn)` with `ev.on(fn)` on a `morphEvent` binding
  Learn more: https://morph.levizr.com/docs/errors/mx-api-removed
```

## Why Morph raises it

The old API keyed everything by runtime strings (`morphShared("count", 0)`,
`morphEvent("saved")`, `morphOn(key, fn)`). String keys cannot be checked
statically — a typo in the key silently creates a second, disconnected store
or channel, and renaming breaks subscribers invisibly. The replacement binds
by **module path and exported name**, which the compiler resolves and the
linter verifies (`mx-shared-scope`, `mx-event-scope`). The old forms were
removed rather than deprecated because they encouraged patterns that do not
scale; the migration is mechanical and strictly better in every dimension.

## Examples that trigger it

```tsx
// ❌ String-key shared store — mx-api-removed
import { morphShared } from 'morph';
export const [count, setCount] = morphShared("count", 0);
```

```tsx
// ❌ String-key event — mx-api-removed
import { morphEvent } from 'morph';
export const saved = morphEvent("saved");
```

```tsx
// ❌ Removed helpers — mx-api-removed
import { morphOn, morphEmit } from 'morph';
morphOn("saved", () => console.log("done"));
morphEmit("saved", "payload");
```

## How to fix

```tsx
// ✅ Single initial value, exported at module scope
import { morphShared } from 'morph';
export const [count, setCount] = morphShared<number>(0);
```

```tsx
// ✅ Argument-free channel, exported at module scope
import { morphEvent } from 'morph';
export const saved = morphEvent<string>();
saved.on((msg) => console.log(msg));
saved.emit("done");
```

| Old | New |
|---|---|
| `morphShared("key", init)` | `export const [v, setV] = morphShared<T>(init)` |
| `morphEvent("key")` | `export const ev = morphEvent<T>()` |
| `morphOn(key, fn)` | `ev.on(fn)` |
| `morphEmit(key, payload)` | `ev.emit(payload)` |
| `channel.emit(...)` | `ev.emit(...)` |

Steps:

1. Drop the string key — the exported binding name *is* the identity now.
2. Replace `morphOn`/`morphEmit` with `.on` / `.emit` on the channel binding.
3. Fix the import: removed names are no longer exported by `'morph'`
   (importing them also trips [mx-import-morph](mx-import-morph.md)).
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. There is no supported semantics for the old forms —
the compiler has no lowering for them.

## See also

- [mx-shared-scope](mx-shared-scope.md) / [mx-event-scope](mx-event-scope.md)
- [mx-import-morph](mx-import-morph.md)
- [Migration Guide](../guides/migration.md)

