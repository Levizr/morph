// CSS `animation` + `@keyframes` driver (feature: MORPH_FEATURE_ANIMATION).
//
// Entire file compiles to nothing without the feature define — apps that
// never use animations ship zero animation code (dead-code elimination).
#include "../node.h"

#ifdef MORPH_FEATURE_ANIMATION

#include <cmath>
#include <cstring>
#include <cstdlib>

namespace morph_anim_detail {

// Mirrors the easing curves in style.cpp (shared with transitions).
inline float applyEasing(float t, Easing e) {
    switch (e) {
        case Easing::Linear:   return t;
        case Easing::EaseIn:   return t * t;
        case Easing::EaseOut:  return 1.0f - (1.0f - t) * (1.0f - t);
        case Easing::EaseInOut:
            return t < 0.5f ? 2.0f * t * t
                            : 1.0f - (float)pow(-2.0f * t + 2.0f, 2.0f) / 2.0f;
    }
    return t;
}

inline const KeyframeValue* findValue(const Keyframe& kf,
                                      KeyframeProperty prop) {
    for (const auto& v : kf.values)
        if (v.prop == prop) return &v;
    return nullptr;
}

// Resolve a raw % length against a base size.
inline bool resolvePct(const std::string& css, float base, float& out) {
    const char* s = css.c_str();
    char* end = nullptr;
    double v = std::strtod(s, &end);
    if (end == s) return false;
    std::string unit = end;
    if (unit == "%") { out = (float)(v / 100.0 * base); return true; }
    if (unit.empty() || unit == "px") { out = (float)v; return true; }
    return false;
}

// Resolve a raw CSS length / color / number into v[4].  Returns false when
// the value is invalid (property left untouched).
inline bool resolveValue(const KeyframeValue& kv, MorphNode* node, float out[4]) {
    switch (kv.prop) {
        case KeyframeProperty::Opacity:
        case KeyframeProperty::BorderRadius:
        case KeyframeProperty::FontSize:
        case KeyframeProperty::Width:
        case KeyframeProperty::Height:
        case KeyframeProperty::Left:
        case KeyframeProperty::Top: {
            if (!kv.css.empty()) {
                // % resolves against the parent's box for width/height/left/top,
                // and the element's own min dimension for border-radius.
                float base = 0.0f;
                switch (kv.prop) {
                    case KeyframeProperty::Width:
                    case KeyframeProperty::Left:
                        base = node->parent ? node->parent->w : node->w;
                        break;
                    case KeyframeProperty::Height:
                    case KeyframeProperty::Top:
                        base = node->parent ? node->parent->h : node->h;
                        break;
                    case KeyframeProperty::BorderRadius:
                        base = node->w < node->h ? node->w : node->h;
                        break;
                    default: break;
                }
                if (!resolvePct(kv.css, base, out[0])) return false;
            } else {
                out[0] = kv.v[0];
            }
            return true;
        }
        case KeyframeProperty::BgColor:
        case KeyframeProperty::Color:
            for (int i = 0; i < 4; i++) out[i] = kv.v[i];
            return true;
        default:
            return false;
    }
}

// Resolve a keyframe's raw transform CSS into a matrix against the node's
// own box.  Returns false when invalid (treated as no-op).
#ifdef MORPH_FEATURE_TRANSFORM
inline bool resolveTransform(const KeyframeValue& kv, MorphNode* node,
                             float out[16]) {
    MorphStyle tmp;
    if (kv.css.empty() || !morph::setCssTransform(tmp, kv.css, node->w, node->h))
        return false;
    std::memcpy(out, tmp.matrix, sizeof(float) * 16);
    return true;
}
#endif

// ── Sampling ────────────────────────────────────────────────────
// Per-property keyframe semantics (CSS Animations L2):
//   - Between two keyframes that both set the property → interpolate.
//   - After the last keyframe that sets it → hold that keyframe's value.
//   - Before any keyframe sets it → use the underlying style value.
// Duplicate offsets were merged at parse time (later block wins per prop).
struct SampledValue {
    bool applies = false;          // false → leave the style value untouched
    const KeyframeValue* left = nullptr;   // last keyframe ≤ t that sets prop
    const KeyframeValue* right = nullptr;  // first keyframe ≥ t that sets prop
    float f = 0.0f;                // interpolation factor within [left, right]
};

inline SampledValue sampleProperty(const std::vector<Keyframe>& kfs,
                                   KeyframeProperty prop, float t) {
    SampledValue out;
    const Keyframe* leftKf = nullptr;
    const Keyframe* rightKf = nullptr;
    for (const auto& kf : kfs) {
        const KeyframeValue* v = findValue(kf, prop);
        if (!v) continue;
        if (kf.offset <= t + 1e-6f) {
            out.left = v;
            leftKf = &kf;
        } else if (!out.right) {
            out.right = v;
            rightKf = &kf;
            break;
        }
    }
    if (!out.left) return out;             // before any keyframe → underlying
    out.applies = true;
    if (!out.right || leftKf == rightKf) { // hold the last defined value
        out.right = nullptr;
        return out;
    }
    float span = rightKf->offset - leftKf->offset;
    if (span <= 1e-9f) { out.right = nullptr; return out; }
    out.f = (t - leftKf->offset) / span;
    return out;
}

// Write a sampled value onto the node's style, marking dirty appropriately.
inline void applyValue(MorphNode* node, KeyframeProperty prop, const float* v) {
    switch (prop) {
#ifdef MORPH_FEATURE_OPACITY
        case KeyframeProperty::Opacity:
            node->style.opacity = v[0] < 0.0f ? 0.0f : (v[0] > 1.0f ? 1.0f : v[0]);
            node->markDirty(PaintDirty);
            break;
#endif
        case KeyframeProperty::BgColor:
            node->style.bgColor[0] = v[0];
            node->style.bgColor[1] = v[1];
            node->style.bgColor[2] = v[2];
            node->style.bgColor[3] = v[3];
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Color:
            node->style.color[0] = v[0];
            node->style.color[1] = v[1];
            node->style.color[2] = v[2];
            node->style.color[3] = v[3];
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::BorderRadius:
            node->style.borderRadius = v[0];
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::FontSize:
            node->style.fontSize = v[0];
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Width:
            node->style.explicitWidth = v[0];
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Height:
            node->style.explicitHeight = v[0];
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
#ifdef MORPH_FEATURE_POSITION
        case KeyframeProperty::Left:
            node->style.left = v[0];
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Top:
            node->style.top = v[0];
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
#endif
        default:
            break;
    }
}

// Restore one property from a pre-animation style snapshot.
inline void restoreProp(MorphNode* node, KeyframeProperty prop,
                        const MorphStyle& base) {
    switch (prop) {
        case KeyframeProperty::Opacity:
            node->style.opacity = base.opacity;
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::BgColor:
            std::memcpy(node->style.bgColor, base.bgColor, sizeof(float) * 4);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Color:
            std::memcpy(node->style.color, base.color, sizeof(float) * 4);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::BorderRadius:
            node->style.borderRadius = base.borderRadius;
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::FontSize:
            node->style.fontSize = base.fontSize;
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Width:
            node->style.explicitWidth = base.explicitWidth;
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Height:
            node->style.explicitHeight = base.explicitHeight;
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Left:
            node->style.left = base.left;
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Top:
            node->style.top = base.top;
            node->markDirty(LayoutDirty);
            node->markDirty(PaintDirty);
            break;
        case KeyframeProperty::Transform:
            std::memcpy(node->style.matrix, base.matrix, sizeof(float) * 16);
            node->style.transformSet = base.transformSet;
            node->markDirty(PaintDirty);
            break;
        default:
            break;
    }
}

// Restore every property in the mask (KeyframeProperty bit indices).
inline void restoreProps(MorphNode* node, const MorphStyle& base,
                         uint32_t mask) {
    for (int p = 0; p <= (int)KeyframeProperty::Transform; p++)
        if (mask & ((uint32_t)1 << p))
            restoreProp(node, (KeyframeProperty)p, base);
}

} // namespace morph_anim_detail

