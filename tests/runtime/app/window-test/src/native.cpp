// ─────────────────────────────────────────────────────────────────────────────
// Window management test — user C++ driving the WindowManager registry.
//
//   JSX → C++:  openPopup() / closePopup() are plain C++ functions called
//               from App.mx buttons (same binary, no FFI).
//   C++ → JSX: app::app::setStatus() updates the main window's morphState,
//               proving native-driven UI updates alongside window ops.
//
// The popup is intentionally content-free for now: mounting a real route
// tree needs the manifest + factories (build step 3). What this proves:
// hidden creation, open(), per-window GL contexts, registry pumping of
// dynamically created windows, on_close firing (button AND user X-button),
// and safe double-close. Screenshots show both windows live.
// ─────────────────────────────────────────────────────────────────────────────
#include "morph_api.h"
#include "core/window_manager.h"

namespace {
// Codegen owns WIDs 0..N for declarative windows (this app has one: 0),
// so native code allocates from 100 up. The manifest owns this table
// once route.mx lands.
constexpr WID kPopupWid = 100;
}

void openPopup() {
    auto& wm = WindowManager::get();
    if (wm.exists(kPopupWid)) {
        wm.open(kPopupWid);
        app::app::setStatus("popup open");
        return;
    }
    auto win = std::make_shared<MorphWindow>("Popup", 320, 240, false);
    wm.registerWindow(kPopupWid, win);
    wm.registerAlias("popup", kPopupWid);
    wm.onClose(kPopupWid, [] { app::app::setStatus("popup closed"); });
    // Main-loop startup already ran startAllCompositors before this
    // window existed — a dynamic window starts its own.
    win->startCompositor(true);
    wm.open(kPopupWid);
    app::app::setStatus("popup open");
}

void closePopup() {
    if (WindowManager::get().close(kPopupWid)) {
        app::app::setStatus("popup closed");
    }
}
