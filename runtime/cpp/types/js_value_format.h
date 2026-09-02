// std::formatter specializations for the Js* types.
//
// Included by default via js_types.h so std::format/std::println work with
// Js* types out of the box. You can also include this header directly when
// you only need formatting without the full js_types.h umbrella.
// Define MORPH_NO_FORMAT before including js_types.h to opt-out of the
// <format> parse cost (~1.5s per TU) in hot-reload critical paths.
// Codegen adds <format> only when `${}` is used.
#pragma once

#include <format>
#include "js_value.h"

template <>
struct std::formatter<JsUndefined> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsUndefined& v, auto& ctx) const {
        return std::format_to(ctx.out(), "undefined");
    }
};

template <>
struct std::formatter<JsNull> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsNull& v, auto& ctx) const {
        return std::format_to(ctx.out(), "null");
    }
};

template <>
struct std::formatter<JsBoolean> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsBoolean& v, auto& ctx) const {
        return std::format_to(ctx.out(), "{}", v.value ? "true" : "false");
    }
};

template <>
struct std::formatter<JsNumber> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsNumber& v, auto& ctx) const {
        if (v.is_int())
            return std::format_to(ctx.out(), "{}", std::get<int64_t>(v.value));
        if (v.is_big())
            return std::format_to(ctx.out(), "{}", std::get<std::string>(v.value));
        return std::format_to(ctx.out(), "{}", std::get<double>(v.value));
    }
};

template <>
struct std::formatter<JsString> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsString& v, auto& ctx) const {
        return std::format_to(ctx.out(), "{}", v.value);
    }
};

template <>
struct std::formatter<JsValue> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsValue& v, auto& ctx) const {
        if (v.is_undefined()) return std::format_to(ctx.out(), "undefined");
        if (v.is_null()) return std::format_to(ctx.out(), "null");
        if (v.is_boolean()) return std::format_to(ctx.out(), "{}", std::get<JsBoolean>(v.inner));
        if (v.is_number()) return std::format_to(ctx.out(), "{}", std::get<JsNumber>(v.inner));
        if (v.is_string()) return std::format_to(ctx.out(), "{}", std::get<JsString>(v.inner));
        if (v.is_array()) return std::format_to(ctx.out(), "[object Array]");
        if (v.is_object()) return std::format_to(ctx.out(), "[object Object]");
        if (v.is_function()) return std::format_to(ctx.out(), "function");
        return std::format_to(ctx.out(), "undefined");
    }
};

template <>
struct std::formatter<JsObject> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsObject& v, auto& ctx) const {
        return std::formatter<JsValue>{}.format(JsValue(v), ctx);
    }
};

template <>
struct std::formatter<JsArray> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsArray& v, auto& ctx) const {
        return std::formatter<JsValue>{}.format(JsValue(v), ctx);
    }
};
