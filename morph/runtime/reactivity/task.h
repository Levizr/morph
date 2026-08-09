#pragma once
#include <coroutine>
#include <functional>
#include <chrono>
#include <vector>
#include <mutex>
#include <atomic>
#include <utility>
#include <cstdint>

namespace morph {

// ── Forward declarations ──
struct Task;
void process_tasks();
void schedule_coroutine(Task t) noexcept;

// ── next_frame awaiter ──
// Suspends the current coroutine and resumes it on the next process_tasks() tick.
struct next_frame {
    bool await_ready() noexcept { return false; }

    bool await_suspend(std::coroutine_handle<> h) noexcept;

    void await_resume() noexcept {}
};

// ── Task (coroutine return type) ──
// Eagerly-started coroutine. The coroutine frame is owned by the scheduler
// and is destroyed automatically after completion in process_tasks().
struct Task {
    struct promise_type {
        Task get_return_object() noexcept {
            return Task{std::coroutine_handle<promise_type>::from_promise(*this)};
        }

        std::suspend_never initial_suspend() noexcept { return {}; }

        std::suspend_always final_suspend() noexcept { return {}; }

        void return_void() noexcept {}

        void unhandled_exception() noexcept { std::terminate(); }
    };

    std::coroutine_handle<promise_type> handle = nullptr;

    Task() = default;

    explicit Task(std::coroutine_handle<promise_type> h) noexcept : handle(h) {}

    bool done() const noexcept { return !handle || handle.done(); }

    ~Task() {
        // If the coroutine completed before we got here, clean up
        if (handle && handle.done()) {
            handle.destroy();
            handle = nullptr;
        }
    }

    Task(Task&& other) noexcept : handle(std::exchange(other.handle, nullptr)) {}

    Task& operator=(Task&& other) noexcept {
        if (this != &other) {
            if (handle && handle.done()) {
                handle.destroy();
            }
            handle = std::exchange(other.handle, nullptr);
        }
        return *this;
    }

    Task(const Task&) = delete;
    Task& operator=(const Task&) = delete;
};

// ── schedule_coroutine ──
// Takes ownership of a Task and tracks it. The coroutine runs eagerly and
// suspends at co_await next_frame(). process_tasks() resumes it.
inline void schedule_coroutine(Task t) noexcept {
    auto h = t.handle;
    t.handle = nullptr; // prevent ~Task from destroying the frame
    if (h && h.done()) {
        h.destroy();
    }
}

// ── Timer types ──

struct TimerEntry {
    int id;
    std::function<void()> fn;
    std::chrono::steady_clock::time_point next_fire;
    int interval_ms; // 0 = one-shot timeout
    bool active = true;
};

// ── Timer API (JS-compatible) ──
int set_timeout(std::function<void()> fn, int ms);
int set_interval(std::function<void()> fn, int ms);
void clear_timer(int id);

} // namespace morph
