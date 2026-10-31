#pragma once
// General Array.prototype methods for Morph's JS runtime.
//
// `JsArray` covers the array itself; native `std::vector<T>` stays the
// fast path for loops and indexing. These helpers accept either shape
// (and a `std::shared_ptr<JsArray>` / `JsValue`) and normalize to
// `JsArray` for callbacks and results. Mutating methods take the
// `JsArray` by value: its elements are shared, so the caller's array is
// updated exactly like JS. Vector overloads mutate the vector in place.
//
// Callback arity is detected with `std::is_invocable`: `(item)`, `(item,
// index)`, `(item, index, array)` and their typed variants all work.
// Untyped params receive `JsValue`; numeric/string annotations receive
// the natural C++ conversions.

#include <algorithm>
#include <cstdint>
#include <functional>
#include <initializer_list>
#include <type_traits>
#include <utility>
#include <vector>

#include "js_convert.h"
#include "js_value.h"

namespace morph {

// ── Receiver normalization ────────────────────────────────────────────

inline JsArray js_array_view(const JsArray& array) { return array; }
inline JsArray js_array_view(JsArray&& array) { return std::move(array); }
inline JsArray js_array_view(const std::shared_ptr<JsArray>& array)
{
    return array ? *array : JsArray{};
}
inline JsArray js_array_view(const JsValue& value)
{
    return value.is_array() ? std::get<JsArray>(value.inner) : JsArray{};
}
template <typename T>
inline JsArray js_array_view(const std::vector<T>& vector)
{
    JsArray out;
    out.elements->reserve(vector.size());
    for (const auto& item : vector)
    {
        out.elements->push_back(JsValue(item));
    }
    return out;
}

template <typename T>
inline void js_array_extend(JsArray& target, const T& source)
{
    const JsArray items = js_array_view(source);
    for (const auto& item : *items.elements)
    {
        target.elements->push_back(item);
    }
}

// ── Callback results ──────────────────────────────────────────────────

template <typename T>
inline JsValue js_array_value(T&& value)
{
    using U = std::remove_cvref_t<T>;
    if constexpr (std::is_void_v<U>)
    {
        return JsValue(JsUndefined{});
    }
    else if constexpr (std::is_same_v<U, JsValue>)
    {
        return std::forward<T>(value);
    }
    else if constexpr (std::is_same_v<U, bool>)
    {
        return JsValue(value);
    }
    else if constexpr (std::is_integral_v<U>)
    {
        return JsValue(static_cast<int64_t>(value));
    }
    else if constexpr (std::is_floating_point_v<U>)
    {
        return JsValue(static_cast<double>(value));
    }
    else
    {
        return JsValue(std::forward<T>(value));
    }
}

inline bool js_array_truthy(const JsValue& value) { return value.truthy(); }

inline double js_array_number(const JsValue& value)
{
    return js_to_number(value).as_double();
}

/// Invoke a callback and normalize either its value or its `void` result
/// to `JsValue` (undefined). Kept separate so `if constexpr` can inspect
/// the return type before the call expression is formed.
template <typename F, typename... Args>
inline JsValue js_array_invoke_value(F&& callback, Args&&... args)
{
    using Result = std::invoke_result_t<F&, Args...>;
    if constexpr (std::is_void_v<Result>)
    {
        std::invoke(callback, std::forward<Args>(args)...);
        return JsValue(JsUndefined{});
    }
    else
    {
        return js_array_value(std::invoke(callback, std::forward<Args>(args)...));
    }
}

// ── Callback invocation ───────────────────────────────────────────────

template <typename F>
inline JsValue js_array_call(F&& callback, const JsValue& item, int64_t index, const JsArray& array)
{
    if constexpr (std::is_invocable_v<F&, JsValue, JsValue, JsArray>)
    {
        return js_array_invoke_value(callback, item, JsValue(index), array);
    }
    else if constexpr (std::is_invocable_v<F&, JsValue, JsValue>)
    {
        return js_array_invoke_value(callback, item, JsValue(index));
    }
    else if constexpr (std::is_invocable_v<F&, JsValue>)
    {
        return js_array_invoke_value(callback, item);
    }
    else if constexpr (std::is_invocable_v<F&, JsValue, int64_t>)
    {
        return js_array_invoke_value(callback, item, index);
    }
    else if constexpr (std::is_invocable_v<F&, JsValue, double>)
    {
        return js_array_invoke_value(callback, item, static_cast<double>(index));
    }
    else if constexpr (std::is_invocable_v<F&, double, double>)
    {
        return js_array_invoke_value(
            callback, js_array_number(item), static_cast<double>(index));
    }
    else if constexpr (std::is_invocable_v<F&, double>)
    {
        return js_array_invoke_value(callback, js_array_number(item));
    }
    else
    {
        return js_array_invoke_value(callback);
    }
}

template <typename F>
inline JsValue js_array_reduce_call(
    F&& callback,
    const JsValue& accumulator,
    const JsValue& item,
    int64_t index,
    const JsArray& array)
{
    if constexpr (std::is_invocable_v<F&, JsValue, JsValue, JsValue, JsArray>)
    {
        return js_array_invoke_value(callback, accumulator, item, JsValue(index), array);
    }
    else if constexpr (std::is_invocable_v<F&, JsValue, JsValue, JsValue>)
    {
        return js_array_invoke_value(callback, accumulator, item, JsValue(index));
    }
    else if constexpr (std::is_invocable_v<F&, JsValue, JsValue>)
    {
        return js_array_invoke_value(callback, accumulator, item);
    }
    else if constexpr (std::is_invocable_v<F&, JsValue, JsValue, int64_t>)
    {
        return js_array_invoke_value(callback, accumulator, item, index);
    }
    else if constexpr (std::is_invocable_v<F&, JsValue, JsValue, double>)
    {
        return js_array_invoke_value(callback, accumulator, item, static_cast<double>(index));
    }
    else if constexpr (std::is_invocable_v<F&, double, double, double>)
    {
        return js_array_invoke_value(
            callback, js_array_number(accumulator), js_array_number(item),
            static_cast<double>(index));
    }
    else if constexpr (std::is_invocable_v<F&, double, double>)
    {
        return js_array_invoke_value(
            callback, js_array_number(accumulator), js_array_number(item));
    }
    else
    {
        return js_array_invoke_value(callback, accumulator, item);
    }
}

// ── Non-mutating methods ──────────────────────────────────────────────

template <typename Arr, typename F>
inline JsArray js_array_map(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    JsArray out;
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        out.push(js_array_call(callback, source[i], i, source));
    }
    return out;
}

