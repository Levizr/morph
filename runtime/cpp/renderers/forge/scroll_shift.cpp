// renderers/forge/scroll_shift.cpp
#include "forge/scroll_shift.h"

#include <algorithm>
#include <cmath>

namespace forge
{

bool detectScrollShift(
    int x, int y, int w, int h, float oldScrollY, float newScrollY, float contentH,
    bool transformed, bool roundedClip, ScrollShift& out)
{
    if (transformed || roundedClip)
    {
        return false;
    }
    if (w <= 0 || h <= 0)
    {
        return false;
    }
    float delta = newScrollY - oldScrollY;
    if (delta == 0.0f)
    {
        return false;
    }
    int shift = static_cast<int>(std::round(delta));
    if (shift == 0)
    {
        return false;
    }
    if (std::abs(shift) >= h)
    {
        return false;
    }
    if (contentH <= static_cast<float>(h))
    {
        return false;
    }
    out.m_x = x;
    out.m_y = y;
    out.m_w = w;
    out.m_h = h;
    out.m_deltaY = shift;
    return true;
}

DamageRect exposedStrip(const ScrollShift& shift)
{
    int exposed = std::abs(shift.m_deltaY);
    if (shift.m_deltaY > 0)
    {
        return {shift.m_x, shift.m_y + shift.m_h - exposed, shift.m_w, exposed};
    }
    return {shift.m_x, shift.m_y, shift.m_w, exposed};
}

void applyScrollShift(GLuint fbo, int fbH, const ScrollShift& shift, GLuint scratchFbo)
{
    if (shift.m_w <= 0 || shift.m_h <= 0 || shift.m_deltaY == 0)
    {
        return;
    }
    int exposed = std::abs(shift.m_deltaY);
    int copyH = shift.m_h - exposed;
    if (copyH <= 0)
    {
        return;
    }
    int gx = shift.m_x;
    int topGy = fbH - (shift.m_y + shift.m_h);
    int srcY0 = 0;
    int dstY0 = 0;
    if (shift.m_deltaY > 0)
    {
        srcY0 = topGy;
        dstY0 = topGy + exposed;
    }
    else
    {
        srcY0 = topGy + exposed;
        dstY0 = topGy;
    }
    glBindFramebuffer(GL_READ_FRAMEBUFFER, fbo);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, scratchFbo);
    glReadBuffer(GL_COLOR_ATTACHMENT0);
    glBlitFramebuffer(gx, srcY0, gx + shift.m_w, srcY0 + copyH, 0, 0, shift.m_w, copyH,
                      GL_COLOR_BUFFER_BIT, GL_NEAREST);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, scratchFbo);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, fbo);
    glReadBuffer(GL_COLOR_ATTACHMENT0);
    glBlitFramebuffer(0, 0, shift.m_w, copyH, gx, dstY0, gx + shift.m_w, dstY0 + copyH,
                      GL_COLOR_BUFFER_BIT, GL_NEAREST);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, 0);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
}

} // namespace forge