void MorphNode::updateCssAnimations(float dt) {
    auto& anims = style.animations;
    if (anims.empty()) return;

    if (m_cssAnimStates.size() != anims.size()) {
        m_cssAnimStates.resize(anims.size());
        m_cssAnimBases.resize(anims.size());
        for (size_t i = 0; i < anims.size(); i++)
            m_cssAnimStates[i].anim = &anims[i];
    }

    for (size_t i = 0; i < anims.size(); i++) {
        const CssAnimation& anim = anims[i];
        CssAnimationState& st = m_cssAnimStates[i];
        if (st.anim != &anim) {
            // Animation list changed (e.g. :hover swapped it in/out) —
            // restart the clock so hover animations begin fresh.
            st.anim = &anim;
            st.elapsed = 0.0f;
            st.started = false;
            st.baseCaptured = false;
            st.reverted = false;
            st.propsMask = 0;
        }
        st.active = false;

        if (anim.duration <= 0.0f || anim.iterations == 0.0f) continue;

        // Advance the clock (play-state: paused freezes the sample position).
        if (anim.running) st.elapsed += dt;

        auto sampleAndApply = [&](float iterPos) {
            // Snapshot the pre-animation style before this animation writes
            // anything, so a non-holding fill can revert it on finish.
            if (!st.baseCaptured) {
                m_cssAnimBases[i] = style;
                st.baseCaptured = true;
            }
            float iter = std::floor(iterPos);
            float local = iterPos - iter;
            int iterIdx = (int)iter;
            float t;
            switch (anim.direction) {
                case AnimDirection::Reverse: t = 1.0f - local; break;
                case AnimDirection::Alternate:
                    t = (iterIdx % 2 == 0) ? local : 1.0f - local;
                    break;
                case AnimDirection::AlternateReverse:
                    t = (iterIdx % 2 == 0) ? 1.0f - local : local;
                    break;
                default: t = local; break;
            }
            float te = morph_anim_detail::applyEasing(t, anim.easing);

            const auto& kfs = morphKeyframes()[anim.name];
            if (kfs.empty()) return;
            // A keyframe at exactly 100% maps to t=1; guard floating error.
            if (te > 1.0f) te = 1.0f;
            if (te < 0.0f) te = 0.0f;

            for (KeyframeProperty prop : {
                     KeyframeProperty::Opacity, KeyframeProperty::BgColor,
                     KeyframeProperty::Color, KeyframeProperty::BorderRadius,
                     KeyframeProperty::FontSize, KeyframeProperty::Width,
                     KeyframeProperty::Height, KeyframeProperty::Left,
                     KeyframeProperty::Top, KeyframeProperty::Transform}) {
                auto sv = morph_anim_detail::sampleProperty(kfs, prop, te);
                if (!sv.applies) continue;

                if (prop == KeyframeProperty::Transform) {
#ifdef MORPH_FEATURE_TRANSFORM
                    if (sv.right && !sv.left->css.empty() &&
                        !sv.right->css.empty()) {
                        // Op-list interpolation: preserves rotation
                        // wraparound (rotate(0deg)→rotate(360deg)) that
                        // matrix interpolation collapses to identity.
                        if (morph::interpolateTransformCss(
                                style, sv.left->css, sv.right->css,
                                sv.f, w, h)) {
                            style.transformSet = true;
                            markDirty(PaintDirty);
                            continue;
                        }
                    }
                    // Fallback: matrix interpolation of composed matrices.
                    float mA[16], mB[16];
                    bool hasA = morph_anim_detail::resolveTransform(
                        *sv.left, this, mA);
                    bool hasB = sv.right &&
                        morph_anim_detail::resolveTransform(*sv.right, this, mB);
                    if (!hasA) continue;              // invalid → underlying
                    if (hasB) {
                        morph::mat4Interpolate(mA, mB, sv.f, style.matrix);
                    } else {
                        std::memcpy(style.matrix, mA, sizeof(float) * 16);
                    }
                    style.transformSet = true;
                    markDirty(PaintDirty);
#endif
                    continue;
                }

                float a[4], b[4];
                if (!morph_anim_detail::resolveValue(*sv.left, this, a))
                    continue;
                float out[4];
                if (sv.right && morph_anim_detail::resolveValue(*sv.right, this, b)) {
                    for (int c = 0; c < 4; c++) out[c] = a[c] + (b[c] - a[c]) * sv.f;
                } else {
                    for (int c = 0; c < 4; c++) out[c] = a[c];   // hold
                }
                morph_anim_detail::applyValue(this, prop, out);
            }
            st.active = true;
        };

        float p = st.elapsed - anim.delay;
        if (p < 0.0f) {
            // Not started: `backwards`/`both` show the first keyframe.
            if (anim.fillMode == AnimFillMode::Backwards ||
                anim.fillMode == AnimFillMode::Both) {
                sampleAndApply(0.0f);
            }
        } else {
            float iterPos = p / anim.duration;
            if (anim.iterations >= 0.0f && iterPos >= anim.iterations) {
                // Finished: `forwards`/`both` hold the end state.  Sampling
                // just *before* the boundary reproduces the final keyframe
                // (exact `iterations` wraps around to iteration 0's first
                // frame — e.g. 1.0 would show the 0% frame instead of the
                // 100%).  A fractional tail (e.g. 2.5 iterations) stops
                // mid-iteration, like browsers.
                if (anim.fillMode == AnimFillMode::Forwards ||
                    anim.fillMode == AnimFillMode::Both) {
                    if (anim.iterations > 0.0f)
                        sampleAndApply(anim.iterations - 1e-5f);
                } else if (st.baseCaptured && !st.reverted) {
                    // fill none/backwards: the animation's effect ends —
                    // revert the properties it animated to their
                    // pre-animation values (browsers drop the effect).
                    if (st.propsMask == 0) {
                        const auto& kfs = morphKeyframes()[anim.name];
                        for (const auto& kf : kfs)
                            for (const auto& v : kf.values)
                                st.propsMask |= (uint32_t)1 << (int)v.prop;
                    }
                    morph_anim_detail::restoreProps(this, m_cssAnimBases[i], st.propsMask);
                    st.reverted = true;
                }
            } else {
                st.started = true;
                sampleAndApply(iterPos);
            }
        }
    }
}

#endif // MORPH_FEATURE_ANIMATION