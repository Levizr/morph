#include "../node.h"
#include <algorithm>

const std::vector<MorphNode*>& MorphNode::paintOrder() {
#ifdef MORPH_FEATURE_ZINDEX
    if (!m_paintOrderValid) ensurePaintOrder();
    return m_paintOrder;
#else
    return children;
#endif
}

#ifdef MORPH_FEATURE_ZINDEX

// CSS 2.1 Appendix E paint order within a stacking context, mapped to a
// single flat sibling list. Only stacking participants — positioned children
// and flex items — are affected by z-index; plain in-flow siblings always
// paint in DOM order between the negative and non-negative participant layers.
void MorphNode::ensurePaintOrder() {
    m_paintOrder.clear();

    std::vector<MorphNode*> neg, blockFlow, inlineFlow, autoLayer, pos;
    bool flexParent = (style.display == "flex");

    for (auto* c : children) {
#ifdef MORPH_FEATURE_DISPLAY_NONE
        if (c->style.display == "none") continue;
#endif
        bool participant = (c->style.position != "static") || flexParent;
        if (participant) {
            if (c->style.zIndexSet && c->style.zIndex < 0) {
                neg.push_back(c);
            } else if (c->style.zIndexSet && c->style.zIndex > 0) {
                pos.push_back(c);
            } else {
                autoLayer.push_back(c);  // z-index: auto / 0
            }
        } else {
            bool isInline = (c->style.display == "inline" ||
                             c->style.display == "inline-block" ||
                             c->type == "__text__");
            if (isInline)
                inlineFlow.push_back(c);
            else
                blockFlow.push_back(c);
        }
    }

    auto byZ = [](const MorphNode* a, const MorphNode* b) {
        return a->style.zIndex < b->style.zIndex;
    };
    std::stable_sort(neg.begin(), neg.end(), byZ);
    std::stable_sort(pos.begin(), pos.end(), byZ);

    m_paintOrder.insert(m_paintOrder.end(), neg.begin(), neg.end());
    m_paintOrder.insert(m_paintOrder.end(), blockFlow.begin(), blockFlow.end());
    m_paintOrder.insert(m_paintOrder.end(), inlineFlow.begin(), inlineFlow.end());
    m_paintOrder.insert(m_paintOrder.end(), autoLayer.begin(), autoLayer.end());
    m_paintOrder.insert(m_paintOrder.end(), pos.begin(), pos.end());

    m_paintOrderValid = true;
}

#endif