template <typename Arr, typename F>
inline JsArray js_array_filter(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    JsArray out;
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        if (js_array_truthy(js_array_call(callback, source[i], i, source)))
        {
            out.push(source[i]);
        }
    }
    return out;
}

template <typename Arr, typename F>
inline JsValue js_array_for_each(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        (void)js_array_call(callback, source[i], i, source);
    }
    return JsValue(JsUndefined{});
}

template <typename Arr, typename F>
inline JsValue js_array_reduce(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    if (source.length() == 0)
    {
        return JsValue(JsUndefined{});
    }
    JsValue accumulator = source[0];
    for (int64_t i = 1; i < static_cast<int64_t>(source.length()); ++i)
    {
        accumulator = js_array_reduce_call(callback, accumulator, source[i], i, source);
    }
    return accumulator;
}

template <typename Arr, typename F, typename Init>
inline JsValue js_array_reduce(const Arr& receiver, F&& callback, Init&& initial)
{
    const JsArray source = js_array_view(receiver);
    JsValue accumulator = js_array_value(std::forward<Init>(initial));
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        accumulator = js_array_reduce_call(callback, accumulator, source[i], i, source);
    }
    return accumulator;
}

template <typename Arr, typename F>
inline JsValue js_array_find(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        if (js_array_truthy(js_array_call(callback, source[i], i, source)))
        {
            return source[i];
        }
    }
    return JsValue(JsUndefined{});
}

template <typename Arr, typename F>
inline int64_t js_array_find_index(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        if (js_array_truthy(js_array_call(callback, source[i], i, source)))
        {
            return i;
        }
    }
    return -1;
}

template <typename Arr, typename F>
inline bool js_array_some(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        if (js_array_truthy(js_array_call(callback, source[i], i, source)))
        {
            return true;
        }
    }
    return false;
}

