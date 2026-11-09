#pragma once
// JavaScript constructor values and `new.target` tracking.
//
// `JsConstructor` is the value class of a constructor function: a stable
// per-class identity token plus the source name. `morph::js_new` publishes
// the active target for the duration of a constructor call, so a base
// constructor sees the most-derived class that was invoked with `new`.

#include <memory>
#include <string>
#include <utility>
#include <vector>

struct JsConstructor {
    const void* identity = nullptr;
    std::string name;

    bool operator==(const JsConstructor& other) const { return identity == other.identity; }
    bool operator!=(const JsConstructor& other) const { return identity != other.identity; }
    std::string toString() const { return name; }
};

namespace morph {

/// Stable per-type identity token. Function-local statics in inline
/// templates are merged across translation units by the linker.
template <typename T>
inline const void* js_constructor_token() {
    static const char token = 0;
    return &token;
}

inline thread_local std::vector<JsConstructor> js_new_targets;

/// RAII guard that keeps the target stack balanced when a constructor throws.
struct JsNewTargetGuard {
    explicit JsNewTargetGuard(JsConstructor target) {
        js_new_targets.push_back(std::move(target));
    }
    ~JsNewTargetGuard() { js_new_targets.pop_back(); }
    JsNewTargetGuard(const JsNewTargetGuard&) = delete;
    JsNewTargetGuard& operator=(const JsNewTargetGuard&) = delete;
};

/// Construct a class instance with JavaScript `new.target` semantics. The
/// source name travels first so constructors can read the target while the
/// guard is active; constructor arguments are evaluated before the guard is
/// installed, matching JS evaluation order.
template <typename T, typename... Args>
std::shared_ptr<T> js_new(const char* name, Args&&... args) {
    JsNewTargetGuard guard{JsConstructor{js_constructor_token<T>(), name}};
    return std::make_shared<T>(std::forward<Args>(args)...);
}

/// `js_new` for single-owner escape slots that need a `unique_ptr`.
template <typename T, typename... Args>
std::unique_ptr<T> js_new_unique(const char* name, Args&&... args) {
    JsNewTargetGuard guard{JsConstructor{js_constructor_token<T>(), name}};
    return std::make_unique<T>(std::forward<Args>(args)...);
}

} // namespace morph
