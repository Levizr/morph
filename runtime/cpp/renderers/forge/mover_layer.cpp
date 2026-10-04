// renderers/forge/mover_layer.cpp
#include "forge/forge.h"

#include <cmath>

namespace forge
{

static float flatScreenY(const RenderFrame* frame, int idx)
{
    const auto& node = frame->nodes[(size_t)idx];
    float sy = node.y;
    for (int p = node.parentId; p >= 0;)
    {
        if (p >= (int)frame->nodes.size())
        {
            break;
        }
        const auto& parent = frame->nodes[(size_t)p];
        if (parent.scrollEnabled && parent.contentH > parent.h)
        {
            sy -= parent.scrollY;
        }
        p = parent.parentId;
    }
    return sy;
}

bool tryMoverLayer(const RenderFrame* frame, int& outIdx, MoverLayerGates& gates,
                   DamageRect& baseBox, DamageRect& newBox)
{
    outIdx = -1;
#ifndef MORPH_FEATURE_IMAGE
    // The layer composite is a blended textured quad (fringe-correct);
    // without the image path the only composite is an opaque blit that
    // halos AA edges, so decline and re-raster instead.
    (void)gates;
    (void)baseBox;
    (void)newBox;
    return false;
#else
    if (frame == nullptr)
    {
        return false;
    }
    int movers = 0;
    int candidate = -1;
    for (size_t i = 0; i < frame->nodes.size(); i++)
    {
        const auto& node = frame->nodes[i];
        if (node.animOffsetX != 0.0f || node.animOffsetY != 0.0f)
        {
            movers++;
            candidate = (int)i;
        }
    }
    if (movers != 1 || candidate < 0)
    {
        return false;
    }
    const auto& node = frame->nodes[(size_t)candidate];
    float screenY = flatScreenY(frame, candidate);
    int bw = (int)std::round(node.w);
    int bh = (int)std::round(node.h);
    int bx = (int)std::round(node.x);
    int by = (int)std::round(screenY);
    int nx = (int)std::round(node.x + node.animOffsetX);
    int ny = (int)std::round(screenY + node.animOffsetY);

    MoverLayerGates g;
    g.m_w = bw;
    g.m_h = bh;
    g.m_hasOffset = true;
    g.m_animOpacityOne = node.animOpacity == 1.0f;
    g.m_opacityOne = node.opacity == 1.0f;
    g.m_isLeaf = node.children.empty();
    g.m_hasText = node.textOpCount != 0;
    g.m_hasRadius = node.borderRadius > 0.0f;
    g.m_hasBorder =
        node.borderWidth > 0.0f && node.borderStyle != CSS::BorderStyle::None;
    g.m_overflowVisible = node.overflow == CSS::Overflow::Visible;
    g.m_scrollEnabled = node.scrollEnabled;
    g.m_hasLayoutTransition = node.hasLayoutTransition;
#ifdef MORPH_FEATURE_TRANSFORM
    g.m_transformed = node.transformSet;
#else
    g.m_transformed = false;
#endif
    g.m_singleRectOpaque = false;
    if (node.dlCount == 1 && node.dlOffset >= 0 &&
        node.dlOffset < (int)frame->drawOps.size())
    {
        const auto& op = frame->drawOps[(size_t)node.dlOffset];
        g.m_singleRectOpaque = op.type == DrawOp::Rect && op.a == 1.0f;
    }
    g.m_colorAnim = false;
    g.m_opacityAnim = false;
    g.m_radiusAnim = false;
    for (const auto& anim : frame->animations)
    {
        if (!anim.running || anim.nodeId != candidate)
        {
            continue;
        }
        switch (anim.prop)
        {
        case CompositorAnimProperty::X:
        case CompositorAnimProperty::Y:
            break;
        case CompositorAnimProperty::Opacity:
            g.m_opacityAnim = true;
            break;
        case CompositorAnimProperty::BorderRadius:
            g.m_radiusAnim = true;
            break;
        default:
            g.m_colorAnim = true;
            break;
        }
    }
    g.m_ancestorTransformed = false;
    g.m_ancestorRounded = false;
    g.m_ancestorScrolled = false;
    g.m_ancestorClipped = false;
    for (int p = node.parentId; p >= 0;)
    {
        if (p >= (int)frame->nodes.size())
        {
            break;
        }
        const auto& parent = frame->nodes[(size_t)p];
        if (parent.borderRadius > 0.0f)
        {
            g.m_ancestorRounded = true;
        }
#ifdef MORPH_FEATURE_TRANSFORM
        if (parent.transformSet)
        {
            g.m_ancestorTransformed = true;
        }
#endif
        if (parent.scrollEnabled && parent.contentH > parent.h)
        {
            g.m_ancestorScrolled = true;
        }
        if (parent.overflow != CSS::Overflow::Visible)
        {
            g.m_ancestorClipped = true;
        }
        p = parent.parentId;
    }
    g.m_overlapsSibling = false;
    g.m_multipleMovers = false;
    DamageRect oldB{bx, by, bw, bh};
    DamageRect newB{nx, ny, bw, bh};
    for (size_t j = 0; j < frame->nodes.size() && !g.m_overlapsSibling; j++)
    {
        if ((int)j == candidate)
        {
            continue;
        }
        bool ancestor = false;
        for (int p = node.parentId; p >= 0;)
        {
            if (p >= (int)frame->nodes.size())
            {
                break;
            }
            if (p == (int)j)
            {
                ancestor = true;
                break;
            }
            p = frame->nodes[(size_t)p].parentId;
        }
        if (ancestor)
        {
            continue;
        }
        const auto& other = frame->nodes[j];
        float osy = flatScreenY(frame, (int)j);
        DamageRect ob{(int)std::round(other.x), (int)std::round(osy),
                      (int)std::round(other.w), (int)std::round(other.h)};
        if (oldB.intersects(ob) || newB.intersects(ob))
        {
            g.m_overlapsSibling = true;
        }
    }
    if (!isMoverLayerEligible(g))
    {
        return false;
    }
    gates = g;
    baseBox = oldB;
    newBox = newB;
    outIdx = candidate;
    return true;
#endif
}

} // namespace forge
