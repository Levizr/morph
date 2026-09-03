#pragma once
// Morph Networking — advanced fetch() for JS async/await.
// Mirrors the browser Fetch API: status, statusText, headers, body, ok, etc.
#include <coroutine>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <algorithm>
#include <cctype>

#include "../types/js_types.h"

namespace morph::net {

// ── Headers ──────────────────────────────────────────────────────
// Case-insensitive header map matching Fetch API Headers.
struct Headers {
    std::map<std::string, std::string> map; // lowercased keys for lookup

    static std::string _lower(std::string s) {
        for (auto& c : s) c = (char)std::tolower((unsigned char)c);
        return s;
    }

    std::string get(const std::string& name) const {
        auto it = map.find(_lower(name));
        return it == map.end() ? "" : it->second;
    }
    bool has(const std::string& name) const {
        return map.find(_lower(name)) != map.end();
    }
    void set(const std::string& name, const std::string& value) {
        map[_lower(name)] = value;
    }
    void append(const std::string& name, const std::string& value) {
        auto key = _lower(name);
        auto it = map.find(key);
        if (it == map.end()) map[key] = value;
        else it->second += ", " + value;
    }
    bool contains(const std::string& name) const { return has(name); }

    // For JS interop: headers["content-type"]
    std::string operator[](const std::string& name) const { return get(name); }

    // For iteration in C++ (for...of over headers)
    auto begin() const { return map.begin(); }
    auto end() const { return map.end(); }
    auto begin() { return map.begin(); }
    auto end() { return map.end(); }

    size_t size() const { return map.size(); }
    bool empty() const { return map.empty(); }
};

// ── Response ─────────────────────────────────────────────────────
// The result of `await fetch(url)`. Mirrors JS Response API:
// status, statusText, headers, body, bodyUsed, ok, redirected, type, url
// plus methods text(), json(), arrayBuffer().
struct Response {
    int status = 0;
    std::string statusText;
    Headers headers;
    std::string body;
    std::string url;
    bool redirected = false;
    std::string type = "basic"; // basic, cors, error, opaque
    bool bodyUsed = false;
    bool ok() const noexcept { return status >= 200 && status < 300; }

    // Raw request head actually sent over the wire (for DevTools)
    std::string requestHead;

    // Body consumption
    std::string text() { bodyUsed = true; return body; }
    std::string arrayBuffer() { bodyUsed = true; return body; }
    // Minimal json() - returns body as JsValue (caller can parse). For now just wraps body.
    JsValue json() {
        bodyUsed = true;
        // Very small JSON parser: if body is JSON, try to parse, else return string
        if (body.empty()) return JsValue(JsNull{});
        // Heuristic: if starts with { or [, try to return as JsObject/JsArray placeholder
        // For now, just return the raw string as JsString - real JSON parsing can be added via json_parser.h
        return JsValue(JsString(body));
    }
    bool bodyUsedFn() const { return bodyUsed; }

    // For JS `await fetch(...)` in string context yields the body text
    operator JsString() const { return JsString(body); }
    operator JsString() { return JsString(body); }
    operator std::string() const { return body; }

    // Clone for JS Response.clone()
    Response clone() const {
        Response c = *this;
        c.bodyUsed = false;
        return c;
    }
};

namespace detail {

struct SharedState {
    std::string url;
    std::string method = "GET";
    Headers requestHeaders;
    std::string requestBody;
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
// JS: `let r = await fetch("https://...")` or `fetch(url, {method, headers, body})`
// C++: `auto r = co_await morph::net::fetch("https://...")`
inline detail::HttpAwaitable fetch(const std::string& url) {
    auto state = std::make_shared<detail::SharedState>();
    state->url = url;
    state->response.url = url;
    return detail::HttpAwaitable{std::move(state)};
}
inline detail::HttpAwaitable fetch(const char* url) {
    return fetch(std::string(url));
}
inline detail::HttpAwaitable fetch(const JsString& url) {
    return fetch(url.value);
}
inline detail::HttpAwaitable fetch(const std::string& url, const JsValue& init) {
    auto state = std::make_shared<detail::SharedState>();
    state->url = url;
    state->response.url = url;
    if (init.is_object()) {
        auto obj = std::get<JsObject>(init.inner);
        if (obj.has("method")) {
            auto m = obj.get("method");
            if (m.is_string()) state->method = std::get<JsString>(m.inner).value;
        }
        if (obj.has("headers") && obj.get("headers").is_object()) {
            auto hobj = std::get<JsObject>(obj.get("headers").inner);
            for (auto& k : hobj.keys()) {
                auto v = hobj.get(k);
                if (v.is_string()) state->requestHeaders.set(k, std::get<JsString>(v.inner).value);
            }
        }
        if (obj.has("body")) {
            auto b = obj.get("body");
            if (b.is_string()) state->requestBody = std::get<JsString>(b.inner).value;
        }
    }
    return detail::HttpAwaitable{std::move(state)};
}
inline detail::HttpAwaitable fetch(const char* url, const JsValue& init) {
    return fetch(std::string(url), init);
}
inline detail::HttpAwaitable fetch(const JsString& url, const JsValue& init) {
    return fetch(url.value, init);
}
inline detail::HttpAwaitable fetch(const std::string& url, const JsObject& init) {
    return fetch(url, JsValue(init));
}
inline detail::HttpAwaitable fetch(const char* url, const JsObject& init) {
    return fetch(std::string(url), JsValue(init));
}
inline detail::HttpAwaitable fetch(const JsString& url, const JsObject& init) {
    return fetch(url.value, JsValue(init));
}
inline detail::HttpAwaitable fetch(const char* url, const char* init) = delete; // avoid ambiguity

// ── Synchronous helpers ──────────────────────────────────────────
Response http_get(const std::string& url);
Response http_request(const std::string& url, const std::string& method, const Headers& headers, const std::string& body);

} // namespace morph::net

// ── Formatter for Response (so std::println and std::format work) ──
#include <format>
template <>
struct std::formatter<morph::net::Response> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const morph::net::Response& r, auto& ctx) const {
        // Brief but Node-like: status, statusText, headers, url
        std::string hdr;
        for (auto& kv : r.headers.map) {
            if (!hdr.empty()) hdr += ", ";
            hdr += kv.first + ": " + kv.second;
        }
        // Truncate body for display
        std::string b = r.body;
        if (b.size() > 200) b = b.substr(0, 200) + "...";
        // Escape quotes in body
        std::string esc;
        esc.reserve(b.size());
        for (char c : b) {
            if (c == '"') esc += "\\\"";
            else if (c == '\n') esc += "\\n";
            else esc += c;
        }
        return std::format_to(ctx.out(),
            "Response {{ status: {}, statusText: \"{}\", headers: {{{}}}, body: \"{}\", bodyUsed: {}, ok: {}, redirected: {}, type: \"{}\", url: \"{}\" }}",
            r.status, r.statusText, hdr, esc, r.bodyUsed ? "true" : "false",
            r.ok() ? "true" : "false", r.redirected ? "true" : "false", r.type, r.url);
    }
};