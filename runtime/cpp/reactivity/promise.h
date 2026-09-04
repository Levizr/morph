#pragma once
// Morph async coroutine return type: morph::Result<T>
// For JS async functions / promises. `co_await result` yields the stored value.
#include <coroutine>
#include <optional>
#include <utility>
#include <type_traits>
#include <exception>

namespace morph {

template <typename T>
class Result {
public:
    struct promise_type {
        std::optional<std::remove_cv_t<T>> value;
        std::exception_ptr eptr = nullptr;

        Result get_return_object() noexcept {
            return Result{std::coroutine_handle<promise_type>::from_promise(*this)};
        }

        std::suspend_never initial_suspend() noexcept { return {}; }

        std::suspend_always final_suspend() noexcept { return {}; }

        template <typename U>
        void return_value(U&& v) {
            value = std::forward<U>(v);
        }

        void unhandled_exception() noexcept { eptr = std::current_exception(); }
    };

    std::coroutine_handle<promise_type> handle = nullptr;

    Result() = default;

    explicit Result(std::coroutine_handle<promise_type> h) noexcept : handle(h) {}

    Result(Result&& other) noexcept : handle(std::exchange(other.handle, nullptr)) {}

    // Create an already-resolved Result from a value
    static Result resolved(T v) {
        return [v = std::move(v)]() mutable -> Result {
            co_return std::move(v);
        }();
    }

    static Result pending() {
        return Result{};
    }

    // Allow implicit construction from T for sync `return x;` in Promise<T> fns
    // (JS `return x as Promise<T>` returns plain value at runtime)
    Result(T v) {
        Result r = resolved(std::move(v));
        handle = std::exchange(r.handle, nullptr);
    }

    Result& operator=(Result&& other) noexcept {
        if (this != &other) {
            handle = std::exchange(other.handle, nullptr);
        }
        return *this;
    }

    Result(const Result&) = delete;
    Result& operator=(const Result&) = delete;

    // Fulfil immediately: the async body runs eagerly (initial_suspend is
    // suspend_never) so by the time we are awaited the value is ready.
    struct ValueAwaiter {
        std::coroutine_handle<promise_type> handle;

        bool await_ready() noexcept { return true; }

        void await_suspend(std::coroutine_handle<>) noexcept {}

        T await_resume() {
            if (handle.promise().eptr) {
                std::rethrow_exception(handle.promise().eptr);
            }
            if (!handle.promise().value) {
                return T{};
            }
            return std::move(handle.promise().value.value());
        }
    };

    auto operator co_await() noexcept {
        return ValueAwaiter{handle};
    }

    auto operator co_await() const noexcept {
        // const version for printing? co_await on const Result not typical
        return ValueAwaiter{handle};
    }
};

} // namespace morph

#include <format>
template <typename T>
struct std::formatter<morph::Result<T>> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const morph::Result<T>& r, auto& ctx) const {
        if (!r.handle) {
            return std::format_to(ctx.out(), "Promise {{ <pending> }}");
        }
        try {
            auto& prom = r.handle.promise();
            if (prom.eptr) {
                return std::format_to(ctx.out(), "Promise {{ <rejected> }}");
            }
            if (!prom.value) {
                return std::format_to(ctx.out(), "Promise {{ <pending> }}");
            }
            return std::format_to(ctx.out(), "Promise {{ {} }}", *prom.value);
        } catch (...) {
            return std::format_to(ctx.out(), "Promise {{ <pending> }}");
        }
    }
};