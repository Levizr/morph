#pragma once
#include <string>

// Clipboard access (GLFW-backed, cross-platform). Implemented in
// core/window.cpp; widgets use these instead of touching GLFW directly.
// No-ops when no window exists yet.
namespace morph {
void setClipboard(const std::string &text);
std::string getClipboard();
}
