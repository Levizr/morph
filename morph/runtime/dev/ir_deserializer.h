#pragma once
#include <cstring>
#include <cctype>
#include <vector>
#include "json_parser.h"
#include "node_registry.h"
#include "../core/node.h"
#include "../ui/rect.h"
#include "../ui/text.h"
#include "../ui/button.h"
#include "../ui/image.h"

static void deleteNodeTree(MorphNode* node) {
    delete node;
}

// ── State var info from IR JSON ──
struct StateVarInfo {
    std::string getter;
    std::string setter;
    std::string init;
    std::string type; // deduced from init
};

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

#ifdef MORPH_FEATURE_ANIMATION
// ── @keyframes / animation deserialization (feature: animation) ─

static KeyframeProperty keyframePropFor(const std::string& cssProp) {
    if (cssProp == "opacity") return KeyframeProperty::Opacity;
    if (cssProp == "background-color") return KeyframeProperty::BgColor;
    if (cssProp == "color") return KeyframeProperty::Color;
    if (cssProp == "border-radius") return KeyframeProperty::BorderRadius;
    if (cssProp == "font-size") return KeyframeProperty::FontSize;
    if (cssProp == "width") return KeyframeProperty::Width;
    if (cssProp == "height") return KeyframeProperty::Height;
    if (cssProp == "left") return KeyframeProperty::Left;
    if (cssProp == "top") return KeyframeProperty::Top;
    if (cssProp == "transform") return KeyframeProperty::Transform;
    return KeyframeProperty::None;
}

static void pushKeyframeValue(std::vector<KeyframeValue>& values,
                              KeyframeProperty prop, float a = 0.0f,
                              float b = 0.0f, float c = 0.0f, float d = 0.0f,
                              const std::string& css = "") {
    KeyframeValue v;
    v.prop = prop;
    v.v[0] = a; v.v[1] = b; v.v[2] = c; v.v[3] = d;
    v.css = css;
    values.push_back(std::move(v));
}

// Extract the animatable values from a serialized (partial) keyframe style
// into KeyframeValues.  The IR serializer emits only the fields the keyframe
// explicitly declares (presence == declared), so no default-value heuristics
// here — `opacity: 1` and `background-color: #000` are legit declarations.
// Raw (% / transform) values come from `raw`.
static void keyframeValuesFromStyle(std::vector<KeyframeValue>& values,
                                    const JsonValue& styleVal,
                                    const JsonValue& rawVal) {
    if (styleVal.type() == JsonType::Object) {
        const JsonValue& o = styleVal["opacity"];
        if (!o.isNull())
            pushKeyframeValue(values, KeyframeProperty::Opacity, o.asFloat());
        const JsonValue& bg = styleVal["bg_color"];
        if (bg.type() == JsonType::Array && bg.size() >= 4)
            pushKeyframeValue(values, KeyframeProperty::BgColor,
                              bg[0].asFloat(), bg[1].asFloat(),
                              bg[2].asFloat(), bg[3].asFloat());
        const JsonValue& col = styleVal["color"];
        if (col.type() == JsonType::Array && col.size() >= 4)
            pushKeyframeValue(values, KeyframeProperty::Color,
                              col[0].asFloat(), col[1].asFloat(),
                              col[2].asFloat(), col[3].asFloat());
        const JsonValue& br = styleVal["border_radius"];
        if (!br.isNull())
            pushKeyframeValue(values, KeyframeProperty::BorderRadius, br.asFloat());
        const JsonValue& fs = styleVal["font_size"];
        if (!fs.isNull())
            pushKeyframeValue(values, KeyframeProperty::FontSize, fs.asFloat());
        const JsonValue& w = styleVal["width"];
        if (!w.isNull()) pushKeyframeValue(values, KeyframeProperty::Width, w.asFloat());
        const JsonValue& h = styleVal["height"];
        if (!h.isNull()) pushKeyframeValue(values, KeyframeProperty::Height, h.asFloat());
        const JsonValue& l = styleVal["left"];
        if (!l.isNull()) pushKeyframeValue(values, KeyframeProperty::Left, l.asFloat());
        const JsonValue& t = styleVal["top"];
        if (!t.isNull()) pushKeyframeValue(values, KeyframeProperty::Top, t.asFloat());
    }
    // Raw (unresolved) values: transforms always; % lengths.
    if (rawVal.type() == JsonType::Object) {
        for (size_t i = 0; i < rawVal.size(); i++) {
            const std::string& prop = rawVal.key(i);
            KeyframeProperty kp = keyframePropFor(prop);
            if (kp == KeyframeProperty::None) continue;
            pushKeyframeValue(values, kp, 0, 0, 0, 0, rawVal.value(i).asString());
        }
    }
}

