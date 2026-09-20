# `fetch()`: HTTP in a Trench Coat

**Part of:** [Dev Docs](../architecture/overview.md)

`await fetch("http://api.ipify.org")` in a `.mx` file looks like the browser. It is not the browser. It is one POSIX/WinSock socket shim, a detached worker thread per request, and an awaitable that resumes your coroutine when the bytes arrive. This page explains the trench coat — what the lowering produces, how the request actually travels, and why the code is shaped the way it is.

The user-facing behavior (async/await, `Response` API) is documented in the main docs' async guide; the coroutine machinery it rides on is in [The Reactivity Engine](reactivity-engine.md).

## The shape (`runtime/cpp/net/net.h`, `net.cpp`)

| Piece | What it is |
|---|---|
| `morph::net::Headers` | Case-insensitive map; `append` joins with `", "` like the Fetch spec |
| `Response` | A Fetch API mirror: `status` / `statusText` / `headers` / `body` / `url` / `redirected` / `type` / `bodyUsed` / `ok()` + `text()` / `json()` / `arrayBuffer()` / `clone()` + a Node-like `std::formatter` |
| `detail::SharedState`, `detail::HttpAwaitable` | The coroutine plumbing between the request and your `co_await` |
| `http_request` | The one HTTP stack for all OSes: `getaddrinfo` → connect → HTTP/1.1 `Connection: close` request → `recv_all` → header parse + redirect detection |

One stack for all OSes, behind a WinSock/POSIX socket shim. HTTP/1.1 with `Connection: close` — no keep-alive pool, no HTTP/2, no regrets. This client does exactly one thing (fetch a URL, return a Response) and does it portably.

## The journey of a `fetch()`

```
your .mx:  const r = await fetch(url)
    │  lowers to  fetch() → morph::Result<JsString>-ish / fetch_response() awaitable
    ▼
HttpAwaitable::await_suspend — spawns a DETACHED worker thread
    │  worker does the blocking request (main thread never blocks)
    │  reports to DevTools via devNetBegin/devNetEnd (MORPH_FEATURE_DEV)
    │  status == 0 → thrown JsValue Error (network failure speaks JS)
    ▼
h.resume() — your coroutine continues on the main-thread heartbeat
    │  (and h.destroy() if done — the fire-and-forget case)
```

Three design decisions worth understanding:

1. **The worker thread is detached and blocking.** No async sockets, no reactor, no event loop inside the event loop. A thread per request is gloriously unfashionable and completely adequate at UI-app scale — and every line of it is debuggable with a normal debugger.
2. **`fetch_response()` returns the awaitable directly.** The comment in the code explains the alternative that was rejected: wrapping it in an eager `Result` lambda would race and yield a default-constructed Response. Returning the awaitable keeps exactly one owner of the completion. If you ever refactor this path, re-read that comment first — it is guarding a real race, not a hypothetical one.
3. **Failures arrive as thrown `JsValue` Errors.** `try`/`catch` around `fetch()` in your `.mx` file catches a genuine JS-flavored error value, which is why the `ipchecker` example's loading/error-state dance works with ordinary syntax.

## Worked example: the `ipchecker` pattern

```tsx
const [ip, setIp] = morphState("…")
const [err, setErr] = morphState("")

morphEffect(() => {
  (async () => {
    try {
      const r = await fetch("http://api.ipify.org")
      if (r.ok()) setIp(r.text())
      else setErr("status " + r.status())
    } catch (e) {
      setErr("network failed")
    }
  })()
}, [])
```

Native-side, that `await` parks the coroutine (see `next_frame`/scheduler semantics in [The Reactivity Engine](reactivity-engine.md)), the worker thread blocks on sockets, and `resume()` continues the coroutine on the main heartbeat — where `setIp` notifies subscribers and the window re-renders. The `.mx` author wrote five lines; four threads' worth of coordination happened underneath.

## DevTools sees everything

Every request is logged through `devNetBegin`/`devNetEnd` (gated by `MORPH_FEATURE_DEV`), which feeds the DevTools Network tab: status, timing, headers, body previews. If a request misbehaves, open the Network tab before reaching for packet captures — the trench coat has glass pockets.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Change request building or response parsing | `net.cpp` (`http_request`, `build_request`, header parse) |
| Change await/resume semantics | `detail::HttpAwaitable` — re-read the `fetch_response()` race comment first |
| Change the `Response` API surface | `net.h` — and mirror the Fetch spec deliberately, not accidentally |
| Add redirect/HTTPS policy | `http_request` redirect detection; note TLS posture is a security decision, see `docs/future/` security notes |

## Verify by

```bash
<binary> --morph-self-test
./tests/runtime/run-selftests.sh
# Plus a manual run of examples/ipchecker under morph dev with the Network tab open
```

Networking changes deserve a live run: build `ipchecker`, watch the request in DevTools, then kill the network and confirm the `catch` path speaks a proper `JsValue` Error.
