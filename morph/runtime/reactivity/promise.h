#pragma once
// Morph async coroutine return type: morph::Result<T>
// For JS async functions / promises. `co_await result` yields the stored value.
#include <coroutine>
#include <optional>
#include <utility>
#include <type_traits>

namespace morph {

template <typename T>
class Result {
public:
    struct promise_type {
        std::optional<std::remove_cv_t<T>> value;

        Result get_return_object() noexcept {
            return Result{std::coroutine_handle<promise_type>::from_promise(*this)};
        }

        std::suspend_never initial_suspend() noexcept { return {}; }

        std::suspend_always final_suspend() noexcept { return {}; }

        template <typename U>
        void return_value(U&& v) {
            value = std::forward<U>(v);
        }

        void unhandled_exception() noexcept { std::terminate(); }
    };

    std::coroutine_handle<promise_type> handle = nullptr;

    Result() = default;

    explicit Result(std::coroutine_handle<promise_type> h) noexcept : handle(h) {}

    Result(Result&& other) noexcept : handle(std::exchange(other.handle, nullptr)) {}

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

        T await_resume() noexcept {
            if (!handle.promise().value) {
                return T{};
            }
            return std::move(handle.promise().value.value());
        }
    };

    auto operator co_await() noexcept {
        return ValueAwaiter{handle};
    }
};

} // namespace morph