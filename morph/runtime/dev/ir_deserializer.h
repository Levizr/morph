#pragma once
#include <cstring>
#include "json_parser.h"
#include "../core/node.h"
#include "../widgets/morph_rect.h"
#include "../widgets/morph_text.h"
#include "../widgets/morph_button.h"
#include "../widgets/morph_image.h"

static void deleteNodeTree(MorphNode* node) {
    if (!node) return;
    for (auto* c : node->children) deleteNodeTree(c);
    delete node;
}

// ── Style inheritance helpers ──────────────────────────────────
struct InheritedStyle {
    float color[4] = {0,0,0,1};
    float fontSize = 16.0f;
    std::string fontWeight = "normal";
    std::string textAlign = "left";
};

static bool isDefaultColor(const float* c) {
    return c[0] == 0.0f && c[1] == 0.0f && c[2] == 0.0f && c[3] == 1.0f;
}

// Apply inherited values when child's value matches default
static void inheritStyle(MorphStyle& s, const InheritedStyle& parent) {
    if (isDefaultColor(s.color) && !isDefaultColor(parent.color))
        memcpy(s.color, parent.color, sizeof(float) * 4);
    if (s.fontSize == 16.0f && parent.fontSize != 16.0f)
        s.fontSize = parent.fontSize;
    if (s.fontWeight == "normal" && parent.fontWeight != "normal")
        s.fontWeight = parent.fontWeight;
    if (s.textAlign == "left" && parent.textAlign != "left")
        s.textAlign = parent.textAlign;
}

// Compute the resolved style to pass to children
static InheritedStyle resolvedStyle(const MorphStyle& s, const InheritedStyle& parent) {
    InheritedStyle r = parent;
    if (!isDefaultColor(s.color)) memcpy(r.color, s.color, sizeof(float) * 4);
    if (s.fontSize != 16.0f) r.fontSize = s.fontSize;
    if (s.fontWeight != "normal") r.fontWeight = s.fontWeight;
    if (s.textAlign != "left") r.textAlign = s.textAlign;
    return r;
}

// ── Style field helpers ────────────────────────────────────────
static void setColorFromJson(float* dst, const JsonValue& arr) {
    if (arr.type() == JsonType::Array && arr.size() >= 4) {
        dst[0] = arr[0].asFloat();
        dst[1] = arr[1].asFloat();
        dst[2] = arr[2].asFloat();
        dst[3] = arr[3].asFloat();
    }
}

static void setFloatOpt(float& dst, const JsonValue& val, float sentinel = -1.0f) {
    if (!val.isNull()) dst = val.asFloat();
    else dst = sentinel;
}

static void setFloat4(float* dst, const JsonValue& arr) {
    if (arr.type() == JsonType::Array && arr.size() >= 4) {
        dst[0] = arr[0].asFloat();
        dst[1] = arr[1].asFloat();
        dst[2] = arr[2].asFloat();
        dst[3] = arr[3].asFloat();
    }
}

static void applyStyle(MorphStyle& s, const JsonValue& styleVal) {
    if (styleVal.type() != JsonType::Object) return;

    setColorFromJson(s.bgColor, styleVal["bg_color"]);
    setColorFromJson(s.color, styleVal["color"]);

    setFloatOpt(s.explicitWidth, styleVal["width"]);
    setFloatOpt(s.explicitHeight, styleVal["height"]);
    setFloatOpt(s.minWidth, styleVal["min_width"]);
    setFloatOpt(s.maxWidth, styleVal["max_width"]);
    setFloatOpt(s.minHeight, styleVal["min_height"]);
    setFloatOpt(s.maxHeight, styleVal["max_height"]);

    setFloat4(s.margin, styleVal["margin"]);
    if (styleVal.has("margin_auto") && styleVal["margin_auto"].type() == JsonType::Array && styleVal["margin_auto"].size() >= 4) {
        for (int i = 0; i < 4; i++)
            s.marginAuto[i] = styleVal["margin_auto"][i].asBool();
    }
    setFloat4(s.padding, styleVal["padding"]);

    if (!styleVal["border_radius"].isNull())
        s.borderRadius = styleVal["border_radius"].asFloat();
    if (!styleVal["font_size"].isNull())
        s.fontSize = styleVal["font_size"].asFloat();

    if (!styleVal["font_weight"].isNull())
        s.fontWeight = styleVal["font_weight"].asString();
    if (!styleVal["text_align"].isNull())
        s.textAlign = styleVal["text_align"].asString();
    if (!styleVal["display"].isNull())
        s.display = styleVal["display"].asString();
    if (!styleVal["overflow"].isNull())
        s.overflow = styleVal["overflow"].asString();
    if (!styleVal["position"].isNull())
        s.position = styleVal["position"].asString();
    if (!styleVal["cursor"].isNull())
        s.cursor = styleVal["cursor"].asString();
    if (!styleVal["box_sizing"].isNull())
        s.boxSizing = styleVal["box_sizing"].asString();
    if (!styleVal["border_style"].isNull())
        s.borderStyle = styleVal["border_style"].asString();
    if (!styleVal["flex_wrap"].isNull())
        s.flexWrap = styleVal["flex_wrap"].asString();

    if (!styleVal["flex_dir"].isNull())
        s.flexDirection = styleVal["flex_dir"].asString();
    if (!styleVal["justify_content"].isNull())
        s.justifyContent = styleVal["justify_content"].asString();
    if (!styleVal["align_items"].isNull())
        s.alignItems = styleVal["align_items"].asString();

    if (!styleVal["gap"].isNull())
        s.gap = styleVal["gap"].asFloat();

    if (!styleVal["left"].isNull())
        s.left = styleVal["left"].asFloat();
    if (!styleVal["right"].isNull())
        s.right = styleVal["right"].asFloat();
    if (!styleVal["top"].isNull())
        s.top = styleVal["top"].asFloat();
    if (!styleVal["bottom"].isNull())
        s.bottom = styleVal["bottom"].asFloat();

    if (!styleVal["scrollbar_width"].isNull())
        s.scrollbarWidth = styleVal["scrollbar_width"].asFloat();
    if (!styleVal["scrollbar_border_radius"].isNull())
        s.scrollbarBorderRadius = styleVal["scrollbar_border_radius"].asFloat();

    setColorFromJson(s.scrollbarTrackColor, styleVal["scrollbar_track_color"]);
    setColorFromJson(s.scrollbarThumbColor, styleVal["scrollbar_thumb_color"]);

    if (!styleVal["border_width"].isNull())
        s.borderWidth = styleVal["border_width"].asFloat();
    setColorFromJson(s.borderColor, styleVal["border_color"]);
}