static bool parseIRKeyframes(const JsonValue& root) {
    // Reset the registry on every (re)load so hot-reloads never carry
    // stale keyframes from a previous version of the stylesheet.
    morphClearKeyframes();
    if (root.type() != JsonType::Object || !root.has("windows"))
        return false;
    auto& win = root["windows"];
    if (win.type() != JsonType::Array || win.size() == 0) return false;
    if (!win[0].has("keyframes") || win[0]["keyframes"].type() != JsonType::Object)
        return true;  // no keyframes — not an error
    auto& kfs = win[0]["keyframes"];
    for (size_t i = 0; i < kfs.size(); i++) {
        const std::string& name = kfs.key(i);
        auto& list = kfs.value(i);
        if (list.type() != JsonType::Array) continue;
        for (size_t j = 0; j < list.size(); j++) {
            auto& kf = list[j];
            float offset = kf.has("offset") && !kf["offset"].isNull()
                ? kf["offset"].asFloat() : 0.0f;
            std::vector<KeyframeValue> values;
            if (kf.has("style"))
                keyframeValuesFromStyle(values, kf["style"], kf["raw"]);
            morphAddKeyframe(name, offset, std::move(values));
        }
    }
    return true;
}

static void deserializeAnimations(MorphNode* node, const JsonValue& val,
                                  bool hover = false) {
    const char* key = hover ? "hover_animations" : "animations";
    if (!val.has(key) || val[key].type() != JsonType::Array)
        return;
    std::vector<CssAnimation>& dst = hover
        ? node->hoverStyle->animations : node->style.animations;
    auto& arr = val[key];
    for (size_t i = 0; i < arr.size(); i++) {
        auto& a = arr[i];
        CssAnimation ca;
        if (a.has("name") && !a["name"].isNull()) ca.name = a["name"].asString();
        if (a.has("duration") && !a["duration"].isNull()) ca.duration = a["duration"].asFloat();
        if (a.has("delay") && !a["delay"].isNull()) ca.delay = a["delay"].asFloat();
        if (a.has("iterations") && !a["iterations"].isNull())
            ca.iterations = a["iterations"].asFloat();
        if (a.has("running") && !a["running"].isNull()) ca.running = a["running"].asBool();
        if (a.has("easing") && !a["easing"].isNull()) {
            std::string e = a["easing"].asString();
            if (e == "linear") ca.easing = Easing::Linear;
            else if (e == "ease-in") ca.easing = Easing::EaseIn;
            else if (e == "ease-out") ca.easing = Easing::EaseOut;
            else if (e == "ease-in-out") ca.easing = Easing::EaseInOut;
        }
        if (a.has("direction") && !a["direction"].isNull()) {
            std::string d = a["direction"].asString();
            if (d == "reverse") ca.direction = AnimDirection::Reverse;
            else if (d == "alternate") ca.direction = AnimDirection::Alternate;
            else if (d == "alternate-reverse") ca.direction = AnimDirection::AlternateReverse;
        }
        if (a.has("fill_mode") && !a["fill_mode"].isNull()) {
            std::string f = a["fill_mode"].asString();
            if (f == "forwards") ca.fillMode = AnimFillMode::Forwards;
            else if (f == "backwards") ca.fillMode = AnimFillMode::Backwards;
            else if (f == "both") ca.fillMode = AnimFillMode::Both;
        }
        if (a.has("play_state") && !a["play_state"].isNull())
            ca.running = a["play_state"].asString() == "running";
        dst.push_back(std::move(ca));
    }
}
#endif // MORPH_FEATURE_ANIMATION

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

