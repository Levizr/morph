# route-test

File-based routing proving ground (build step 2: manifest pass).

- `src/settings/route.mx` → `/settings` (with `windowConfig`)
- `src/auth/login/route.mx` → `/auth/login` (no `windowConfig` — app defaults)
- `src/blog/_components/route.mx` → **not a route** (private folder, skipped)

The manifest is asserted at runtime: the self-test checks every RID
const (`route:/auth/login`, `route:/settings`) plus `route:count == 2`,
proving the private folder stayed out. `morph_routes.h` + project-root
`morph-routes.d.ts` are emitted every build.

Verify: `morph build --no-upx`, `<binary> --morph-self-test`.
