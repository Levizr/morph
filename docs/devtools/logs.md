# Logs

The Logs tab streams your app's output — everything you print from component logic lands here.

## Logging from Your App

`console.log`, `console.warn`, `console.error`, and `console.info` in `.mx` code map to the dev runtime's log sinks, so they show up in the panel:

```tsx
morphEffect(() => {
  console.log(`count is now ${count}`)
}, [count])
```

Runtime messages land here too — for example, errors from loading a hot-reloaded logic module appear as `error` entries without you printing anything.

## Entries

Every entry has:

| Field | Description |
|---|---|
| Level | `info`, `ok`, `warn`, or `error` |
| Timestamp | Relative time |
| Message | The logged text |

The **Clear** button empties the log.

## Buffer

Logs live in a thread-safe ring buffer. Your code can log safely from anywhere — effects, timers, or `fetch()` callbacks running on worker threads — and the UI thread reads a consistent snapshot.
