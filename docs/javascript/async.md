# Async & Networking

Morph supports async/await with coroutines and has a built-in HTTP `fetch()` client.

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

### Response API

| Method/Property | Description |
|---|---|
| `response.status` | HTTP status code (200, 404, etc.) |
| `response.ok()` | Returns `true` if status is 200-299 |
| `response.text()` | Returns the response body as a string |
| `response.headers` | Response headers |

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

## Coroutines

Async functions compile to C++ coroutines (`morph::Task`). Each `await` point suspends the coroutine and resumes it when the result is available, without blocking the UI thread.

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
async function animate() {
  // runs on the next frame tick
  await next_frame
  console.log("next frame")
}
```
