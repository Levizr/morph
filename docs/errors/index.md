# Morph Error Reference

Every error and warning reported by `morph check` and `morph build` has a stable
rule code starting with `mx-`. When a diagnostic fires, the terminal shows the
code, what went wrong, a hint, a code frame, and a **Learn more** link that
opens the matching page in this reference:

```
error : mx-tag : Unknown tag <vidio>
  hint: Did you mean <video>?
  Learn more: https://morph.levizr.com/docs/errors/mx-tag
```

## How to use this reference

1. Copy the `mx-*` code from your terminal output.
2. Open the page below — each page explains **why** the rule exists (Morph
   compiles `.mx` files to native C++, so browser-only patterns cannot pass
   through), shows the broken code, the fixed code, and how to tune the rule.
3. Errors block `morph build` and dev-mode reloads until fixed. Warnings are
   reported and never block — but they describe real bugs (missing `key`,
   stub elements), so fix them too.

Pages marked **(rule)** document a real constraint that `morph check` does not
report yet — violations surface as build failures or silently ignored code
instead of a labeled diagnostic. The cause and fix on those pages still apply
when you hit the underlying problem. Only implemented, triggerable checks are
listed here: there is no page for a feature that does not exist.

## Rule severity

Severity is per-code with a documented default (see
[lint configuration](../getting-started/configuration.md#lint)). You can turn a
rule off or change its severity in `morph.config.json`:

```json
{
  "lint": {
    "disable": ["mx-list-key"],
    "severities": { "mx-tag-stub": "error" }
  }
}
```

## All error codes

### Structure

- [mx-export](mx-export.md) — missing or duplicate default-export component
- [mx-component-unknown](mx-component-unknown.md) — unknown component tag
- [mx-component-prop](mx-component-prop.md) — unknown prop on a component
- [mx-component-required](mx-component-required.md) — missing required prop

### Window

- [mx-window-missing](mx-window-missing.md) — no window created
- [mx-windowconfig-key](mx-windowconfig-key.md) **(rule)** — invalid `windowConfig` key
- [mx-windowconfig-type](mx-windowconfig-type.md) **(rule)** — invalid `windowConfig` value type

### Elements and props

- [mx-tag](mx-tag.md) — unknown element tag
- [mx-tag-stub](mx-tag-stub.md) — registered but unimplemented element
- [mx-prop](mx-prop.md) — invalid or misspelled prop
- [mx-img-src](mx-img-src.md) **(rule)** — `<img>` without `src`
- [mx-event-value](mx-event-value.md) **(rule)** — event handler is not a function
- [mx-key-misuse](mx-key-misuse.md) **(rule)** — `key` used outside a list
- [mx-dup-class](mx-dup-class.md) **(rule)** — `class` and `className` used together

### Styles

- [mx-style-prop](mx-style-prop.md) **(rule)** — unsupported inline style property
- [mx-style-value](mx-style-value.md) **(rule)** — invalid inline style value
- [mx-tailwind-class](mx-tailwind-class.md) **(rule)** — unresolvable Tailwind class
- [mx-css-prop](mx-css-prop.md) **(rule)** — unsupported CSS file property
- [mx-css-file-missing](mx-css-file-missing.md) **(rule)** — imported CSS file not found
- [mx-css-load-deprecated](mx-css-load-deprecated.md) — `CSS.load()` is deprecated

### Imports and names

- [mx-import-morph](mx-import-morph.md) **(rule)** — bad import from `'morph'`
- [mx-no-morph-import](mx-no-morph-import.md) — Morph API used without import
- [mx-undefined](mx-undefined.md) — name is not defined or imported
- [mx-import-type](mx-import-type.md) **(rule)** — unsupported import type
- [mx-naming](mx-naming.md) — invalid module file or directory name

### State and effects

- [mx-state-scope](mx-state-scope.md) — `morphState` outside a component
- [mx-shared-scope](mx-shared-scope.md) — `morphShared` not at exported module scope
- [mx-event-scope](mx-event-scope.md) — `morphEvent` not at exported module scope
- [mx-api-removed](mx-api-removed.md) — removed string-key API in use
- [mx-state-pattern](mx-state-pattern.md) **(rule)** — `morphState` not destructured as pair
- [mx-effect-cb](mx-effect-cb.md) **(rule)** — `morphEffect` first argument is not a function
- [mx-effect-deps](mx-effect-deps.md) **(rule)** — effect dependency is not state

### JavaScript support

- [mx-transpile](mx-transpile.md) **(rule)** — JavaScript that cannot compile to C++
- [mx-js-global](mx-js-global.md) — browser global with no native counterpart
- [mx-js-member](mx-js-member.md) **(rule)** — unsupported builtin member access
- [mx-js-method](mx-js-method.md) **(rule)** — method the runtime types do not implement
- [mx-js-op](mx-js-op.md) **(rule)** — operator the translator cannot emit
- [mx-js-syntax](mx-js-syntax.md) **(rule)** — syntax construct the translator cannot handle
- [mx-list-key](mx-list-key.md) — list item without `key`

### Routes

- [mx-route-unknown](mx-route-unknown.md) — unknown route id
- [mx-route-no-export](mx-route-no-export.md) — route file without default export
