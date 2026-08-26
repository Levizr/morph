#pragma once
#include <string>
#include <unordered_map>
#include <memory>
#include "../reactivity/signal.h"

struct SignalStore {
    template<typename T>
    morph::Signal<T>& get_or_create(const std::string& name, T default_val) {
        auto it = signals_.find(name);
        if (it != signals_.end()) {
            return *static_cast<morph::Signal<T>*>(it->second.get());
        }
        auto sig = std::make_unique<morph::Signal<T>>(default_val);
        auto* ptr = sig.get();
        signals_[name] = std::move(sig);
        return *ptr;
    }

    bool has(const std::string& name) const {
        return signals_.find(name) != signals_.end();
    }

    void erase(const std::string& name) {
        signals_.erase(name);
    }

    void clear() {
        signals_.clear();
    }

private:
    std::unordered_map<std::string, std::unique_ptr<morph::SignalBase>> signals_;
};
