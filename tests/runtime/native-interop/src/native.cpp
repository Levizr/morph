// ─────────────────────────────────────────────────────────────────────────────
// Native interop test — user C++ code imported from App.mx.
//
//   JSX → C++:  doubleIt(), area(), callJsxFromCpp() are plain C++ functions
//               called directly from JSX event handlers (same binary, no FFI).
//   C++  → JSX: app::app::setCount()/setStatus()/setAreaVal() are generated
//               wrappers in _morph_state.h that update morphState signals
//               (from any thread). Every call is namespace-qualified —
//               generated wrappers are never bare globals.
//               callJsxFromCpp() calls back into the JSX-defined jsxHelper().
// ─────────────────────────────────────────────────────────────────────────────
#include "morph_api.h"

#include <thread>
#include <chrono>

int doubleIt(int x) {
    return x * 2;
}

double area(double w, double h) {
    return w * h;
}

// C++ → JSX: calls a function defined in App.mx (declared namespaced in
// morph_api.h — always the defining module's namespace).
int callJsxFromCpp(int x) {
    return app::app::jsxHelper(x);
}

// C++ → JSX state, from a worker thread: set() is mutex-protected, effects
// are queued and run on the main loop — no UI thread hopping needed.
void runAsync(int start) {
    app::app::setStatus("working...");
    std::thread([start]() {
        std::this_thread::sleep_for(std::chrono::milliseconds(60));
        app::app::setCount(start + 100);
        app::app::setStatus("done");
    }).detach();
}

// Prove external headers can be included too (system includes only here).
#include <algorithm>
#include <vector>
std::vector<int> sorted(std::vector<int> v) {
    std::sort(v.begin(), v.end());
    return v;
}

// ─────────────────────────────────────────────────────────────────────────────
// Generated-API interop (morph_api.h): shared store, events, mid instances.
// Namespaces are human-computable from file paths:
//   CartStore.mx → app::cartstore, Counter.mx → app::counter,
//   App.mx (entry) → app::app.
// ─────────────────────────────────────────────────────────────────────────────

// Shared store: thin wrappers generated per project (see morph_api.h).
// Names match the JSX bindings exactly (`cart` / `setCart`).
void resetCartNative() {
    app::cartstore::setCart(0);
    app::cartstore::notify_cartChanged();
}

int getCartNative() {
    return app::cartstore::cart();
}

// Native-initiated event emission (same channel JSX subscribes to).
void announceCartNative() {
    app::cartstore::emit_cartChanged(JsObject{{"cart", app::cartstore::cart()}});
}

// Specific instance from native via its opt-in `mid` tag:
// <Counter mid="hero" /> in App.mx → app::counter::MID_HERO.
// Accessor names match the JSX suffix names exactly (`setCount`/`count`).
void resetHeroCounter() {
    app::counter::setCount(app::counter::MID_HERO, 0);
}

int heroCountNative() {
    return app::counter::count(app::counter::MID_HERO);
}
