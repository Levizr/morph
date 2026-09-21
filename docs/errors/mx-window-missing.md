# mx-window-missing — No Window Created

**Severity:** warning | **Blocks `morph build`:** no

## What this error means

Neither your entry file nor your project config declares a window: there is no
`windowConfig` export in the entry file **and** no `window` values in
`morph.config.json`:

```
warning : mx-window-missing : No window declared — no `windowConfig` export and no `window` section in morph.config.json
  hint: Add `"window": { "title": "App", "width": 800, "height": 600 }` to morph.config.json
  Learn more: https://morph.levizr.com/docs/errors/mx-window-missing
```

This is a **warning**: the file still lints, but a build with no window has
nothing to show.

## Why Morph raises it

A Morph app is a native window containing your component tree. The window
settings resolve in two layers:

1. **Project defaults** — the `window` section of `morph.config.json`
   (`title`, `width`, `height`). `morph new` always scaffolds these, so most
   projects already have them.
2. **Per-file override** — `export const windowConfig = {...}` in the entry
   (or route) file, which additionally supports size limits (`minWidth`,
   `maxWidth`, ...) the config section does not carry.

Exporting `windowConfig` is **not required**. When the entry file has no
export, Morph falls back to the config values. This warning fires only when
the fallback is missing too — no export in code *and* no values in config —
meaning the runtime would have to guess the title and size, risking an
invisible or zero-sized window with no indication of why. Non-entry component
files never trigger it (only the entry module renders as a window).

## Example that triggers it

`morph.config.json` with no `window` section:

```json
{
  "name": "my-app",
  "entry": "src/App.mx"
}
```

```tsx
// ❌ No windowConfig export either — mx-window-missing
export default function App() {
  return <div>Hello</div>;
}
```

With a normal scaffolded config (which includes `"window"`), the same file
produces no warning at all.

## How to fix

Prefer the config fix — one place, applies to the whole app:

```json
// ✅ morph.config.json — project-wide window defaults
{
  "name": "My App",
  "entry": "src/App.mx",
  "window": { "title": "My App", "width": 800, "height": 600 }
}
```

Or override per file with `windowConfig` (also the only way to set size
limits):

```tsx
// ✅ Entry-file override with size limits
export const windowConfig = {
  title: "My App",
  width: 800,
  height: 600,
  minWidth: 400,
  minHeight: 300,
};

export default function App() {
  return <div>Hello</div>;
}
```

Steps:

1. Check `morph.config.json` for a `window` object with `title`/`width`/
   `height` — if present, this warning should already be gone; if you still
   see it, the config file is not being picked up (wrong directory).
2. If the section is missing, add it (preferred), or export `windowConfig`
   from the entry file.
3. Re-run `morph check` — the warning clears.

## Tuning this rule

Promote it to an error if you want missing windows to fail CI:

```json
{
  "lint": {
    "severities": { "mx-window-missing": "error" }
  }
}
```

## See also

- [mx-windowconfig-key](mx-windowconfig-key.md) — unknown `windowConfig` key
- [mx-windowconfig-type](mx-windowconfig-type.md) — wrong `windowConfig`
  value type
- [mx-route-no-export](mx-route-no-export.md) — route file without component
- [Windows and Routing Guide](../guides/windows-and-routing.md)
- [How to Configure a Morph Project](../getting-started/configuration.md)
