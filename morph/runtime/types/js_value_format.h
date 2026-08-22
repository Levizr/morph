// std::formatter specializations for the Js* types — OPT-IN header.
//
// <format> costs ~1.5s of parse time per translation unit, which is brutal
// for dev hot-reload where every millisecond counts. The formatter specs are
// only needed by code that actually formats a Js* value via std::format /
// std::println, so they live here instead of the js_*.h type headers.
// Include this header explicitly in those (rare) TUs:
//
//     #include "types/js_value_format.h"
#pragma once

#include <format>
#include "js_value.h"

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
