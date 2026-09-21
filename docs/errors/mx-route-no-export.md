# mx-route-no-export — Route File Without Default Export

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

A `route.mx` file has no default-export component for the router to mount:

```
error : mx-route-no-export : route src/auth/login/route.mx has no default-export component
  Learn more: https://morph.levizr.com/docs/errors/mx-route-no-export
```

Like [mx-route-unknown](mx-route-unknown.md), this fires at **build time**
when the route's module graph is compiled into a mountable window.

## Why Morph raises it

Every `route.mx` becomes a mount: the build generates a mount function that
instantiates the route's root component with mount-time props. With no default
export there is no root — the mount function would be empty, and navigating to
the route would show nothing. The file may still contain helpers, stores, or
channels, but the router needs its one component. Failing here (instead of a
blank window at runtime) names the exact file to fix.

## Example that triggers it

```tsx
// ❌ src/auth/login/route.mx — helpers only, no UI — mx-route-no-export
import { morphShared } from 'morph';

export const [attempts, setAttempts] = morphShared(0);

export function LoginForm() {
  return <div>Login</div>;
}
```

## How to fix

```tsx
// ✅ Default export mounts at /auth/login; helpers stay as named exports
import { morphShared } from 'morph';

export const [attempts, setAttempts] = morphShared(0);

export function LoginForm() {
  return <div>Login</div>;
}

export default function LoginRoute() {
  return <LoginForm />;
}
```

Steps:

1. Add `export default function <Name>() { ... }` rendering the route's UI.
2. Keep helpers/stores as named exports alongside it — they are still
   importable.
3. The default export may declare props — they bind to mount-time props.
4. Rebuild.

## Tuning this rule

Do not disable this rule. A routeless mount cannot render.

## See also

- [mx-route-unknown](mx-route-unknown.md) — referencing a route that is not
  in the manifest
- [mx-export](mx-export.md) — the general single-entry rule
- [Windows and Routing Guide](../guides/windows-and-routing.md)

