#pragma once
#include <string>
#include <functional>
#include <utility>

#include "core/window_manager.h"
#include "types/js_object.h"

// Native window contract (`native.cpp`): same WID/RID integers JSX
// lowers to. Manifest-dependent calls (`open`, `navigate`) are defined
// per-project in app.cpp (they need the route table + mount switch);
// everything here delegates straight to the registry. Include via
// `morph_api.h` — never directly.
namespace app::windows {

// Creation overrides: any subset (the rest falls back exactly like
// `new Window` — overrides → file `windowConfig` → app defaults).
struct OpenConfig {
    int width = 0;
    int height = 0;
    std::string title;
    std::string id;
    JsObject data;
};

inline bool close(WID wid)
{
    return WindowManager::get().close(wid);
}

inline void show(WID wid)
{
    WindowManager::get().open(wid);
}

inline void hide(WID wid)
{
    WindowManager::get().hide(wid);
}

inline void set_title(WID wid, const std::string& title)
{
    WindowManager::get().setTitle(wid, title);
}

inline std::string title(WID wid)
{
    return WindowManager::get().title(wid);
}

inline bool closed(WID wid)
{
    return WindowManager::get().closed(wid);
}

inline void on_close(WID wid, std::function<void()> fn)
{
    WindowManager::get().onClose(wid, std::move(fn));
}

// Manifest-dependent calls: defined per-project in app.cpp (they need
// the route table + mount switch), declared here so user C++
// (`native.cpp`, included mid-TU) can call them before their
// definitions.
WID open(int rid);
WID open(int rid, const OpenConfig& cfg);
bool navigate(WID wid, int rid);
bool navigate(WID wid, int rid, const JsObject& props);

} // namespace app::windows
