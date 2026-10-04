// renderers/forge/scroll_shift.h
#pragma once

#include "damage.h"
#include "vendor/glad/glad.h"

namespace forge
{

struct ScrollShift
{
    int m_x = 0;
    int m_y = 0;
    int m_w = 0;
    int m_h = 0;
    int m_deltaY = 0;
    // Full container width including the scrollbar column. The shift
    // itself covers only the content area (m_w excludes the scrollbar:
    // shifting translucent thumb pixels would ghost outside thumb
    // damage); m_fullW survives for conservative fallbacks.
    int m_fullW = 0;
};

bool detectScrollShift(
    int x, int y, int w, int h, float oldScrollY, float newScrollY, float contentH,
    bool transformed, bool roundedClip, ScrollShift& out);

DamageRect exposedStrip(const ScrollShift& shift);

// GPU shift of cached tile content. The copy runs through a caller-owned
// scratch FBO: same-surface overlapping blits are driver-undefined (some
// drivers copy bottom-up, smearing already-shifted rows back over the
// source), while two non-overlapping blits are exact everywhere.
// scratchFbo must be at least (shift.m_w x shift.m_h); scratch origin is
// its bottom-left corner.
void applyScrollShift(GLuint fbo, int fbH, const ScrollShift& shift, GLuint scratchFbo);

} // namespace forge
