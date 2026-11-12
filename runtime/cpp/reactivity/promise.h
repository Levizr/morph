#pragma once
// Morph async coroutine return type: morph::Result<T>
// For JS async functions / promises. `co_await result` yields the stored value.
#include <coroutine>
#include <optional>
#include <string>
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

    ~Result() {
        // Reclaim the coroutine frame once it completed. A suspended frame
        // (not done) is owned by whoever suspends it and must survive us.
        if (handle && handle.done()) {
            handle.destroy();
            handle = nullptr;
        }
    }

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
            if (handle && handle.done()) {
                handle.destroy();
            }
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

template <typename T>
struct is_result : std::false_type {};

template <typename T>
struct is_result<Result<T>> : std::true_type {};

// Promise.resolve(value): an already-fulfilled Result carrying the value.
// The overloads keep existing Result values intact instead of wrapping a
// Result inside another Result. Pending Results (fetch/async work) are not
// safe to copy: codegen rejects them at `morph check` time.
template <typename T>
    requires(!is_result<std::decay_t<T>>::value)
Result<std::decay_t<T>> promise_resolve(T&& value) {
    return Result<std::decay_t<T>>::resolved(std::forward<T>(value));
}

// Promise.resolve(existingPromise) for an already-resolved promise binding.
// Copy the stored value so both bindings stay usable, matching JS promise
// identity semantics for the observable value.
template <typename T>
Result<T> promise_resolve(Result<T>& other) {
    if (!other.handle) {
        return Result<T>::pending();
    }
    auto& promise = other.handle.promise();
    if (promise.eptr) {
        auto eptr = promise.eptr;
        return [eptr]() -> Result<T> {
            std::rethrow_exception(eptr);
            co_return T{};
        }();
    }
    if (!promise.value) {
        return Result<T>::pending();
    }
    return Result<T>::resolved(*promise.value);
}

// Promise.resolve(existingPromise) with a temporary promise: transfer the
// frame instead of copying it.
template <typename T>
Result<T> promise_resolve(Result<T>&& other) {
    return std::move(other);
}

template <typename T>
Result<T> promise_resolve(const Result<T>&) = delete;

// JSX text and other `morph::str` conversions render a promise like the JS
// inspector instead of failing to find `std::to_string(Result)`.
template <typename T>
std::string str(const Result<T>& promise) {
    if (!promise.handle) {
        return "Promise { <pending> }";
    }
    auto& state = promise.handle.promise();
    if (state.eptr) {
        return "Promise { <rejected> }";
    }
    if (!state.value) {
        return "Promise { <pending> }";
    }
    return "Promise { " + str(*state.value) + " }";
}

} // namespace morph

#include <format>
template <typename T>
struct std::formatter<morph::Result<T>> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const morph::Result<T>& r, auto& ctx) const {
        if (!r.handle) {
            return std::format_to(ctx.out(), "Promise {{ <pending> }}");
        }
        auto& prom = r.handle.promise();
        if (prom.eptr) {
            return std::format_to(ctx.out(), "Promise {{ <rejected> }}");
        }
        if (!prom.value) {
            return std::format_to(ctx.out(), "Promise {{ <pending> }}");
        }
        return std::format_to(ctx.out(), "Promise {{ {} }}", *prom.value);
    }
};