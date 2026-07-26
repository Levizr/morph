#include "task.h"
#include <algorithm>
#include <vector>

namespace morph {

// ── Internal state ──────────────────────────────────────────────

namespace {

// Coroutines pending resumption (pushed by next_frame::await_suspend)
static std::vector<std::coroutine_handle<>> s_pending;
static std::mutex s_mutex;

// Timer management
static std::vector<TimerEntry> s_timers;
static std::mutex s_timer_mutex;
static std::atomic<int> s_next_timer_id{1};

} // anonymous namespace

// ── next_frame awaiter ──

bool next_frame::await_suspend(std::coroutine_handle<> h) noexcept {
    std::lock_guard<std::mutex> lock(s_mutex);
    s_pending.push_back(h);
    return true; // always suspend
}

// ── process_tasks ──

void process_tasks() {
    // Drain pending coroutines once per frame.
    // Each coroutine runs exactly one iteration of its loop, then re-queues
    // itself via co_await next_frame(). It will be picked up next frame.
    std::vector<std::coroutine_handle<>> batch;
    {
        std::lock_guard<std::mutex> lock(s_mutex);
        batch.swap(s_pending);
    }
    for (auto h : batch) {
        if (!h.done()) {
            h.resume();
        }
        if (h.done()) {
            h.destroy();
        }
    }

    // Fire due timers
    auto now = std::chrono::steady_clock::now();
    std::lock_guard<std::mutex> lock(s_timer_mutex);
    for (auto& t : s_timers) {
        if (!t.active) continue;
        if (now >= t.next_fire) {
            t.fn();
            if (t.interval_ms > 0) {
                t.next_fire = now + std::chrono::milliseconds(t.interval_ms);
            } else {
                t.active = false;
            }
        }
    }
    // Garbage collect inactive one-shot timers
    std::erase_if(s_timers, [](const TimerEntry& e) { return !e.active; });
}

// ── Timer API ──

int set_timeout(std::function<void()> fn, int ms) {
    int id = s_next_timer_id++;
    std::lock_guard<std::mutex> lock(s_timer_mutex);
    s_timers.push_back(TimerEntry{
        .id = id,
        .fn = std::move(fn),
        .next_fire = std::chrono::steady_clock::now() + std::chrono::milliseconds(ms),
        .interval_ms = 0,
        .active = true,
    });
    return id;
}

int set_interval(std::function<void()> fn, int ms) {
    int id = s_next_timer_id++;
    auto now = std::chrono::steady_clock::now();
    std::lock_guard<std::mutex> lock(s_timer_mutex);
    s_timers.push_back(TimerEntry{
        .id = id,
        .fn = std::move(fn),
        .next_fire = now + std::chrono::milliseconds(ms),
        .interval_ms = ms,
        .active = true,
    });
    return id;
}

void clear_timer(int id) {
    std::lock_guard<std::mutex> lock(s_timer_mutex);
    for (auto& t : s_timers) {
        if (t.id == id) {
            t.active = false;
            return;
        }
    }
}

} // namespace morph
