#pragma once
#include <string>
#include <vector>
#include <algorithm>
#include <unordered_map>
#include <cstdint>
#include <initializer_list>
#include "../../core/render_frame.h"

#ifdef MORPH_FEATURE_ANIMATION

// ── CSS `animation` + `@keyframes` runtime ─────────────────────
// Everything in this header is compiled out unless the feature define is
// set, so apps that never use animations ship zero animation code.

// Keyframe properties the runtime can interpolate.  Values are baked to
// pixels/colors at build time; transforms and % lengths stay raw CSS and
// resolve against the element's box at sample time.
enum class KeyframeProperty : uint8_t {
    None = 0,
    Opacity,
    BgColor,
    Color,
    BorderRadius,
    FontSize,
    Width,
    Height,
    Left,
    Top,
    Transform,
};

// One typed value inside a keyframe.
struct KeyframeValue {
    KeyframeProperty prop = KeyframeProperty::None;
    float v[4] = {0, 0, 0, 0};     // color [r,g,b,a] or a single float v[0]
    std::string css;               // raw CSS (Transform always; % lengths)
};

struct Keyframe {
    float offset = 0.0f;           // 0..1
    std::vector<KeyframeValue> values;
};

// `animation-direction` keywords.
enum class AnimDirection : uint8_t {
    Normal = 0,
    Reverse,
    Alternate,
    AlternateReverse,
};

// `animation-fill-mode` keywords.
enum class AnimFillMode : uint8_t {
    None = 0,
    Forwards,
    Backwards,
    Both,
};

// Immutable per-node `animation` config (parsed from CSS at build time).
struct CssAnimation {
    std::string name;
    float duration = 0.0f;         // seconds; 0 = no animation
    Easing easing = Easing::Linear;
    float delay = 0.0f;            // seconds
    float iterations = 1.0f;       // < 0 = infinite
    AnimDirection direction = AnimDirection::Normal;
    AnimFillMode fillMode = AnimFillMode::None;
    bool running = true;           // animation-play-state
};

// Per-node mutable clock for one running animation.
struct CssAnimationState {
    const CssAnimation* anim = nullptr;
    float elapsed = 0.0f;          // seconds since start (includes delay)
    bool started = false;          // delay elapsed
    bool active = false;           // producing values this frame
    // Revert bookkeeping for finite animations with fill-mode none/backwards
    // (the pre-animation style snapshot itself lives in MorphNode's
    // m_cssAnimBases, in parallel with this state vector).
    uint32_t propsMask = 0;        // KeyframeProperty bitmask
    bool baseCaptured = false;
    bool reverted = false;
};

// ── Global @keyframes registry ─────────────────────────────────
// Populated once at startup (prod: generated code; dev: IR JSON).  The dev
// runtime clears it before every reload so stale keyframes never linger.
inline std::unordered_map<std::string, std::vector<Keyframe>>& morphKeyframes() {
    static std::unordered_map<std::string, std::vector<Keyframe>> reg;
    return reg;
}

inline void morphClearKeyframes() { morphKeyframes().clear(); }

inline void morphAddKeyframe(const std::string& name, float offset,
                             std::initializer_list<KeyframeValue> values) {
    auto& kfs = morphKeyframes()[name];
    kfs.push_back(Keyframe{offset, std::vector<KeyframeValue>(values)});
    std::sort(kfs.begin(), kfs.end(),
              [](const Keyframe& a, const Keyframe& b) {
                  return a.offset < b.offset;
              });
}

// Overload used by the dev IR deserializer (no initializer_list needed).
inline void morphAddKeyframe(const std::string& name, float offset,
                             std::vector<KeyframeValue> values) {
    auto& kfs = morphKeyframes()[name];
    kfs.push_back(Keyframe{offset, std::move(values)});
    std::sort(kfs.begin(), kfs.end(),
              [](const Keyframe& a, const Keyframe& b) {
                  return a.offset < b.offset;
              });
}

// Style extension carried on MorphStyle behind the feature define.
struct AnimationStyle {
    std::vector<CssAnimation> animations;
};

#endif // MORPH_FEATURE_ANIMATION