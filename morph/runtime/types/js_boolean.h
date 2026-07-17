#pragma once
#include <string>
#include "js_string.h"

struct JsBoolean {
    bool value;

    JsBoolean() : value(false) {}
    JsBoolean(bool v) : value(v) {}

    explicit operator bool() const { return value; }
    bool operator!() const { return !value; }

    bool operator==(const JsBoolean& o) const { return value == o.value; }
    bool operator!=(const JsBoolean& o) const { return value != o.value; }
    JsString toString() const { return value ? JsString("true") : JsString("false"); }
};

// ── std::formatter for std::println / std::format ──

#include <format>

template <>
struct std::formatter<JsBoolean> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsBoolean& v, auto& ctx) const {
        return std::format_to(ctx.out(), "{}", v.value ? "true" : "false");
    }
};
