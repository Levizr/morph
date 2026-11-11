#pragma once
#include <coroutine>
#include <functional>
#include <chrono>
#include <vector>
#include <mutex>
#include <atomic>
#include <utility>
#include <cstdint>
#include <tuple>
#include <type_traits>

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

namespace detail {

// Invoke a timer callback with the captured trailing arguments. JavaScript
// ignores arguments the callback does not declare, so drop trailing values
// until the call fits. A callback that needs more arguments than were
// supplied fails to compile (the linter rejects that shape first).
template <typename Fn, typename Tuple, std::size_t... I>
void invoke_timer_impl(Fn& fn, Tuple& args, std::index_sequence<I...>) {
    if constexpr (std::is_invocable_v<Fn&, decltype(std::get<I>(args))...>) {
        std::invoke(fn, std::get<I>(args)...);
    } else if constexpr (sizeof...(I) > 0) {
        invoke_timer_impl(fn, args, std::make_index_sequence<sizeof...(I) - 1>{});
    } else {
        static_assert(std::is_invocable_v<Fn&>,
                      "timer callback cannot be called with the supplied arguments");
        std::invoke(fn);
    }
}

template <typename Fn, typename... Args>
void invoke_timer(Fn& fn, std::tuple<Args...>& args) {
    invoke_timer_impl(fn, args, std::index_sequence_for<Args...>{});
}

} // namespace detail

// Trailing timer arguments are evaluated once, at scheduling time, then
// passed to the callback on every fire — matching JS. The `requires` clause
// keeps two-argument calls on the non-template overload above.
template <typename Fn, typename... Args>
    requires(sizeof...(Args) > 0)
int set_timeout(Fn fn, int ms, Args... args) {
    return set_timeout(
        [fn = std::move(fn), args = std::make_tuple(std::move(args)...)]() mutable {
            detail::invoke_timer(fn, args);
        },
        ms);
}

template <typename Fn, typename... Args>
    requires(sizeof...(Args) > 0)
int set_interval(Fn fn, int ms, Args... args) {
    return set_interval(
        [fn = std::move(fn), args = std::make_tuple(std::move(args)...)]() mutable {
            detail::invoke_timer(fn, args);
        },
        ms);
}

} // namespace morph

#include <format>
template <>
struct std::formatter<morph::Task> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const morph::Task&, auto& ctx) const {
        return std::format_to(ctx.out(), "Promise {{ undefined }}");
    }
};
