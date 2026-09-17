// ─────────────────────────────────────────────────────────────────────────────
// Native interop test — user C++ code imported from App.mx.
//
//   JSX → C++:  doubleIt(), area(), callJsxFromCpp() are plain C++ functions
//               called directly from JSX event handlers (same binary, no FFI).
//   C++  → JSX: setCount()/setStatus()/setAreaVal() are generated wrappers in
//               _morph_state.h that update morphState signals (from any thread).
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
    return morph_mods::app::jsxHelper(x);
}

// C++ → JSX state, from a worker thread: set() is mutex-protected, effects
// are queued and run on the main loop — no UI thread hopping needed.
void runAsync(int start) {
    setStatus("working...");
    std::thread([start]() {
        std::this_thread::sleep_for(std::chrono::milliseconds(60));
        setCount(start + 100);
        setStatus("done");
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
//   CartStore.mx → morph_mods::cartstore, Counter.mx → morph_mods::counter.
// ─────────────────────────────────────────────────────────────────────────────

// Shared store: thin wrappers generated per project (see morph_api.h).
// Names match the JSX bindings exactly (`cart` / `setCart`).
void resetCartNative() {
    morph_mods::cartstore::setCart(0);
    morph_mods::cartstore::notify_cartChanged();
}

int getCartNative() {
    return morph_mods::cartstore::cart();
}

// Native-initiated event emission (same channel JSX subscribes to).
void announceCartNative() {
    morph_mods::cartstore::emit_cartChanged(JsObject{{"cart", morph_mods::cartstore::cart()}});
}

// Specific instance from native via its opt-in `mid` tag:
// <Counter mid="hero" /> in App.mx → morph_mods::counter::MID_HERO.
void resetHeroCounter() {
    morph_mods::counter::set_count(morph_mods::counter::MID_HERO, 0);
}

int heroCountNative() {
    return morph_mods::counter::get_count(morph_mods::counter::MID_HERO);
}