use morph_ir::{IRNode, IRStyle, IRWindow};
use std::collections::HashSet;

#[derive(Default)]
pub struct FeatureSet {
    pub features: HashSet<String>,
}

impl FeatureSet {
    pub fn new() -> Self {
        Self::default()
    }

    fn scan_style(&mut self, s: &IRStyle) {
        if s.border_radius > 0.0 {
            self.features.insert("radius".into());
        }
        if s.font_weight != "normal" && !s.font_weight.is_empty() {
            self.features.insert("bold".into());
        }
        if s.overflow == "auto" || s.overflow == "scroll" {
            self.features.insert("scroll".into());
        }
        if s.scrollbar_width != 8.0
            || s.scrollbar_track_color != [0.85, 0.85, 0.85, 0.4]
            || s.scrollbar_thumb_color != [0.5, 0.5, 0.5, 0.6]
            || s.scrollbar_border_radius != 4.0
        {
            self.features.insert("scroll".into());
        }
        if s.position != "static" {
            self.features.insert("position".into());
        }
        if s.left.is_some() || s.right.is_some() || s.top.is_some() || s.bottom.is_some() {
            self.features.insert("position".into());
        }
        if s.z_index.is_some() {
            self.features.insert("zindex".into());
        }
        if (s.opacity - 1.0).abs() > f32::EPSILON {
            self.features.insert("opacity".into());
        }
        if s.display == "none" {
            self.features.insert("display_none".into());
        }
        if s.display == "inline" || s.display == "inline-block" {
            self.features.insert("inline".into());
        }
        if s.margin.iter().any(|&m| m != 0.0) {
            self.features.insert("margin_collapse".into());
        }
        if s.min_width.is_some()
            || s.max_width.is_some()
            || s.min_height.is_some()
            || s.max_height.is_some()
        {
            self.features.insert("min_max".into());
        }
        if s.box_sizing != "content-box" {
            self.features.insert("border_box".into());
        }
        if s.display == "flex" {
            self.features.insert("flex".into());
        }
        if s.gap > 0.0 {
            self.features.insert("flex".into());
        }
        if s.justify_content != "flex-start"
            || s.align_items != "stretch"
            || s.flex_wrap != "nowrap"
            || s.flex_grow != 0.0
            || s.flex_shrink != 1.0
            || s.flex_basis != "auto"
        {
            self.features.insert("flex".into());
        }
        if s.cursor != "default" && !s.cursor.is_empty() {
            self.features.insert("cursor".into());
        }
        // Any border member access needs the mixin — including a lone
        // border-color (e.g. hover-only), which carries no width/style.
        if s.border_width > 0.0
            || (!s.border_style.is_empty() && s.border_style != "none")
            || s.border_color != [0.0, 0.0, 0.0, 1.0]
        {
            self.features.insert("border".into());
        }
        if s.transform_ops.is_some() || s.transform_origin.is_some() {
            self.features.insert("transform".into());
        }
    }

    fn scan_reactive(&mut self, reactive_style: &std::collections::HashMap<String, String>) {
        for prop in reactive_style.keys() {
            for f in Self::reactive_feature(prop) {
                self.features.insert(f.into());
            }
            // Color-typed props lower through the runtime setColor parser
            // (node_emitter/logic_emitter "color" arms) — static palettes
            // are constants, only reactive ones need it.
            if prop.contains("color") {
                self.features.insert("reactive_color".into());
            }
        }
    }

