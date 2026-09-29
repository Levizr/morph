# mx-route-state — Route Helper Reads Mount State From Module Scope

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

A helper hoisted to module scope captures route state or props, which only
exist on the mount context at runtime:

```
error : mx-route-state : route /add references route state/props from module scope (morph::Result<JsValue> inst2_checkIp()); move it into the component body or pass explicit parameters
  Learn more: https://morph.levizr.com/docs/errors/mx-route-state
```

## Why Morph raises it

Route state (`morphState`) lives per mount on a generated context struct, not
in globals — two windows on one route stay independent. A namespace-scope
helper has no context pointer, so plain helpers that capture setters/getters
are rewritten into context-taking templates (`template <typename __MorphCtx>`
with a `std::shared_ptr` context parameter, threaded through at every call
site). That covers ordinary functions. What still cannot compile — and keeps
this error — is mount state referenced from places that take no parameters:

- class/struct bodies and lambdas at module scope,
- mount-state reads inside a keyed-list item template (factories are
  namespace-scope functions with no context).

The entry build has no mount context, which is why the same component can
compile fine as the app entry but fail as a route.

## Example that triggers it

```tsx
// ❌ Class members, module-scope lambdas/consts, and keyed-list item
//    templates cannot take a context parameter — mx-route-state.
//    (Plain helpers like the one below compile via context templates.)
export default function IpBadge() {
  const [ip, setIp] = morphState("")
  async function checkIp() {
    const r = await fetch("http://api.ipify.org")
    if (r.ok()) { setIp(r.text()) }
  }
  morphEffect(() => { checkIp() }, [])
  return (<div>{ip}</div>)
}
```

## How to fix

Ordinary stateful helpers (like `checkIp` above) compile as-is — the build
threads the mount context through automatically. If you still see this
error, the state access sits somewhere parameterless:

1. Move it into the component body or effect — no hoisted helper, class
   member, or module-scope lambda:
   ```tsx
   // ✅ no module-scope helper; everything runs with the mount context
   morphEffect(() => {
     fetch("http://api.ipify.org").then((r) => { if (r.ok()) { setIp(r.text()) } })
   }, [])
   ```
2. Or pass state explicitly — helpers take setters as parameters instead of
   closing over them.
3. Or move mount-state reads out of keyed-list item templates (or into
   `morphShared` stores, which stay global across mounts).
4. Or keep the stateful component out of routes: mount it from the entry
   tree (`src/App.mx`) and use stateless components plus `morphShared`
   stores inside routes.

## Tuning this rule

Do not disable this rule. A context-free helper cannot reach mount state.

## See also

- [mx-route-no-export](mx-route-no-export.md) — route file without default export
- [mx-state-scope](mx-state-scope.md) — `morphState` outside a component
- [Route mounts (design)](../future/windows/route-mounts.md) — per-mount contexts and what remains
