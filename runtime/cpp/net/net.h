#pragma once
// Morph Networking — async fetch() for JS async/await.
// Everything network-related lives here: morph::net
#include <coroutine>
#include <map>
#include <memory>
#include <string>
#include <utility>

#include "../types/js_types.h"

namespace morph::net {

// ── Response ─────────────────────────────────────────────────────
// The intermediate result of `await fetch(url)`. Mirrors the JS Response
// API minimally: status, headers, body text.
struct Response {
    int status = 0;
    std::map<std::string, std::string> headers;
    std::string body;
    // Raw request head (request line + headers) actually sent over the wire.
    // Populated by http_get for the DevTools Network tab.
    std::string requestHead;

    bool ok() const noexcept { return status >= 200 && status < 300; }

    std::string text() const { return body; }

    // JS `await fetch(...)` in a string context yields the body text
    operator JsString() const { return JsString(body); }
};

namespace detail {

struct SharedState {
    std::string url;
    Response response;
    // Set when the request failed at the transport level (DNS, connect,
    // timeout, or empty reply) rather than returning an HTTP error status.
    std::string error;
};

// Performs a blocking HTTP GET on a worker thread, then resumes the
// awaiting coroutine. The result is stashed in SharedState so await_resume
// can return it by value after the thread finishes.
struct HttpAwaitable {
    std::shared_ptr<SharedState> state;

    bool await_ready() noexcept { return false; }

    void await_suspend(std::coroutine_handle<> h) noexcept;

    Response await_resume() {
        if (!state->error.empty()) {
            throw JsValue(JsObject{{"name", JsString("Error")},
                                   {"message", JsString(state->error)}});
        }
        return state->response;
    }
};

} // namespace detail

// ── fetch ────────────────────────────────────────────────────────
// JS: `let r = await fetch("https://...")`
// C++: `auto r = co_await morph::net::fetch("https://...")`
inline detail::HttpAwaitable fetch(std::string url) {
    auto state = std::make_shared<detail::SharedState>();
    state->url = std::move(url);
    return detail::HttpAwaitable{std::move(state)};
}

// ── Synchronous helper ───────────────────────────────────────────
// Blocking HTTP GET. Used internally by the awaitable, also exposed for
// non-coroutine contexts.
Response http_get(const std::string& url);

} // namespace morph::net