#ifdef MORPH_FEATURE_TRANSFORM
    // Serialized only when the feature is compiled in; a null value means
    // "no transform" (identity).
    if (!styleVal["transform_matrix"].isNull()) {
        const JsonValue& m = styleVal["transform_matrix"];
        if (m.type() == JsonType::Array && m.size() >= 16) {
            for (int i = 0; i < 16; i++)
                s.matrix[i] = m[i].asFloat();
            s.transformSet = true;
        }
    }
    // transform-origin as fractions of the border box (default center).
    if (!styleVal["transform_origin"].isNull()) {
        const JsonValue& o = styleVal["transform_origin"];
        if (o.type() == JsonType::Array && o.size() >= 2) {
            s.originX = o[0].asFloat();
            s.originY = o[1].asFloat();
            s.originSet = true;
        }
    }
#endif
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

    if (!styleVal["flex_grow"].isNull())
        s.flexGrow = styleVal["flex_grow"].asFloat();
    if (!styleVal["flex_shrink"].isNull())
        s.flexShrink = styleVal["flex_shrink"].asFloat();
    if (!styleVal["flex_basis"].isNull())
        s.flexBasis = styleVal["flex_basis"].asString();

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
#ifdef MORPH_FEATURE_ZINDEX
    if (!styleVal["z_index"].isNull()) {
        s.zIndex = styleVal["z_index"].asInt();
        s.zIndexSet = true;
    }
#endif
#ifdef MORPH_FEATURE_OPACITY
    if (!styleVal["opacity"].isNull())
        s.opacity = styleVal["opacity"].asFloat();
