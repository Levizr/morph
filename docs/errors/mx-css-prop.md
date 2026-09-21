# mx-css-prop — Unsupported CSS File Property

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

A `.css` file imported by your app uses a property Morph does not support:

```
warning : mx-css-prop : Unsupported CSS property `float` in app.css (line 4)
  hint: Use Flexbox (`display: flex`) — see the supported CSS list
  Learn more: https://morph.levizr.com/docs/errors/mx-css-prop
```

This is a **warning**: the build proceeds and the declaration is skipped.

## Why Morph raises it

CSS files are parsed at compile time into the same native style structs as
inline styles. Unsupported properties have no native field, so they are
dropped. The warning names the file and line so you can find the dead
declaration — without it, your stylesheet would silently do less than it says.
The inline-style equivalent is [mx-style-prop](mx-style-prop.md) (an error);
the file variant warns because stylesheets are often shared or generated.

## Example that triggers it

```css
/* ⚠️ app.css — float is dropped with mx-css-prop */
.card {
  float: left;
  width: 200px;
}
```

## How to fix

```css
/* ✅ Flexbox-based layout */
.row {
  display: flex;
  flex-direction: row;
}

.card {
  width: 200px;
}
```

Steps:

1. Open the flagged file/line and check the property against the
   [supported list](../css/properties.md).
2. Rewrite with Flexbox or another supported feature.
3. Re-run `morph check`.

## Tuning this rule

```json
{
  "lint": {
    "disable": ["mx-css-prop"]
  }
}
```

Escalate to error if your team wants stylesheets held to the same bar as
inline styles:

```json
{
  "lint": {
    "severities": { "mx-css-prop": "error" }
  }
}
```

## See also

- [mx-style-prop](mx-style-prop.md) — inline-style equivalent (error)
- [mx-css-file-missing](mx-css-file-missing.md) — the import target is missing
- [Which CSS Properties Morph Supports](../css/properties.md)

