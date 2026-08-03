#pragma once
#include <mutex>
#include <cmath>
#include <vector>
#include <functional>
#include <string>
#include <cstddef>
#include "../types/js_types.h"
namespace morph {

// ── Thread-local effect tracking for auto-subscription ──
struct EffectNode;
struct EffectContext {
    EffectNode* current = nullptr;
};
extern thread_local EffectContext g_effect_ctx;

// ── Effect node ──
struct EffectNode {
    std::function<void()> fn;
    std::function<void()> cleanup_fn;
    bool pending = false;
    bool dead = false;

    struct Dep {
        struct SignalBase* sig;
    };
    std::vector<Dep> deps;

    void run();
    void cleanup();
};

// ── SignalBase (type-erased subscriber management) ──
struct SignalBase {
    std::vector<EffectNode*> subscribers_;
    std::mutex m_mutex;

    void subscribe(EffectNode* node) {
        for (auto* s : subscribers_)
            if (s == node) return;
        subscribers_.push_back(node);
    }

    void unsubscribe(EffectNode* node) {
        for (size_t i = 0; i < subscribers_.size(); i++) {
            if (subscribers_[i] == node) {
                subscribers_[i] = subscribers_.back();
                subscribers_.pop_back();
                return;
            }
        }
    }

    void notify_all();
};

// ── Signal<T> ──
template<typename T>
struct Signal : SignalBase {
    T value_;

    explicit Signal(T v) : value_(v) {}

    T get() {
        if (g_effect_ctx.current) {
            std::lock_guard<std::mutex> lock(m_mutex);
            subscribe(g_effect_ctx.current);
        }
        return value_;
    }

    void set(T v) {
        std::lock_guard<std::mutex> lock(m_mutex);
        value_ = v;
        notify_all();
    }

    T& operator*() { return value_; }
};

// ── create_effect ──
EffectNode* create_effect(std::function<void()> fn);

// ── Run all pending effects ──
void run_pending_effects();

// ── Destroy all effects (cleanup at shutdown) ──
void destroy_all_effects();

// ── to_string helper for reactive text ──
inline std::string str(const std::string& s) { return s; }

// Format a double the way users expect in a UI: "8" (not "8.000000"),
// "2.5" (not "2.500000") and "Error" for non-finite results instead of
// leaking "nan"/"inf" into the interface.
inline std::string fmt_double(double v) {
    if (std::isnan(v) || std::isinf(v)) return "Error";
    std::string s = std::to_string(v);
    auto dot = s.find('.');
    if (dot != std::string::npos) {
        auto last = s.find_last_not_of('0');   // last non-trailing-zero char
        if (last == std::string::npos) {
            s = "0";
        } else if (last < dot) {
            s.erase(dot);                       // fractional part is all zeros
        } else {
            s.erase(last + 1);                  // drop trailing fractional zeros
        }
        if (!s.empty() && s.back() == '.') s.pop_back();
    }
    if (s == "-0") return "0";
    return s;
}
inline std::string str(int64_t v) { return std::to_string(v); }
inline std::string str(int v) { return std::to_string(v); }
inline std::string str(unsigned v) { return std::to_string(v); }
inline std::string str(long long v) { return std::to_string(v); }
inline std::string str(unsigned long long v) { return std::to_string(v); }
inline std::string str(float v) { return fmt_double((double)v); }
inline std::string str(double v) { return fmt_double(v); }
inline std::string str(JsValue v) { return v.toString().to_std_string(); }
inline std::string str(JsString v) { return v.value; }
inline std::string str(const char* s) { return std::string(s); }
inline std::string str(bool v) { return v ? "true" : "false"; }
template<typename T> std::string str(T v) { return std::to_string(v); }

} // namespace morph