template <typename Arr, typename F>
inline bool js_array_every(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        if (!js_array_truthy(js_array_call(callback, source[i], i, source)))
        {
            return false;
        }
    }
    return true;
}

// ── sort ──────────────────────────────────────────────────────────────

inline bool js_array_default_less(const JsValue& a, const JsValue& b)
{
    return js_to_string(a).value < js_to_string(b).value;
}

template <typename F>
inline double js_array_sort_result(F&& callback, const JsValue& a, const JsValue& b)
{
    if constexpr (std::is_invocable_v<F&, JsValue, JsValue>)
    {
        return js_array_number(js_array_value(std::invoke(callback, a, b)));
    }
    else if constexpr (std::is_invocable_v<F&, double, double>)
    {
        return js_array_number(
            js_array_value(std::invoke(callback, js_array_number(a), js_array_number(b))));
    }
    else if constexpr (std::is_invocable_v<F&, std::string, std::string>)
    {
        return js_array_number(js_array_value(
            std::invoke(callback, js_to_string(a).value, js_to_string(b).value)));
    }
    else
    {
        return 0.0;
    }
}

inline JsArray js_array_sort(JsArray array)
{
    std::stable_sort(
        array.elements->begin(), array.elements->end(), js_array_default_less);
    return array;
}

template <typename F>
inline JsArray js_array_sort(JsArray array, F&& callback)
{
    std::stable_sort(
        array.elements->begin(), array.elements->end(),
        [&](const JsValue& a, const JsValue& b)
        { return js_array_sort_result(callback, a, b) < 0.0; });
    return array;
}

template <typename T>
inline std::vector<T>& js_array_sort(std::vector<T>& array)
{
    std::stable_sort(
        array.begin(), array.end(),
        [](const T& a, const T& b)
        { return js_array_default_less(JsValue(a), JsValue(b)); });
    return array;
}

template <typename T, typename F>
inline std::vector<T>& js_array_sort(std::vector<T>& array, F&& callback)
{
    std::stable_sort(
        array.begin(), array.end(),
        [&](const T& a, const T& b)
        {
            return js_array_sort_result(callback, JsValue(a), JsValue(b)) < 0.0;
        });
    return array;
}

// ── Mutating methods ──────────────────────────────────────────────────

inline JsArray js_array_reverse(JsArray array)
{
    std::reverse(array.elements->begin(), array.elements->end());
    return array;
}

template <typename T>
inline std::vector<T>& js_array_reverse(std::vector<T>& array)
{
    std::reverse(array.begin(), array.end());
    return array;
}

inline JsValue js_array_pop(JsArray array) { return array.pop(); }

template <typename T>
inline JsValue js_array_pop(std::vector<T>& array)
{
    if (array.empty())
    {
        return JsValue(JsUndefined{});
    }
    JsValue value(array.back());
    array.pop_back();
    return value;
}

inline int64_t js_array_push(JsArray array, std::initializer_list<JsValue> items)
{
    for (const auto& item : items)
    {
        array.elements->push_back(item);
    }
    return static_cast<int64_t>(array.length());
}

inline int64_t js_array_push(JsArray array, const JsArray& items)
{
    for (const auto& item : *items.elements)
    {
        array.elements->push_back(item);
    }
    return static_cast<int64_t>(array.length());
}

inline JsValue js_array_shift(JsArray array)
{
    if (array.elements->empty())
    {
        return JsValue(JsUndefined{});
    }
    JsValue value = array.elements->front();
    array.elements->erase(array.elements->begin());
    return value;
}

template <typename T>
inline JsValue js_array_shift(std::vector<T>& array)
{
    if (array.empty())
    {
        return JsValue(JsUndefined{});
    }
    JsValue value(array.front());
    array.erase(array.begin());
    return value;
}

inline int64_t js_array_unshift(JsArray array, std::initializer_list<JsValue> items)
{
    array.elements->insert(array.elements->begin(), items.begin(), items.end());
    return static_cast<int64_t>(array.length());
}

inline int64_t js_array_unshift(JsArray array, const JsArray& items)
{
    array.elements->insert(
        array.elements->begin(), items.elements->begin(), items.elements->end());
    return static_cast<int64_t>(array.length());
}

