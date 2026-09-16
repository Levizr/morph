#pragma once
#include <functional>
#include <map>
#include <mutex>
#include <string>
#include <vector>

#include "../types/js_value.h"

namespace morph {

// Named pub/sub bus for fire-and-forget messaging between components.
// Listeners run synchronously on the emitter's thread; emission copies the
// listener list under lock so subscribing from inside a handler is safe.
struct Channel
{
    using Listener = std::function<void(const JsValue&)>;

    void emit(const JsValue& payload)
    {
        std::vector<Listener> copy;
        {
            std::lock_guard<std::mutex> lock(m_mutex);
            copy = m_listeners;
        }
        for (auto& fn : copy)
        {
            fn(payload);
        }
    }

    size_t on(Listener cb)
    {
        std::lock_guard<std::mutex> lock(m_mutex);
        m_listeners.push_back(std::move(cb));
        return m_listeners.size() - 1;
    }

    void off(size_t id)
    {
        std::lock_guard<std::mutex> lock(m_mutex);
        if (id < m_listeners.size())
        {
            m_listeners[id] = [](const JsValue&) {};
        }
    }

private:
    std::vector<Listener> m_listeners;
    std::mutex m_mutex;
};

inline std::map<std::string, Channel>& channel_registry()
{
    static std::map<std::string, Channel> registry;
    return registry;
}

inline std::mutex& channel_registry_mutex()
{
    static std::mutex m;
    return m;
}

inline Channel& channel(const std::string& name)
{
    std::lock_guard<std::mutex> lock(channel_registry_mutex());
    return channel_registry()[name];
}

inline void clear_channels()
{
    std::lock_guard<std::mutex> lock(channel_registry_mutex());
    channel_registry().clear();
}

} // namespace morph
