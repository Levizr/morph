#pragma once
#include <chrono>
#include <thread>

// Test-only helper: keep the process alive after module initialization so
// the fetch worker threads can resume the lowered coroutines before
// `--morph-self-test` exits.
inline void morph_test_wait_ms(int ms) {
    std::this_thread::sleep_for(std::chrono::milliseconds(ms));
}
