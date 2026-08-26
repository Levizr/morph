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

// ── std::formatter for std::format ──
// Guarded so older libc++ (macOS Xcode <= 16) still compiles.
// std::formatter<JsBoolean> moved to the opt-in types/js_value_format.h
// (<format> costs ~1.5s parse per TU — bad for dev hot-reload).