#endif

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
                                   NodeRegistry& registry,
                                   const InheritedStyle& parentStyle = InheritedStyle()) {
    std::string type = val["type"].asString();
    MorphNode* node = nullptr;

    if (type == "__conditional__") {
        node = new RectNode(0.0f, 0.0f, 0.0f, 0.0f);
    } else if (type == "__text__") {
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
    if (val.has("id") && !val["id"].isNull())
        node->nodeId = val["id"].asString();
    if (!node->nodeId.empty())
        registry.put(node->nodeId, node);

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
        bool colorWasDefault = isDefaultColor(node->style.color);
        inheritStyle(node->style, parentStyle);
        if (colorWasDefault && !isDefaultColor(node->style.color))
            node->m_colorInherited = true;
    }

    // Apply hover style if present
    if (val.has("hover_style") && val["hover_style"].type() == JsonType::Object) {
        node->hoverStyle = new MorphStyle();
        // Copy base as starting point, then override with hover properties
        *node->hoverStyle = node->style;
        applyStyle(*node->hoverStyle, val["hover_style"]);
    }
#ifdef MORPH_FEATURE_ANIMATION
    // `:hover` animations — live on hoverStyle so the shared hover runtime
    // swaps them into style on hover enter/leave.
    if (val.has("hover_animations") && val["hover_animations"].type() == JsonType::Array) {
        if (!node->hoverStyle) node->hoverStyle = new MorphStyle();
        deserializeAnimations(node, val, /*hover=*/true);
    }
#endif

    // Apply active style if present
    if (val.has("active_style") && val["active_style"].type() == JsonType::Object) {
        node->activeStyle = new MorphStyle();
        // Copy base as starting point, then override with active properties
        *node->activeStyle = node->style;
        applyStyle(*node->activeStyle, val["active_style"]);
    }

    // Transition config
    if (val.has("transition_duration") && !val["transition_duration"].isNull())
        node->m_transitionDuration = val["transition_duration"].asFloat();
    if (val.has("transition_easing") && !val["transition_easing"].isNull()) {
        std::string e = val["transition_easing"].asString();
        if (e == "linear")      node->m_transitionEasing = Easing::Linear;
        else if (e == "ease-in")   node->m_transitionEasing = Easing::EaseIn;
        else if (e == "ease-out")  node->m_transitionEasing = Easing::EaseOut;
        else if (e == "ease-in-out") node->m_transitionEasing = Easing::EaseInOut;
    }

#ifdef MORPH_FEATURE_ANIMATION
    // CSS `animation` configs
    deserializeAnimations(node, val);
#endif

    // Deserialize ancestor hover rules
    if (val.has("ancestor_hover_rules") && val["ancestor_hover_rules"].type() == JsonType::Array) {
        auto& rules = val["ancestor_hover_rules"];
        for (size_t i = 0; i < rules.size(); i++) {
            auto& r = rules[i];
            AncestorHoverRule rule;
            if (r.has("ancestor_tag") && !r["ancestor_tag"].isNull())
                rule.ancestorTag = r["ancestor_tag"].asString();
            if (r.has("style") && r["style"].type() == JsonType::Object)
                applyStyle(rule.style, r["style"]);
            node->m_ancestorHoverRules.push_back(rule);
        }
    }

    // Deserialize ancestor active rules
    if (val.has("ancestor_active_rules") && val["ancestor_active_rules"].type() == JsonType::Array) {
        auto& rules = val["ancestor_active_rules"];
        for (size_t i = 0; i < rules.size(); i++) {
            auto& r = rules[i];
            AncestorHoverRule rule;
            if (r.has("ancestor_tag") && !r["ancestor_tag"].isNull())
                rule.ancestorTag = r["ancestor_tag"].asString();
            if (r.has("style") && r["style"].type() == JsonType::Object)
                applyStyle(rule.style, r["style"]);
            node->m_ancestorActiveRules.push_back(rule);
        }
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
            MorphNode* child = deserializeNode(val["children"][i], registry, resolved);
            if (child) node->addChild(child);
        }
    }

    // Deserialize conditional branch nodes into the registry (not added as children).
    // The logic .so attaches/detaches them dynamically via effects.
    if (val.has("then_nodes") && val["then_nodes"].type() == JsonType::Array) {
        for (size_t i = 0; i < val["then_nodes"].size(); i++) {
            deserializeNode(val["then_nodes"][i], registry, resolved);
        }
    }
    if (val.has("else_nodes") && val["else_nodes"].type() == JsonType::Array) {
        for (size_t i = 0; i < val["else_nodes"].size(); i++) {
            deserializeNode(val["else_nodes"][i], registry, resolved);
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
    std::string renderer = "flash"; // "flash" (default) | "forge"
};

static bool parseIR(const JsonValue& root, MorphNode*& outRoot,
                    DevWindowConfig& config,
                    NodeRegistry& registry,
                    std::vector<StateVarInfo>& stateVars) {
    outRoot = nullptr;
    registry.clear();
    stateVars.clear();

    if (root.type() != JsonType::Object) return false;
    if (!root.has("windows")) return false;

#ifdef MORPH_FEATURE_ANIMATION
    // Register @keyframes (cleared first so hot reloads never leak stale
    // keyframes from a previous stylesheet).
    parseIRKeyframes(root);
#endif

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
    if (win.has("renderer") && !win["renderer"].isNull())
        config.renderer = win["renderer"].asString();

    // Extract state vars
    if (win.has("state_vars") && win["state_vars"].type() == JsonType::Array) {
        auto& svs = win["state_vars"];
        for (size_t i = 0; i < svs.size(); i++) {
            StateVarInfo sv;
            auto& s = svs[i];
            if (s.has("getter") && !s["getter"].isNull())
                sv.getter = s["getter"].asString();
            if (s.has("setter") && !s["setter"].isNull())
                sv.setter = s["setter"].asString();
            if (s.has("init") && !s["init"].isNull())
                sv.init = s["init"].asString();
            // Deduce type from init
            std::string raw = sv.init;
            if (raw == "true" || raw == "false")
                sv.type = "bool";
            else if (raw.size() >= 2 && raw[0] == '\'' && raw.back() == '\'')
                sv.type = "std::string";
            else if (raw.find('.') != std::string::npos)
                sv.type = "double";
            else if (!raw.empty() && (std::isdigit(raw[0]) || raw[0] == '-'))
                sv.type = "int";
            else
                sv.type = "auto";
            stateVars.push_back(sv);
        }
    }

    // Build node tree from window's top-level nodes
    if (win.has("nodes") && win["nodes"].type() == JsonType::Array) {
        std::vector<MorphNode*> topLevel;
        InheritedStyle rootStyle;
        for (size_t i = 0; i < win["nodes"].size(); i++) {
            MorphNode* n = deserializeNode(win["nodes"][i], registry, rootStyle);
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
