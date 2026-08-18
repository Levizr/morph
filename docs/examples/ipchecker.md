# IP Checker

Async networking example that fetches and displays the user's public IP address.

## Files

| File | Description |
|---|---|
| `src/App.mx` | JSX template with async state |
| `src/style.css` | Animations (fade, bounce, glow) |
| `morph.config.json` | Window config (400×300) |

## Features Demonstrated

- `fetch()` for HTTP requests (worker-thread, non-blocking)
- Coroutine-based async/await
- `morphState` with multiple signals: `ip`, `loading`, `error`
- Error handling with try/catch in coroutines
- Dynamic className ternaries driven by state
- CSS `@keyframes` (fade-in, bounce, glow)
- Tailwind utilities + custom animations

## Run

```bash
morph run
```

See the [full README](../../examples/ipchecker/README.md) for more details.
