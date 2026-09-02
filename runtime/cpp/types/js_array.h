#pragma once
#include <memory>
#include <vector>
#include <cstdint>
#include <initializer_list>

struct JsValue;
struct JsNumber;

struct JsArray {
    // shared_ptr for JS-like reference semantics (no deep copy on assignment)
    std::shared_ptr<std::vector<JsValue>> elements;

    // Defined in js_value.h AFTER JsValue is complete: the make_shared
    // instantiation requires a complete element type (clang enforces this).
    JsArray();

    JsArray(std::initializer_list<JsValue> init);

    // Copy constructor / assignment — shared_ptr copies the pointer, not the data
    JsArray(const JsArray&) = default;
    JsArray& operator=(const JsArray&) = default;

    // ── Accessors ──
    // Defined in js_value.h after JsValue is complete: the inline bodies
    // instantiate vector<JsValue> members, which need a complete element
    // type (clang rejects them from this point).
    size_t length() const;
    bool empty() const;

    // ── JS Array methods ──

    void push(const JsValue& item);
    JsValue pop();
    JsArray slice(int64_t start, int64_t end) const;
    JsArray slice(int64_t start) const;

    JsValue operator[](int64_t idx) const;
    JsValue& operator[](int64_t idx);
    JsValue operator[](const JsNumber& idx) const;
    JsValue& operator[](const JsNumber& idx);

    // ── Range-for support (for...of) ──
    auto begin() const { return elements->begin(); }
    auto end() const { return elements->end(); }
    auto begin() { return elements->begin(); }
    auto end() { return elements->end(); }
};
