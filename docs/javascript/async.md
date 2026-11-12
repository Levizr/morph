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

`response.json()` returns a `JsValue`; dot access and bracket access are equivalent:

```tsx
const data = response.json()
console.log(data.name, data["name"])
```

The browser-shaped `await response.json()` works and lowers to the same synchronous reader.

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

Timers forward trailing arguments to the callback, matching JS. The values are evaluated once when the timer is scheduled:

```tsx
setTimeout((a, b) => console.log(a + b), 0, 1, 2) // 3
setInterval((label, n) => console.log(label, n), 500, "tick", 7)
```

Trailing arguments require an inline arrow callback with a parameter for each value. Extra arguments beyond the parameter list are evaluated and ignored. Every callback parameter needs a supplied value — a defaulted parameter is not filled in when its argument is omitted. Identifier/function-expression callbacks, rest-parameter callbacks, and callbacks that declare more parameters than supplied arguments are rejected by `morph check` — pass values through a closure instead.

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

## `.then` / `.catch` chains

For compatibility with browser-style code, a statement-level `fetch(...)` chain inside an async function lowers to the same awaited fetch plus callbacks:

```tsx
async function load() {
  fetch("https://api.example.com/data")
    .then((response) => console.log(response.text()))
    .catch((error) => console.log(error.message))
}
```

The `.then` callback receives a `Response`; the `.catch` callback receives the error as a `JsValue`. The callback runs inside a `try`/`catch`, so an exception thrown by `.then` reaches `.catch`, matching JavaScript ordering. Supported shapes are one `.then`, one `.catch`, or `.then` followed by `.catch`, with a non-async arrow callback of at most one plain parameter.

Everything else is rejected by `morph check` instead of silently dropping work: assignment/return positions, `await fetch(...).then(...)`, non-`fetch` roots, duplicate steps, `.catch` before `.then`, `.then` with an onRejected second argument, async or function-expression callbacks, and destructured callback parameters. Prefer `async`/`await` with `try`/`catch` for new code.

## Promise statics

`Promise.resolve(value)` returns an already-fulfilled promise. It accepts plain values, another `Promise.resolve(...)`, `new Promise(...)`, and already-resolved promise bindings:

```tsx
const one = await Promise.resolve(1)
const same = await Promise.resolve(one)

const bound = Promise.resolve("hello")
const copy = Promise.resolve(bound)
console.log(await copy, await bound) // hello hello
```

`Promise.all` and `Promise.allSettled` must be the whole `await` of a `const`/`let` declaration or an expression statement inside an async function. The argument must be an array literal; elements can be `fetch(...)`, `Promise.resolve(value)`, or plain expressions. Results stay in input order, and `fetch` elements contribute the response body text:

```tsx
const bodies = await Promise.all([
  fetch("https://api.example.com/a"),
  Promise.resolve("local"),
  "plain",
])
console.log(bodies.join(","))

const settled = await Promise.allSettled([
  Promise.resolve("ok"),
  fetch("https://api.example.com/maybe-down"),
])
console.log(settled[0].status, settled[0].value) // fulfilled ok
console.log(settled[1].status)                   // rejected
```

A rejecting element rejects `Promise.all`, so wrap it in `try`/`catch` when a failure is expected. `Promise.allSettled` never rejects for an element failure; it records `{ status: "fulfilled", value }` or `{ status: "rejected", reason }`.

`Promise.race` and `Promise.any` accept only already-settled values — plain expressions or `Promise.resolve(value)`. The first element wins, while later elements are still evaluated for their side effects:

```tsx
const first = await Promise.race([Promise.resolve("a"), "b"])
console.log(first) // a
```

`return Promise.resolve(value)` adopts the settled value. Async functions return it directly; sync functions and arrows annotated `Promise<T>` build the matching `Result<T>`:

```tsx
async function load(): Promise<number> {
  return Promise.resolve(1)
}

function wrap(): Promise<number> {
  return Promise.resolve(2)
}

const arrow = async (): Promise<number> => Promise.resolve(3)
console.log(await load(), await wrap(), await arrow()) // 1 2 3
```

An unannotated async arrow is fire-and-forget (`Task`) and has no value slot, so `async () => Promise.resolve(1)` and `return Promise.resolve(...)` from a `void`/`Promise<void>` function are rejected. Annotate the arrow `async (): Promise<T> => ...` or use an async function declaration.

Evaluation is sequential; parallel execution is an optimization, not a requirement. `morph check` rejects the shapes the eager lowering cannot represent: `Promise.resolve(fetch(...))`, nested `await`, promise-valued identifiers inside combinators, nested combinators, `new Promise(...)` inside combinators, `fetch` inside `race`/`any`, empty `race`/`any` arrays, spread/elided elements, `return Promise.resolve()` without a value, and combinators outside the supported `await` positions. `Promise.reject` remains unsupported.

## Not Supported Yet

- `Promise.reject` and Promise statics outside the supported shapes above
- General Promise-chain runtimes and `.then`/`.catch` forms outside the supported `fetch` chain shape above

These are tracked in the [TypeScript App-Logic Coverage](../future/javascript/coverage.md) plan.

## Implementation Notes

- `fetch()` is implemented in `runtime/cpp/net/net.h` using `HttpAwaitable` coroutine
- Requests run on a worker thread pool; responses resume on the main thread via the scheduler
- `morph::Result<T>` and `morph::Task` are in `runtime/cpp/reactivity/promise.h` and `task.h`
- The scheduler (`process_tasks()`) drives all coroutines and timers
- Escape analysis determines smart pointer strategy for async variables