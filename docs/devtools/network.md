# Network

The Network tab logs every `fetch()` call your app makes — status, timing, headers, and body previews.

## Request List

A summary bar at the top shows **total / ok / err / bytes** across all captured requests.

Each request row shows:

- Colored status dot + HTTP status code
- Method (`GET`, `POST`, ...)
- URL
- Duration
- Body size

Pending requests appear immediately and live-update until they finish. The list is scrollable, and the **Clear** button empties the log.

The log is a 100-entry ring buffer — older requests fall off as new ones arrive.

## Detail View

Click any request to open its detail view:

- **GENERAL** — method, URL, status, timing
- **RESPONSE HEADERS** — every response header
- **REQUEST HEADERS** — every request header
- **BODY** — preview of the response body

Press **‹ Back** to return to the list.

The raw request and response heads are captured from the actual socket — you see exactly what went over the wire, not a reconstruction after parsing.

## Threading

`fetch()` runs on worker threads. The ring buffer is thread-safe, and the UI thread reads from a snapshot, so logging never blocks or races with rendering.
