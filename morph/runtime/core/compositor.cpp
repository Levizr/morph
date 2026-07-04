#include "compositor.h"
#include <chrono>
#include <thread>
#include <cmath>

Compositor::Compositor(GLFWwindow* window, int fbWidth, int fbHeight)
    : m_window(window), m_fbWidth(fbWidth), m_fbHeight(fbHeight) {}

Compositor::~Compositor() {
    stop();
}

void Compositor::start() {
    m_running.store(true);
    m_thread = std::thread(&Compositor::run, this);
}

void Compositor::stop() {
    m_running.store(false);
    if (m_thread.joinable()) m_thread.join();
}

double Compositor::getTime() const {
    auto now = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration<double>(now).count();
}

void Compositor::run() {
    while (m_running.load()) {
        // Wait for a pending frame (CPU-only, no GL)
        while (!g_framePending.load(std::memory_order_acquire) && m_running.load()) {
            std::this_thread::yield();
        }
        if (!m_running.load()) break;

        // Consume the signal immediately so we don't re-interpolate
        // while the main thread is still rendering this frame.
        g_framePending.store(false, std::memory_order_release);

        auto* frame = g_frontFrame.load(std::memory_order_acquire);
        if (frame) {
            double now = getTime();

            // Interpolate compositor-safe animations (CPU only)
            for (auto& anim : frame->animations) {
                if (!anim.running) continue;
                float t = (float)((now - anim.startTime) / (double)anim.duration);
                if (t >= 1.0f) {
                    t = 1.0f;
                    anim.running = false;
                    if (!anim.reported) {
                        anim.reported = true;
                        g_feedbackQueue.push({anim.nodeId, anim.prop});
                    }
                }
                switch ((Easing)anim.easing) {
                    case Easing::Linear:   break;
                    case Easing::EaseIn:   t = t * t; break;
                    case Easing::EaseOut:  t = 1.0f - (1.0f - t) * (1.0f - t); break;
                    case Easing::EaseInOut:
                        t = t < 0.5f ? 2.0f * t * t : 1.0f - (float)pow(-2.0f * t + 2.0f, 2.0f) / 2.0f;
                        break;
                }
                float val = anim.from + (anim.to - anim.from) * t;
                auto& node = frame->nodes[anim.nodeId];
                switch (anim.prop) {
                    case CompositorAnimProperty::X:
                        node.animOffsetX = val - node.x; break;
                    case CompositorAnimProperty::Y:
                        node.animOffsetY = val - node.y; break;
                    case CompositorAnimProperty::Opacity:
                        node.animOpacity = val; break;
                    case CompositorAnimProperty::BgColorR: node.bgColor[0] = val; break;
                    case CompositorAnimProperty::BgColorG: node.bgColor[1] = val; break;
                    case CompositorAnimProperty::BgColorB: node.bgColor[2] = val; break;
                    case CompositorAnimProperty::BgColorA: node.bgColor[3] = val; break;
                    case CompositorAnimProperty::ColorR: node.color[0] = val; break;
                    case CompositorAnimProperty::ColorG: node.color[1] = val; break;
                    case CompositorAnimProperty::ColorB: node.color[2] = val; break;
                    case CompositorAnimProperty::ColorA: node.color[3] = val; break;
                    case CompositorAnimProperty::BorderRadius: node.borderRadius = val; break;
                }
            }

            g_frameInterpolated.store(true, std::memory_order_release);
        }
    }
}
