# IP Checker

Fetches your public IP address from an external API. Demonstrates async networking with `fetch()`, error handling, and loading states.

## What it shows

- **Async `fetch()`** — `await fetch("http://api.ipify.org")` runs HTTP on a worker thread
- **`Response` API** — `r.ok()`, `r.status`, `r.text()` mirror the JS Fetch API
- **try/catch** — network errors surface as `Error` objects with `.message`
- **Loading/error states** — `morphState` toggles a "Fetching..." indicator and error display

## Run

```bash
cd examples/ipchecker
morph dev
# or
morph run
```