// ── Node deserialization with inheritance ──────────────────────
static MorphNode* deserializeNode(const JsonValue& val,
                                   const InheritedStyle& parentStyle = InheritedStyle()) {
    std::string type = val["type"].asString();
    MorphNode* node = nullptr;

    if (type == "__text__") {
        std::string text;
        if (val.has("text") && !val["text"].isNull())
            text = val["text"].asString();
        node = new TextNode(text);
    } else if (type == "button") {
        node = new RectNode(0, 0, 0, 0);
    } else if (type == "img") {
        std::string src, alt;
        if (val.has("attrs") && val["attrs"].type() == JsonType::Object) {
            auto& a = val["attrs"];
            if (a.has("src") && !a["src"].isNull()) src = a["src"].asString();
            if (a.has("alt") && !a["alt"].isNull()) alt = a["alt"].asString();
        }
        node = new ImageNode(src, alt);
    } else {
        node = new RectNode(0, 0, 0, 0);
    }
    node->type = type;

    // Apply style from JSON
    if (val.has("style"))
        applyStyle(node->style, val["style"]);

    // Apply style inheritance (same logic as codegen)
    // For text nodes, skip color inheritance — text reads parent's
    // color at render time to support hover animation.
    if (type == "__text__") {
        InheritedStyle noColor = parentStyle;
        noColor.color[0] = 0; noColor.color[1] = 0;
        noColor.color[2] = 0; noColor.color[3] = 1;
        inheritStyle(node->style, noColor);
        // After noColor inheritance, any non-default color was explicitly set
        // in CSS/JSON (since noColor prevents parent color inheritance).
        if (!isDefaultColor(node->style.color))
            static_cast<TextNode*>(node)->m_colorOverridden = true;
    } else {
        inheritStyle(node->style, parentStyle);
    }

    // Snapshot base style (for hover restore)
    node->m_baseStyle = node->style;

    // Apply hover style if present
    if (val.has("hover_style") && val["hover_style"].type() == JsonType::Object) {
        node->hoverStyle = new MorphStyle();
        // Copy base as starting point, then override with hover properties
        *node->hoverStyle = node->style;
        applyStyle(*node->hoverStyle, val["hover_style"]);
    }

    // Compute resolved style for children
    InheritedStyle resolved = resolvedStyle(node->style, parentStyle);

    // Set initial positions (layout recalculates them anyway)
    if (val.has("x") && !val["x"].isNull()) node->x = val["x"].asFloat();
    if (val.has("y") && !val["y"].isNull()) node->y = val["y"].asFloat();
    if (val.has("w") && !val["w"].isNull()) node->w = val["w"].asFloat();
    if (val.has("h") && !val["h"].isNull()) node->h = val["h"].asFloat();

    // Recurse children with resolved parent style
    if (val.has("children") && val["children"].type() == JsonType::Array) {
        for (size_t i = 0; i < val["children"].size(); i++) {
            MorphNode* child = deserializeNode(val["children"][i], resolved);
            if (child) node->addChild(child);
        }
    }

    return node;
}

// ── Window config ──────────────────────────────────────────────
struct DevWindowConfig {
    std::string title;
    int width = 800;
    int height = 600;
    bool visible = true;
};

static bool parseIR(const JsonValue& root, MorphNode*& outRoot,
                    DevWindowConfig& config) {
    outRoot = nullptr;

    if (root.type() != JsonType::Object) return false;
    if (!root.has("windows")) return false;

    auto& windows = root["windows"];
    if (windows.type() != JsonType::Array || windows.size() == 0)
        return false;

    auto& win = windows[0];

    if (win.has("title") && !win["title"].isNull())
        config.title = win["title"].asString();
    if (win.has("width") && !win["width"].isNull())
        config.width = win["width"].asInt();
    if (win.has("height") && !win["height"].isNull())
        config.height = win["height"].asInt();
    if (win.has("visible") && !win["visible"].isNull())
        config.visible = win["visible"].asBool();

    // Build node tree from window's top-level nodes
    if (win.has("nodes") && win["nodes"].type() == JsonType::Array) {
        std::vector<MorphNode*> topLevel;
        InheritedStyle rootStyle;
        for (size_t i = 0; i < win["nodes"].size(); i++) {
            MorphNode* n = deserializeNode(win["nodes"][i], rootStyle);
            if (n) topLevel.push_back(n);
        }

        if (topLevel.empty()) return true;

        if (topLevel.size() == 1) {
            outRoot = topLevel[0];
        } else {
            auto* root = new RectNode(0, 0, (float)config.width, (float)config.height);
            for (auto* c : topLevel) root->addChild(c);
            outRoot = root;
        }
    }

    return true;
}
