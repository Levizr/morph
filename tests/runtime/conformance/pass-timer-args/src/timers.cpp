#pragma once
#include <chrono>
#include <thread>

#include "reactivity/task.h"

// Test-only helper: drive the timer queue without a window loop so the
// headless self-test can observe `setTimeout`/`setInterval` callbacks.
inline void morph_test_run_timers_ms(int ms) {
    auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(ms);
    while (std::chrono::steady_clock::now() < deadline) {
        morph::process_tasks();
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }
}
