# Login

A sign-in form built with Morph. Demonstrates controlled text inputs, password masking, form validation, and conditional mounting/unmounting of screens.

## What it shows

- **Controlled inputs** — `<input>` bound with `value={user}` and updated via `onInput={(e) => setUser(e.value)}`
- **Password masking** — `type="password"` hides characters as you type
- **Form validation** — `doLogin()` checks for an empty username and a 4+ character password, showing inline errors
- **Conditional error rendering** — `{error == "" ? <div className="err-space"/> : <div className="err">{error}</div>}` reserves space only when there's no error
- **Screen swap by unmounting** — `loggedIn` toggles a ternary that unmounts the whole login card (freeing its nodes) and mounts a welcome card instead
- **Typed functions** — `doLogin():int` and `logout():int` return values from event-driven logic
- **Window constraints** — `windowConfig` locks the window to exactly 490×600 via `minWidth`/`maxWidth`/`minHeight`/`maxHeight`
- **CSS polish** — `:hover` / `:active` states on fields and buttons, `transition: all 0.15s ease`, ambient blurred glow backgrounds

## Run

```bash
cd examples/login
morph dev          # live window with hot reload
# or
morph run          # build + run optimized binary
```

## Try it

1. Press **Sign in** with empty fields — "Username required" appears.
2. Type a username and a short password — the length check fires.
3. Sign in successfully — the form unmounts and the welcome screen greets you by name.
4. **Sign out** — state resets and the login form remounts.
