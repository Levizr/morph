# Async & Networking

Morph supports async/await with C++20 coroutines and has a built-in HTTP `fetch()` client.

## fetch()

```tsx
async function load() {
  const response = await fetch("https://api.example.com/data")
  if (response.ok()) {
    const body = response.text()
    console.log(body)
  }
}
```

`fetch()` runs the HTTP request on a worker thread and resumes the coroutine when the response arrives. The UI stays responsive.

### Request options

`fetch()` accepts a request object for method, headers, and body:

```tsx
const response = await fetch("https://api.example.com/users", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ name: "Ada" }),
})
```

The runtime supports `method`, `headers`, and a string `body`. Response headers expose `.get(name)` and `.has(name)`:

```tsx
const contentType = response.headers.get("content-type")
```

### Response API

| Method/Property | Description |
|---|---|
| `response.status` | HTTP status code (200, 404, etc.) |
| `response.ok()` | Returns `true` if status is 200-299 |
| `response.text()` | Returns the response body as a string (synchronous) |
| `response.headers` | Response headers; `.get(name)` / `.has(name)` |
| `response.statusText` | HTTP status text |
| `response.url` | Final URL (after redirects) |
| `response.redirected` | Whether the request was redirected |
| `response.type` | Response type: `"basic"`, `"cors"`, `"error"`, `"opaque"` |
| `response.bodyUsed` | Whether the body has been consumed |
| `response.json()` | Parses body as JSON (synchronous, returns `JsValue`) |
| `response.arrayBuffer()` | Returns body as raw bytes (synchronous) |
| `response.clone()` | Creates a copy for multiple reads |

`response.json()` returns a `JsValue`, so read fields with bracket access today:

```tsx
const data = response.json()
console.log(data["name"])
```

The browser-shaped `await response.json()` works and lowers to the same synchronous reader. Dot access (`data.name`) is still tracked in the [TypeScript App-Logic Coverage](../future/javascript/coverage.md) plan.

### Error Handling

```tsx
async function load() {
  try {
    let r = await fetch("https://api.example.com/data")
    if (!r.ok()) {
      setError("HTTP error " + r.status)
      return
    }
    let body = r.text()
    setData(body)
  } catch (e) {
    setError("Network error: " + e.message)
  }
}
```

### Example: IP Checker

```tsx
const [ip, setIp] = morphState("")
const [loading, setLoading] = morphState(0)
const [error, setError] = morphState("")

async function fetchIp() {
  setLoading(1)
  setError("")
  try {
    let r = await fetch("http://api.ipify.org")
    if (!r.ok()) {
      setError("HTTP error " + r.status)
      setLoading(0)
      return
    }
    setIp(r.text())
  } catch (e) {
    setError("Network error: " + e.message)
  }
  setLoading(0)
}
```

## Timers

```tsx
// Run once after 1 second
setTimeout(() => {
  console.log("fired")
}, 1000)

// Run every 500ms
const id = setInterval(() => {
  console.log("tick")
}, 500)

// Stop timers
clearTimeout(id)   // cancels a timeout
clearInterval(id)  // cancels an interval
```

Timer callbacks currently take no arguments. Pass values through a closure instead of using the trailing-argument form:

```tsx
const label = "done"
setTimeout(() => console.log(label), 1000)
```

## Coroutines

Async functions compile to C++ coroutines (`morph::Result<T>` for `Promise<T>`, `morph::Task` for `Promise<void>`). Each `await` point suspends the coroutine and resumes it when the result is available, without blocking the UI thread.

```tsx
async function loadData() {
  const response = await fetch("https://api.example.com/users")
  const data = response.text()
  // continues here after fetch completes
  setUsers(data)
}
```

You can also await the next frame:

```tsx
import { next_frame } from 'morph'

async function animate() {
  // runs on the next frame tick
  await next_frame
  console.log("next frame")
}
```

## Top-Level Await

Top-level `await` is supported in entry files. Morph wraps the module in an async main coroutine:

```tsx
// src/App.mx (top-level)
const data = await fetch("https://api.example.com/config")
console.log("Loaded config:", data)
```

This compiles to:

```cpp
int main() {
    morph::Task _main_task = [&]() -> morph::Task {
{
    auto data = co_await morph::net::fetch("https://api.example.com/config");
    std::println("{} {}", "Loaded config:", data);
co_return;
}
    }();
    while (!_main_task.done()) {
        morph::process_tasks();
    }
    return 0;
}
```

## Not Supported Yet

- `Promise.resolve` / `Promise.all` / `Promise.allSettled` / `Promise.race` / `Promise.any`
- `.then` / `.catch` chains on `fetch` results; use `async`/`await` (`morph check` rejects them)
- Trailing timer arguments (`setTimeout(fn, ms, ...args)`; `morph check` rejects them)

These are tracked in the [TypeScript App-Logic Coverage](../future/javascript/coverage.md) plan.

## Implementation Notes

- `fetch()` is implemented in `runtime/cpp/net/net.h` using `HttpAwaitable` coroutine
- Requests run on a worker thread pool; responses resume on the main thread via the scheduler
- `morph::Result<T>` and `morph::Task` are in `runtime/cpp/reactivity/promise.h` and `task.h`
- The scheduler (`process_tasks()`) drives all coroutines and timers
- Escape analysis determines smart pointer strategy for async variables