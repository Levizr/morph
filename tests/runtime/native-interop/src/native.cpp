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

// C++ → JSX: calls a function defined in App.mx (declared in _morph_state.h).
int callJsxFromCpp(int x) {
    return jsxHelper(x);
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