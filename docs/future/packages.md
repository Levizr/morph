# Package JS→C++ Build Bridge

**Status:** future · **Priority:** medium

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Making `morph pkg` packages actually compile into your binary. The package CLI works — `morph pkg add/remove/install/list/search` — but packages are **downloaded, not compiled**. Full spec: `help/package_authoring.md`.

## Why it matters

- Reusable components (`morph-icons`, `morph-animate`, community widgets) as installable packages
- A package ships JS + C++ runtime headers; today the JS is dead weight
- Unlocks the ecosystem: `morph pkg install morph-icons` should just work

## Current state

| Piece | State |
|---|---|
| Package CLI (add/remove/install/list/search) | ✅ Shipped |
| `morph.pkg.json` manifest parsing | ✅ Shipped |
| Dependency resolver (versions, topological sort) | ⚠️ Stub — `# TODO: topological sort + version conflict detection` (`pkg/resolver.py:5`) |
| **JS → C++ bridge at build time** | ❌ Not implemented |

## Planned mechanism (already specified in the authoring doc)

### Package manifest

```json
// morph.pkg.json
{
  "name": "my-widget",
  "js_entry": "src/index.ts",        // JS entry the compiler translates
  "runtime_headers": ["runtime/renderer.h"]  // C++ headers baked into the build
}
```

### JS annotations → C++ wiring

Package JS can annotate components that map to C++ nodes:

```ts
// src/index.ts
// @morph-component: MyWidgetNode
// @morph-header: runtime/renderer.h
export function MyWidget() { ... }
```

```cpp
// runtime/renderer.h — the package's C++ node
class MyWidgetNode : public MorphNode {
public:
    void draw(Renderer& r) override { /* custom paint */ }
};
```

At build time the compiler reads the annotations, includes `runtime_headers` into the generated translation unit, and wires JSX usage of `<MyWidget/>` to `MyWidgetNode` — exactly like the existing user-side [C++ interop](../../docs/guides/native-cpp.md), but packaged and versioned.

### Dependency resolution

- Resolve `morph.pkg.json` dependencies by version (semver ranges)
- Topological sort for install order
- Conflict detection (two packages wanting different versions of a dependency)

## Open questions

- **Sandboxing** — package C++ headers are compiled straight into your binary; trust model needed (like `npm`'s postinstall scripts, but worse: arbitrary C++)
- **Registry** — is there a central registry, or git URLs like `morph pkg add user/repo`?
- **Codegen integration** — annotated components need JSX tag registration in `morph check` (`SUPPORTED_TAGS`) + the IR builder

## Build steps (when picked up)

1. Topological resolver + version conflict detection in `pkg/resolver.py`
2. Build-time bridge: read `js_entry` + `runtime_headers`, feed the translator, include headers in the generated TU
3. `@morph-component` / `@morph-header` annotation parsing + tag registration
4. Example package: a widget that ships both JSX and a C++ node, consumed by an app