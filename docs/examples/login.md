# Login

A native sign-in form with validation and screen transitions.

## Files

| File | Description |
|---|---|
| `src/App.mx` | JSX with login/welcome screens, controlled inputs, validation logic |
| `src/style.css` | Card styling, glow backgrounds, hover/active states, transitions |
| `morph.config.json` | Window size (420×500), entry point |

## Features Demonstrated

- Controlled `<input>` fields — `value` + `onInput` with `{e.value}` updates
- `type="password"` masked input
- Inline validation errors rendered conditionally
- Ternary screen swap — the login form unmounts (nodes freed) when you sign in
- Typed functions (`doLogin():int`, `logout():int`) called from event handlers
- Window locked to 490×600 via `windowConfig` size constraints
- CSS `:hover` / `:active` with smooth transitions

## Run

```bash
morph run
```

See the [full README](../../examples/login/README.md) for more details.
