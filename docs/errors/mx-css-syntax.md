# mx-css-syntax — Unparseable Stylesheet

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** reported by `morph check`.

## What this error means

A `.css` file contains a syntax error the CSS parser cannot recover from:

```
warning : mx-css-syntax : Stylesheet has a syntax error and may lose rules: Unexpected token ...
  hint: Fix the syntax error so every rule applies
  Learn more: https://morph.levizr.com/docs/errors/mx-css-syntax
```

This is a **warning**: the build proceeds, but rules around the error may be
silently dropped — the app renders with fewer styles than the file declares.

## Why Morph raises it

Stylesheets are bundled at compile time. When parsing fails, the build skips
what it cannot read instead of failing the whole app — so without this
warning a single typo could swallow half your stylesheet with no indication
why. Fix the flagged syntax and every rule applies again.

## Example that triggers it

```css
/* ⚠️ missing value/colons confuse rule boundaries */
.card {
  color: red;
  widht 10px;
  margin: 0;
}
```

## How to fix

Fix the reported syntax error. Declarations that parse but name an
unsupported property are a separate warning ([mx-css-prop](mx-css-prop.md)).