// ── splice ────────────────────────────────────────────────────────────

inline int64_t js_array_splice_index(int64_t length, int64_t start)
{
    if (start < 0)
    {
        start = std::max<int64_t>(length + start, 0);
    }
    return std::min(start, length);
}

inline JsArray js_array_splice_range(
    JsArray array, int64_t start, int64_t delete_count, const JsArray& items)
{
    auto& elements = *array.elements;
    const int64_t length = static_cast<int64_t>(elements.size());
    const int64_t from = js_array_splice_index(length, start);
    const int64_t count =
        delete_count < 0 ? 0 : std::min(delete_count, length - from);

    JsArray removed;
    for (int64_t i = 0; i < count; ++i)
    {
        removed.elements->push_back(elements[static_cast<size_t>(from + i)]);
    }
    elements.erase(
        elements.begin() + from, elements.begin() + from + count);
    elements.insert(
        elements.begin() + from, items.elements->begin(), items.elements->end());
    return removed;
}

inline JsArray js_array_splice(JsArray array, int64_t start)
{
    return js_array_splice_range(array, start, INT64_MAX, JsArray{});
}

inline JsArray js_array_splice(JsArray array, int64_t start, int64_t delete_count)
{
    return js_array_splice_range(array, start, delete_count, JsArray{});
}

inline JsArray js_array_splice(
    JsArray array,
    int64_t start,
    int64_t delete_count,
    std::initializer_list<JsValue> items)
{
    JsArray list;
    for (const auto& item : items)
    {
        list.elements->push_back(item);
    }
    return js_array_splice_range(array, start, delete_count, list);
}

inline JsArray js_array_splice(
    JsArray array, int64_t start, int64_t delete_count, const JsArray& items)
{
    return js_array_splice_range(array, start, delete_count, items);
}

// ── fill ──────────────────────────────────────────────────────────────

inline JsArray js_array_fill_range(
    JsArray array, const JsValue& value, int64_t start, int64_t end)
{
    auto& elements = *array.elements;
    const int64_t length = static_cast<int64_t>(elements.size());
    int64_t from = start < 0 ? std::max<int64_t>(length + start, 0) : std::min(start, length);
    int64_t to = end < 0 ? std::max<int64_t>(length + end, 0) : std::min(end, length);
    for (int64_t i = from; i < to; ++i)
    {
        elements[static_cast<size_t>(i)] = value;
    }
    return array;
}

inline JsArray js_array_fill(JsArray array, const JsValue& value)
{
    return js_array_fill_range(array, value, 0, INT64_MAX);
}

inline JsArray js_array_fill(JsArray array, const JsValue& value, int64_t start)
{
    return js_array_fill_range(array, value, start, INT64_MAX);
}

inline JsArray js_array_fill(JsArray array, const JsValue& value, int64_t start, int64_t end)
{
    return js_array_fill_range(array, value, start, end);
}

// ── flat / flatMap ────────────────────────────────────────────────────

inline void js_array_flatten_into(JsArray& out, const JsArray& source, int64_t depth)
{
    for (const auto& item : *source.elements)
    {
        if (depth > 0 && item.is_array())
        {
            js_array_flatten_into(out, std::get<JsArray>(item.inner), depth - 1);
        }
        else
        {
            out.elements->push_back(item);
        }
    }
}

template <typename Arr>
inline JsArray js_array_flat(const Arr& receiver, int64_t depth)
{
    const JsArray source = js_array_view(receiver);
    JsArray out;
    js_array_flatten_into(out, source, depth);
    return out;
}

template <typename Arr>
inline JsArray js_array_flat(const Arr& receiver)
{
    return js_array_flat(receiver, 1);
}

template <typename Arr, typename F>
inline JsArray js_array_flat_map(const Arr& receiver, F&& callback)
{
    const JsArray source = js_array_view(receiver);
    JsArray out;
    for (int64_t i = 0; i < static_cast<int64_t>(source.length()); ++i)
    {
        const JsValue value = js_array_call(callback, source[i], i, source);
        if (value.is_array())
        {
            js_array_flatten_into(out, std::get<JsArray>(value.inner), 0);
        }
        else
        {
            out.elements->push_back(value);
        }
    }
    return out;
}

} // namespace morph