    fn reactive_feature(prop: &str) -> Vec<&'static str> {
        match prop {
            "z-index" => vec!["zindex"],
            "opacity" => vec!["opacity"],
            "position" | "left" | "right" | "top" | "bottom" => vec!["position"],
            "cursor" => vec!["cursor"],
            "border-width" | "border-style" | "border-color" => vec!["border"],
            "scrollbar-width"
            | "scrollbar-track-color"
            | "scrollbar-thumb-color"
            | "scrollbar-border-radius" => vec!["scroll"],
            "flex-direction" | "flex-wrap" | "flex-basis" | "flex-grow" | "flex-shrink"
            | "justify-content" | "align-items" | "gap" => vec!["flex"],
            "overflow" => vec!["scroll"],
            "display" => vec!["flex", "display_none", "inline"],
            "font-weight" => vec!["bold"],
            "border-radius" => vec!["radius"],
            "min-width" | "max-width" | "min-height" | "max-height" => vec!["min_max"],
            "box-sizing" => vec!["border_box"],
            "margin" => vec!["margin_collapse"],
            "transform" => vec!["transform"],
            "animation" | "animation-name" | "animation-duration" => vec!["animation"],
            _ => vec![],
        }
    }

    pub fn scan<'a>(&mut self, windows: impl IntoIterator<Item = &'a IRWindow>) {
        for win in windows {
            if win.renderer == "forge" {
                self.features.insert("forge".into());
            }
            // Reactive runtime: any state/shared/event/effect/channel/mid
            // decl means signals + effect machinery must link. (Static
            // apps with none of these get a loop without the pump calls
            // and skip effect.cpp — the lean-binary promise.)
            if !win.state_vars.is_empty()
                || !win.shared_vars.is_empty()
                || !win.event_decls.is_empty()
                || !win.effect_decls.is_empty()
                || !win.channel_subs.is_empty()
                || !win.mid_assignments.is_empty()
            {
                self.features.insert("reactivity".into());
            }
            if !win.event_decls.is_empty() || !win.channel_subs.is_empty() {
                self.features.insert("channels".into());
            }
            // Body markers: lowered handler/effect/premain text carries
            // API usage no decl list captures (fetch, timers, coroutines,
            // ownership keys inside `new Window` opts).
            let mut bodies: Vec<&str> = Vec::new();
            for f in &win.premain_functions {
                bodies.push(f.as_str());
            }
            for ed in &win.effect_decls {
                if let Some(l) = ed.get("lambda") {
                    bodies.push(l.as_str());
                }
            }
            for sub in &win.channel_subs {
                if let Some(b) = sub.get("body") {
                    bodies.push(b.as_str());
                }
            }
            let has = |m: &str| bodies.iter().any(|b| b.contains(m));
            if has("morph::net::") {
                self.features.insert("net".into());
                // Network workers + coroutine resume run through the
                // task scheduler — net implies tasks.
                self.features.insert("tasks".into());
            }
            if has("set_timeout(") || has("set_interval(") || has("co_await") || has("morph::Task")
            {
                self.features.insert("tasks".into());
            }
            // Ownership keys inside `new Window` opts (pre-morpher text
            // keeps `parent:`/`modal:`/`role:` literally). Over-approx is
            // safe: a user data object with a `parent` key just keeps the
            // (small) follow machinery linked.
            if has("parent:") || has("modal:") || has("role:") {
                self.features.insert("ownership".into());
            }
            for kfs in win.keyframes.values() {
                for kf in kfs {
                    self.scan_style(&kf.style);
                    if kf.raw.keys().any(|p| p == "opacity") {
                        self.features.insert("opacity".into());
                    }
                    if kf.raw.keys().any(|p| p == "transform") {
                        self.features.insert("transform".into());
                    }
                    if kf.raw.keys().any(|p| p == "left" || p == "top") {
                        self.features.insert("position".into());
                    }
                }
            }
            for node in Self::walk(&win.nodes) {
                if node.node_type == "__text__" || node.node_type == "__expr__" {
                    self.features.insert("text".into());
                    // Inline text flow is core behavior, not an optional
                    // feature: mixed static/dynamic runs must group on one
                    // line even when no CSS rule uses `display: inline`.
                    self.features.insert("inline".into());
                }
                if node.node_type == "button" {
                    self.features.insert("button".into());
                    self.features.insert("radius".into());
                }
                if node.node_type == "input" {
                    self.features.insert("input".into());
                    self.features.insert("text".into());
                    self.features.insert("cursor".into());
                    self.features.insert("radius".into());
                    self.features.insert("event".into());
                    // Fields accept arbitrary typing (emoji included).
                    self.features.insert("harfbuzz".into());
                }
                if node.node_type == "img" {
                    self.features.insert("image".into());
                }
                if node.node_type == "__list__" {
                    self.features.insert("list".into());
                }
                if node.item_template.is_some() {
                    self.features.insert("list".into());
                }
                self.scan_style(&node.style);
                if let Some(ref hs) = node.hover_style {
                    self.features.insert("hover".into());
                    self.scan_style(hs);
                }
                if let Some(ref hs) = node.active_style {
                    self.features.insert("active".into());
                    self.scan_style(hs);
                }
                if !node.events.is_empty() {
                    self.features.insert("event".into());
                }
                if !node.animations.is_empty() || !node.hover_animations.is_empty() {
                    self.features.insert("animation".into());
                    // Keyframe/transition drivers run through effects.
                    self.features.insert("reactivity".into());
                }
                // Any dynamic binding (reactive text/attrs/style, show/hide
                // conditionals, lists) lowers to create_effect.
                if !node.reactive_attrs.is_empty()
                    || !node.reactive_text.is_empty()
                    || !node.reactive_class.is_empty()
                    || !node.reactive_style.is_empty()
                    || !node.class_conditional_effects.is_empty()
                    || !node.condition_expr.is_empty()
                    || !node.list_expr.is_empty()
                    || node.item_template.is_some()
                {
                    self.features.insert("reactivity".into());
                }
                if !node.reactive_text.is_empty() {
                    // Runtime content can be anything (emoji included).
                    self.features.insert("harfbuzz".into());
                }
                if Self::needs_harfbuzz(&node.text_content) {
                    self.features.insert("harfbuzz".into());
                }
                if !node.reactive_style.is_empty() {
                    self.scan_reactive(&node.reactive_style);
                }
                // Conditional class swaps apply whole stylesheets at runtime,
                // so their props must enable features exactly like reactive
                // styles do — otherwise e.g. a swapped-in `display: none`
                // compiles against a runtime with the behavior compiled out.
                for eff in &node.class_conditional_effects {
                    for (prop, val) in eff.on_styles.iter().chain(eff.off_styles.iter()) {
                        for f in Self::reactive_feature(prop) {
                            self.features.insert(f.into());
                        }
                        if prop.contains("color") {
                            self.features.insert("reactive_color".into());
                        }
                        if prop == "display" && val.trim() == "none" {
                            self.features.insert("display_none".into());
                        }
                        match prop.as_str() {
                            "flex" | "flex-grow" | "flex-shrink" | "flex-basis" => {
                                self.features.insert("flex".into());
                            }
                            "border" | "border-top" | "border-right" | "border-bottom"
                            | "border-left" => {
                                self.features.insert("border".into());
                            }
                            "overflow-x" | "overflow-y" => {
                                self.features.insert("scroll".into());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if ["scroll", "event", "cursor", "animation", "hover", "active"]
            .iter()
            .any(|f| self.features.contains(*f))
        {
            self.features.insert("dirty_rendering".into());
        }
        // The runtime's CSS-animation engine (core/node/animation.cpp) always
        // touches the transform-revert path, so enabling animation requires the
        // transform feature to be compiled in too.
        if self.features.contains("animation") {
            self.features.insert("transform".into());
        }
    }

    /// Route-level ownership: any non-default parent/modal/role in a
    /// route `windowConfig` keeps the follow/policy machinery linked.
    /// Call once per manifest route (build command owns the loop).
    pub fn note_route_ownership(&mut self, parent: &str, modal: bool, role: &str) {
        if !parent.is_empty() || modal || !role.is_empty() {
            self.features.insert("ownership".into());
        }
    }

    /// App-level `[window]` ownership defaults (`role` already interned).
    pub fn note_app_ownership(&mut self, parent: &str, modal: bool, role: i64) {
        if !parent.is_empty() || modal || role != 0 {
            self.features.insert("ownership".into());
        }
    }

    /// Page cache from resolved `navigation.cache` (0 = destroy path).
    pub fn note_page_cache(&mut self, on: bool) {
        if on {
            self.features.insert("pagecache".into());
        }
    }

    /// True when text needs HarfBuzz shaping: emoji/symbol codepoints,
    /// complex scripts (Arabic, Hebrew, Indic, Thai/Lao/Tibetan/Myanmar,
    /// Khmer, Hangul Jamo), or joiners that only occur in emoji sequences.
    /// Everything else (Latin, Cyrillic, Greek, CJK ideographs, kana,
    /// precomposed Hangul) shapes 1:1 in FreeType — typographically correct.
    fn needs_harfbuzz(text: &str) -> bool {
        for c in text.chars() {
            let cp = c as u32;
            let emoji = (0x1F000..=0x1FAFF).contains(&cp)
                || (0x2600..=0x26FF).contains(&cp)
                || (0x2700..=0x27BF).contains(&cp)
                || (0x2B00..=0x2BFF).contains(&cp)
                || cp == 0x20E3
                || cp == 0xFE0F
                || cp == 0x200D
                || (0x1F1E6..=0x1F1FF).contains(&cp);
            let complex = (0x0590..=0x05FF).contains(&cp)
                || (0x0600..=0x077F).contains(&cp)
                || (0x0900..=0x0DFF).contains(&cp)
                || (0x0E00..=0x0EFF).contains(&cp)
                || (0x0F00..=0x0FFF).contains(&cp)
                || (0x1000..=0x109F).contains(&cp)
                || (0x1780..=0x17FF).contains(&cp)
                || (0x1100..=0x11FF).contains(&cp);
            if emoji || complex {
                return true;
            }
        }
        false
    }

    pub fn required_headers(&self) -> Vec<String> {
        let mut h = vec!["ui/rect.h".to_string()];
        // The main loop calls process_tasks()/run_pending_effects() only
        // when the template emits them (reactive/tasks flags), so these
        // headers follow the same flags instead of riding unconditional.
        if self.features.contains("reactivity") || self.features.contains("tasks") {
            h.push("reactivity/task.h".into());
        }
        if self.features.contains("reactivity") {
            h.push("reactivity/signal.h".into());
        }
        if self.features.contains("channels") {
            h.push("reactivity/channel.h".into());
        }
        if self.features.contains("text") {
            h.push("ui/text.h".into());
        }
        if self.features.contains("button") {
            h.push("ui/button.h".into());
        }
        if self.features.contains("input") {
            h.push("ui/input.h".into());
        }
        if self.features.contains("event") {
            h.push("core/event.h".into());
        }
        if self.features.contains("image") {
            h.push("ui/image.h".into());
        }
        if self.features.contains("list") {
            h.push("ui/morph_list.h".into());
        }
        h
    }

    pub fn required_defines(&self) -> Vec<String> {
        let mut d = Vec::new();
        if self.features.contains("scroll") {
            d.push("MORPH_FEATURE_SCROLL".into());
        }
        if self.features.contains("radius") {
            d.push("MORPH_FEATURE_RADIUS".into());
        }
        if self.features.contains("text") {
            d.push("MORPH_FEATURE_TEXT".into());
        }
        if self.features.contains("bold") {
            d.push("MORPH_FEATURE_BOLD".into());
        }
        if self.features.contains("position") {
            d.push("MORPH_FEATURE_POSITION".into());
        }
        if self.features.contains("zindex") {
            d.push("MORPH_FEATURE_ZINDEX".into());
        }
        if self.features.contains("opacity") {
            d.push("MORPH_FEATURE_OPACITY".into());
        }
        if self.features.contains("flex") {
            d.push("MORPH_FEATURE_FLEX".into());
        }
        if self.features.contains("cursor") {
            d.push("MORPH_FEATURE_CURSOR".into());
        }
        if self.features.contains("border") {
            d.push("MORPH_FEATURE_BORDER".into());
        }
        if self.features.contains("transform") {
            d.push("MORPH_FEATURE_TRANSFORM".into());
        }
        if self.features.contains("animation") {
            d.push("MORPH_FEATURE_ANIMATION".into());
        }
        if self.features.contains("display_none") {
            d.push("MORPH_FEATURE_DISPLAY_NONE".into());
        }
        if self.features.contains("inline") {
            d.push("MORPH_FEATURE_INLINE".into());
        }
        if self.features.contains("margin_collapse") {
            d.push("MORPH_FEATURE_MARGIN_COLLAPSE".into());
        }
        if self.features.contains("min_max") {
            d.push("MORPH_FEATURE_MIN_MAX".into());
        }
        if self.features.contains("border_box") {
            d.push("MORPH_FEATURE_BORDER_BOX".into());
        }
        if self.features.contains("image") {
            d.push("MORPH_FEATURE_IMAGE".into());
        }
        if self.features.contains("input") {
            d.push("MORPH_FEATURE_INPUT".into());
        }
        if self.features.contains("dirty_rendering") || self.features.contains("scroll") {
            d.push("MORPH_FEATURE_DIRTY_RENDERING".into());
        }
        if self.features.contains("forge") {
            d.push("MORPH_RENDERER_FORGE".into());
        }
        // Cursor-position callback (hover/active visuals, cursor shapes,
        // scrollbar drags) — one gate for the whole hover pipeline.
        if self.features.contains("hover")
            || self.features.contains("active")
            || self.features.contains("cursor")
            || self.features.contains("scroll")
        {
            d.push("MORPH_FEATURE_HOVER".into());
        }
        // No runtime `#ifdef` consumes this one (list machinery is
        // header-only) — it exists so the build can pick -O2 for
        // list-heavy apps (see opt_flag).
        if self.features.contains("list") {
            d.push("MORPH_FEATURE_LIST".into());
        }
        // Lean-binary subsystem flags (derived from scan, never user-set
        // — same rule as every CSS flag above). Each gates whole TUs
        // and/or template branches; dev builds define them all.
        if self.features.contains("reactivity") {
            d.push("MORPH_FEATURE_REACTIVITY".into());
        }
        if self.features.contains("tasks") {
            d.push("MORPH_FEATURE_TASKS".into());
        }
        if self.features.contains("net") {
            d.push("MORPH_FEATURE_NET".into());
            // Workers + coroutine resume run through the scheduler.
            if !self.features.contains("tasks") {
                d.push("MORPH_FEATURE_TASKS".into());
            }
        }
        if self.features.contains("ownership") {
            d.push("MORPH_FEATURE_OWNERSHIP".into());
        }
        if self.features.contains("pagecache") {
            d.push("MORPH_FEATURE_PAGECACHE".into());
        }
        // HarfBuzz text shaping: emoji, complex scripts, dynamic text,
        // and input fields. Everything else shapes 1:1 in FreeType.
        if self.features.contains("harfbuzz") {
            d.push("MORPH_FEATURE_HARFBUZZ".into());
        }
        d
    }

    fn walk(nodes: &[IRNode]) -> Vec<&IRNode> {
        let mut out = Vec::new();
        for n in nodes {
            out.push(n);
            out.extend(Self::walk(&n.children));
            out.extend(Self::walk(&n.then_nodes));
            out.extend(Self::walk(&n.else_nodes));
            if let Some(ref tmpl) = n.item_template {
                out.push(tmpl);
                out.extend(Self::walk(&tmpl.children));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_with_hover_border_color() -> IRNode {
        let mut node = IRNode {
            node_id: "node_0001".to_string(),
            node_type: "div".to_string(),
            ..Default::default()
        };
        let mut hover = IRStyle::default();
        hover.border_color = [0.5, 0.5, 0.5, 1.0];
        node.hover_style = Some(hover);
        node
    }

    #[test]
    fn hover_only_border_color_enables_border_feature() {
        // Regression: hover border-color with no width/style still emits
        // `->borderColor`, which needs MORPH_FEATURE_BORDER compiled in.
        let win = IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            modal: false,
            renderer: "flash".to_string(),
            nodes: vec![node_with_hover_border_color()],
            startup_logs: Vec::new(),
            premain_functions: Vec::new(),
            extra_headers: Vec::new(),
            state_vars: Vec::new(),
            reactive_consts: Vec::new(),
            route_props: Vec::new(),
            shared_vars: Vec::new(),
            effect_decls: Vec::new(),
            cpp_imports: Vec::new(),
            event_decls: Vec::new(),
            mid_assignments: Vec::new(),
            module_bindings: Vec::new(),
            channel_subs: Vec::new(),
            keyframes: std::collections::HashMap::new(),
        };
        let mut fs = FeatureSet::new();
        fs.scan(std::slice::from_ref(&win));
        assert!(fs.features.contains("border"));
        assert!(fs.required_defines().contains(&"MORPH_FEATURE_BORDER".to_string()));
    }

    fn text_window(text: &str, node_type: &str) -> IRWindow {
        let mut node = IRNode {
            node_id: "node_0001".to_string(),
            node_type: node_type.to_string(),
            text_content: text.to_string(),
            ..Default::default()
        };
        if node_type == "input" {
            node.text_content = String::new();
        }
        IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            modal: false,
            renderer: "flash".to_string(),
            nodes: vec![node],
            startup_logs: Vec::new(),
            premain_functions: Vec::new(),
            extra_headers: Vec::new(),
            state_vars: Vec::new(),
            reactive_consts: Vec::new(),
            route_props: Vec::new(),
            shared_vars: Vec::new(),
            effect_decls: Vec::new(),
            cpp_imports: Vec::new(),
            event_decls: Vec::new(),
            mid_assignments: Vec::new(),
            module_bindings: Vec::new(),
            channel_subs: Vec::new(),
            keyframes: std::collections::HashMap::new(),
        }
    }

    fn defines_for(text: &str, node_type: &str) -> Vec<String> {
        let win = text_window(text, node_type);
        let mut fs = FeatureSet::new();
        fs.scan(std::slice::from_ref(&win));
        fs.required_defines()
    }

    #[test]
    fn harfbuzz_triggers_on_emoji_and_complex_scripts() {
        for text in ["hello 🎉", "مرحبا", "नमस्ते", "שלום", "สวัสดี", "👨‍👩‍👧"]
        {
            let d = defines_for(text, "__text__");
            assert!(
                d.contains(&"MORPH_FEATURE_HARFBUZZ".to_string()),
                "{text:?} should shape via HarfBuzz"
            );
        }
    }

    #[test]
    fn harfbuzz_stays_off_for_plain_and_cjk_text() {
        for text in [
            "hello world",
            "123",
            "Bonjour le monde",
            "日本語テスト",
            "한글",
            "Ελληνικά",
            "Кириллица",
        ] {
            let d = defines_for(text, "__text__");
            assert!(
                !d.contains(&"MORPH_FEATURE_HARFBUZZ".to_string()),
                "{text:?} should shape in FreeType"
            );
        }
    }

    #[test]
    fn harfbuzz_forced_by_reactive_text_and_inputs() {
        let d = defines_for("", "input");
        assert!(d.contains(&"MORPH_FEATURE_HARFBUZZ".to_string()), "inputs take anything");
        let mut win = text_window("plain", "__text__");
        win.nodes[0].reactive_text = "{name}".to_string();
        let mut fs = FeatureSet::new();
        fs.scan(std::slice::from_ref(&win));
        assert!(fs.required_defines().contains(&"MORPH_FEATURE_HARFBUZZ".to_string()));
    }
}
