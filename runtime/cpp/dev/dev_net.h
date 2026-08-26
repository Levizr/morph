#pragma once
#include <deque>
#include <mutex>
#include <string>
#include <vector>
#include <cstddef>
#include "dev_log.h"

// ── Dev network entries ──────────────────────────────────────────
// Records every morph::net request so the DevTools Network tab can
// display it (URL, status, timing, size). Header-only + inline so any
// TU can push to the shared buffer.
struct DevNetEntry {
    int id = 0;
    double startTime = 0.0;  // seconds since program start
    double duration = 0.0;   // seconds (0 until done)
    std::string method;
    std::string url;
    int status = 0;          // 0 while pending / on transport error
    size_t bytes = 0;        // response body size
    std::string error;
    std::string requestHeaders;   // raw request head (request line + headers)
    std::string responseHeaders;  // raw response head (status line + headers)
    std::string bodyPreview;      // response body, capped for display
    bool done = false;
};

inline std::mutex& devNetMutex() {
    static std::mutex mtx;
    return mtx;
}

inline std::deque<DevNetEntry>& devNetEntries() {
    static std::deque<DevNetEntry> entries;
    return entries;
}

// Records the start of a request and returns an id to finish later.
inline int devNetBegin(const std::string& method, const std::string& url) {
    static const size_t kMaxEntries = 100;
    static int nextId = 1;
    auto& entries = devNetEntries();
    std::lock_guard<std::mutex> lock(devNetMutex());
    if (entries.size() >= kMaxEntries)
        entries.pop_front();
    DevNetEntry e;
    e.id = nextId++;
    e.startTime = devLogNow();
    e.method = method;
    e.url = url;
    entries.push_back(std::move(e));
    return e.id;
}

// Finishes a request by id; the entry may already have been evicted.
inline void devNetEnd(int id, int status, size_t bytes, const std::string& error,
                      const std::string& requestHeaders,
                      const std::string& responseHeaders,
                      const std::string& bodyPreview) {
    auto& entries = devNetEntries();
    std::lock_guard<std::mutex> lock(devNetMutex());
    for (auto& e : entries) {
        if (e.id == id) {
            e.status = status;
            e.bytes = bytes;
            e.error = error;
            e.requestHeaders = requestHeaders;
            e.responseHeaders = responseHeaders;
            e.bodyPreview = bodyPreview;
            e.duration = devLogNow() - e.startTime;
            e.done = true;
            return;
        }
    }
}

inline void devNetClear() {
    auto& entries = devNetEntries();
    std::lock_guard<std::mutex> lock(devNetMutex());
    entries.clear();
}

// Snapshot of the live entries, safe for the UI thread to iterate while a
// worker thread may still be mutating the real deque.
inline std::vector<DevNetEntry> devNetSnapshot() {
    auto& entries = devNetEntries();
    std::lock_guard<std::mutex> lock(devNetMutex());
    return std::vector<DevNetEntry>(entries.begin(), entries.end());
}
