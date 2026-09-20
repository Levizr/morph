// ─────────────────────────────────────────────────────────────────────────────
// Route mount proof — drives the generated mount functions directly.
//
// The translator lowering (`new Window`, `navigate`) lands later; until
// then this exercises the same machinery it will call: mount a route
// with props into a dynamic window, unmount it, verify independence.
// Settings carries a prop (userId), state (tab), and a handler — the
// per-mount Context proven live: two mounts would hold separate state.
// ─────────────────────────────────────────────────────────────────────────────
#include "morph_api.h"
#include "core/window_manager.h"

namespace {
// Codegen owns WIDs 0..N for declarative windows; native allocates up.
constexpr WID kSettingsWin = 100;
}

void mountSettings() {
    auto& wm = WindowManager::get();
    if (wm.exists(kSettingsWin)) {
        wm.open(kSettingsWin);
        return;
    }
    auto win = std::make_shared<MorphWindow>("Settings", 500, 400, false);
    wm.registerWindow(kSettingsWin, win);
    win->startCompositor(true);
    JsObject props;
    props.set("userId", JsValue(42));
    // Generated mount: props convert once in the prologue (plain C++
    // downstream), state mounts fresh, effects scope to the context.
    app::routes::settings::mount_kSettings(win.get(), kSettingsWin, props);
    wm.open(kSettingsWin);
    app::app::setStatus("settings mounted");
}

void unmountSettings() {
    app::routes::settings::unmount_kSettings(kSettingsWin);
    WindowManager::get().close(kSettingsWin);
    app::app::setStatus("settings gone");
}
