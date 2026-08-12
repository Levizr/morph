#pragma once
#include <string>
#include <deque>
#include <mutex>
#include <vector>
#include <cstddef>
#include <chrono>

// ── Dev log levels ──────────────────────────────────────────────
enum DevLogLevel {
    LOG_INFO  = 0,
    LOG_OK    = 1,
    LOG_WARN  = 2,
    LOG_ERROR = 3,
};

constexpr const char* devLogLevelName(int level) {
    switch (level) {
        case LOG_INFO:  return "info";
        case LOG_OK:    return "ok";
        case LOG_WARN:  return "warn";
        case LOG_ERROR: return "error";
        default:        return "?";
    }
}

struct DevLogEntry {
    double time = 0.0;   // seconds since program start
    int level = LOG_INFO;
    std::string msg;
};

// ── Thread-safe ring buffer (inline so multiple TUs can share it) ──
inline double devLogNow() {
    static const auto start = std::chrono::steady_clock::now();
    return std::chrono::duration<double>(std::chrono::steady_clock::now() - start).count();
}

inline std::deque<DevLogEntry>& devLogEntries() {
    static std::deque<DevLogEntry> entries;
    return entries;
}

inline void devLogAdd(int level, const std::string& msg) {
    static const size_t kMaxEntries = 300;
    auto& entries = devLogEntries();
    {
        static std::mutex mtx;
        std::lock_guard<std::mutex> lock(mtx);
        if (entries.size() >= kMaxEntries)
            entries.pop_front();
        entries.push_back({devLogNow(), level, msg});
    }
}

inline void devLogClear() {
    auto& entries = devLogEntries();
    static std::mutex mtx;
    std::lock_guard<std::mutex> lock(mtx);
    entries.clear();
}

// Snapshot of the live log entries, safe for the UI thread to iterate while
// a worker thread may still be appending to the real deque.
inline std::vector<DevLogEntry> devLogSnapshot() {
    auto& entries = devLogEntries();
    static std::mutex mtx;
    std::lock_guard<std::mutex> lock(mtx);
    return std::vector<DevLogEntry>(entries.begin(), entries.end());
}
