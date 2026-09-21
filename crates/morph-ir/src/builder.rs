use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::GetSpan;

use crate::css_registry;
use crate::node::{IREvent, IRNode, IRWindow};
use crate::style::IRStyle;
use crate::tailwind::TailwindResolver;
use crate::transforms;

pub struct IRBuilder {
    tailwind: TailwindResolver,
    counter: std::cell::Cell<usize>,
    type_mode: morpher::TypeMode,
}

impl IRBuilder {
    pub fn new() -> Self {
        Self {
            tailwind: TailwindResolver::new(),
            counter: std::cell::Cell::new(0),
            type_mode: morpher::TypeMode::default(),
        }
    }

    /// Type mode for translating embedded logic (handlers, effects, globals).
    /// Defaults to inference; `morph build --types` overrides it.
    #[must_use]
    pub const fn with_type_mode(mut self, mode: morpher::TypeMode) -> Self {
        self.type_mode = mode;
        self
    }

    /// Assign the next flat `node_NNNN` id (Python-style global counter).
    fn next_id(&self) -> String {
        let n = self.counter.get();
        self.counter.set(n + 1);
        format!("node_{n:04}")
    }

    // Splitting this builder function risks behavior change.
    #[allow(clippy::too_many_lines)]
    pub fn build(
        &self,
        source: &morph_parser::MxSource,
        css_rules: &[(String, morph_parser::CssRule)],
        css_keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> Vec<IRWindow> {
        let mut windows = Vec::new();
        // Ambient state map shared by every embedded-logic translation:
        // getters read signals, setters write them, reactive const lambdas
        // re-evaluate. Extended with component consts below, mirroring the
        // order Python builds its per-component translator state.
        let mut ambient_vars: HashMap<String, String> = HashMap::new();
        let mut ambient_types: HashMap<String, String> = HashMap::new();
        let events: HashMap<String, String> = HashMap::new();
        let mut all_state: Vec<HashMap<String, String>> = Vec::new();
        for sv in source
            .state_vars
            .iter()
            .chain(source.components.iter().flat_map(|c| c.state_vars.iter()))
        {
            let mut m = HashMap::new();
            m.insert("getter".into(), sv.getter.clone());
            m.insert("setter".into(), sv.setter.clone());
            m.insert("init".into(), sv.init.clone());
            all_state.push(m);
            if !sv.getter.is_empty() {
                ambient_vars.insert(sv.getter.clone(), format!("__st_{}.get()", sv.getter));
                if let Some(ty) = infer_state_type(&sv.init) {
                    ambient_types.insert(sv.getter.clone(), ty);
                }
            }
            if !sv.setter.is_empty() {
                ambient_vars.insert(sv.setter.clone(), format!("__st_{}.set", sv.getter));
            }
        }
        let wc = source.window_config.as_ref();
        let mut extra_headers: Vec<String> = source.extra_headers.clone();
        // ── Module-level logic first (Python: function_declarations, global_vars) ──
        let mut premain: Vec<String> = Vec::new();
        for fd in &source.function_declarations {
            self.push_snippet(
                &mut premain,
                &mut extra_headers,
                &fd.source,
                &ambient_vars,
                &ambient_types,
                &events,
                None,
            );
        }
        for gv in &source.global_vars {
            self.push_snippet(
                &mut premain,
                &mut extra_headers,
                gv,
                &ambient_vars,
                &ambient_types,
                &events,
                None,
            );
        }
        // ── Component consts become reactive lambdas; later translations ──
        // see them as `name()` calls (Python: `auto x = []() { return …; };`).
        let mut reactive_consts: Vec<String> = Vec::new();
        for c in &source.components {
            for cst in &c.consts {
                if cst.name.is_empty() || cst.rhs.is_empty() {
                    continue;
                }
                if let Some(out) =
                    self.translate_logic(&cst.rhs, &ambient_vars, &ambient_types, &events)
                {
                    let expr = out.body.trim().trim_end_matches(';').trim();
                    if expr.is_empty() {
                        continue;
                    }
                    extra_headers.extend(include_lines(&out.includes));
                    premain.push(format!("auto {} = []() {{ return ({}); }};", cst.name, expr));
                    ambient_vars.insert(cst.name.clone(), format!("{}()", cst.name));
                    reactive_consts.push(cst.name.clone());
                }
            }
        }
        // ── Inner functions (module + component) with the extended map ──
        for f in source
            .inner_functions
            .iter()
            .chain(source.components.iter().flat_map(|c| c.inner_functions.iter()))
        {
            self.push_snippet(
                &mut premain,
                &mut extra_headers,
                &f.source,
                &ambient_vars,
                &ambient_types,
                &events,
                None,
            );
        }
        // ── Effects: transpile callbacks now, emit `create_effect` later ──
        let mut all_effects: Vec<HashMap<String, String>> = Vec::new();
        for e in
            source.effects.iter().chain(source.components.iter().flat_map(|c| c.effects.iter()))
        {
            if let Some(out) =
                self.translate_logic(&e.callback, &ambient_vars, &ambient_types, &events)
            {
                let lambda = out.body.trim().trim_end_matches(';').trim().to_string();
                if lambda.is_empty() {
                    continue;
                }
                extra_headers.extend(include_lines(&out.includes));
                let mut m = HashMap::new();
                m.insert("lambda".into(), lambda);
                m.insert("deps".into(), e.deps.clone());
                all_effects.push(m);
            }
        }
        extra_headers.sort();
        extra_headers.dedup();
        // Startup logs: module logs, then each component's body logs
        // (Python: per-component `body_logs` → window `startup_logs`).
        let mut startup_logs = source.console_logs.clone();
        for c in &source.components {
            for log in &c.console_logs {
                if !startup_logs.contains(log) {
                    startup_logs.push(log.clone());
                }
            }
        }
        let mut nodes = Vec::new();
        for comp in &source.components {
            nodes.push(self.build_node(
                &comp.jsx,
                css_rules,
                0,
                &[],
                &ambient_vars,
                &ambient_types,
                &mut extra_headers,
                css_keyframes,
            ));
        }
        extra_headers.sort();
        extra_headers.dedup();
        let mut window = IRWindow {
            window_id: self.next_id(),
            title: wc.map_or_else(|| "Morph App".into(), |w| w.title.clone()),
            width: wc.map_or(800, |w| w.width),
            height: wc.map_or(600, |w| w.height),
            visible: true,
            min_width: wc.and_then(|w| w.min_width),
            max_width: wc.and_then(|w| w.max_width),
            min_height: wc.and_then(|w| w.min_height),
            max_height: wc.and_then(|w| w.max_height),
            modal: wc.is_some_and(|w| w.modal),
            renderer: "flash".into(),
            nodes: vec![],
            startup_logs,
            premain_functions: premain,
            extra_headers,
            state_vars: all_state,
            reactive_consts,
            route_props: Vec::new(),
            effect_decls: all_effects,
            // Legacy single-file path: no shared store or channels (use
            // `build_with_graph`).
            shared_vars: Vec::new(),
            event_decls: Vec::new(),
            channel_subs: Vec::new(),
            mid_assignments: Vec::new(),
            module_bindings: Vec::new(),
            cpp_imports: source
                .cpp_imports
                .iter()
                .map(|ci| {
                    let base =
                        Path::new(&source.filename).parent().unwrap_or_else(|| Path::new("."));
                    let path = base.join(&ci.path);
                    let abs_path = path.canonicalize().unwrap_or(path);
                    let mut m = HashMap::new();
                    m.insert("path".into(), abs_path.display().to_string());
                    m.insert("specifiers".into(), ci.specifiers.join(", "));
                    m
                })
                .collect(),
            keyframes: self.convert_keyframes(css_keyframes),
        };
        window.nodes = nodes;
        windows.push(window);
        windows
    }

    /// Translate embedded JS/TS against ambient app state. `None` when the
    /// snippet does not parse or has no translatable content (the caller
    /// skips it, mirroring Python's per-block try/except).
    fn translate_logic(
        &self,
        source: &str,
        ambient_vars: &HashMap<String, String>,
        ambient_types: &HashMap<String, String>,
        events: &HashMap<String, String>,
    ) -> Option<morpher::SnippetOutput> {
        let options = morpher::TranslateOptions {
            type_mode: self.type_mode,
            state_vars: ambient_vars.clone(),
            state_types: ambient_types.clone(),
            ..Default::default()
        };
        let rebased = rewrite_event_emits(source, events);
        morpher::translate_snippet(&rewrite_emit_for_js(&rebased), "snippet.ts", options)
            .ok()
            .filter(|out| !out.body.trim().is_empty())
            .map(|mut out| {
                out.body = rewrite_emit_for_cpp(&out.body);
                out
            })
    }

    /// Translate a statement-level snippet and splice its body into `premain`
    /// with external linkage (mirrors Python's `strip_static_function`).
    /// With `wrap_ns`, the body is wrapped in its defining file's
    /// namespace first (identical wrapped bodies dedupe); without it the
    /// legacy flat emission is kept (single-file builds have no imports).
    fn push_snippet(
        &self,
        premain: &mut Vec<String>,
        extra_headers: &mut Vec<String>,
        source: &str,
        ambient_vars: &HashMap<String, String>,
        ambient_types: &HashMap<String, String>,
        events: &HashMap<String, String>,
        wrap_ns: Option<&str>,
    ) {
        if let Some(out) = self.translate_logic(source, ambient_vars, ambient_types, events) {
            extra_headers.extend(include_lines(&out.includes));
            let body = strip_static_linkage(&out.body);
            if body.is_empty() {
                return;
            }
            let body = match wrap_ns {
                Some(ns) => {
                    format!("namespace {MODULE_NS_ROOT} {{\nnamespace {ns} {{\n{body}\n}}\n}}")
                }
                None => body,
            };
            if !premain.contains(&body) {
                premain.push(body);
            }
        }
    }

    // Reusable components expand at IR-build time: each `<Tag />` gets a
    // scope frame with per-instance signals, mangled helpers, and prop
    // bindings, reusing the ambient-map machinery downstream untouched.

    /// Build windows from a resolved multi-file module graph.
    ///
    /// Only the entry module's default-export component renders as a root;
    /// every other component renders solely where instantiated.
    ///
    /// # Errors
    /// Rejects unknown components, cyclic instantiation, bad props, and
    /// entry components declaring props.
    // Splitting this builder function risks behavior change.
    #[allow(clippy::too_many_lines)]
    pub fn build_with_graph(
        &self,
        graph: &morph_parser::ModuleGraph,
        css_rules: &[(String, morph_parser::CssRule)],
        css_keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> anyhow::Result<Vec<IRWindow>> {
        let entry_mod = graph
            .entry_module()
            .ok_or_else(|| anyhow::anyhow!("component graph has no entry module"))?;
        let root_comp = entry_mod
            .source
            .components
            .iter()
            .find(|c| c.is_default)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "entry {} has no default-export component (expected `export default function App()`)",
                    entry_mod.path.display()
                )
            })?;
        if !root_comp.props.is_empty() || !root_comp.props_param.is_empty() {
            anyhow::bail!(
                "entry component `{}` must not declare props ({}); props flow downhill into child components",
                root_comp.name,
                entry_mod.path.display()
            );
        }

        // Human-computable namespaces are 1:1 by construction; fail fast
        // on any normalized collision (mirrors the mx-naming lint).
        validate_namespaces(graph)?;

        for mod_path in &graph.order {
            let Some(module) = graph.modules.get(mod_path) else {
                continue;
            };
            for comp in &module.source.components {
                Self::validate_member_names(comp, &module.path)?;
            }
        }

        let mut ctx = BuilderCtx::new(graph);
        let mut extra_headers: Vec<String> = Vec::new();
        let mut frame = InstanceFrame::root(entry_mod.path.clone());

        Self::seed_module_bindings(
            &entry_mod.source,
            &entry_mod.path,
            graph,
            &mut frame,
            &mut ctx,
        )?;

        self.translate_module_globals(&entry_mod.source, &frame, &mut ctx, &mut extra_headers);
        ctx.modules_emitted.insert(entry_mod.path.clone());

        for sv in &root_comp.state_vars {
            let entry_ns = ns_of(graph, &entry_mod.path).unwrap_or_default();
            ctx.states.push(state_slot(&sv.getter, &sv.setter, &sv.init, false, &entry_ns));
            if !sv.getter.is_empty() {
                Self::seed_state_var(
                    &mut frame,
                    &sv.getter,
                    &sv.setter,
                    &sv.init,
                    &sv.getter,
                    &sv.setter,
                    ctx.signal_prefix,
                );
            }
        }
        Self::preseed_sibling_names(root_comp, &mut frame, None);
        self.expand_component_sources(root_comp, &frame, &mut ctx, &mut extra_headers)?;
        for log in root_comp.console_logs.iter().chain(entry_mod.source.console_logs.iter()) {
            if !ctx.logs.contains(log) {
                ctx.logs.push(log.clone());
            }
        }

        let root_node = self.build_node_in(
            &root_comp.jsx,
            css_rules,
            0,
            &[],
            &frame,
            &mut ctx,
            &mut extra_headers,
            css_keyframes,
        )?;

        // Pure-helper modules (no components, never instantiated) still
        // need their globals translated + imports registered: seed a
        // neutral frame and emit once, in graph order for determinism.
        // (Component-bearing modules were already handled via expansion.)
        for mod_path in &graph.order {
            if ctx.modules_emitted.contains(mod_path) {
                continue;
            }
            ctx.modules_emitted.insert(mod_path.clone());
            if let Some(module) = graph.modules.get(mod_path) {
                let mut neutral = InstanceFrame::root(module.path.clone());
                Self::seed_module_bindings(
                    &module.source,
                    &module.path,
                    graph,
                    &mut neutral,
                    &mut ctx,
                )?;
                let snapshot = std::mem::take(&mut ctx.premain);
                self.translate_module_globals(
                    &module.source,
                    &neutral,
                    &mut ctx,
                    &mut extra_headers,
                );
                let mut added = std::mem::replace(&mut ctx.premain, snapshot);
                added.retain(|p| !ctx.premain.contains(p));
                ctx.premain.extend(added);
                for log in &module.source.console_logs {
                    if !ctx.logs.contains(log) {
                        ctx.logs.push(log.clone());
                    }
                }
            }
        }

        extra_headers.sort();
        extra_headers.dedup();
        let wc = entry_mod.source.window_config.as_ref();
        let mut cpp_imports = Vec::new();
        let mut seen_cpp = HashSet::new();
        for module_path in &graph.order {
            let Some(module) = graph.modules.get(module_path) else { continue };
            if module_path != &entry_mod.path && !ctx.modules_emitted.contains(module_path) {
                continue;
            }
            let base = module.dir.clone();
            for ci in &module.source.cpp_imports {
                let path = base.join(&ci.path);
                let abs_path = path.canonicalize().unwrap_or(path);
                let key = abs_path.display().to_string();
                if !seen_cpp.insert(key.clone()) {
                    continue;
                }
                let mut m = HashMap::new();
                m.insert("path".into(), key);
                m.insert("specifiers".into(), ci.specifiers.join(", "));
                cpp_imports.push(m);
            }
        }
        let window = IRWindow {
            window_id: self.next_id(),
            title: wc.map_or_else(|| "Morph App".into(), |w| w.title.clone()),
            width: wc.map_or(800, |w| w.width),
            height: wc.map_or(600, |w| w.height),
            visible: true,
            min_width: wc.and_then(|w| w.min_width),
            max_width: wc.and_then(|w| w.max_width),
            min_height: wc.and_then(|w| w.min_height),
            max_height: wc.and_then(|w| w.max_height),
            modal: wc.is_some_and(|w| w.modal),
            renderer: "flash".into(),
            nodes: vec![root_node],
            startup_logs: ctx.logs.clone(),
            premain_functions: ctx.premain.clone(),
            extra_headers,
            state_vars: ctx.states.clone(),
            reactive_consts: ctx.const_names.clone(),
            route_props: Vec::new(),
            effect_decls: ctx.effects.clone(),
            shared_vars: ctx.shared_entries.clone(),
            event_decls: ctx.event_entries.clone(),
            channel_subs: ctx.channels.clone(),
            mid_assignments: Self::mid_assignments(graph, &ctx, ctx.signal_prefix)?,
            module_bindings: ctx.module_bindings.clone(),
            cpp_imports,
            keyframes: self.convert_keyframes(css_keyframes),
        };
        Ok(vec![window])
    }

    /// Build one route file into a mountable window IR.
    ///
    /// Mirrors `build_with_graph` (keep the two in sync) with route deltas:
    /// the root component **may** declare props — they bind to mount-time
    /// `__props` locals (`__morph_prop_<name>`, converted in the mount
    /// prologue) instead of JSX literals. State, effects, channels and
    /// `mid` assignments record identically; codegen interprets them as
    /// per-mount context members. Window chrome comes from the manifest
    /// (`0`/empty = unspecified → app defaults at `new Window` time).
    /// A missing default export is a hard error (`mx-route-no-export`).
    pub fn build_route(
        &self,
        graph: &morph_parser::ModuleGraph,
        route: &morph_parser::routes::RouteEntry,
        css_rules: &[(String, morph_parser::CssRule)],
        css_keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> anyhow::Result<IRWindow> {
        let route_mod = graph
            .entry_module()
            .ok_or_else(|| anyhow::anyhow!("route graph has no entry module"))?;
        let root_comp =
            route_mod.source.components.iter().find(|c| c.is_default).ok_or_else(|| {
                anyhow::anyhow!(
                    "mx-route-no-export: route {} has no default-export component\nLearn more: https://morph.levizr.com/docs/errors/mx-route-no-export",
                    route_mod.path.display()
                )
            })?;

        validate_namespaces(graph)?;

        let mut ctx = BuilderCtx::new(graph);
        ctx.signal_prefix = "ctx->";
        let mut extra_headers: Vec<String> = Vec::new();
        let mut frame = InstanceFrame::root(route_mod.path.clone());

        Self::seed_module_bindings(
            &route_mod.source,
            &route_mod.path,
            graph,
            &mut frame,
            &mut ctx,
        )?;

        self.translate_module_globals(&route_mod.source, &frame, &mut ctx, &mut extra_headers);
        ctx.modules_emitted.insert(route_mod.path.clone());

        // Route root props bind to mount-time locals converted from the
        // runtime `__props` object (see the mount prologue). Plain C++
        // types flow downstream — no JsValue past this point.
        let mut route_props: Vec<HashMap<String, String>> = Vec::new();
        if !root_comp.props_param.is_empty() {
            frame.props_param.clone_from(&root_comp.props_param);
        }
        for prop in &root_comp.props {
            let class = ts_type_class(&prop.prop_type);
            // Props live in the mount context like state (uniform `ctx->`
            // access for nodes and handlers; the context also carries them
            // across cache detach/restore). The mount prologue assigns
            // each member from `__props` before building nodes.
            let member = format!("ctx->{}", prop.name);
            frame.vars.insert(prop.name.clone(), member.clone());
            if !class.is_empty() {
                frame.types.insert(prop.name.clone(), class.clone());
            }
            frame.prop_binds.insert(prop.name.clone(), format!("({member})"));
            let mut m = HashMap::new();
            m.insert("name".into(), prop.name.clone());
            m.insert("class".into(), class);
            m.insert("optional".into(), prop.optional.to_string());
            route_props.push(m);
        }
        if !root_comp.props_param.is_empty() {
            frame.vars.insert(root_comp.props_param.clone(), "__props".to_string());
        }
        // Whole-object access needs no prologue local (`__props` is the
        // mount parameter) — record it only when no inline type declares
        // individual props.
        if root_comp.props.is_empty() && !root_comp.props_param.is_empty() {
            let mut m = HashMap::new();
            m.insert("name".into(), root_comp.props_param.clone());
            m.insert("class".into(), "JsObject".to_string());
            m.insert("optional".into(), "true".to_string());
            route_props.push(m);
        }

        for sv in &root_comp.state_vars {
            let route_ns = ns_of(graph, &route_mod.path).unwrap_or_default();
            ctx.states.push(state_slot(&sv.getter, &sv.setter, &sv.init, false, &route_ns));
            if !sv.getter.is_empty() {
                Self::seed_state_var(
                    &mut frame,
                    &sv.getter,
                    &sv.setter,
                    &sv.init,
                    &sv.getter,
                    &sv.setter,
                    ctx.signal_prefix,
                );
            }
        }
        Self::preseed_sibling_names(root_comp, &mut frame, None);
        self.expand_component_sources(root_comp, &frame, &mut ctx, &mut extra_headers)?;
        for log in root_comp.console_logs.iter().chain(route_mod.source.console_logs.iter()) {
            if !ctx.logs.contains(log) {
                ctx.logs.push(log.clone());
            }
        }

        let root_node = self.build_node_in(
            &root_comp.jsx,
            css_rules,
            0,
            &[],
            &frame,
            &mut ctx,
            &mut extra_headers,
            css_keyframes,
        )?;

        for mod_path in &graph.order {
            if ctx.modules_emitted.contains(mod_path) {
                continue;
            }
            ctx.modules_emitted.insert(mod_path.clone());
            if let Some(module) = graph.modules.get(mod_path) {
                let mut neutral = InstanceFrame::root(module.path.clone());
                Self::seed_module_bindings(
                    &module.source,
                    &module.path,
                    graph,
                    &mut neutral,
                    &mut ctx,
                )?;
                let snapshot = std::mem::take(&mut ctx.premain);
                self.translate_module_globals(
                    &module.source,
                    &neutral,
                    &mut ctx,
                    &mut extra_headers,
                );
                let mut added = std::mem::replace(&mut ctx.premain, snapshot);
                added.retain(|p| !ctx.premain.contains(p));
                ctx.premain.extend(added);
                for log in &module.source.console_logs {
                    if !ctx.logs.contains(log) {
                        ctx.logs.push(log.clone());
                    }
                }
            }
        }

        extra_headers.sort();
        extra_headers.dedup();
        let mut cpp_imports = Vec::new();
        let mut seen_cpp = HashSet::new();
        for module_path in &graph.order {
            let Some(module) = graph.modules.get(module_path) else { continue };
            if module_path != &route_mod.path && !ctx.modules_emitted.contains(module_path) {
                continue;
            }
            let base = module.dir.clone();
            for ci in &module.source.cpp_imports {
                let path = base.join(&ci.path);
                let abs_path = path.canonicalize().unwrap_or(path);
                let key = abs_path.display().to_string();
                if !seen_cpp.insert(key.clone()) {
                    continue;
                }
                let mut m = HashMap::new();
                m.insert("path".into(), key);
                m.insert("specifiers".into(), ci.specifiers.join(", "));
                cpp_imports.push(m);
            }
        }
        let window = IRWindow {
            window_id: format!("route:{}", route.id),
            title: route.title.clone().unwrap_or_default(),
            width: route.width.unwrap_or(0),
            height: route.height.unwrap_or(0),
            visible: true,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            modal: false,
            renderer: "flash".into(),
            nodes: vec![root_node],
            startup_logs: ctx.logs.clone(),
            premain_functions: ctx.premain.clone(),
            extra_headers,
            state_vars: ctx.states.clone(),
            reactive_consts: ctx.const_names.clone(),
            route_props,
            effect_decls: ctx.effects.clone(),
            shared_vars: ctx.shared_entries.clone(),
            event_decls: ctx.event_entries.clone(),
            channel_subs: ctx.channels.clone(),
            mid_assignments: Self::mid_assignments(graph, &ctx, ctx.signal_prefix)?,
            module_bindings: ctx.module_bindings.clone(),
            cpp_imports,
            keyframes: self.convert_keyframes(css_keyframes),
        };
        Ok(window)
    }

    /// Translate a module's top-level helpers + globals into premain,
    /// each wrapped in its defining file's namespace (the universal
    /// module-binding rule: definitions live once at the definition
    /// site, calls rewrite to qualified names via the frame maps).
    fn translate_module_globals(
        &self,
        source: &morph_parser::MxSource,
        frame: &InstanceFrame,
        ctx: &mut BuilderCtx,
        extra_headers: &mut Vec<String>,
    ) {
        let ns = ns_of(ctx.graph, &frame.module).unwrap_or_default();
        let wrap = (!ns.is_empty()).then_some(ns.as_str());
        for fd in &source.function_declarations {
            self.push_snippet(
                &mut ctx.premain,
                extra_headers,
                &fd.source,
                &frame.vars,
                &frame.types,
                &frame.events,
                wrap,
            );
        }
        for cd in &source.class_declarations {
            self.push_snippet(
                &mut ctx.premain,
                extra_headers,
                &cd.source,
                &frame.vars,
                &frame.types,
                &frame.events,
                wrap,
            );
        }
        for gv in source.global_vars.iter().chain(source.exported_vars.iter().map(|v| &v.source)) {
            self.push_snippet(
                &mut ctx.premain,
                extra_headers,
                gv,
                &frame.vars,
                &frame.types,
                &frame.events,
                wrap,
            );
        }
    }

    /// Seed mangled const/helper names into `frame` under both original and
    /// mangled keys.
    fn preseed_sibling_names(
        comp: &morph_parser::MxComponent,
        frame: &mut InstanceFrame,
        mangle_prefix: Option<&str>,
    ) {
        let mangle =
            |name: &str| mangle_prefix.map_or_else(|| name.to_string(), |p| format!("{p}_{name}"));
        for cst in &comp.consts {
            if cst.name.is_empty() {
                continue;
            }
            let mangled = mangle(&cst.name);
            let expr = format!("{mangled}()");
            frame.vars.insert(cst.name.clone(), expr.clone());
            frame.vars.insert(mangled.clone(), expr);
            frame.renames.insert(cst.name.clone(), mangled);
        }
        for f in &comp.inner_functions {
            if f.name.is_empty() {
                continue;
            }
            let mangled = mangle(&f.name);
            frame.vars.insert(f.name.clone(), mangled.clone());
            frame.vars.insert(mangled.clone(), mangled.clone());
            frame.renames.insert(f.name.clone(), mangled);
        }
    }

    /// Seed one state variable into `frame` under both original and emitted
    /// names.
    fn seed_state_var(
        frame: &mut InstanceFrame,
        getter: &str,
        setter: &str,
        init: &str,
        emitted_getter: &str,
        emitted_setter: &str,
        // Signal owner expression: `"__st_"` for entry globals,
        // `"ctx->"` for route mount contexts (documented codegen contract:
        // the mount function names its context `ctx`).
        signal_prefix: &str,
    ) {
        let get_expr = format!("{signal_prefix}{emitted_getter}.get()");
        let set_expr = format!("{signal_prefix}{emitted_getter}.set");
        frame.vars.insert(getter.to_string(), get_expr.clone());
        frame.vars.insert(emitted_getter.to_string(), get_expr);
        if let Some(ty) = infer_state_type(init) {
            frame.types.insert(getter.to_string(), ty.clone());
            frame.types.insert(emitted_getter.to_string(), ty);
        }
        if !setter.is_empty() {
            frame.vars.insert(setter.to_string(), set_expr.clone());
            frame.vars.insert(emitted_setter.to_string(), set_expr);
        }
        if getter != emitted_getter {
            frame.renames.insert(getter.to_string(), emitted_getter.to_string());
        }
        if !setter.is_empty() && setter != emitted_setter {
            frame.renames.insert(setter.to_string(), emitted_setter.to_string());
        }
    }

    /// Translate one component's consts / inner functions / effects against
    /// `frame` (names must be pre-seeded). Sources are pre-renamed before
    /// morpher so no post-pass can corrupt parent-scope text.
    fn expand_component_sources(
        &self,
        comp: &morph_parser::MxComponent,
        frame: &InstanceFrame,
        ctx: &mut BuilderCtx,
        extra_headers: &mut Vec<String>,
    ) -> anyhow::Result<()> {
        for cst in &comp.consts {
            if cst.name.is_empty() || cst.rhs.is_empty() {
                continue;
            }
            let rhs = prepare_source(&cst.rhs, frame);
            let Some(out) = self.translate_logic(&rhs, &frame.vars, &frame.types, &frame.events)
            else {
                continue;
            };
            let expr = out.body.trim().trim_end_matches(';').trim();
            if expr.is_empty() {
                continue;
            }
            extra_headers.extend(include_lines(&out.includes));
            let mangled = frame.renames.get(&cst.name).cloned().unwrap_or_else(|| cst.name.clone());
            ctx.premain.push(format!("auto {mangled} = []() {{ return ({expr}); }};"));
            if !ctx.const_names.contains(&mangled) {
                ctx.const_names.push(mangled);
            }
        }
        for f in &comp.inner_functions {
            if f.name.is_empty() {
                continue;
            }
            let src = prepare_source(&f.source, frame);
            let Some(out) = self.translate_logic(&src, &frame.vars, &frame.types, &frame.events)
            else {
                continue;
            };
            let body = strip_static_linkage(&out.body);
            if body.is_empty() {
                continue;
            }
            extra_headers.extend(include_lines(&out.includes));
            if !ctx.premain.contains(&body) {
                ctx.premain.push(body);
            }
        }
        for e in &comp.effects {
            let callback = prepare_source(&e.callback, frame);
            let Some(out) =
                self.translate_logic(&callback, &frame.vars, &frame.types, &frame.events)
            else {
                continue;
            };
            let lambda = out.body.trim().trim_end_matches(';').trim().to_string();
            if lambda.is_empty() {
                continue;
            }
            extra_headers.extend(include_lines(&out.includes));
            let deps = rename_symbols(&e.deps, &frame.renames, frame);
            let mut m = HashMap::new();
            m.insert("lambda".into(), lambda);
            m.insert("deps".into(), deps);
            ctx.effects.push(m);
        }
        for sub in &comp.event_subs {
            let body = self.translate_event_sub(sub, &comp.name, frame, ctx, extra_headers)?;
            let mut m = HashMap::new();
            m.insert("channel".into(), frame.events.get(&sub.event).cloned().unwrap_or_default());
            m.insert("body".into(), body);
            ctx.channels.push(m);
        }
        Ok(())
    }

    /// Reject ambiguous member names within one component. An inner
    /// function or const sharing a name with a prop, state variable, or
    /// shared binding would mangle to the same `instN_` symbol and emit
    /// corrupt C++ (e.g. a prop read resolving to a function pointer).
    fn validate_member_names(
        comp: &morph_parser::MxComponent,
        module: &Path,
    ) -> anyhow::Result<()> {
        let mut reserved: HashMap<&str, &str> = HashMap::new();
        for p in &comp.props {
            reserved.entry(p.name.as_str()).or_insert("prop");
        }
        if !comp.props_param.is_empty() {
            reserved.entry(comp.props_param.as_str()).or_insert("props parameter");
        }
        for sv in &comp.state_vars {
            if !sv.getter.is_empty() {
                reserved.entry(sv.getter.as_str()).or_insert("state variable");
            }
            if !sv.setter.is_empty() {
                reserved.entry(sv.setter.as_str()).or_insert("state setter");
            }
        }
        for f in &comp.inner_functions {
            if f.name.is_empty() {
                continue;
            }
            if let Some(kind) = reserved.get(f.name.as_str()) {
                anyhow::bail!(
                    "component `{}` has an inner function named `{}` that collides with its {} (in {}); rename one",
                    comp.name,
                    f.name,
                    kind,
                    module.display()
                );
            }
        }
        for c in &comp.consts {
            if c.name.is_empty() {
                continue;
            }
            if let Some(kind) = reserved.get(c.name.as_str()) {
                anyhow::bail!(
                    "component `{}` has a const named `{}` that collides with its {} (in {}); rename one",
                    comp.name,
                    c.name,
                    kind,
                    module.display()
                );
            }
        }
        Ok(())
    }

    /// Translate one `<event>.on(handler)` subscription into a
    /// `[](const JsValue& __ch_N) { ... }` listener for the event's
    /// module-scoped channel id. Codegen pairs the returned listener with the
    /// subscription's `channel`; this function must not emit `.on(...)`
    /// itself. The handler must be an inline arrow/function
    /// of at most one parameter; `p.field` reads are rewritten to
    /// `p["field"]` for JsValue.
    fn translate_event_sub(
        &self,
        sub: &morph_parser::EventSub,
        comp_name: &str,
        frame: &InstanceFrame,
        ctx: &mut BuilderCtx,
        extra_headers: &mut Vec<String>,
    ) -> anyhow::Result<String> {
        frame.events.get(&sub.event).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown event `{}` used in `{comp_name}` ({}): export it from a module with `morphEvent` and import it here",
                sub.event,
                frame.module.display()
            )
        })?;
        if sub.handler.is_empty() {
            anyhow::bail!("`{}.on(...)` needs a handler (in `{comp_name}`)", sub.event);
        }
        let (params, body, is_block) = parse_callable_source(&sub.handler).map_err(|e| {
            anyhow::anyhow!("`{}.on(...)` in `{comp_name}`: {e} (pass an inline arrow)", sub.event)
        })?;
        if params.len() > 1 {
            anyhow::bail!(
                "`{}.on(...)` handler takes at most one parameter (in `{comp_name}`)",
                sub.event
            );
        }
        let ch_param = format!("__ch_{}", ctx.next_channel);
        ctx.next_channel += 1;
        let inner_src = if is_block {
            let b = body.trim();
            b[1..b.len().saturating_sub(1)].trim().to_string()
        } else {
            body
        };
        let scoped = match params.first() {
            Some(p) => {
                let mut m = HashMap::new();
                m.insert(p.clone(), ch_param.clone());
                rename_symbols(&inner_src, &m, frame)
            }
            None => inner_src,
        };
        let prepared = prepare_source(&scoped, frame);
        let Some(out) = self.translate_logic(&prepared, &frame.vars, &frame.types, &frame.events)
        else {
            anyhow::bail!(
                "`{}.on(...)` handler could not be translated (in `{comp_name}`)",
                sub.event
            );
        };
        extra_headers.extend(include_lines(&out.includes));
        let cpp = out.body.trim().trim_end_matches(';').trim().to_string();
        let stmts = if cpp.is_empty() || cpp.ends_with('}') { cpp } else { format!("{cpp};") };
        let lambda = format!("[](const JsValue& {ch_param}) {{ {stmts} }}");
        Ok(rewrite_channel_access(&lambda, &ch_param))
    }

    /// Register a module's exported `morphShared`/`morphEvent` bindings plus
    /// its imported ones into `frame` (the module's ambient maps) and record
    /// their namespaced signal accessors / channel ids in `ctx`.
    ///
    /// Identity = canonical module path + binding name, so same-named
    /// bindings in different files never collide. Visible bindings are the
    /// module's own exports plus named imports from directly-imported
    /// modules; importing the same local name from two different modules is
    /// ambiguous and rejected.
    ///
    /// # Errors
    /// Rejects ambiguous imported binding names.
    // Splitting this builder function risks behavior change.
    #[allow(clippy::too_many_lines)]
    fn seed_module_bindings(
        module: &morph_parser::MxSource,
        module_path: &Path,
        graph: &morph_parser::ModuleGraph,
        frame: &mut InstanceFrame,
        ctx: &mut BuilderCtx,
    ) -> anyhow::Result<()> {
        let ns = ns_of(graph, module_path)?;
        let mut seeded: HashSet<String> = HashSet::new();
        for sb in &module.shared_bindings {
            if sb.init.is_empty() {
                anyhow::bail!(
                    "export {{ {getter}, {setter} }} must give morphShared an initial value (in {path})",
                    getter = sb.getter,
                    setter = sb.setter,
                    path = module_path.display()
                );
            }
            let accessor = shared_signal_accessor(&sb.getter);
            let ty = shared_binding_type(&sb.type_arg, &sb.init);
            Self::register_binding(
                module_path,
                &ns,
                &accessor,
                &ty,
                &sb.init,
                &sb.getter,
                &sb.setter,
                ctx,
            );
            frame
                .vars
                .insert(sb.getter.clone(), format!("::{MODULE_NS_ROOT}::{ns}::{accessor}().get()"));
            frame
                .vars
                .insert(sb.setter.clone(), format!("::{MODULE_NS_ROOT}::{ns}::{accessor}().set"));
            if ty != "auto" {
                frame.types.insert(sb.getter.clone(), ty);
            }
            seeded.insert(sb.getter.clone());
            seeded.insert(sb.setter.clone());
        }
        for eb in &module.event_bindings {
            frame.events.insert(eb.name.clone(), event_channel_id(module_path, &eb.name));
            Self::register_event(module_path, &ns, &eb.name, ctx);
            seeded.insert(eb.name.clone());
        }
        // Own functions/vars/classes: every binding lives in its defining
        // file's namespace (same universal rule as shared/events), so even
        // unexported helpers rewrite to qualified calls at use sites.
        for fd in &module.function_declarations {
            Self::register_module_binding(module_path, &ns, "function", &fd.name, ctx);
            let qualified = format!("::{MODULE_NS_ROOT}::{ns}::{}", binding_ident(&fd.name));
            Self::seed_frame_name(frame, module_path, &fd.name, &qualified)?;
            seeded.insert(fd.name.clone());
        }
        for ev in &module.exported_vars {
            Self::register_module_binding(module_path, &ns, "var", &ev.name, ctx);
            let qualified = format!("::{MODULE_NS_ROOT}::{ns}::{}", binding_ident(&ev.name));
            Self::seed_frame_name(frame, module_path, &ev.name, &qualified)?;
            seeded.insert(ev.name.clone());
        }
        for cd in &module.class_declarations {
            Self::register_module_binding(module_path, &ns, "class", &cd.name, ctx);
            let qualified = format!("::{MODULE_NS_ROOT}::{ns}::{}", binding_ident(&cd.name));
            Self::seed_frame_name(frame, module_path, &cd.name, &qualified)?;
            seeded.insert(cd.name.clone());
        }
        // Own re-export aliases (`export { x } from`, `export *`).
        Self::seed_re_exports(module, module_path, graph, &ns, ctx)?;
        // Named imports of directly-imported modules expose their bindings.
        let Some(resolved_mod) = graph.modules.get(module_path) else {
            return Ok(());
        };
        for (raw_path, target_path) in &resolved_mod.module_imports {
            let Some(target_mod) = graph.modules.get(target_path) else { continue };
            for imp in &module.imports {
                let morph_parser::MxImportKind::Component { path, specifiers, default } = &imp.kind
                else {
                    continue;
                };
                if path != raw_path {
                    continue;
                }
                if let Some(local) = default {
                    Self::seed_default_import(
                        &target_mod.source,
                        target_path,
                        graph,
                        local,
                        module_path,
                        frame,
                        ctx,
                    )?;
                }
                for (local, imported) in specifiers {
                    if !seeded.insert(local.clone()) {
                        anyhow::bail!(
                            "ambiguous import of `{local}` in {}: it is both an export and an import, or imported from two different modules; import it from only one module",
                            module_path.display()
                        );
                    }
                    Self::seed_named_import(
                        &target_mod.source,
                        target_path,
                        graph,
                        local,
                        imported,
                        module_path,
                        frame,
                        ctx,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Resolve one named import specifier against the target's full
    /// inventory, following re-exports to the ultimate definition site.
    /// Components resolve at use sites (`resolve_binding`) and are skipped
    /// here; anything else unknown is a hard error.
    #[allow(clippy::too_many_arguments)]
    fn seed_named_import(
        target: &morph_parser::MxSource,
        target_path: &Path,
        graph: &morph_parser::ModuleGraph,
        local: &str,
        imported: &str,
        module_path: &Path,
        frame: &mut InstanceFrame,
        ctx: &mut BuilderCtx,
    ) -> anyhow::Result<()> {
        let target_ns = ns_of(graph, target_path)?;
        // Components keep use-site resolution.
        if target.components.iter().any(|c| c.name == imported && c.exported) {
            return Ok(());
        }
        if let Some(sb) = target.shared_bindings.iter().find(|b| b.getter == imported) {
            let accessor = shared_signal_accessor(&sb.getter);
            let ty = shared_binding_type(&sb.type_arg, &sb.init);
            Self::register_binding(
                target_path,
                &target_ns,
                &accessor,
                &ty,
                &sb.init,
                &sb.getter,
                &sb.setter,
                ctx,
            );
            let expr = format!("::{MODULE_NS_ROOT}::{target_ns}::{accessor}().get()");
            Self::seed_frame_name(frame, module_path, local, &expr)?;
            Self::register_import(module_path, local, &target_ns, &sb.getter, &expr, ctx);
            if !sb.setter.is_empty() {
                let setter_expr = format!("::{MODULE_NS_ROOT}::{target_ns}::{accessor}().set");
                Self::seed_frame_name(frame, module_path, &sb.setter, &setter_expr)?;
                Self::register_import(
                    module_path,
                    &sb.setter,
                    &target_ns,
                    &sb.setter,
                    &setter_expr,
                    ctx,
                );
            }
            if ty != "auto" {
                frame.types.insert(local.to_string(), ty);
            }
            return Ok(());
        }
        if let Some(sb) = target.shared_bindings.iter().find(|b| b.setter == imported) {
            let accessor = shared_signal_accessor(&sb.getter);
            let ty = shared_binding_type(&sb.type_arg, &sb.init);
            Self::register_binding(
                target_path,
                &target_ns,
                &accessor,
                &ty,
                &sb.init,
                &sb.getter,
                &sb.setter,
                ctx,
            );
            let expr = format!("::{MODULE_NS_ROOT}::{target_ns}::{accessor}().set");
            Self::seed_frame_name(frame, module_path, local, &expr)?;
            Self::register_import(module_path, local, &target_ns, &sb.setter, &expr, ctx);
            return Ok(());
        }
        if target.event_bindings.iter().any(|b| b.name == imported) {
            let id = event_channel_id(target_path, imported);
            if frame.events.insert(local.to_string(), id).is_some() {
                anyhow::bail!(
                    "ambiguous import of `{local}` in {}: bound by two different modules; import it from only one module",
                    module_path.display()
                );
            }
            Self::register_event(target_path, &target_ns, imported, ctx);
            return Ok(());
        }
        if target.function_declarations.iter().any(|f| f.name == imported && f.exported)
            || target.named_exports.iter().any(|(l, _)| l == imported)
        {
            Self::register_module_binding(target_path, &target_ns, "function", imported, ctx);
            let qualified = format!("::{MODULE_NS_ROOT}::{target_ns}::{}", binding_ident(imported));
            Self::seed_frame_name(frame, module_path, local, &qualified)?;
            Self::register_import(module_path, local, &target_ns, imported, &qualified, ctx);
            return Ok(());
        }
        if target.exported_vars.iter().any(|v| v.name == imported) {
            Self::register_module_binding(target_path, &target_ns, "var", imported, ctx);
            let qualified = format!("::{MODULE_NS_ROOT}::{target_ns}::{}", binding_ident(imported));
            Self::seed_frame_name(frame, module_path, local, &qualified)?;
            Self::register_import(module_path, local, &target_ns, imported, &qualified, ctx);
            return Ok(());
        }
        if target.class_declarations.iter().any(|c| c.name == imported && c.exported) {
            Self::register_module_binding(target_path, &target_ns, "class", imported, ctx);
            let qualified = format!("::{MODULE_NS_ROOT}::{target_ns}::{}", binding_ident(imported));
            Self::seed_frame_name(frame, module_path, local, &qualified)?;
            Self::register_import(module_path, local, &target_ns, imported, &qualified, ctx);
            return Ok(());
        }
        // Through re-exports: resolve to the ultimate definition site.
        if let Some((ultimate_ns, ultimate_name)) =
            Self::resolve_through_reexports(target_path, graph, imported, module_path)?
        {
            let qualified =
                format!("::{MODULE_NS_ROOT}::{ultimate_ns}::{}", binding_ident(&ultimate_name));
            Self::seed_frame_name(frame, module_path, local, &qualified)?;
            Self::register_import(
                module_path,
                local,
                &ultimate_ns,
                &ultimate_name,
                &qualified,
                ctx,
            );
            return Ok(());
        }
        anyhow::bail!(
            "unknown import `{imported}` in {}: {} exports no such binding (function, var, class, component, shared, or event)",
            module_path.display(),
            target_path.display()
        )
    }

    /// Resolve a default import against the target's default export (any
    /// non-component kind). Components keep use-site resolution and are
    /// skipped; unresolvable defaults stay silent here (use sites still
    /// error, preserving existing behavior for unused imports).
    #[allow(clippy::too_many_arguments)]
    fn seed_default_import(
        target: &morph_parser::MxSource,
        target_path: &Path,
        graph: &morph_parser::ModuleGraph,
        local: &str,
        module_path: &Path,
        frame: &mut InstanceFrame,
        ctx: &mut BuilderCtx,
    ) -> anyhow::Result<()> {
        let Some(decl) = target.default_export.clone() else {
            return Ok(());
        };
        if target.components.iter().any(|c| c.name == decl) {
            return Ok(());
        }
        let target_ns = ns_of(graph, target_path)?;
        let kind = if target.function_declarations.iter().any(|f| f.name == decl) {
            "function"
        } else if target.class_declarations.iter().any(|c| c.name == decl) {
            "class"
        } else if target.exported_vars.iter().any(|v| v.name == decl) {
            "var"
        } else if let Some(sb) = target.shared_bindings.iter().find(|b| b.getter == decl) {
            // Shared default: map to the getter read.
            let accessor = shared_signal_accessor(&sb.getter);
            let ty = shared_binding_type(&sb.type_arg, &sb.init);
            Self::register_binding(
                target_path,
                &target_ns,
                &accessor,
                &ty,
                &sb.init,
                &sb.getter,
                &sb.setter,
                ctx,
            );
            let expr = format!("::{MODULE_NS_ROOT}::{target_ns}::{accessor}().get()");
            Self::seed_frame_name(frame, module_path, local, &expr)?;
            Self::register_import(module_path, local, &target_ns, &sb.getter, &expr, ctx);
            return Ok(());
        } else {
            return Ok(());
        };
        Self::register_module_binding(target_path, &target_ns, kind, &decl, ctx);
        let qualified = format!("::{MODULE_NS_ROOT}::{target_ns}::{}", binding_ident(&decl));
        Self::seed_frame_name(frame, module_path, local, &qualified)?;
        Self::register_import(module_path, local, &target_ns, &decl, &qualified, ctx);
        Ok(())
    }

    /// Follow a target's re-exports to the ultimate `(ns, name)` definition
    /// site for `imported`. `None` when the target re-exports nothing by
    /// that name. Cycles are hard errors.
    fn resolve_through_reexports(
        target_path: &Path,
        graph: &morph_parser::ModuleGraph,
        imported: &str,
        module_path: &Path,
    ) -> anyhow::Result<Option<(String, String)>> {
        // NOTE: no early exit when the first module lacks re-exports — it
        // may directly define the name (the loop's first iteration checks).
        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut current_path = target_path.to_path_buf();
        let mut current_name = imported.to_string();
        loop {
            if !visited.insert(current_path.clone()) {
                anyhow::bail!(
                    "re-export cycle while resolving `{imported}` imported in {}",
                    module_path.display()
                );
            }
            let Some(current_mod) = graph.modules.get(&current_path) else {
                return Ok(None);
            };
            // Direct definition wins over further re-exports.
            if Self::target_defines(&current_mod.source, &current_name) {
                let ns = ns_of(graph, &current_path)?;
                return Ok(Some((ns, current_name)));
            }
            let mut advanced = false;
            for re in &current_mod.source.re_exports {
                let Some(orig) = Self::reexport_provides(re, &current_name) else {
                    continue;
                };
                let Some(next) = Self::resolve_module_path(&current_mod, &re.path) else {
                    continue;
                };
                current_path = next;
                current_name = orig;
                advanced = true;
                break;
            }
            if !advanced {
                return Ok(None);
            }
        }
    }

    /// True when the module directly defines `name` as an importable value
    /// binding (components resolve separately at use sites).
    fn target_defines(source: &morph_parser::MxSource, name: &str) -> bool {
        source.shared_bindings.iter().any(|b| b.getter == name || b.setter == name)
            || source.event_bindings.iter().any(|b| b.name == name)
            || source.function_declarations.iter().any(|f| f.name == name && f.exported)
            || source.exported_vars.iter().any(|v| v.name == name)
            || source.class_declarations.iter().any(|c| c.name == name && c.exported)
            || source.named_exports.iter().any(|(l, _)| l == name)
            || source.default_export.as_deref() == Some(name)
    }

    /// Original name a re-export provides under `exported`, if any. Star
    /// re-exports provide every name (resolved against the target's
    /// inventory by the caller loop).
    fn reexport_provides(re: &morph_parser::ReExport, exported: &str) -> Option<String> {
        if re.star {
            return Some(exported.to_string());
        }
        re.names.iter().find(|(_, e)| e == exported).map(|(o, _)| o.clone())
    }

    /// Resolve a re-export/import path against the owning module's resolved
    /// imports.
    fn resolve_module_path(owner: &morph_parser::ResolvedModule, raw: &str) -> Option<PathBuf> {
        owner.module_imports.iter().find(|(p, _)| p == raw).map(|(_, t)| t.clone())
    }

    /// Register the module's own re-export aliases (`using` targets in the
    /// re-exporting namespace, so C++ can call re-exported things).
    /// Re-exported names are NOT local bindings (ES semantics), so the
    /// frame map stays untouched here.
    fn seed_re_exports(
        module: &morph_parser::MxSource,
        module_path: &Path,
        graph: &morph_parser::ModuleGraph,
        ns: &str,
        ctx: &mut BuilderCtx,
    ) -> anyhow::Result<()> {
        let Some(resolved_mod) = graph.modules.get(module_path) else {
            return Ok(());
        };
        for re in &module.re_exports {
            let Some(target_path) = Self::resolve_module_path(resolved_mod, &re.path) else {
                continue;
            };
            let Some(target_mod) = graph.modules.get(&target_path) else {
                continue;
            };
            if re.star {
                if re.star_as.is_some() {
                    anyhow::bail!(
                        "`export * as ns` in {} is not supported yet; name re-exports explicitly",
                        module_path.display()
                    );
                }
                // Alias every function/var/class binding of the target
                // (components/shared/events resolve through to the
                // ultimate namespace on import and need no alias).
                let mut names = Self::exportable_names(&target_mod.source);
                names.sort();
                for (name, kind) in names {
                    if kind != "function" && kind != "var" && kind != "class" {
                        continue;
                    }
                    let resolved =
                        Self::resolve_through_reexports(&target_path, graph, &name, module_path)?;
                    let (ultimate_ns, ultimate_name) = match resolved {
                        Some(r) => r,
                        None => (ns_of(graph, &target_path)?, name.clone()),
                    };
                    Self::register_alias(
                        module_path,
                        ns,
                        &name,
                        &ultimate_ns,
                        &ultimate_name,
                        ctx,
                    )?;
                }
                continue;
            }
            for (orig, exported) in &re.names {
                if exported == "default" {
                    anyhow::bail!(
                        "cannot re-export `default` under its own name in {}: rename it (`export {{ default as x }}`)",
                        module_path.display()
                    );
                }
                let (ultimate_ns, ultimate_name) =
                    Self::resolve_through_reexports(&target_path, graph, orig, module_path)?
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "unknown re-export `{orig}` in {}: {} exports no such binding",
                                module_path.display(),
                                target_path.display()
                            )
                        })?;
                Self::register_alias(module_path, ns, exported, &ultimate_ns, &ultimate_name, ctx)?;
            }
        }
        Ok(())
    }

    /// Every exportable value binding of a module (no default) with its
    /// kind: components, shared, events, functions, vars, classes, and
    /// `export {}`-listed locals.
    fn exportable_names(source: &morph_parser::MxSource) -> Vec<(String, &'static str)> {
        let mut names = HashSet::new();
        for c in &source.components {
            if c.exported {
                names.insert((c.name.clone(), "component"));
            }
        }
        for sb in &source.shared_bindings {
            names.insert((sb.getter.clone(), "shared"));
            if !sb.setter.is_empty() {
                names.insert((sb.setter.clone(), "shared"));
            }
        }
        for eb in &source.event_bindings {
            names.insert((eb.name.clone(), "event"));
        }
        for fd in &source.function_declarations {
            if fd.exported {
                names.insert((fd.name.clone(), "function"));
            }
        }
        for ev in &source.exported_vars {
            names.insert((ev.name.clone(), "var"));
        }
        for cd in &source.class_declarations {
            if cd.exported {
                names.insert((cd.name.clone(), "class"));
            }
        }
        for (l, e) in &source.named_exports {
            names.insert((l.clone(), "named"));
            names.insert((e.clone(), "named"));
        }
        names.into_iter().collect()
    }

    /// Register one shared binding's C++ emission metadata, deduped by its
    /// identity key (module path + getter name).
    fn register_binding(
        module_path: &Path,
        ns: &str,
        accessor: &str,
        ty: &str,
        init: &str,
        getter: &str,
        setter: &str,
        ctx: &mut BuilderCtx,
    ) {
        let mut m = HashMap::new();
        m.insert("key".into(), binding_identity(module_path, getter));
        m.insert("ns".into(), ns.to_string());
        m.insert("accessor".into(), accessor.to_string());
        m.insert("type".into(), ty.to_string());
        m.insert("init".into(), init.to_string());
        m.insert("getter".into(), getter.to_string());
        m.insert("setter".into(), setter.to_string());
        if !ctx.shared_entries.iter().any(|e| e.get("key") == m.get("key")) {
            ctx.shared_entries.push(m);
        }
    }

    /// Register one function/var/class binding's C++ emission metadata,
    /// deduped by its identity key (module path + name). Every binding
    /// lives in its defining file's namespace — the universal rule that
    /// makes cross-file same-name definitions coexist.
    fn register_module_binding(
        module_path: &Path,
        ns: &str,
        kind: &str,
        name: &str,
        ctx: &mut BuilderCtx,
    ) {
        let mut m = HashMap::new();
        m.insert("key".into(), binding_identity(module_path, name));
        m.insert("kind".into(), kind.to_string());
        m.insert("ns".into(), ns.to_string());
        m.insert("name".into(), name.to_string());
        m.insert("module".into(), module_path.display().to_string());
        if !ctx.module_bindings.iter().any(|e| e.get("key") == m.get("key")) {
            ctx.module_bindings.push(m);
        }
    }

    /// Register one import mapping for codegen (`local` → `expr` in the
    /// importing module): reactive text and list factories substitute
    /// through codegen maps, never builder frames, so imports need IR
    /// entries just like definitions do. Deduped by importer + local.
    fn register_import(
        importer_path: &Path,
        local: &str,
        ultimate_ns: &str,
        ultimate_name: &str,
        expr: &str,
        ctx: &mut BuilderCtx,
    ) {
        let mut m = HashMap::new();
        m.insert("key".into(), binding_identity(importer_path, local));
        m.insert("kind".into(), "import".to_string());
        m.insert("ns".into(), ultimate_ns.to_string());
        m.insert("name".into(), ultimate_name.to_string());
        m.insert("local".into(), local.to_string());
        m.insert("expr".into(), expr.to_string());
        m.insert("module".into(), importer_path.display().to_string());
        if !ctx.module_bindings.iter().any(|e| e.get("key") == m.get("key")) {
            ctx.module_bindings.push(m);
        }
    }

    /// Register a re-export alias (`using` target in the re-exporting
    /// namespace, so C++ can call re-exported things). Deduped by the
    /// alias identity (re-exporting module + exported name).
    fn register_alias(
        module_path: &Path,
        ns: &str,
        name: &str,
        target_ns: &str,
        target_name: &str,
        ctx: &mut BuilderCtx,
    ) -> anyhow::Result<()> {
        let mut m = HashMap::new();
        m.insert("key".into(), binding_identity(module_path, name));
        m.insert("kind".into(), "alias".to_string());
        m.insert("ns".into(), ns.to_string());
        m.insert("name".into(), name.to_string());
        m.insert("target_ns".into(), target_ns.to_string());
        m.insert("target_name".into(), target_name.to_string());
        m.insert("module".into(), module_path.display().to_string());
        if let Some(existing) = ctx.module_bindings.iter().find(|e| e.get("key") == m.get("key")) {
            // Same identity, different meaning (e.g. own `function x` plus
            // `export { x } from ...`): the exported name is ambiguous.
            let same = existing.get("kind") == m.get("kind")
                && existing.get("target_ns") == m.get("target_ns")
                && existing.get("target_name") == m.get("target_name");
            if !same {
                anyhow::bail!(
                    "ambiguous export `{}` in {}: locally defined and re-exported; rename one",
                    name,
                    module_path.display()
                );
            }
            return Ok(());
        }
        ctx.module_bindings.push(m);
        Ok(())
    }

    /// Seed one module-level name into the frame (`name` →
    /// `app::<ns>::<safe_name>`). A different existing mapping is
    /// an ambiguity hard error; an identical one is a harmless re-seed.
    /// Never touches `seeded` (companion names like shared setters must
    /// not trip the explicit-import ambiguity gate); callers track
    /// explicitly-listed names themselves.
    fn seed_frame_name(
        frame: &mut InstanceFrame,
        module_path: &Path,
        name: &str,
        qualified: &str,
    ) -> anyhow::Result<()> {
        if let Some(existing) = frame.vars.get(name) {
            if existing != qualified {
                anyhow::bail!(
                    "ambiguous binding `{name}` in {}: bound by two different modules; import it from only one module",
                    module_path.display()
                );
            }
            return Ok(());
        }
        frame.vars.insert(name.to_string(), qualified.to_string());
        Ok(())
    }

    /// Register one event binding's C++ emission metadata, deduped by its
    /// identity key (module path + event name). The build TU emits one
    /// static `Channel` per entry and lowers string channel references
    /// to the namespace accessor.
    fn register_event(module_path: &Path, ns: &str, name: &str, ctx: &mut BuilderCtx) {
        let mut m = HashMap::new();
        m.insert("key".into(), binding_identity(module_path, name));
        m.insert("ns".into(), ns.to_string());
        m.insert("accessor".into(), event_channel_accessor(name));
        m.insert("event".into(), name.to_string());
        m.insert("channel".into(), event_channel_id(module_path, name));
        m.insert("module".into(), module_path.display().to_string());
        if !ctx.event_entries.iter().any(|e| e.get("key") == m.get("key")) {
            ctx.event_entries.push(m);
        }
    }

    /// Validate a `mid` (Morph ID) attribute value. `mid` exists only at
    /// component use-sites (`<Hero mid="something" />`) for native C++
    /// identification — never inside a component definition, never on
    /// native elements (frontend identity is `id`; the two coexist on one
    /// component). Literal-only, letters-only, canonicalized to
    /// lowercase: `mid="hero"` is valid; `mid={x}` / `mid="item1"` / bare
    /// `mid` are hard errors.
    fn validate_mid(
        tag: &str,
        value: &morph_parser::JsxPropValue,
        line: usize,
        col: usize,
    ) -> anyhow::Result<String> {
        let raw = match value {
            morph_parser::JsxPropValue::String(s) => s.clone(),
            _ => {
                anyhow::bail!(
                    "`mid` on <{tag}> ({line}:{col}) must be a string literal (`mid=\"hero\"`); dynamic values (`mid={{...}}`) are rejected so instance identity always resolves at build time"
                );
            }
        };
        if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_alphabetic()) {
            anyhow::bail!(
                "`mid=\"{raw}\"` on <{tag}> ({line}:{col}) is invalid: letters only (`[a-zA-Z]`, any case), no digits or symbols; use descriptive names like `heroPrimary`"
            );
        }
        Ok(raw.to_lowercase())
    }

    /// C++ constant name for a `mid` value: `hero` → `MID_HERO`.
    fn mid_const_name(mid: &str) -> String {
        let safe: String = mid
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .collect();
        format!("MID_{}", safe.to_uppercase())
    }

    /// Emit one assignment entry per (tagged instance × state slot) for
    /// indexed native access. Index is the ordinal among TAGGED instances
    /// of the component type (untagged instances never shift `MID_*`
    /// values). Entries carry everything codegen needs: component
    /// namespace, constant, index, backing signal, type init, and source
    /// location for header mapping comments.
    fn mid_assignments(
        graph: &morph_parser::ModuleGraph,
        ctx: &BuilderCtx,
        // Matches `seed_state_var`: `"__st_"` for entry globals,
        // `"ctx->"` for route mount contexts.
        signal_prefix: &str,
    ) -> anyhow::Result<Vec<HashMap<String, String>>> {
        let mut out = Vec::new();
        let mut types: Vec<(&(PathBuf, String), &Vec<(String, usize, usize, usize, PathBuf)>)> =
            ctx.mid_tags.iter().collect();
        types.sort_by(|a, b| a.0.cmp(b.0));
        for ((module, comp_name), type_tags) in types {
            let module_ns = ns_of(graph, module)?;
            // Component scope: module stem already names single-component
            // modules (`Counter.mx` → `...::counter`), so only append the
            // component name when it differs.
            let stem_last = module_ns.rsplit("::").next().unwrap_or("");
            let comp_seg: String = comp_name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
                .collect::<String>()
                .to_lowercase();
            let comp_ns = if comp_seg == stem_last {
                module_ns.clone()
            } else {
                format!("{module_ns}::{comp_seg}")
            };
            let mut tags = type_tags.clone();
            tags.sort_by_key(|(_, instance_no, _, _, _)| *instance_no);
            for (index, (mid, instance_no, line, col, use_site)) in tags.iter().enumerate() {
                let Some(slots) = ctx.mid_states.get(instance_no) else {
                    continue;
                };
                for (suffix_getter, suffix_setter, init) in slots {
                    let mut m = HashMap::new();
                    m.insert("key".into(), format!("{}::{comp_name}::{mid}", module.display()));
                    m.insert("ns".into(), comp_ns.clone());
                    m.insert("comp".into(), comp_name.clone());
                    m.insert("mid".into(), mid.clone());
                    m.insert("const".into(), Self::mid_const_name(mid));
                    m.insert("index".into(), index.to_string());
                    m.insert(
                        "signal".into(),
                        format!("{signal_prefix}inst{instance_no}_{suffix_getter}"),
                    );
                    m.insert("getter".into(), suffix_getter.clone());
                    m.insert("setter".into(), suffix_setter.clone());
                    m.insert("init".into(), init.clone());
                    m.insert("module".into(), use_site.display().to_string());
                    m.insert("loc".into(), format!("{line}:{col}"));
                    out.push(m);
                }
            }
        }
        Ok(out)
    }

    /// Expand one `<Tag ... />` instantiation into its root IR node.
    ///
    /// # Errors
    /// Rejects children, unresolvable tags, render cycles, and bad props.
    #[allow(clippy::too_many_arguments)]
    fn expand_instance(
        &self,
        graph: &morph_parser::ModuleGraph,
        ctx: &mut BuilderCtx,
        parent_frame: &InstanceFrame,
        tag: &str,
        call_props: &HashMap<String, morph_parser::JsxPropValue>,
        children: &[morph_parser::JsxNode],
        line: usize,
        col: usize,
        css_rules: &[(String, morph_parser::CssRule)],
        depth: usize,
        ancestors: &[AncestorHint],
        extra_headers: &mut Vec<String>,
        keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> anyhow::Result<IRNode> {
        if !children.is_empty() {
            anyhow::bail!(
                "component <{tag}> does not accept children in v1 ({line}:{col}); pass content via props instead"
            );
        }
        let (target_module, comp) = resolve_binding(graph, &parent_frame.module, tag)
            .map_err(|e| anyhow::anyhow!("{e} (used at {line}:{col})"))?;
        if ctx.stack.iter().any(|(m, c)| m == &target_module && c == &comp.name) {
            let mut chain: Vec<String> = ctx.stack.iter().map(|(_, c)| c.clone()).collect();
            chain.push(comp.name.clone());
            anyhow::bail!("cyclic component instantiation: {}", chain.join(" → "));
        }
        if comp.props.iter().any(|p| p.name == "mid") || comp.props_param == "mid" {
            anyhow::bail!(
                "component `{}` declares a prop named `mid` ({}): `mid` is reserved for native instance identity; rename it",
                comp.name,
                target_module.display()
            );
        }
        let instance_no = ctx.next_instance;
        ctx.next_instance += 1;
        let prefix = format!("inst{instance_no}");
        // `mid` is reserved (like `key`): claim it here so it never
        // reaches prop binding, and record the tag for native indexing.
        let bound_props: HashMap<String, morph_parser::JsxPropValue> = call_props
            .iter()
            .filter(|(k, _)| k.as_str() != "mid")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if let Some(mid_value) = call_props.get("mid") {
            // Canonical form: lowercased (`Hero` ≡ `hero`).
            let mid = Self::validate_mid(tag, mid_value, line, col)?;
            if ctx.list_depth > 0 {
                anyhow::bail!(
                    "`mid=\"{mid}\"` on <{tag}> ({line}:{col}): list items share one state slot per template, so per-item identity does not exist; remove `mid` here"
                );
            }
            let tags = ctx.mid_tags.entry((target_module.clone(), comp.name.clone())).or_default();
            if let Some((_, _, first_line, first_col, _)) =
                tags.iter().find(|(m, _, _, _, _)| m == &mid)
            {
                anyhow::bail!(
                    "duplicate `mid=\"{mid}\"` on <{tag}> ({line}:{col}): already used at {first_line}:{first_col}; `mid` must be unique per component type"
                );
            }
            tags.push((mid, instance_no, line, col, parent_frame.module.clone()));
        }
        ctx.stack.push((target_module.clone(), comp.name.clone()));

        if !ctx.modules_emitted.contains(&target_module) {
            ctx.modules_emitted.insert(target_module.clone());
            if let Some(module) = graph.modules.get(&target_module) {
                let mut neutral = InstanceFrame::root(module.path.clone());
                Self::seed_module_bindings(&module.source, &module.path, graph, &mut neutral, ctx)?;
                let snapshot = std::mem::take(&mut ctx.premain);
                self.translate_module_globals(&module.source, &neutral, ctx, extra_headers);
                let mut added = std::mem::replace(&mut ctx.premain, snapshot);
                added.retain(|p| !ctx.premain.contains(p));
                ctx.premain.extend(added);
                for log in &module.source.console_logs {
                    if !ctx.logs.contains(log) {
                        ctx.logs.push(log.clone());
                    }
                }
            }
        }

        let comp_module_source = graph
            .modules
            .get(&target_module)
            .map(|m| &m.source)
            .ok_or_else(|| anyhow::anyhow!("module for <{tag}> is missing from the graph"))?;
        let mut frame = InstanceFrame::instance(target_module.clone());
        frame.props_param.clone_from(&comp.props_param);

        Self::seed_module_bindings(comp_module_source, &target_module, graph, &mut frame, ctx)?;

        for sv in &comp.state_vars {
            let getter = format!("{prefix}_{}", sv.getter);
            let setter = format!("{prefix}_{}", sv.setter);
            ctx.states.push(state_slot(
                &getter,
                &setter,
                &sv.init,
                true,
                &ns_of(graph, &target_module).unwrap_or_default(),
            ));
            if !sv.getter.is_empty() {
                Self::seed_state_var(
                    &mut frame,
                    &sv.getter,
                    &sv.setter,
                    &sv.init,
                    &getter,
                    &setter,
                    ctx.signal_prefix,
                );
                // State slots backing indexed native access (`mid`).
                ctx.mid_states.entry(instance_no).or_default().push((
                    sv.getter.clone(),
                    sv.setter.clone(),
                    sv.init.clone(),
                ));
            }
        }
        Self::preseed_sibling_names(&comp, &mut frame, Some(&prefix));
        self.bind_props(
            ctx,
            &comp,
            &bound_props,
            &mut frame,
            parent_frame,
            extra_headers,
            &prefix,
            tag,
            line,
            col,
        )?;
        self.expand_component_sources(&comp, &frame, ctx, extra_headers)?;
        for log in &comp.console_logs {
            if !ctx.logs.contains(log) {
                ctx.logs.push(log.clone());
            }
        }

        let node = self.build_node_in(
            &comp.jsx,
            css_rules,
            depth,
            ancestors,
            &frame,
            ctx,
            extra_headers,
            keyframes,
        )?;
        ctx.stack.pop();
        Ok(node)
    }

    /// Bind call-site JSX attributes to a component's declared props.
    /// `frame` must already carry the instance's states + sibling names.
    ///
    /// # Errors
    /// Rejects missing required props, unknown props, and props on
    /// components that declare none.
    #[allow(clippy::too_many_arguments)]
    fn bind_props(
        &self,
        ctx: &mut BuilderCtx,
        comp: &morph_parser::MxComponent,
        call_props: &HashMap<String, morph_parser::JsxPropValue>,
        frame: &mut InstanceFrame,
        parent_frame: &InstanceFrame,
        extra_headers: &mut Vec<String>,
        prefix: &str,
        tag: &str,
        line: usize,
        col: usize,
    ) -> anyhow::Result<()> {
        let call: HashMap<&String, &morph_parser::JsxPropValue> =
            call_props.iter().filter(|(k, _)| k.as_str() != "key").collect();
        if comp.props.is_empty() && comp.props_param.is_empty() {
            if let Some((name, _)) = call.iter().next() {
                anyhow::bail!(
                    "component <{tag}> takes no props but got `{name}` ({}:{}); declare `props: {{ ... }}` on `{}` to accept it",
                    line,
                    col,
                    comp.name
                );
            }
            return Ok(());
        }
        if comp.props.is_empty() {
            for (name, value) in &call {
                let expr = self.translate_call_prop(
                    ctx,
                    value,
                    parent_frame,
                    extra_headers,
                    None,
                    prefix,
                    tag,
                    line,
                    col,
                )?;
                frame.vars.insert((*name).clone(), expr.clone());
                frame.prop_binds.insert((*name).clone(), format!("({expr})"));
            }
            return Ok(());
        }
        for prop in &comp.props {
            match call.get(&prop.name) {
                Some(value) => {
                    let expr = self.translate_call_prop(
                        ctx,
                        value,
                        parent_frame,
                        extra_headers,
                        Some(prop),
                        prefix,
                        tag,
                        line,
                        col,
                    )?;
                    frame.vars.insert(prop.name.clone(), expr.clone());
                    if !prop.is_function() {
                        let class = ts_type_class(&prop.prop_type);
                        if !class.is_empty() {
                            frame.types.insert(prop.name.clone(), class);
                        }
                    }
                    frame.prop_binds.insert(prop.name.clone(), format!("({expr})"));
                }
                None if prop.optional => match prop_zero_value(prop) {
                    Some((expr, class)) => {
                        frame.vars.insert(prop.name.clone(), expr.clone());
                        frame.types.insert(prop.name.clone(), class);
                        frame.prop_binds.insert(prop.name.clone(), format!("({expr})"));
                    }
                    None => {
                        anyhow::bail!(
                                "optional prop `{}` of <{tag}> was omitted ({}:{}) but has no synthesizable default (type `{}`); pass it explicitly",
                                prop.name,
                                line,
                                col,
                                prop.prop_type
                            );
                    }
                },
                None => {
                    anyhow::bail!(
                        "component <{tag}> is missing required prop `{}` ({}:{}); declared props of `{}`: {}",
                        prop.name,
                        line,
                        col,
                        comp.name,
                        comp.props.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
                    );
                }
            }
        }
        for name in call.keys() {
            if comp.prop(name).is_none() {
                let known =
                    comp.props.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ");
                anyhow::bail!(
                    "unknown prop `{name}` on <{tag}> ({}:{}); `{}` declares: {known}",
                    line,
                    col,
                    comp.name
                );
            }
        }
        Ok(())
    }

    /// Translate one call-site prop value against the parent scope.
    ///
    /// Inline arrows on function-typed props become mangled premain
    /// functions typed from the declared signature.
    ///
    /// # Errors
    /// Rejects untranslatable values, inline functions on non-function
    /// props, and style objects as props.
    #[allow(clippy::too_many_arguments)]
    fn translate_call_prop(
        &self,
        ctx: &mut BuilderCtx,
        value: &morph_parser::JsxPropValue,
        parent_frame: &InstanceFrame,
        extra_headers: &mut Vec<String>,
        declared: Option<&morph_parser::ComponentProp>,
        prefix: &str,
        tag: &str,
        line: usize,
        col: usize,
    ) -> anyhow::Result<String> {
        match value {
            morph_parser::JsxPropValue::String(s) => {
                // `"1"` vs `1`: quote only for string-ish (or unknown) props.
                let string_like = declared
                    .is_none_or(|p| p.prop_type.is_empty() || p.prop_type.contains("string"));
                if string_like {
                    Ok(format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                } else {
                    Ok(s.clone())
                }
            }
            morph_parser::JsxPropValue::Bool => Ok("true".to_string()),
            morph_parser::JsxPropValue::Ref(name) => self
                .translate_logic(
                    name,
                    &parent_frame.vars,
                    &parent_frame.types,
                    &parent_frame.events,
                )
                .map(|out| {
                    extra_headers.extend(include_lines(&out.includes));
                    out.body.trim().trim_end_matches(';').trim().to_string()
                })
                .filter(|b| !b.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!("cannot resolve prop value `{name}` on <{tag}> ({line}:{col})")
                }),
            morph_parser::JsxPropValue::Expr(src) | morph_parser::JsxPropValue::Template(src) => {
                let src = subst_props_refs(src, parent_frame);
                self.translate_logic(
                    &src,
                    &parent_frame.vars,
                    &parent_frame.types,
                    &parent_frame.events,
                )
                .map(|out| {
                    extra_headers.extend(include_lines(&out.includes));
                    out.body.trim().trim_end_matches(';').trim().to_string()
                })
                .filter(|b| !b.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!("cannot translate prop value on <{tag}> ({line}:{col})")
                })
            }
            morph_parser::JsxPropValue::Fn(src) => {
                let prop = declared.filter(|p| p.is_function()).ok_or_else(|| {
                    anyhow::anyhow!(
                        "inline function passed to non-function prop on <{tag}> ({line}:{col}); declare the prop as `(...) => ...` to accept it"
                    )
                })?;
                self.synthesize_adapter(
                    ctx,
                    prop,
                    src,
                    parent_frame,
                    extra_headers,
                    prefix,
                    tag,
                    line,
                    col,
                )
            }
            morph_parser::JsxPropValue::Style(_) => {
                anyhow::bail!("style objects cannot be passed as props on <{tag}> ({line}:{col})");
            }
        }
    }

    /// Turn an inline arrow into a mangled premain function typed from the
    /// declared prop signature. The body captures the call site, so it
    /// translates against the parent frame.
    ///
    /// # Errors
    /// Rejects unsupported signatures (generics, optional/rest params),
    /// async bodies, arity mismatches, and untranslatable bodies.
    #[allow(clippy::too_many_arguments)]
    fn synthesize_adapter(
        &self,
        ctx: &mut BuilderCtx,
        prop: &morph_parser::ComponentProp,
        arrow_src: &str,
        parent_frame: &InstanceFrame,
        extra_headers: &mut Vec<String>,
        prefix: &str,
        tag: &str,
        line: usize,
        col: usize,
    ) -> anyhow::Result<String> {
        let (sig_params, ret) = parse_fn_type(&prop.prop_type).map_err(|e| {
            anyhow::anyhow!("function prop `{}` on <{tag}> ({}:{}): {e}", prop.name, line, col)
        })?;
        let (arrow_params, body, is_block) = parse_callable_source(arrow_src).map_err(|e| {
            anyhow::anyhow!("function prop `{}` on <{tag}> ({}:{}): {e}", prop.name, line, col)
        })?;
        if arrow_params.len() > sig_params.len() {
            anyhow::bail!(
                "function prop `{}` on <{tag}> ({}:{}) takes {} parameter(s) but the callback declares {}",
                prop.name, line, col, sig_params.len(), arrow_params.len()
            );
        }
        // Signature: arrow names (what the body references) with declared
        // types positionally; pad missing trailing params as unused.
        let mut params: Vec<String> = Vec::new();
        for (i, (pname, ptype, _)) in sig_params.iter().enumerate() {
            let name = arrow_params.get(i).cloned().unwrap_or_else(|| format!("_unused{i}"));
            params.push(format!("{name}: {ptype}"));
            let _ = pname;
        }
        let ret_ann = if ret.is_empty() { "void".to_string() } else { ret };
        let body_src = if is_block {
            body
        } else if ret_ann.trim() == "void" {
            format!("{body};")
        } else {
            format!("return ({body});")
        };
        let mangled = format!("{prefix}_prop_{}", prop.name);
        let ts_src =
            format!("function {mangled}({}): {ret_ann} {{ {body_src} }}", params.join(", "));
        let Some(out) = self.translate_logic(
            &ts_src,
            &parent_frame.vars,
            &parent_frame.types,
            &parent_frame.events,
        ) else {
            anyhow::bail!(
                "function prop `{}` on <{tag}> ({}:{}) could not be translated",
                prop.name,
                line,
                col
            );
        };
        let cpp = strip_static_linkage(&out.body);
        if cpp.is_empty() {
            anyhow::bail!(
                "function prop `{}` on <{tag}> ({}:{}) translated to nothing",
                prop.name,
                line,
                col
            );
        }
        extra_headers.extend(include_lines(&out.includes));
        if !ctx.premain.contains(&cpp) {
            ctx.premain.push(cpp);
        }
        Ok(mangled)
    }

    /// Legacy single-file entry point: custom tags pass through as plain
    /// elements (no module graph). Preserved for `build()` and existing tests.
    fn build_node(
        &self,
        jsx: &morph_parser::JsxNode,
        css_rules: &[(String, morph_parser::CssRule)],
        depth: usize,
        ancestors: &[AncestorHint],
        ambient_vars: &HashMap<String, String>,
        ambient_types: &HashMap<String, String>,
        extra_headers: &mut Vec<String>,
        keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> IRNode {
        let empty_graph = morph_parser::ModuleGraph {
            entry: PathBuf::new(),
            modules: HashMap::new(),
            order: Vec::new(),
        };
        let mut ctx = BuilderCtx::new(&empty_graph);
        let mut frame = InstanceFrame::root(PathBuf::new());
        frame.vars.clone_from(ambient_vars);
        frame.types.clone_from(ambient_types);
        // Infallible here: with an empty graph, instance expansion never
        // triggers, and capture/subst helpers cannot fail.
        self.build_node_in(
            jsx,
            css_rules,
            depth,
            ancestors,
            &frame,
            &mut ctx,
            extra_headers,
            keyframes,
        )
        .expect("legacy build_node without a module graph cannot fail")
    }

    #[allow(clippy::too_many_arguments)]
    // Splitting this builder function risks behavior change.
    #[allow(clippy::too_many_lines)]
    fn build_node_in(
        &self,
        jsx: &morph_parser::JsxNode,
        css_rules: &[(String, morph_parser::CssRule)],
        depth: usize,
        ancestors: &[AncestorHint],
        frame: &InstanceFrame,
        ctx: &mut BuilderCtx,
        extra_headers: &mut Vec<String>,
        keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> anyhow::Result<IRNode> {
        match jsx {
            morph_parser::JsxNode::Element { tag, props, children, line, col, .. } => {
                // ── Component instantiation ──
                if is_custom_tag(tag) && !ctx.graph.modules.is_empty() {
                    let graph: &morph_parser::ModuleGraph = ctx.graph;
                    return self.expand_instance(
                        graph,
                        ctx,
                        frame,
                        tag,
                        props,
                        children,
                        *line,
                        *col,
                        css_rules,
                        depth,
                        ancestors,
                        extra_headers,
                        keyframes,
                    );
                }
                // `mid` is C++-side identity for component instances
                // only. Frontend elements use `id`. The two coexist on
                // one component (`<Comp id="x" mid="y" />`) but `mid`
                // never appears on a native element.
                if props.contains_key("mid") {
                    anyhow::bail!(
                        "`mid` on `<{tag}>` ({line}:{col}): `mid` is only for component use-sites (`<Name mid=\"...\" />`), never native elements and never inside a component definition; frontend identity is `id`"
                    );
                }
                let node_id = self.next_id();
                let mut node = IRNode { node_id, node_type: tag.clone(), ..Default::default() };
                let mut style = IRStyle::new();
                apply_ua_defaults(&mut style, tag);
                let mut hover_style = IRStyle::new();
                for (prop, val) in ua_hover_defaults(tag) {
                    apply_css_prop(&mut hover_style, prop, val);
                }
                let mut active_style = IRStyle::new();
                for (prop, val) in ua_active_defaults(tag) {
                    apply_css_prop(&mut active_style, prop, val);
                }
                let (classes, id) = element_classes_id(props);
                // Cascade: collect every matching declaration with its
                // specificity and source order, then apply weakest-first so
                // the winner writes last. Stable sort keeps source order
                // among equal specificities (later sheets win ties).
                let mut declarations: Vec<(Specificity, usize, String, String, PseudoKind)> =
                    Vec::new();
                for (order, (selector, rule)) in css_rules.iter().enumerate() {
                    if let Some((pseudo, specificity)) =
                        match_selector_detailed(tag, &classes, id.as_deref(), ancestors, selector)
                    {
                        for (prop, val) in &rule.properties {
                            declarations.push((
                                specificity,
                                order,
                                prop.clone(),
                                val.clone(),
                                pseudo,
                            ));
                        }
                    }
                }
                declarations.sort();
                // Winning declarations per bucket, for animation parsing
                // (Python: matched CSS merged before tailwind/attrs/inline).
                let mut base_props: HashMap<String, String> = HashMap::new();
                let mut hover_props: HashMap<String, String> = HashMap::new();
                for (_, _, prop, val, pseudo) in &declarations {
                    let target = match pseudo {
                        PseudoKind::Base => &mut style,
                        PseudoKind::Hover => &mut hover_style,
                        PseudoKind::Active => &mut active_style,
                    };
                    apply_css_prop(target, prop, val);
                    match pseudo {
                        PseudoKind::Base => {
                            base_props.insert(prop.clone(), val.clone());
                        }
                        PseudoKind::Hover => {
                            hover_props.insert(prop.clone(), val.clone());
                        }
                        PseudoKind::Active => {}
                    }
                }
                if let Some(morph_parser::JsxPropValue::String(cls)) =
                    props.get("className").or_else(|| props.get("class"))
                {
                    for (prop, val) in self.tailwind.resolve_many(cls) {
                        apply_css_prop(&mut style, &prop, &val);
                        base_props.insert(prop, val);
                    }
                }
                // Presentational hints lose to every stylesheet rule: HTML
                // width/height attributes apply only when the cascade left
                // the property unset (browsers treat them as weakest).
                if style.width.is_none() {
                    if let Some(morph_parser::JsxPropValue::String(raw)) = props.get("width") {
                        if let Some(px) = parse_length(raw) {
                            style.width = Some(px);
                            base_props.insert("width".to_string(), raw.clone());
                        }
                    }
                }
                if style.height.is_none() {
                    if let Some(morph_parser::JsxPropValue::String(raw)) = props.get("height") {
                        if let Some(px) = parse_length(raw) {
                            style.height = Some(px);
                            base_props.insert("height".to_string(), raw.clone());
                        }
                    }
                }
                if let Some(morph_parser::JsxPropValue::Style(map)) = props.get("style") {
                    for (prop, val) in map {
                        match val {
                            morph_parser::StyleValue::Static(s) => {
                                apply_css_prop(&mut style, prop, s);
                                base_props.insert(prop.clone(), s.clone());
                            }
                            morph_parser::StyleValue::Expr(e) => {
                                node.reactive_style.insert(prop.clone(), capture_raw(e, frame));
                            }
                        }
                    }
                }
                node.style = style;
                if !hover_style.is_empty_style() {
                    node.hover_style = Some(hover_style);
                }
                if !active_style.is_empty_style() {
                    node.active_style = Some(active_style);
                }
                // CSS animations from merged declarations; keyframe names
                // unknown to the build are dropped like browsers do.
                node.animations = parse_animations(&base_props)
                    .into_iter()
                    .filter(|anim| keyframes.contains_key(&anim.name))
                    .collect();
                node.hover_animations = parse_animations(&hover_props)
                    .into_iter()
                    .filter(|anim| keyframes.contains_key(&anim.name))
                    .collect();
                // `<a href>` desugar (navigation links): when `href` is
                // present the link keys are consumed here — never rendered
                // as node attrs — and synthesized into a click event.
                // Internal paths lower to navigate/new-window placeholders
                // (manifest-checked at codegen); external schemes open the
                // OS browser. Props ride `data={…}` (never query strings).
                let link_href = if tag == "a" {
                    match props.iter().find(|(k, _)| k.as_str() == "href") {
                        Some((_, morph_parser::JsxPropValue::String(href))) => Some(href.clone()),
                        Some(_) => anyhow::bail!(
                            "link `href` must be a string literal ({line}:{col}); dynamic hrefs cannot be manifest-checked"
                        ),
                        None => None,
                    }
                } else {
                    None
                };
                if let Some(href) = link_href {
                    let target_blank = matches!(
                        props.iter().find(|(k, _)| k.as_str() == "target"),
                        Some((_, morph_parser::JsxPropValue::String(t))) if t == "_blank"
                    );
                    let data_src = match props.iter().find(|(k, _)| k.as_str() == "data") {
                        Some((_, morph_parser::JsxPropValue::Expr(e))) => Some(e.clone()),
                        Some(_) => anyhow::bail!(
                            "link `data` must be an object expression ({line}:{col}); props ride `data={{…}}`"
                        ),
                        None => None,
                    };
                    let body = if is_external_href(&href) {
                        format!(
                            "() => {{ __morph_open_browser(\"{}\") }}",
                            href.replace('"', "\\\"")
                        )
                    } else if target_blank {
                        let mut cfg = Vec::new();
                        for key in ["width", "height", "title"] {
                            if let Some((_, value)) = props.iter().find(|(k, _)| k.as_str() == key)
                            {
                                match value {
                                    // Numeric literals arrive as strings —
                                    // emit them raw so strict runtime
                                    // coercion (`as_int`, no parsing) works.
                                    morph_parser::JsxPropValue::String(s)
                                        if s.parse::<f64>().is_ok() =>
                                    {
                                        cfg.push(format!("{key}: {s}"));
                                    }
                                    morph_parser::JsxPropValue::String(s) => {
                                        cfg.push(format!("{key}: \"{}\"", s.replace('"', "\\\"")));
                                    }
                                    morph_parser::JsxPropValue::Expr(e) => {
                                        cfg.push(format!("{key}: {e}"));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if let Some(data) = data_src {
                            cfg.push(format!("data: {}", strip_braces(&data)));
                        }
                        format!(
                            "() => {{ const __w = new Window(\"{href}\", {{{}}}) }}",
                            cfg.join(", ")
                        )
                    } else {
                        format!(
                            "() => {{ const __w = useWindow(); __w.navigate(\"{href}\"{}) }}",
                            data_src.map_or_else(String::new, |data| format!(
                                ", {}",
                                strip_braces(&data)
                            ))
                        )
                    };
                    node.events.push(IREvent {
                        trigger: "click".into(),
                        action: "call".into(),
                        target: capture_raw(&body, frame),
                    });
                }
                for (k, v) in props {
                    if tag == "a"
                        && (k == "href"
                            || k == "target"
                            || k == "data"
                            || k == "width"
                            || k == "height"
                            || k == "title")
                    {
                        continue;
                    }
                    if let morph_parser::JsxPropValue::Fn(f) = v {
                        if let Some(trigger) = event_trigger(k) {
                            node.events.push(IREvent {
                                trigger: trigger.into(),
                                action: "call".into(),
                                target: capture_raw(f, frame),
                            });
                            continue;
                        }
                    }
                    match (k.as_str(), v) {
                        ("id", morph_parser::JsxPropValue::String(s)) => {
                            node.attrs.insert("id".into(), s.clone());
                        }
                        ("src", morph_parser::JsxPropValue::String(s)) => {
                            node.attrs.insert("src".into(), s.clone());
                        }
                        ("placeholder", morph_parser::JsxPropValue::String(s)) => {
                            node.attrs.insert("placeholder".into(), s.clone());
                        }
                        ("type", morph_parser::JsxPropValue::String(s)) => {
                            node.attrs.insert("type".into(), s.clone());
                        }
                        // Static class strings only drive build-time matching;
                        // only dynamic className={...} becomes a reactive
                        // expression (translated with state at emit time).
                        // Stuffing static strings through JS translation
                        // mangles any word colliding with state (`key op`
                        // with an `op` signal became `key __st_op.get()`).
                        (
                            "className" | "class",
                            morph_parser::JsxPropValue::Expr(s)
                            | morph_parser::JsxPropValue::Template(s)
                            | morph_parser::JsxPropValue::Ref(s),
                        ) => {
                            // Runtime class string via the full translator.
                            // (Instance `props.x` refs are rewritten to bare
                            // names and own locals pre-renamed so the
                            // ambient map resolves them.)
                            let class_src = prepare_source(s, frame);
                            if let Some(out) = self.translate_logic(
                                &class_src,
                                &frame.vars,
                                &frame.types,
                                &frame.events,
                            ) {
                                extra_headers.extend(include_lines(&out.includes));
                                let body = out.body.trim().trim_end_matches(';').trim().to_string();
                                if !body.is_empty() {
                                    node.reactive_class = body;
                                }
                            }
                            // Build-time branch resolution for ternary arms.
                            let mut fx = analyze_dynamic_class(
                                &class_src,
                                tag,
                                css_rules,
                                &self.tailwind,
                                &frame.vars,
                                &frame.types,
                                self.type_mode,
                                extra_headers,
                            );
                            node.class_conditional_effects.append(&mut fx);
                        }
                        _ => {}
                    }
                }
                let text_parts: Vec<String> = children
                    .iter()
                    .filter_map(|c| {
                        if let morph_parser::JsxNode::Text(t) = c {
                            Some(t.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !text_parts.is_empty() {
                    node.text_content = text_parts.join("");
                }
                let mut child_ancestors = ancestors.to_vec();
                child_ancestors.insert(0, AncestorHint { tag: tag.clone(), classes, id });
                for child in children {
                    let child_node = self.build_node_in(
                        child,
                        css_rules,
                        depth + 1,
                        &child_ancestors,
                        frame,
                        ctx,
                        extra_headers,
                        keyframes,
                    )?;
                    if child_node.node_type == "__text__"
                        && child_node.text_content.trim().is_empty()
                    {
                        continue;
                    }
                    node.children.push(child_node);
                }
                Ok(node)
            }
            morph_parser::JsxNode::Fragment { children, .. } => {
                let mut node = IRNode {
                    node_id: self.next_id(),
                    node_type: "__fragment__".into(),
                    ..Default::default()
                };
                for child in children {
                    node.children.push(self.build_node_in(
                        child,
                        css_rules,
                        depth,
                        ancestors,
                        frame,
                        ctx,
                        extra_headers,
                        keyframes,
                    )?);
                }
                Ok(node)
            }
            morph_parser::JsxNode::Text(t) => {
                let mut node = IRNode {
                    node_id: self.next_id(),
                    node_type: "__text__".into(),
                    ..Default::default()
                };
                node.text_content = t.clone();
                Ok(node)
            }
            morph_parser::JsxNode::Expression(e) => {
                let mut node = IRNode {
                    node_id: self.next_id(),
                    node_type: "__expr__".into(),
                    ..Default::default()
                };
                node.reactive_text = capture_raw(e, frame);
                Ok(node)
            }
            morph_parser::JsxNode::Conditional { condition, then_branch, else_branch, .. } => {
                let mut node = IRNode {
                    node_id: self.next_id(),
                    node_type: "__conditional__".into(),
                    ..Default::default()
                };
                node.condition_expr = capture_raw(condition, frame);
                for c in then_branch {
                    node.then_nodes.push(self.build_node_in(
                        c,
                        css_rules,
                        depth,
                        ancestors,
                        frame,
                        ctx,
                        extra_headers,
                        keyframes,
                    )?);
                }
                for c in else_branch {
                    node.else_nodes.push(self.build_node_in(
                        c,
                        css_rules,
                        depth,
                        ancestors,
                        frame,
                        ctx,
                        extra_headers,
                        keyframes,
                    )?);
                }
                Ok(node)
            }
            morph_parser::JsxNode::List {
                array_expr,
                key_expr,
                item_template,
                item_param,
                index_param,
                ..
            } => {
                let mut node = IRNode {
                    node_id: self.next_id(),
                    node_type: "__list__".into(),
                    // Preserve the map callback's parameter names
                    // (`items.map((it, i) => ...)`): codegen binds them to
                    // `__it` / `__index` in item factories.
                    list_item_param: if item_param.is_empty() {
                        "item".to_string()
                    } else {
                        item_param.clone()
                    },
                    list_index_param: index_param.clone(),
                    ..Default::default()
                };
                node.list_expr = capture_raw(array_expr, frame);
                node.list_key_expr = capture_raw(key_expr, frame);
                // Item templates share one state slot per template: `mid`
                // on anything instantiated inside is a hard error.
                ctx.list_depth += 1;
                let template = self.build_node_in(
                    item_template,
                    css_rules,
                    depth,
                    ancestors,
                    frame,
                    ctx,
                    extra_headers,
                    keyframes,
                );
                ctx.list_depth -= 1;
                node.item_template = Some(Box::new(template?));
                Ok(node)
            }
        }
    }

    fn convert_keyframes(
        &self,
        css_keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> HashMap<String, Vec<crate::node::IRKeyframe>> {
        let mut result: HashMap<String, Vec<crate::node::IRKeyframe>> = HashMap::new();
        for (name, kfs) in css_keyframes {
            let mut converted = Vec::new();
            for kf in kfs {
                let mut raw: HashMap<String, String> = HashMap::new();
                let mut style = IRStyle::new();
                let mut declared: Vec<String> = Vec::new();
                for (prop, val) in &kf.properties {
                    if !is_animatable(prop) {
                        continue;
                    }
                    if prop == "transform" || needs_layout(val) {
                        raw.insert(prop.clone(), val.clone());
                        continue;
                    }
                    if let Some(field) = apply_css_prop(&mut style, prop, val) {
                        declared.push(field.to_string());
                    }
                }
                converted.push(crate::node::IRKeyframe { offset: kf.offset, style, declared, raw });
            }
            result.insert(name.clone(), converted);
        }
        result
    }
}

/// Scope frame for JSX expansion. Components are closed over their own
/// scope + props: parent-scope names stay invisible here and cross-component
/// data flows exclusively through props.
struct InstanceFrame {
    module: PathBuf,
    vars: HashMap<String, String>,
    types: HashMap<String, String>,
    /// Visible event channel ids keyed by local name (own exports + imports).
    events: HashMap<String, String>,
    /// `props` in `function C(props: {...})`; empty when destructured/absent.
    props_param: String,
    renames: HashMap<String, String>,
    prop_binds: HashMap<String, String>,
}

impl InstanceFrame {
    fn root(module: PathBuf) -> Self {
        Self {
            module,
            vars: HashMap::new(),
            types: HashMap::new(),
            events: HashMap::new(),
            props_param: String::new(),
            renames: HashMap::new(),
            prop_binds: HashMap::new(),
        }
    }

    fn instance(module: PathBuf) -> Self {
        Self {
            module,
            vars: HashMap::new(),
            types: HashMap::new(),
            events: HashMap::new(),
            props_param: String::new(),
            renames: HashMap::new(),
            prop_binds: HashMap::new(),
        }
    }
}

struct BuilderCtx<'a> {
    graph: &'a morph_parser::ModuleGraph,
    next_instance: usize,
    next_channel: usize,
    stack: Vec<(PathBuf, String)>,
    channels: Vec<HashMap<String, String>>,
    states: Vec<HashMap<String, String>>,
    const_names: Vec<String>,
    premain: Vec<String>,
    effects: Vec<HashMap<String, String>>,
    logs: Vec<String>,
    modules_emitted: HashSet<PathBuf>,
    shared_entries: Vec<HashMap<String, String>>,
    event_entries: Vec<HashMap<String, String>>,
    /// Function/var/class bindings (universal module-namespace rule).
    module_bindings: Vec<HashMap<String, String>>,
    /// Depth inside `.map()` item templates (`mid` is rejected there).
    list_depth: usize,
    /// Tagged instances: (module, component) →
    /// [(mid, instance_no, line, col, use_site module)]. The use-site
    /// module is where the `<Tag mid="…">` was written (mapping comments);
    /// the key module owns the component type.
    mid_tags: HashMap<(PathBuf, String), Vec<(String, usize, usize, usize, PathBuf)>>,
    /// Instance states for indexed native access: instance_no →
    /// [(suffix getter, suffix setter, init)].
    mid_states: HashMap<usize, Vec<(String, String, String)>>,
    /// Signal owner expression prefix (`"__st_"` for entry globals,
    /// `"ctx->"` for route mount contexts). Threaded to instance
    /// seeding so shared expansion paths stay build-agnostic.
    signal_prefix: &'static str,
}

impl<'a> BuilderCtx<'a> {
    fn new(graph: &'a morph_parser::ModuleGraph) -> Self {
        Self {
            graph,
            next_instance: 0,
            next_channel: 0,
            stack: Vec::new(),
            channels: Vec::new(),
            states: Vec::new(),
            const_names: Vec::new(),
            premain: Vec::new(),
            effects: Vec::new(),
            logs: Vec::new(),
            modules_emitted: HashSet::new(),
            shared_entries: Vec::new(),
            event_entries: Vec::new(),
            module_bindings: Vec::new(),
            list_depth: 0,
            mid_tags: HashMap::new(),
            mid_states: HashMap::new(),
            signal_prefix: "__st_",
        }
    }
}

/// A `window.state_vars` entry. The emitter derives `__st_<getter>` from the
/// getter, so instance signals only need mangled names. The `instance` mark
/// keeps them out of the native interop header (ambiguous across instances);
/// `ns` (empty for legacy slots) selects the wrapper namespace in the header.
fn state_slot(
    getter: &str,
    setter: &str,
    init: &str,
    instance: bool,
    ns: &str,
) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("getter".into(), getter.to_string());
    m.insert("setter".into(), setter.to_string());
    m.insert("init".into(), init.to_string());
    if instance {
        m.insert("instance".into(), "1".to_string());
    }
    if !ns.is_empty() {
        m.insert("ns".into(), ns.to_string());
    }
    m
}

/// True for per-instance state slots.
pub fn is_instance_slot(slot: &HashMap<String, String>) -> bool {
    slot.get("instance").is_some_and(|v| v == "1")
}

fn is_custom_tag(tag: &str) -> bool {
    tag.chars().next().is_some_and(char::is_uppercase)
}

/// Resolve `<Tag />` used in `module` to (owning module, component).
/// Module-local components shadow imports.
///
/// # Errors
/// Rejects unknown tags, default imports of modules without a default
/// export, and named imports of non-exported components.
fn resolve_binding(
    graph: &morph_parser::ModuleGraph,
    module: &Path,
    local: &str,
) -> anyhow::Result<(PathBuf, morph_parser::MxComponent)> {
    let src = graph
        .modules
        .get(module)
        .ok_or_else(|| anyhow::anyhow!("unknown module {}", module.display()))?;
    if let Some(c) = src.source.components.iter().find(|c| c.name == local) {
        return Ok((module.to_path_buf(), c.clone()));
    }
    for imp in &src.source.imports {
        let (raw, default, specifiers) = match &imp.kind {
            morph_parser::MxImportKind::Component { path, default, specifiers }
                if path != "morph" =>
            {
                (path, default, specifiers)
            }
            _ => continue,
        };
        let is_default_binding = default.as_deref() == Some(local);
        let is_named = specifiers.iter().any(|(l, _)| l == local);
        if !is_default_binding && !is_named {
            continue;
        }
        let target = src.module_imports.iter().find(|(p, _)| p == raw).map(|(_, t)| t.clone());
        let Some(target) = target else { continue };
        let target_mod = graph
            .modules
            .get(&target)
            .ok_or_else(|| anyhow::anyhow!("unresolved component module {}", target.display()))?;
        if is_default_binding {
            if let Some(c) = target_mod.source.components.iter().find(|c| c.is_default) {
                return Ok((target, c.clone()));
            }
            anyhow::bail!(
                "`{}` has no default export (imported by {})",
                target.display(),
                module.display()
            );
        }
        if let Some(c) = target_mod.source.components.iter().find(|c| c.name == local && c.exported)
        {
            return Ok((target, c.clone()));
        }
        anyhow::bail!(
            "`{}` has no exported component `{local}` (imported by {})",
            target.display(),
            module.display()
        );
    }
    anyhow::bail!("unknown component `<{local}>` in {}", module.display())
}

/// Rewrite `props.x` → `x` for identifier-form props.
fn subst_props_refs(src: &str, frame: &InstanceFrame) -> String {
    if frame.props_param.is_empty() {
        return src.to_string();
    }
    let param = frame.props_param.as_str();
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' || c == '\'' {
            let end = str_lit_end(src, i);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if c == '`' {
            let (end, _) = copy_template(src, i, &HashMap::new(), frame);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if is_ident_start(c) && src[i..].starts_with(param) {
            let after_param = i + param.len();
            let boundary_before = i == 0 || !is_ident_char_at(src, i - 1);
            let preceded_by_dot = i > 0 && bytes[i - 1] == b'.';
            if boundary_before && !preceded_by_dot {
                let mut j = after_param;
                if j < bytes.len() && bytes[j] == b'.' {
                    j += 1;
                    let name_start = j;
                    while j < bytes.len() && is_ident_char_at(src, j) {
                        j += src[j..].chars().next().map_or(1, char::len_utf8);
                    }
                    if j > name_start {
                        out.push_str(&src[name_start..j]);
                        i = j;
                        continue;
                    }
                }
            }
        }
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Word-boundary rename, skipping literals, comments, and member accesses.
fn rename_symbols(src: &str, renames: &HashMap<String, String>, frame: &InstanceFrame) -> String {
    if renames.is_empty() {
        return src.to_string();
    }
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' || c == '\'' {
            let end = str_lit_end(src, i);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if c == '`' {
            let (end, rendered) = copy_template(src, i, renames, frame);
            out.push_str(&rendered);
            i = end;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                if let Some(nl) = src[i..].find('\n') {
                    out.push_str(&src[i..i + nl]);
                    i += nl;
                    continue;
                }
                out.push_str(&src[i..]);
                break;
            }
            if bytes[i + 1] == b'*' {
                if let Some(end) = src[i..].find("*/") {
                    out.push_str(&src[i..i + end + 2]);
                    i += end + 2;
                    continue;
                }
                out.push_str(&src[i..]);
                break;
            }
        }
        if is_ident_start(c) {
            let mut j = i + c.len_utf8();
            while j < bytes.len() && is_ident_char_at(src, j) {
                let ch_len = src[j..].chars().next().map_or(1, char::len_utf8);
                j += ch_len;
            }
            let word = &src[i..j];
            let preceded_by_dot = i > 0 && bytes[i - 1] == b'.';
            if !preceded_by_dot {
                if let Some(repl) = renames.get(word) {
                    out.push_str(repl);
                    i = j;
                    continue;
                }
            }
            out.push_str(word);
            i = j;
            continue;
        }
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Prepare a raw JSX-captured expression for emit-time translation.
/// Prop bindings inline last so bound text is never re-scanned.
fn capture_raw(raw: &str, frame: &InstanceFrame) -> String {
    let reemit = rewrite_event_emits(raw, &frame.events);
    let emit = rewrite_emit_for_cpp(&reemit);
    let sub = subst_props_refs(&emit, frame);
    let renamed = rename_symbols(&sub, &frame.renames, frame);
    rename_symbols(&renamed, &frame.prop_binds, frame)
}

/// Prepare a component-definition source for morpher translation.
/// Pre-renaming removes the need for a post-pass over morpher output.
fn prepare_source(src: &str, frame: &InstanceFrame) -> String {
    let sub = subst_props_refs(src, frame);
    rename_symbols(&sub, &frame.renames, frame)
}

/// Copy a template literal, renaming inside `${...}` interpolations only.
fn copy_template(
    src: &str,
    start: usize,
    renames: &HashMap<String, String>,
    frame: &InstanceFrame,
) -> (usize, String) {
    let bytes = src.as_bytes();
    let mut out = String::from("`");
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            out.push_str(&src[i..(i + 2).min(src.len())]);
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            out.push('`');
            return (i + 1, out);
        }
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            // Balanced scan to the matching `}` (nesting + strings aware).
            let mut j = i + 2;
            let mut depth = 1i32;
            let mut in_str: Option<u8> = None;
            while j < bytes.len() {
                let b = bytes[j];
                if let Some(q) = in_str {
                    if b == b'\\' {
                        j += 2;
                        continue;
                    }
                    if b == q {
                        in_str = None;
                    }
                    j += 1;
                    continue;
                }
                match b {
                    b'"' | b'\'' | b'`' => in_str = Some(b),
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let inner = &src[i + 2..j.min(src.len())];
            out.push_str("${");
            out.push_str(&rename_symbols(inner, renames, frame));
            out.push('}');
            i = (j + 1).min(src.len());
            continue;
        }
        let ch_len = src[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&src[i..i + ch_len]);
        i += ch_len;
    }
    (i, out)
}

/// End index (exclusive) of a `"` / `'` literal starting at `start`.
const fn str_lit_end(src: &str, start: usize) -> usize {
    let bytes = src.as_bytes();
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

fn is_ident_char_at(src: &str, byte_idx: usize) -> bool {
    src[byte_idx..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Human-computable C++ namespace path for a module: entry-relative
/// segments joined with `::` (`src/components/ShopStore.mx` →
/// `components::shopstore`). No hash leaf — `mx-naming` hard gates make
/// the mapping 1:1 (see `validate_namespaces`).
fn ns_of(graph: &morph_parser::ModuleGraph, module: &Path) -> anyhow::Result<String> {
    morph_parser::module_ns_path(&graph.entry, module).map_err(|msg| anyhow::anyhow!("{msg}"))
}

/// Reject normalized namespace collisions across the graph before
/// emitting code (mirrors the `mx-naming` lint; `morph build` does not
/// run the linter).
fn validate_namespaces(graph: &morph_parser::ModuleGraph) -> anyhow::Result<()> {
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    for path in graph.all_paths() {
        let ns = ns_of(graph, path)?;
        if let Some(first) = seen.get(&ns) {
            anyhow::bail!(
                "module {} normalizes to namespace `{ns}`, already claimed by {}: rename one (mx-naming)",
                path.display(),
                first.display()
            );
        }
        seen.insert(ns, path.clone());
    }
    Ok(())
}

/// C++ identifier for a module-level binding name. Must match what
/// morpher emits for the same source name exactly (it preserves
/// `[A-Za-z0-9_$]`, which g++ accepts as an extension); anything else
/// becomes `_`. Shared by the builder (frame maps) and codegen
/// (reactive-text substitution) so both sides agree.
pub fn binding_ident(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '$' { c } else { '_' })
        .collect();
    if safe.is_empty() {
        "binding".to_string()
    } else {
        safe
    }
}

/// Root namespace for generated module bindings (`app::<path…>`): short
/// for user DX, isolated from the `morph::` runtime namespace and the
/// global scope (a module literally named like a runtime entity can
/// never collide). Single source of truth — every emitter builds
/// qualified names and namespace blocks from this.
pub const MODULE_NS_ROOT: &str = "app";

/// Fully-qualified `<root>::<ns>::<binding>` reference.
pub fn qualified_binding_ref(ns: &str, name: &str) -> String {
    if ns.is_empty() {
        binding_ident(name)
    } else {
        format!("::{MODULE_NS_ROOT}::{ns}::{}", binding_ident(name))
    }
}

/// Internal identity of a binding: canonical module path + binding name.
fn binding_identity(module: &Path, name: &str) -> String {
    format!("{}::{name}", module.display())
}

/// C++ identifier of a module's signal accessor function. Lives inside the
/// module namespace, so only getter names within one file must differ.
fn shared_signal_accessor(getter: &str) -> String {
    let safe: String = getter
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    let safe = if safe.is_empty() { "sig".to_string() } else { safe };
    format!("shared_{safe}")
}

/// C++ type of a shared binding from its explicit type argument, inferred
/// from its initializer, or `auto` when unknowable.
fn shared_binding_type(type_arg: &Option<String>, init: &str) -> String {
    if let Some(arg) = type_arg {
        let t = arg.trim();
        match t {
            "string" => return "std::string".to_string(),
            "boolean" => return "bool".to_string(),
            "number" => {
                // Match the init's numeric kind when present, else double.
                if let Some(ty) = infer_state_type(init) {
                    if ty == "int" || ty == "double" {
                        return ty;
                    }
                }
                return "double".to_string();
            }
            "any" | "unknown" => return "auto".to_string(),
            _ => {}
        }
    }
    infer_state_type(init).unwrap_or_else(|| "auto".to_string())
}

/// C++ identifier of a module's event channel accessor function. Lives
/// inside the module namespace, so only event names within one file
/// must differ.
fn event_channel_accessor(name: &str) -> String {
    let safe: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    let safe = if safe.is_empty() { "evt".to_string() } else { safe };
    format!("evt_{safe}")
}

/// Runtime channel id of an event binding: `evt:<module path>:<name>`.
/// Used by the dev TU string registry only; the build TU lowers every
/// reference to the namespace accessor.
fn event_channel_id(module: &Path, name: &str) -> String {
    format!("evt:{}::{name}", module.display())
}

/// Escape a runtime channel id for embedding in a C++ string literal.
fn escape_channel(id: &str) -> String {
    id.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Byte index of a top-level callee call, skipping strings, comments, and
/// member calls.
fn find_emit_callee(src: &str, callee: &str, from: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' || c == '\'' || c == '`' {
            i = str_lit_end(src, i);
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
            if bytes[i + 1] == b'/' {
                i = src[i..].find('\n').map_or(bytes.len(), |n| i + n);
            } else {
                i = src[i..].find("*/").map_or(bytes.len(), |n| i + n + 2);
            }
            continue;
        }
        if is_ident_start(c) && src[i..].starts_with(callee) {
            let end = i + callee.len();
            let before_ok = i == 0 || {
                let b = bytes[i - 1];
                !(b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b == b'.')
            };
            let after_ok = src[end..].chars().next().is_some_and(|nc| nc == '(');
            if before_ok && after_ok {
                return Some(i);
            }
        }
        i += c.len_utf8();
    }
    None
}

/// Split the argument list of the call opening at `open`.
fn split_call_args(src: &str, open: usize) -> Option<(Vec<String>, usize)> {
    let bytes = src.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut args = Vec::new();
    let mut start = open + 1;
    let mut depth = 0i32;
    let mut i = open + 1;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' || b == b'`' {
            i = str_lit_end(src, i);
            continue;
        }
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' if depth == 0 => {
                args.push(src[start..i].trim().to_string());
                return Some((args, i + 1));
            }
            b')' | b']' | b'}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b',' if depth == 0 => {
                args.push(src[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Rewrite `{event}.emit(x)` to a `morphEmit("<channel_id>", x)` placeholder
/// for every event name visible in `events`, so the shared emit pipeline
/// lowers it to `morph::channel("<channel_id>").emit(x)`. Only top-level
/// identifiers are matched (never member expressions); strings and comments
/// are skipped, and arguments are scanned for nested event emits.
fn rewrite_event_emits(src: &str, events: &HashMap<String, String>) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' || c == '\'' || c == '`' {
            let end = str_lit_end(src, i);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
            if bytes[i + 1] == b'/' {
                let nl = src[i..].find('\n').map_or(bytes.len(), |n| i + n);
                out.push_str(&src[i..nl]);
                i = nl;
            } else {
                let end = src[i..].find("*/").map_or(bytes.len(), |n| i + n + 2);
                out.push_str(&src[i..end]);
                i = end;
            }
            continue;
        }
        if is_ident_start(c) {
            let j = ident_end(src, i);
            if let Some(id) = events.get(&src[i..j]) {
                let before_ok = i == 0
                    || !(bytes[i - 1].is_ascii_alphanumeric()
                        || bytes[i - 1] == b'_'
                        || bytes[i - 1] == b'$'
                        || bytes[i - 1] == b'.');
                if before_ok {
                    let mut k = j;
                    while k < bytes.len() && (bytes[k] as char).is_whitespace() {
                        k += 1;
                    }
                    if bytes.get(k) == Some(&b'.') {
                        let mut m = k + 1;
                        while m < bytes.len() && (bytes[m] as char).is_whitespace() {
                            m += 1;
                        }
                        if src[m..].starts_with("emit") {
                            let mut o = m + 4;
                            while o < bytes.len() && (bytes[o] as char).is_whitespace() {
                                o += 1;
                            }
                            if bytes.get(o) == Some(&b'(') {
                                out.push_str(&format!("morphEmit(\"{}\", ", escape_channel(id)));
                                i = o + 1;
                                continue;
                            }
                        }
                    }
                }
            }
            out.push_str(&src[i..j]);
            i = j;
            continue;
        }
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// End index (exclusive) of an identifier token starting at `start`.
const fn ident_end(src: &str, start: usize) -> usize {
    let bytes = src.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'$' {
            i += 1;
        } else {
            break;
        }
    }
    i
}

/// Rewrite `morphEmit(a, b)` to a morpher-safe placeholder for the JS
/// context.
fn rewrite_emit_for_js(src: &str) -> String {
    rewrite_emit_calls(src, "morphEmit", false)
}

/// Rewrite emit calls to `morph::channel(a).emit(b)` for the C++ context.
/// Malformed calls are left untouched to fail loudly downstream.
fn rewrite_emit_for_cpp(src: &str) -> String {
    rewrite_emit_calls(&rewrite_emit_calls(src, "__morphEmit", true), "morphEmit", true)
}

fn rewrite_emit_calls(src: &str, callee: &str, to_channel: bool) -> String {
    let placeholder = "__morphEmit";
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while let Some(found) = find_emit_callee(src, callee, i) {
        out.push_str(&src[i..found]);
        let open = found + callee.len();
        match split_call_args(src, open) {
            Some((args, end)) if args.len() == 2 => {
                if to_channel {
                    out.push_str(&format!("morph::channel({}).emit({})", args[0], args[1]));
                } else {
                    out.push_str(placeholder);
                    out.push_str(&src[open..end]);
                }
                i = end;
            }
            _ => {
                out.push_str(&src[found..open]);
                i = open;
            }
        }
    }
    out.push_str(&src[i..]);
    out
}

/// Rewrite `p.field` → `p["field"]` for a channel payload param.
fn rewrite_channel_access(src: &str, param: &str) -> String {
    if param.is_empty() {
        return src.to_string();
    }
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' || c == '\'' || c == '`' {
            let end = str_lit_end(src, i);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
            if bytes[i + 1] == b'/' {
                let end = src[i..].find('\n').map_or(bytes.len(), |n| i + n);
                out.push_str(&src[i..end]);
                i = end;
            } else {
                let end = src[i..].find("*/").map_or(bytes.len(), |n| i + n + 2);
                out.push_str(&src[i..end]);
                i = end;
            }
            continue;
        }
        if is_ident_start(c) && src[i..].starts_with(param) {
            let end = i + param.len();
            let after = &src[end..];
            let boundary = after
                .chars()
                .next()
                .map_or(true, |nc| !nc.is_alphanumeric() && nc != '_' && nc != '$');
            if boundary {
                // Skip whitespace, expect `.ident`.
                let mut j = end;
                while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'.' {
                    let mut k = j + 1;
                    while k < bytes.len() && (bytes[k] as char).is_whitespace() {
                        k += 1;
                    }
                    let name_start = k;
                    while k < bytes.len() && is_ident_char_at(src, k) {
                        let ch_len = src[k..].chars().next().map_or(1, char::len_utf8);
                        k += ch_len;
                    }
                    if k > name_start {
                        out.push_str(&format!("{param}[\"{}\"]", &src[name_start..k]));
                        i = k;
                        continue;
                    }
                }
            }
        }
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Map a declared TS prop type to a morpher operand class (`int`,
/// `double`, `bool`, `std::string`, `JsArray`). Empty = unknown, omit it
/// and let morpher infer.
fn ts_type_class(t: &str) -> String {
    let lower = t.trim().to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.contains(&"string") {
        return "std::string".to_string();
    }
    if tokens.iter().any(|t| *t == "boolean" || *t == "bool") {
        return "bool".to_string();
    }
    if t.contains("[]") || tokens.contains(&"array") {
        return "JsArray".to_string();
    }
    if tokens.iter().any(|t| *t == "int" || *t == "integer") {
        return "int".to_string();
    }
    if tokens.iter().any(|t| *t == "number" || *t == "double" || *t == "float") {
        return "double".to_string();
    }
    String::new()
}

fn find_top_level_arrow(src: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' || b == b'`' {
            i = str_lit_end(src, i);
            continue;
        }
        match b {
            b'<' | b'(' | b'[' | b'{' => depth += 1,
            b'>' | b')' | b']' | b'}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b'=' if depth == 0 && i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                return Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn split_top_level_commas(s: &str) -> Vec<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' || b == b'`' {
            i = str_lit_end(s, i);
            continue;
        }
        match b {
            b'<' | b'(' | b'[' | b'{' => depth += 1,
            b'>' | b')' | b']' | b'}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b',' if depth == 0 => {
                out.push(s[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(s[start..].trim().to_string());
    out
}

/// Parse `(params) => return` into ([(name, type, optional)], return).
///
/// # Errors
/// Rejects generics, optional/rest params, and untyped parameters (C++
/// arity is fixed).
fn parse_fn_type(t: &str) -> anyhow::Result<(Vec<(String, String, bool)>, String)> {
    let s = t.trim();
    if s.starts_with('<') {
        anyhow::bail!("generic function types are not supported: `{s}`");
    }
    let Some(arrow) = find_top_level_arrow(s) else {
        anyhow::bail!("`{s}` is not a function type (expected `(params) => return`)");
    };
    let (left, right) = (s[..arrow].trim(), s[arrow + 2..].trim().to_string());
    if !left.starts_with('(') || !left.ends_with(')') || left.len() < 2 {
        anyhow::bail!("function type parameters must be parenthesized in `{s}`");
    }
    let inner = left[1..left.len() - 1].trim();
    let mut params = Vec::new();
    if !inner.is_empty() {
        for p in split_top_level_commas(inner) {
            let p = p.trim().to_string();
            if p.starts_with("...") {
                anyhow::bail!("rest parameters are not supported in `{s}`");
            }
            let name_end = p
                .char_indices()
                .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '$')
                .map(|(i, c)| i + c.len_utf8())
                .last()
                .unwrap_or(0);
            if name_end == 0 {
                anyhow::bail!("cannot parse parameter `{p}` in `{s}`");
            }
            let name = p[..name_end].to_string();
            let rest = p[name_end..].trim_start();
            let (optional, rest) = match rest.strip_prefix('?') {
                Some(r) => (true, r.trim_start()),
                None => (false, rest),
            };
            if optional {
                anyhow::bail!(
                    "optional parameter `{name}` is not supported in `{s}` (C++ arity is fixed)"
                );
            }
            let Some(ty) = rest.strip_prefix(':') else {
                anyhow::bail!("parameter `{name}` needs a type in `{s}`");
            };
            params.push((name, ty.trim().to_string(), false));
        }
    }
    Ok((params, right))
}

/// Parse an inline arrow or `function` expression into (params, body,
/// is_block).
///
/// # Errors
/// Rejects async callables, non-arrow references, and destructured params.
fn parse_callable_source(src: &str) -> anyhow::Result<(Vec<String>, String, bool)> {
    let mut s = src.trim();
    if s.starts_with("async ") || s.starts_with("async(") || s.starts_with("async{") {
        anyhow::bail!("async function props are not supported");
    }
    if let Some(rest) = s.strip_prefix("function") {
        // `function name?(params) body` — skip an optional name.
        let mut rest = rest.trim_start();
        if !rest.starts_with('(') {
            let name_end = rest
                .char_indices()
                .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '$')
                .map(|(i, c)| i + c.len_utf8())
                .last()
                .unwrap_or(0);
            rest = rest[name_end..].trim_start();
        }
        s = rest;
        let Some(open) = s.find('(') else {
            anyhow::bail!("cannot parse function parameters in `{src}`");
        };
        // Balance from the open paren.
        let (params_src, body_src) = split_paren_body(&s[open..])
            .ok_or_else(|| anyhow::anyhow!("cannot parse function shape in `{src}`"))?;
        return Ok((
            param_names(&params_src)?,
            body_src.trim().to_string(),
            is_block_body(&body_src),
        ));
    }
    let Some(arrow) = find_top_level_arrow(s) else {
        anyhow::bail!("`{src}` is not an arrow/function (expected `(params) => body`)");
    };
    let (left, right) = (s[..arrow].trim(), s[arrow + 2..].trim().to_string());
    let params_src = if left.starts_with('(') && left.ends_with(')') && left.len() >= 2 {
        left[1..left.len() - 1].trim()
    } else {
        left
    };
    Ok((param_names(params_src)?, right.trim().to_string(), is_block_body(&right)))
}

fn split_paren_body(src: &str) -> Option<(String, String)> {
    let bytes = src.as_bytes();
    if bytes.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' | b'`' => {
                i = str_lit_end(src, i);
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((src[1..i].to_string(), src[i + 1..].trim().to_string()));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn is_block_body(body: &str) -> bool {
    let b = body.trim();
    b.starts_with('{') && b.ends_with('}') && b.len() >= 2
}

/// Bare identifier names from a parameter list.
///
/// # Errors
/// Rejects destructured/complex parameters (positional mapping needs names).
fn param_names(params_src: &str) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    let p = params_src.trim();
    if p.is_empty() {
        return Ok(out);
    }
    for part in split_top_level_commas(p) {
        let part = part.trim().trim_start_matches("...").trim();
        // Strip any `: type` / `= default` annotation, keep the name.
        let name_end = part
            .char_indices()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '$')
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or(0);
        let name = part[..name_end].trim();
        if name.is_empty()
            || name_end < part.len() && {
                let rest = part[name_end..].trim_start();
                !(rest.starts_with(':') || rest.starts_with('=') || rest.starts_with('?'))
            }
        {
            anyhow::bail!("destructured/complex parameters like `{part}` are not supported in function props (use plain names)");
        }
        out.push(name.to_string());
    }
    Ok(out)
}

/// Zero value for an omitted optional prop, or `None` for types without
/// one (functions, objects), which must be passed explicitly.
fn prop_zero_value(prop: &morph_parser::ComponentProp) -> Option<(String, String)> {
    let class = ts_type_class(&prop.prop_type);
    match class.as_str() {
        "std::string" => Some(("std::string{}".to_string(), class)),
        "bool" => Some(("false".to_string(), class)),
        "JsArray" => Some(("JsArray{}".to_string(), class)),
        "int" => Some(("0".to_string(), class)),
        "double" => Some(("0.0".to_string(), class)),
        _ => None,
    }
}

/// Which style bucket a matched CSS rule targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PseudoKind {
    Base,
    Hover,
    Active,
}

fn is_animatable(prop: &str) -> bool {
    matches!(
        prop,
        "opacity"
            | "background-color"
            | "color"
            | "border-radius"
            | "font-size"
            | "width"
            | "height"
            | "left"
            | "top"
            | "transform"
    )
}

fn needs_layout(val: &str) -> bool {
    let v = val.trim();
    if v.is_empty() || v == "auto" {
        return true;
    }
    v.ends_with('%') || v.ends_with("vh") || v.ends_with("vw")
}

/// User-agent default styles for HTML tags (lowest priority — overridden by
/// every later cascade stage). Mirrors Python's `_UA_DEFAULTS` verbatim; the
/// `color: #010101` near-black sentinel on form controls signals "explicit
/// value, never inherit the parent's color".
fn ua_defaults(tag: &str) -> &'static [(&'static str, &'static str)] {
    match tag {
        // ── Document ────────────────────────────────────────────
        "html" => &[("display", "block")],
        "body" => &[("display", "block"), ("padding", "8px")],

        // ── Headings ────────────────────────────────────────────
        "h1" => &[
            ("display", "block"),
            ("font-size", "32px"),
            ("font-weight", "bold"),
            ("margin", "21.44px 0"),
        ],
        "h2" => &[
            ("display", "block"),
            ("font-size", "24px"),
            ("font-weight", "bold"),
            ("margin", "19.92px 0"),
        ],
        "h3" => &[
            ("display", "block"),
            ("font-size", "18.72px"),
            ("font-weight", "bold"),
            ("margin", "18.72px 0"),
        ],
        "h4" => &[
            ("display", "block"),
            ("font-size", "16px"),
            ("font-weight", "bold"),
            ("margin", "21.28px 0"),
        ],
        "h5" => &[
            ("display", "block"),
            ("font-size", "13.28px"),
            ("font-weight", "bold"),
            ("margin", "22.18px 0"),
        ],
        "h6" => &[
            ("display", "block"),
            ("font-size", "10.72px"),
            ("font-weight", "bold"),
            ("margin", "24.97px 0"),
        ],

        // ── Grouping ────────────────────────────────────────────
        "div" => &[("display", "block")],
        "p" => &[("display", "block"), ("margin", "16px 0")],
        "pre" => &[("display", "block"), ("margin", "16px 0")],
        "blockquote" => &[("display", "block"), ("margin", "16px 40px")],
        "hr" => &[("display", "block")],
        "figure" => &[("display", "block"), ("margin", "16px 40px")],
        "figcaption" => &[("display", "block")],
        "main" => &[("display", "block")],
        "header" => &[("display", "block")],
        "footer" => &[("display", "block")],
        "nav" => &[("display", "block")],
        "section" => &[("display", "block")],
        "article" => &[("display", "block")],
        "aside" => &[("display", "block")],

        // ── Lists ───────────────────────────────────────────────
        "ul" => &[("display", "block"), ("margin", "16px 0")],
        "ol" => &[("display", "block"), ("margin", "16px 0")],
        "li" => &[("display", "block")],
        "dl" => &[("display", "block"), ("margin", "16px 0")],
        "dt" => &[("display", "block")],
        "dd" => &[("display", "block"), ("margin-left", "40px")],

        // ── Text-level ──────────────────────────────────────────
        "span" => &[("display", "inline")],
        "a" => &[("display", "inline"), ("color", "#0000ee"), ("cursor", "pointer")],
        "strong" => &[("font-weight", "bold")],
        "b" => &[("font-weight", "bold")],
        "small" => &[("font-size", "13.28px")],
        "mark" => &[("background-color", "#ffff00"), ("color", "#000000")],
        "sub" => &[("font-size", "13.28px")],
        "sup" => &[("font-size", "13.28px")],
        "code" => &[("display", "inline")],
        "kbd" => &[("display", "inline")],
        "samp" => &[("display", "inline")],
        "em" => &[("display", "inline")],
        "i" => &[("display", "inline")],
        "ins" => &[("display", "inline")],
        "u" => &[("display", "inline")],
        "del" => &[("display", "inline")],
        "s" => &[("display", "inline")],
        "q" => &[("display", "inline")],

        // ── Embedded ────────────────────────────────────────────
        "img" => &[("display", "inline-block")],

        // ── Forms ───────────────────────────────────────────────
        "button" => &[
            ("display", "inline-block"),
            ("background-color", "#efefef"),
            ("color", "#010101"),
            ("border-width", "1px"),
            ("border-style", "solid"),
            ("border-color", "#767676"),
            ("border-radius", "4px"),
            ("padding", "1px 6px"),
            ("font-size", "13.33px"),
            ("text-align", "center"),
        ],
        "input" => &[
            ("display", "inline-block"),
            ("background-color", "#ffffff"),
            ("color", "#010101"),
            ("border-width", "1px"),
            ("border-style", "solid"),
            ("border-color", "#767676"),
            ("border-radius", "4px"),
            ("padding", "1px 6px"),
            ("font-size", "13.33px"),
            ("cursor", "text"),
        ],
        "select" => &[("display", "inline-block")],
        "textarea" => &[("display", "inline-block")],
        "label" => &[("display", "inline")],
        "fieldset" => &[
            ("display", "block"),
            ("border-width", "2px"),
            ("border-style", "groove"),
            ("margin", "0 2px"),
            ("padding", "5px 12px 10px"),
        ],
        "legend" => &[("display", "block"), ("padding", "0 2px")],
        "form" => &[("display", "block")],

        // ── Tables ──────────────────────────────────────────────
        "table" => &[("display", "block")],
        "caption" => &[("display", "block")],
        "thead" => &[("display", "block")],
        "tbody" => &[("display", "block")],
        "tfoot" => &[("display", "block")],
        "tr" => &[("display", "block")],
        "td" => &[("display", "block")],
        "th" => &[("display", "block"), ("font-weight", "bold"), ("text-align", "center")],

        // ── Interactive ─────────────────────────────────────────
        "details" => &[("display", "block")],
        "summary" => &[("display", "block")],
        "dialog" => &[("display", "block")],

        _ => &[],
    }
}

/// User-agent default :hover styles (lowest priority — merged FIRST, any
/// matching user `:hover` rule overrides per-property, like browsers).
fn ua_hover_defaults(tag: &str) -> &'static [(&'static str, &'static str)] {
    match tag {
        "button" => &[("background-color", "#e6e6e6")],
        _ => &[],
    }
}

/// User-agent default :active styles (pressed state — darker face + border,
/// mirroring the browser's buttonface → buttonhighlight/buttonshadow shift).
fn ua_active_defaults(tag: &str) -> &'static [(&'static str, &'static str)] {
    match tag {
        "button" => &[("background-color", "#d4d4d4"), ("border-color", "#5a5a5a")],
        _ => &[],
    }
}

fn apply_ua_defaults(style: &mut IRStyle, tag: &str) {
    for (prop, val) in ua_defaults(tag) {
        apply_css_prop(style, prop, val);
    }
}

/// Infer a C++ type for a state init literal so snippet translations get
/// operand classes and member-access style for ambient reads. Mirrors the
/// init-based inference in `morph-codegen`'s emitter.
fn infer_state_type(init: &str) -> Option<String> {
    let s = init.trim();
    if s == "true" || s == "false" {
        return Some("bool".to_string());
    }
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        return Some("std::string".to_string());
    }
    if s.starts_with('[') && s.ends_with(']') {
        return Some("JsArray".to_string());
    }
    if s.parse::<i64>().is_ok() {
        return Some("int".to_string());
    }
    if s.parse::<f64>().is_ok() {
        return Some("double".to_string());
    }
    None
}

/// Remove `static`/`static inline` linkage from a translated top-level
/// function so it gets external linkage in the app TU (mirrors Python's
/// `strip_static_function`, visible to user .cpp code).
fn strip_static_linkage(cpp: &str) -> String {
    let mut text = cpp.trim().to_string();
    for prefix in ["static inline", "static"] {
        if text.starts_with(prefix)
            && text[prefix.len()..].chars().next().is_some_and(char::is_whitespace)
        {
            text = text[prefix.len()..].trim_start().to_string();
            break;
        }
    }
    text
}

/// Analyze a dynamic `className` template/expression for ternary branches
/// with string-literal arms (`... ${cond ? "a" : "b"}`). Each branch's
/// classes resolve to CSS declarations at build time (Tailwind + matching
/// stylesheet rules, pseudo rules skipped), and the condition becomes a
/// translated C++ bool expression. Mirrors Python's
/// `_analyze_class_template` / `_analyze_class_expression`.
/// Returns the conditional effects; best-effort (unparseable input yields none).
fn analyze_dynamic_class(
    source: &str,
    tag: &str,
    css_rules: &[(String, morph_parser::CssRule)],
    tailwind: &TailwindResolver,
    ambient_vars: &HashMap<String, String>,
    ambient_types: &HashMap<String, String>,
    type_mode: morpher::TypeMode,
    extra_headers: &mut Vec<String>,
) -> Vec<crate::node::IRConditionalClassEffect> {
    let mut effects = Vec::new();
    let allocator = Allocator::default();
    let source_type = SourceType::from_path("snippet.ts").unwrap_or_default().with_typescript(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        return effects;
    }
    let first = parsed.program.body.first();
    let expr = match first {
        Some(Statement::ExpressionStatement(stmt)) => &stmt.expression,
        _ => return effects,
    };
    // Collect top-level ternary expressions: the template's ${...} parts
    // plus a bare ternary expression. For template parts, carry the
    // surrounding static text: resolving the arm alone ("on" / "") would
    // match no rule, while the full branch string ("chip on" / "chip ")
    // resolves to the real declarations.
    let mut ternaries: Vec<(&ConditionalExpression, String, String)> = Vec::new();
    match expr {
        Expression::TemplateLiteral(tpl) => {
            for (i, part) in tpl.expressions.iter().enumerate() {
                if let Expression::ConditionalExpression(cond) = part {
                    let quasi_text = |q: &TemplateElement| {
                        q.value
                            .cooked
                            .as_ref()
                            .map(|c| c.as_str().to_string())
                            .unwrap_or_else(|| q.value.raw.as_str().to_string())
                    };
                    let prefix: String = tpl.quasis.iter().take(i + 1).map(quasi_text).collect();
                    let suffix: String = tpl.quasis.iter().skip(i + 1).map(quasi_text).collect();
                    ternaries.push((cond, prefix, suffix));
                }
            }
        }
        Expression::ConditionalExpression(cond) => {
            ternaries.push((cond, String::new(), String::new()))
        }
        _ => {}
    }
    for (ternary, prefix, suffix) in ternaries {
        let on_str = format!("{prefix}{}{suffix}", string_branch(&ternary.consequent));
        let off_str = format!("{prefix}{}{suffix}", string_branch(&ternary.alternate));
        let on_styles = resolve_branch_classes(&on_str, tag, css_rules, tailwind);
        let off_styles = resolve_branch_classes(&off_str, tag, css_rules, tailwind);
        if on_styles.is_empty() && off_styles.is_empty() {
            continue;
        }
        let cond_src =
            &source[ternary.test.span().start as usize..ternary.test.span().end as usize];
        let mut options = morpher::TranslateOptions::default();
        options.type_mode = type_mode;
        options.state_vars = ambient_vars.clone();
        options.state_types = ambient_types.clone();
        let cond_cpp = morpher::translate_snippet(cond_src, "snippet.ts", options)
            .ok()
            .map(|out| {
                extra_headers.extend(include_lines(&out.includes));
                out.body.trim().trim_end_matches(';').trim().to_string()
            })
            .unwrap_or_default();
        if cond_cpp.is_empty() {
            continue;
        }
        effects.push(crate::node::IRConditionalClassEffect {
            condition: cond_cpp,
            on_styles,
            off_styles,
        });
    }
    effects
}

/// The class string of a ternary branch when it is a plain string literal.
fn string_branch(expr: &Expression) -> String {
    match expr {
        Expression::StringLiteral(lit) => lit.value.to_string(),
        _ => String::new(),
    }
}

/// Resolve a branch's class tokens to CSS declarations: Tailwind first,
/// then matching non-pseudo stylesheet rules (later winners overwrite).
fn resolve_branch_classes(
    class_str: &str,
    tag: &str,
    css_rules: &[(String, morph_parser::CssRule)],
    tailwind: &TailwindResolver,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let tokens: Vec<String> = class_str
        .split_whitespace()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    for token in &tokens {
        for (prop, val) in tailwind.resolve(token) {
            out.insert(prop, val);
        }
    }
    for (selector, rule) in css_rules {
        if selector.contains(":hover") || selector.contains(":active") {
            continue;
        }
        if match_selector_detailed(tag, &tokens, None, &[], selector).is_some() {
            for (prop, val) in &rule.properties {
                out.insert(prop.clone(), val.clone());
            }
        }
    }
    out
}

/// Collect bare `#include` specs (`<string>`, `"x.h"`) from a snippet's
/// split-off header block. The app template adds the `#include` keyword
/// itself, mirroring Python's translator `_needed` set.
fn include_lines(header: &str) -> Vec<String> {
    header
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("#include"))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}
/// Split a leading/trailing `:hover` / `:active` pseudo-class off a simple or
/// compound selector. Only the *last* component's pseudo applies to the element
/// itself; earlier-ancestor pseudos are not handled by this builder.
fn split_trailing_pseudo(sel: &str) -> Option<(&str, Option<PseudoKind>)> {
    let mut pseudo: Option<PseudoKind> = None;
    let mut s = sel;
    loop {
        let t = s.trim_end();
        if let Some(rest) = t.strip_suffix(":hover") {
            pseudo = Some(PseudoKind::Hover);
            s = rest;
        } else if let Some(rest) = t.strip_suffix(":active") {
            pseudo = Some(PseudoKind::Active);
            s = rest;
        } else {
            break;
        }
    }
    Some((s.trim_end(), pseudo))
}

/// Match a single (possibly compound, pseudo-stripped) selector against the
/// element. No combinators/descendant selectors are supported here.
fn match_selector_compound(tag: &str, classes: &[String], id: Option<&str>, sel: &str) -> bool {
    let sel = sel.trim();
    if sel.is_empty() {
        return false;
    }
    if sel == "*" {
        return true;
    }

    let mut matched_tag = false;
    let mut tag_found = false;
    let mut required_classes: Vec<&str> = Vec::new();
    let mut has_id = false;
    let mut id_ok = true;

    let bytes = sel.as_bytes();
    let mut i = 0;
    let mut buf = String::new();

    while i < bytes.len() {
        let ch = sel[i..].chars().next().unwrap();
        match ch {
            '.' => {
                flush_tag(&mut buf, &mut tag_found, &mut matched_tag, tag);
                i += 1;
                let start = i;
                while i < bytes.len() && !" .#:[]>~+*".contains(sel[i..].chars().next().unwrap()) {
                    i += sel[i..].chars().next().unwrap().len_utf8();
                }
                if i > start {
                    required_classes.push(&sel[start..i]);
                }
            }
            '#' => {
                flush_tag(&mut buf, &mut tag_found, &mut matched_tag, tag);
                i += 1;
                let start = i;
                while i < bytes.len() && !" .#:[]>~+*".contains(sel[i..].chars().next().unwrap()) {
                    i += sel[i..].chars().next().unwrap().len_utf8();
                }
                has_id = true;
                if id.is_some_and(|v| v == &sel[start..i]) {
                    // id matches
                } else {
                    id_ok = false;
                }
            }
            ':' | '[' | '>' | '~' | '+' | ' ' => {
                // Unsupported pseudo/attribute/descendant — remaining structural
                // tail is not a valid element matcher for this builder.
                break;
            }
            _ => {
                buf.push(ch);
                i += 1;
            }
        }
    }
    // Trailing tag text after the last class/id token.
    flush_tag(&mut buf, &mut tag_found, &mut matched_tag, tag);

    if has_id && !id_ok {
        return false;
    }
    if tag_found && !matched_tag {
        return false;
    }
    for c in required_classes {
        if !classes.iter().any(|cl| cl.as_str() == c) {
            return false;
        }
    }
    true
}

/// Strip one `{…}` container layer (attribute expression spans cover
/// their braces; call sites need the bare object expression as-is).
fn strip_braces(expr: &str) -> String {
    let trimmed = expr.trim();
    trimmed
        .strip_prefix('{')
        .and_then(|b| b.strip_suffix('}'))
        .map_or_else(|| trimmed.to_string(), |inner| inner.trim().to_string())
}

/// True for external link targets: any URI scheme (`https:`, `http:`,
/// `mailto:`, …). Everything else is an internal route id.
fn is_external_href(href: &str) -> bool {
    if href.contains("://") {
        return true;
    }
    href.find(':').is_some_and(|i| {
        href[..i].chars().enumerate().all(|(n, c)| {
            c.is_ascii_alphabetic() || (n > 0 && (c.is_ascii_digit() || "+.-".contains(c)))
        })
    })
}

/// Map a JSX event prop to its trigger name. Anything unlisted is not
/// an event the runtime wires, so it falls through to attribute handling.
fn event_trigger(prop: &str) -> Option<&'static str> {
    match prop {
        "onClick" => Some("click"),
        "onInput" => Some("input"),
        "onChange" => Some("change"),
        "onFocus" => Some("focus"),
        "onBlur" => Some("blur"),
        "onKeyUp" => Some("keyup"),
        "onKeyDown" => Some("keydown"),
        "onMouseEnter" => Some("mouseenter"),
        "onMouseLeave" => Some("mouseleave"),
        "onMouseDown" => Some("mousedown"),
        "onMouseUp" => Some("mouseup"),
        _ => None,
    }
}

/// Class list and id of an element, shared by matching and ancestry.
fn element_classes_id(
    props: &HashMap<String, morph_parser::JsxPropValue>,
) -> (Vec<String>, Option<String>) {
    let classes = match props.get("className").or_else(|| props.get("class")) {
        Some(morph_parser::JsxPropValue::String(c)) => {
            c.split_whitespace().map(std::string::ToString::to_string).collect()
        }
        _ => Vec::new(),
    };
    let id = props.get("id").and_then(|v| match v {
        morph_parser::JsxPropValue::String(s) => Some(s.clone()),
        _ => None,
    });
    (classes, id)
}

/// Flush a buffered bare tag token (e.g. `button` in `button.btn.ghost`).
fn flush_tag(buf: &mut String, tag_found: &mut bool, matched_tag: &mut bool, tag: &str) {
    let s = buf.trim();
    if !s.is_empty() && s != "*" {
        *tag_found = true;
        if s == tag {
            *matched_tag = true;
        }
    }
    buf.clear();
}

/// One ancestor step for descendant-selector matching.
#[derive(Clone, Default)]
struct AncestorHint {
    tag: String,
    classes: Vec<String>,
    id: Option<String>,
}

/// Selector specificity as (ids, classes, tags): higher wins regardless
/// of source order, matching browser cascade. Pseudo-classes count as
/// classes. `!important` is not tracked (the parser merges it away), and
/// sibling combinators are unsupported (see below).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
struct Specificity(u32, u32, u32);

/// Count specificity of one comma-free alternative: `#` per id, `.` and
/// `:` runs per class/pseudo-class, bare leading words per tag.
fn selector_specificity(alternative: &str) -> Specificity {
    let mut ids = 0;
    let mut classes = 0;
    let mut tags = 0;
    let mut word_start = true;
    let mut chars = alternative.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '#' => {
                ids += 1;
                word_start = false;
            }
            '.' => {
                classes += 1;
                word_start = false;
            }
            ':' => {
                if chars.peek() == Some(&':') {
                    chars.next();
                }
                classes += 1;
                word_start = false;
            }
            '[' => {
                classes += 1;
                word_start = false;
            }
            ' ' | '>' | '+' | '~' | ',' | '*' => {
                word_start = true;
            }
            _ => {
                if word_start && ch.is_alphabetic() {
                    tags += 1;
                }
                word_start = false;
            }
        }
    }
    Specificity(ids, classes, tags)
}

/// How one compound attaches to the previous one. Sibling combinators
/// (`+`, `~`) have no meaning in a flat single-node walk.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Combinator {
    Descendant,
    Child,
}

/// Split `div > p` into per-compound steps with their combinators.
/// `None` for sibling combinators and attribute selectors: ignoring the
/// rule beats applying it to the wrong element.
fn split_selector_sequence(selector: &str) -> Option<Vec<(Option<Combinator>, String)>> {
    if selector.contains('[') {
        return None;
    }
    let mut steps: Vec<(Option<Combinator>, String)> = Vec::new();
    let mut current = String::new();
    let mut pending = None;
    let mut chars = selector.chars().peekable();
    for ch in chars {
        match ch {
            ' ' | '\t' => {
                if !current.trim().is_empty() {
                    steps.push((pending.take(), current.trim().to_string()));
                    current = String::new();
                }
                if pending.is_none() {
                    pending = Some(Combinator::Descendant);
                }
            }
            '>' => {
                if !current.trim().is_empty() {
                    steps.push((pending.take(), current.trim().to_string()));
                    current = String::new();
                }
                pending = Some(Combinator::Child);
            }
            '+' | '~' => {
                return None;
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.trim().is_empty() {
        steps.push((pending.take(), current.trim().to_string()));
    }
    if steps.is_empty() {
        return None;
    }
    Some(steps)
}

/// Match full steps right-to-left: the last compound hits the element,
/// the rest walk the ancestor chain (`ancestors[0]` is the parent).
fn match_sequence(
    tag: &str,
    classes: &[String],
    id: Option<&str>,
    ancestors: &[AncestorHint],
    steps: &[(Option<Combinator>, String)],
) -> bool {
    let Some((_, last)) = steps.last() else {
        return false;
    };
    if !match_selector_compound(tag, classes, id, last) {
        return false;
    }
    let mut ancestor_at = 0;
    for index in (1..steps.len()).rev() {
        let compound = &steps[index - 1].1;
        match steps[index].0 {
            None | Some(Combinator::Child) => {
                let Some(ancestor) = ancestors.get(ancestor_at) else {
                    return false;
                };
                if !match_selector_compound(
                    &ancestor.tag,
                    &ancestor.classes,
                    ancestor.id.as_deref(),
                    compound,
                ) {
                    return false;
                }
                ancestor_at += 1;
            }
            Some(Combinator::Descendant) => {
                let mut found = false;
                while let Some(ancestor) = ancestors.get(ancestor_at) {
                    ancestor_at += 1;
                    if match_selector_compound(
                        &ancestor.tag,
                        &ancestor.classes,
                        ancestor.id.as_deref(),
                        compound,
                    ) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
        }
    }
    true
}

/// Match with specificity of the winning alternative. Unknown pseudos
/// and attribute selectors never match (unsupported, ignored); among
/// matching alternatives the most specific one counts, not the first.
fn match_selector_detailed(
    tag: &str,
    classes: &[String],
    id: Option<&str>,
    ancestors: &[AncestorHint],
    selector: &str,
) -> Option<(PseudoKind, Specificity)> {
    let mut best: Option<(PseudoKind, Specificity)> = None;
    for alternative in selector.trim().split(',') {
        let alternative = alternative.trim();
        if alternative.is_empty() {
            continue;
        }
        let (structural, pseudo) = match split_trailing_pseudo(alternative) {
            Some(split) => split,
            None => (alternative, None),
        };
        if structural.contains(':') || structural.contains('[') {
            continue;
        }
        let Some(steps) = split_selector_sequence(structural) else {
            continue;
        };
        if match_sequence(tag, classes, id, ancestors, &steps) {
            let specificity = selector_specificity(alternative);
            let better = best.map_or(true, |(_, held)| specificity > held);
            if better {
                best = Some((pseudo.unwrap_or(PseudoKind::Base), specificity));
            }
        }
    }
    best
}

/// Apply a CSS property to a style, returning the IR field name that was set
/// (used for `@keyframes` declared-field tracking), or None if unsupported.
fn apply_css_prop(style: &mut IRStyle, prop: &str, val: &str) -> Option<&'static str> {
    if !css_registry::is_known_property(prop) {
        return None;
    }
    match prop {
        "background-color" | "background" => {
            if let Some(c) = parse_color(val) {
                style.bg_color = c;
                Some("bg_color")
            } else {
                None
            }
        }
        "color" => {
            if let Some(c) = parse_color(val) {
                style.color = c;
                Some("color")
            } else {
                None
            }
        }
        "width" => {
            if let Some(v) = parse_length(val) {
                style.width = Some(v);
                Some("width")
            } else {
                None
            }
        }
        "height" => {
            if let Some(v) = parse_length(val) {
                style.height = Some(v);
                Some("height")
            } else {
                None
            }
        }
        "min-width" => {
            if let Some(v) = parse_length(val) {
                style.min_width = Some(v);
                Some("min_width")
            } else {
                None
            }
        }
        "max-width" => {
            if let Some(v) = parse_length(val) {
                style.max_width = Some(v);
                Some("max_width")
            } else {
                None
            }
        }
        "min-height" => {
            if let Some(v) = parse_length(val) {
                style.min_height = Some(v);
                Some("min_height")
            } else {
                None
            }
        }
        "max-height" => {
            if let Some(v) = parse_length(val) {
                style.max_height = Some(v);
                Some("max_height")
            } else {
                None
            }
        }
        "padding" => {
            if let Some(v) = parse_box_sides(val) {
                style.padding = v;
                Some("padding")
            } else {
                None
            }
        }
        "margin" => {
            if let Some(v) = parse_box_sides(val) {
                style.margin = v;
                Some("margin")
            } else {
                None
            }
        }
        "border-radius" => {
            if let Some(v) = parse_length(val) {
                style.border_radius = v;
                Some("border_radius")
            } else {
                None
            }
        }
        "font-size" => {
            if let Some(v) = parse_length(val) {
                style.font_size = v;
                Some("font_size")
            } else {
                None
            }
        }
        "font-weight" => {
            style.font_weight = val.to_string();
            Some("font_weight")
        }
        "text-align" => {
            style.text_align = val.to_string();
            Some("text_align")
        }
        "display" => {
            style.display = val.to_string();
            Some("display")
        }
        "flex-direction" => {
            style.flex_dir = val.to_string();
            Some("flex_dir")
        }
        "gap" => {
            if let Some(v) = parse_length(val) {
                style.gap = v;
                Some("gap")
            } else {
                None
            }
        }
        "position" => {
            style.position = val.to_string();
            Some("position")
        }
        "left" => {
            style.left = parse_length(val);
            if style.left.is_some() {
                Some("left")
            } else {
                None
            }
        }
        "right" => {
            style.right = parse_length(val);
            if style.right.is_some() {
                Some("right")
            } else {
                None
            }
        }
        "top" => {
            style.top = parse_length(val);
            if style.top.is_some() {
                Some("top")
            } else {
                None
            }
        }
        "bottom" => {
            style.bottom = parse_length(val);
            if style.bottom.is_some() {
                Some("bottom")
            } else {
                None
            }
        }
        "justify-content" => {
            style.justify_content = val.to_string();
            Some("justify_content")
        }
        "align-items" => {
            style.align_items = val.to_string();
            Some("align_items")
        }
        "flex-wrap" => {
            style.flex_wrap = val.to_string();
            Some("flex_wrap")
        }
        "flex-grow" => {
            if let Ok(v) = val.trim().parse::<f32>() {
                style.flex_grow = v;
                Some("flex_grow")
            } else {
                None
            }
        }
        "flex-shrink" => {
            if let Ok(v) = val.trim().parse::<f32>() {
                style.flex_shrink = v;
                Some("flex_shrink")
            } else {
                None
            }
        }
        "flex-basis" => {
            let v = val.trim();
            style.flex_basis = if v == "auto" {
                "auto".to_string()
            } else if let Some(px) = parse_length(v) {
                format!("{px}px")
            } else {
                v.to_string()
            };
            Some("flex_basis")
        }
        "flex" => {
            parse_flex_shorthand(&mut *style, val);
            Some("flex")
        }
        "border" => {
            parse_border_shorthand(&mut *style, val);
            Some("border")
        }
        "margin-top" => {
            if let Some(v) = parse_length(val) {
                style.margin[0] = v;
                Some("margin")
            } else {
                None
            }
        }
        "margin-right" => {
            if let Some(v) = parse_length(val) {
                style.margin[1] = v;
                Some("margin")
            } else {
                None
            }
        }
        "margin-bottom" => {
            if let Some(v) = parse_length(val) {
                style.margin[2] = v;
                Some("margin")
            } else {
                None
            }
        }
        "margin-left" => {
            if let Some(v) = parse_length(val) {
                style.margin[3] = v;
                Some("margin")
            } else {
                None
            }
        }
        "padding-top" => {
            if let Some(v) = parse_length(val) {
                style.padding[0] = v;
                Some("padding")
            } else {
                None
            }
        }
        "padding-right" => {
            if let Some(v) = parse_length(val) {
                style.padding[1] = v;
                Some("padding")
            } else {
                None
            }
        }
        "padding-bottom" => {
            if let Some(v) = parse_length(val) {
                style.padding[2] = v;
                Some("padding")
            } else {
                None
            }
        }
        "padding-left" => {
            if let Some(v) = parse_length(val) {
                style.padding[3] = v;
                Some("padding")
            } else {
                None
            }
        }
        "cursor" => {
            style.cursor = val.to_string();
            Some("cursor")
        }
        "overflow" => {
            style.overflow = val.to_string();
            Some("overflow")
        }
        "opacity" => {
            if let Ok(v) = val.trim().parse::<f32>() {
                style.opacity = v;
                Some("opacity")
            } else {
                None
            }
        }
        "transform" => {
            // Resolve at build time so the emitted style carries a concrete
            // matrix (Python resolves via its layout engine in dev; prod leaves
            // it unresolved, so Rust is strictly a superset here).
            match transforms::parse_transform(val) {
                Some(ops) => {
                    style.transform_ops = Some(ops.clone());
                    if !ops.is_empty() {
                        style.transform_matrix =
                            Some(transforms::compose_transform(&ops, 0.0, 0.0));
                    }
                    Some("transform")
                }
                None => None,
            }
        }
        "transform-origin" => match transforms::parse_transform_origin(val) {
            Some((raw, resolved)) => {
                style.transform_origin = Some(raw);
                style.transform_origin_resolved = resolved;
                Some("transform_origin")
            }
            None => None,
        },
        "z-index" => {
            if let Ok(v) = val.parse::<i32>() {
                style.z_index = Some(v);
                Some("z_index")
            } else {
                None
            }
        }
        "border-width" => {
            if let Some(v) = parse_length(val) {
                style.border_width = v;
                Some("border_width")
            } else {
                None
            }
        }
        "border-color" => {
            if let Some(c) = parse_color(val) {
                style.border_color = c;
                Some("border_color")
            } else {
                None
            }
        }
        "border-style" => {
            style.border_style = val.to_string();
            Some("border_style")
        }
        "box-sizing" => {
            style.box_sizing = val.to_string();
            Some("box_sizing")
        }
        _ => None,
    }
}

/// Parse the CSS `flex` shorthand into grow/shrink/basis.
/// Mirrors Python `_parse_flex_shorthand`.
fn parse_flex_shorthand(style: &mut IRStyle, val: &str) {
    let kw = val.trim();
    let parts: Vec<&str> = kw.split_whitespace().collect();
    match kw {
        "none" => {
            style.flex_grow = 0.0;
            style.flex_shrink = 0.0;
            style.flex_basis = "auto".to_string();
        }
        "auto" => {
            style.flex_grow = 1.0;
            style.flex_shrink = 1.0;
            style.flex_basis = "auto".to_string();
        }
        "initial" => {
            style.flex_grow = 0.0;
            style.flex_shrink = 1.0;
            style.flex_basis = "auto".to_string();
        }
        _ => match parts.len() {
            1 => {
                if let Ok(v) = parts[0].parse::<f32>() {
                    style.flex_grow = v;
                    style.flex_shrink = 1.0;
                    style.flex_basis = "0%".to_string();
                }
            }
            2 => {
                if let (Ok(g), Ok(s)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                    style.flex_grow = g;
                    style.flex_shrink = s;
                    style.flex_basis = "0%".to_string();
                }
            }
            _ => {
                if parts.len() >= 3 {
                    if let (Ok(g), Ok(s)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                        style.flex_grow = g;
                        style.flex_shrink = s;
                        style.flex_basis = parts[2].to_string();
                    }
                }
            }
        },
    }
}

/// Split the CSS `border` shorthand (`1px solid #232b3d`) into
/// width/style/color. Mirrors Python's `_css_to_ir_kw` branch.
fn parse_border_shorthand(style: &mut IRStyle, val: &str) {
    for part in val.split_whitespace() {
        if matches!(part, "solid" | "dashed" | "dotted" | "none") {
            style.border_style = part.to_string();
        } else if part.starts_with('#') || part.starts_with("rgb") || part == "transparent" {
            if let Some(c) = parse_color(part) {
                style.border_color = c;
            }
        } else if let Some(w) = parse_length(part) {
            style.border_width = w;
        }
    }
}

/// Parse CSS 1-4 value box shorthand (`10px`, `6px 12px`, ...) into
/// [top, right, bottom, left], mirroring Python's per-side conversion.
fn parse_box_sides(s: &str) -> Option<[f32; 4]> {
    let parts: Vec<Option<f32>> = s.split_whitespace().map(parse_length).collect();
    if parts.is_empty() || parts.len() > 4 || parts.iter().any(std::option::Option::is_none) {
        return None;
    }
    let v: Vec<f32> = parts.into_iter().map(|p| p.unwrap_or(0.0)).collect();
    Some(match v.len() {
        1 => [v[0], v[0], v[0], v[0]],
        2 => [v[0], v[1], v[0], v[1]],
        3 => [v[0], v[1], v[2], v[1]],
        _ => [v[0], v[1], v[2], v[3]],
    })
}

/// Parse a CSS time (`0.3s` / `500ms` / unitless seconds) to seconds.
fn parse_css_time(raw: &str) -> Option<f32> {
    let s = raw.trim().to_lowercase();
    if let Some(ms) = s.strip_suffix("ms") {
        return ms.trim().parse::<f32>().ok().map(|v| v / 1000.0);
    }
    if let Some(sec) = s.strip_suffix('s') {
        return sec.trim().parse::<f32>().ok();
    }
    s.parse::<f32>().ok()
}

fn map_easing(low: &str) -> Option<&'static str> {
    // The runtime has no separate `ease` curve: it is ease-in-out,
    // mirroring Python's _EASING_KEYWORDS.
    Some(match low {
        "linear" => "linear",
        "ease" | "ease-in-out" => "ease-in-out",
        "ease-in" => "ease-in",
        "ease-out" => "ease-out",
        _ => return None,
    })
}

/// Split `animation: a, b` on top-level commas (ignores parens).
fn split_animation_list(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in raw.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                if !cur.trim().is_empty() {
                    parts.push(cur.trim().to_string());
                }
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

/// Parse one comma-separated `animation` shorthand value. Mirrors Python's
/// `_parse_animation_component` (first time = duration, second = delay,
/// bare numbers = iteration count, first unclassified token = name).
fn parse_animation_component(raw: &str) -> crate::node::IRAnimation {
    let mut anim = crate::node::IRAnimation::default();
    let mut unclassified: Vec<String> = Vec::new();
    for tok in raw.split_whitespace() {
        let low = tok.to_lowercase();
        if let Some(easing) = map_easing(&low) {
            anim.easing = easing.to_string();
        } else if matches!(low.as_str(), "normal" | "reverse" | "alternate" | "alternate-reverse") {
            anim.direction = low;
        } else if matches!(low.as_str(), "none" | "forwards" | "backwards" | "both") {
            anim.fill_mode = low;
        } else if matches!(low.as_str(), "running" | "paused") {
            anim.play_state = low;
        } else if low == "infinite" {
            anim.iterations = -1.0;
        } else if low.parse::<f32>().is_ok() {
            // Fractional counts (2.5) must not misparse as times.
            anim.iterations = low.parse::<f32>().unwrap_or(1.0);
        } else if let Some(t) = parse_css_time(&low) {
            if anim.duration == 0.0 {
                anim.duration = t;
            } else {
                anim.delay = t;
            }
        } else if !low.contains('(') {
            // Unsupported easing functions (cubic-bezier, steps) ignored.
            unclassified.push(tok.to_string());
        }
    }
    if !unclassified.is_empty() {
        anim.name = unclassified.into_iter().next().unwrap_or_default();
    }
    anim
}

fn parse_animation_shorthand(raw: &str) -> Vec<crate::node::IRAnimation> {
    split_animation_list(raw)
        .iter()
        .map(|part| parse_animation_component(part))
        .filter(|anim| !anim.name.is_empty())
        .collect()
}

const ANIMATION_LONGHANDS: &[&str] = &[
    "animation-name",
    "animation-duration",
    "animation-timing-function",
    "animation-delay",
    "animation-iteration-count",
    "animation-direction",
    "animation-fill-mode",
    "animation-play-state",
];

fn apply_animation_longhand(anim: &mut crate::node::IRAnimation, prop: &str, value: &str) -> bool {
    let val = value.trim().to_lowercase();
    match prop {
        "animation-name" => anim.name = val,
        "animation-duration" => {
            anim.duration = parse_css_time(&val).unwrap_or(anim.duration);
            if parse_css_time(&val).is_none() {
                return false;
            }
        }
        "animation-timing-function" => {
            if let Some(easing) = map_easing(&val) {
                anim.easing = easing.to_string();
            }
        }
        "animation-delay" => {
            if parse_css_time(&val).is_none() {
                return false;
            }
            anim.delay = parse_css_time(&val).unwrap_or(anim.delay);
        }
        "animation-iteration-count" => {
            if val == "infinite" {
                anim.iterations = -1.0;
            } else if let Ok(n) = val.parse::<f32>() {
                anim.iterations = n;
            } else {
                return false;
            }
        }
        "animation-direction" => {
            if matches!(val.as_str(), "normal" | "reverse" | "alternate" | "alternate-reverse") {
                anim.direction = val;
            }
        }
        "animation-fill-mode" => {
            if matches!(val.as_str(), "none" | "forwards" | "backwards" | "both") {
                anim.fill_mode = val;
            }
        }
        "animation-play-state" => {
            if matches!(val.as_str(), "running" | "paused") {
                anim.play_state = val;
            }
        }
        _ => {}
    }
    true
}

/// Build a node's animation list from merged CSS declarations: the
/// `animation` shorthand first, then longhands as per-index overrides
/// (CSS list semantics: the last value repeats). Animations without a
/// name are dropped; play-state alone never creates one.
fn parse_animations(merged: &HashMap<String, String>) -> Vec<crate::node::IRAnimation> {
    let mut anims: Vec<crate::node::IRAnimation> =
        merged.get("animation").map(|raw| parse_animation_shorthand(raw)).unwrap_or_default();
    let mut longhands: Vec<(&str, Vec<String>)> = Vec::new();
    for prop in ANIMATION_LONGHANDS {
        if let Some(raw) = merged.get(*prop) {
            longhands.push((prop, split_animation_list(raw)));
        }
    }
    if longhands.is_empty() {
        return anims.into_iter().filter(|a| !a.name.is_empty()).collect();
    }
    let count =
        longhands.iter().map(|(_, values)| values.len()).max().unwrap_or(0).max(anims.len());
    while anims.len() < count {
        anims.push(crate::node::IRAnimation::default());
    }
    for (prop, values) in &longhands {
        for (i, anim) in anims.iter_mut().enumerate().take(count) {
            let val = values.get(i).or_else(|| values.last());
            if let Some(val) = val {
                apply_animation_longhand(anim, prop, val);
            }
        }
    }
    anims.into_iter().filter(|a| !a.name.is_empty()).collect()
}

fn parse_length(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix("px") {
        return num.trim().parse().ok();
    }
    if let Some(num) = s.strip_suffix("rem") {
        return num.trim().parse::<f32>().ok().map(|v| v * 16.0);
    }
    if let Some(num) = s.strip_suffix("em") {
        return num.trim().parse::<f32>().ok().map(|v| v * 16.0);
    }
    if s.ends_with('%') {
        return None;
    }
    s.parse().ok()
}

fn parse_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim().to_lowercase();
    if s.starts_with('#') {
        let hex = s.trim_start_matches('#');
        let (r, g, b, a) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                (r, g, b, 255)
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                (r, g, b, a)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b, 255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                (r, g, b, a)
            }
            _ => return None,
        };
        return Some([
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        ]);
    }
    if s.starts_with("rgb") {
        return parse_rgb(&s);
    }
    match s.as_str() {
        "transparent" => Some([0.0, 0.0, 0.0, 0.0]),
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "red" => Some([1.0, 0.0, 0.0, 1.0]),
        "green" => Some([0.0, 0.5, 0.0, 1.0]),
        "blue" => Some([0.0, 0.0, 1.0, 1.0]),
        "gray" | "grey" => Some([0.5, 0.5, 0.5, 1.0]),
        _ => None,
    }
}

/// Parse `rgb(r,g,b)` / `rgba(r,g,b,a)` — components may be ints (0-255) or
/// percentages, and alpha may be a 0..1 float or percentage.
fn parse_rgb(s: &str) -> Option<[f32; 4]> {
    let inner = s.find('(')?;
    let end = s.rfind(')')?;
    let args = &s[inner + 1..end];
    let parts: Vec<&str> = args.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return None;
    }

    let comp = |p: &str| -> Option<f32> {
        let p = p.trim();
        if let Some(v) = p.strip_suffix('%') {
            Some(v.trim().parse::<f32>().ok()? / 100.0)
        } else {
            Some(p.parse::<f32>().ok()? / 255.0)
        }
    };

    let r = comp(parts[0])?;
    let g = comp(parts[1])?;
    let b = comp(parts[2])?;
    let a = if parts.len() >= 4 {
        let p = parts[3].trim();
        if let Some(v) = p.strip_suffix('%') {
            v.trim().parse::<f32>().ok()? / 100.0
        } else {
            p.parse::<f32>().ok()?
        }
    } else {
        1.0
    };
    Some([r, g, b, a])
}

impl Default for IRBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morph_parser::JsxPropValue;

    fn match_sel(sel: &str, cls: &[&str]) -> bool {
        let mut props = HashMap::new();
        props.insert("className".to_string(), JsxPropValue::String(cls.join(" ")));
        let (classes, id) = element_classes_id(&props);
        match_selector_detailed("button", &classes, id.as_deref(), &[], sel).is_some()
    }

    #[test]
    fn compound_ghost() {
        assert!(match_sel(".btn.ghost", &["btn", "ghost"]), ".btn.ghost should match btn ghost");
        assert!(!match_sel(".btn.ghost", &["btn"]), ".btn.ghost should NOT match btn only");
        assert!(match_sel(".btn", &["btn", "ghost"]), ".btn should match btn ghost");
        assert!(match_sel(".btn.ghost:hover", &["btn", "ghost"]), "hover compound should match");
    }

    #[test]
    fn transparent_parsed() {
        // lightningcss serializes `background-color: transparent` as #0000 (4-digit)
        // and `rgba(79,123,255,0)` as #4f7cff00 (8-digit). Both must parse to alpha 0.
        let close = |a: Option<[f32; 4]>, b: [f32; 4]| -> bool {
            match a {
                Some(v) => (0..4).all(|i| (v[i] - b[i]).abs() < 0.001),
                None => false,
            }
        };
        assert!(close(parse_color("transparent"), [0.0, 0.0, 0.0, 0.0]));
        assert!(close(parse_color("#0000"), [0.0, 0.0, 0.0, 0.0]));
        assert!(close(parse_color("#4f7cff00"), [0.3098, 0.4863, 1.0, 0.0]));
        assert!(close(parse_color("rgba(79, 124, 255, 0)"), [0.3098, 0.4863, 1.0, 0.0]));
        assert!(close(parse_color("rgba(255,255,255,0.5)"), [1.0, 1.0, 1.0, 0.5]));
        assert!(close(parse_color("rgb(255,0,0)"), [1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn specificity_orders_id_over_class_over_tag() {
        assert!(selector_specificity("#a") > selector_specificity(".b.c.d"));
        assert!(selector_specificity(".b") > selector_specificity("div"));
        assert!(selector_specificity("div p") > selector_specificity("p"));
        assert_eq!(selector_specificity(".a"), selector_specificity(".b"));
    }

    fn detailed(
        tag: &str,
        classes: &[&str],
        ancestors: &[(&str, &[&str])],
        selector: &str,
    ) -> Option<(PseudoKind, Specificity)> {
        let owned_classes: Vec<String> =
            classes.iter().map(std::string::ToString::to_string).collect();
        let owned_ancestors: Vec<AncestorHint> = ancestors
            .iter()
            .map(|(tag, classes)| AncestorHint {
                tag: tag.to_string(),
                classes: classes.iter().map(std::string::ToString::to_string).collect(),
                id: None,
            })
            .collect();
        match_selector_detailed(tag, &owned_classes, None, &owned_ancestors, selector)
    }

    #[test]
    fn descendant_matches_through_ancestors() {
        let wrap: &[&str] = &["wrap"];
        let ancestors = [("div", wrap)];
        assert!(detailed("p", &[], &ancestors, "div p").is_some());
        assert!(detailed("p", &[], &[], "div p").is_none());
        assert!(detailed("span", &[], &ancestors, "div > span").is_some());
    }

    #[test]
    fn sibling_combinators_never_match() {
        let empty: &[&str] = &[];
        let ancestors = [("h2", empty)];
        assert!(detailed("p", &[], &ancestors, "h2 + p").is_none());
        assert!(detailed("p", &[], &ancestors, "h2 ~ p").is_none());
    }

    #[test]
    fn grouped_alternatives_use_matching_specificity() {
        let matched = detailed("p", &["note"], &[], ".note, #other").unwrap();
        assert_eq!(matched.1, selector_specificity(".note"));
    }

    #[test]
    fn width_attribute_loses_to_stylesheet() {
        let builder = IRBuilder::new();
        let mut props = HashMap::new();
        props.insert("width".to_string(), JsxPropValue::String("400".to_string()));
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "img".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
        );
        assert_eq!(node.style.width, Some(400.0));

        let mut props = HashMap::new();
        props.insert("width".to_string(), JsxPropValue::String("400".to_string()));
        let rules = vec![(
            ".wide".to_string(),
            morph_parser::CssRule {
                selector: ".wide".to_string(),
                properties: [("width".to_string(), "100px".to_string())].into_iter().collect(),
            },
        )];
        props.insert("className".to_string(), JsxPropValue::String("wide".to_string()));
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "img".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &rules,
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
        );
        assert_eq!(node.style.width, Some(100.0));
    }

    #[test]
    fn embedded_logic_transpiles_to_cpp_premain() {
        use morph_parser::{
            ComponentConst, InnerFunction, MxComponent, MxEffect, MxSource, StateVar,
        };
        let source = MxSource {
            filename: "app.mx".to_string(),
            imports: Vec::new(),
            window_config: None,
            components: vec![MxComponent {
                name: "App".to_string(),
                exported: true,
                event_subs: Vec::new(),
                is_default: true,
                props_param: String::new(),
                props: Vec::new(),
                params: Vec::new(),
                jsx: morph_parser::JsxNode::Text("hi".to_string()),
                state_vars: vec![StateVar {
                    getter: "count".to_string(),
                    setter: "setCount".to_string(),
                    init: "0".to_string(),
                    type_arg: None,
                }],
                effects: vec![
                    MxEffect {
                        callback: "() => { console.log(count); }".to_string(),
                        deps: "[]".to_string(),
                    },
                    MxEffect {
                        callback: "() => { console.log(count); }".to_string(),
                        deps: "[count]".to_string(),
                    },
                ],
                inner_functions: vec![InnerFunction {
                    name: "doLogin".to_string(),
                    source: "function doLogin() { setCount(count + 1); }".to_string(),
                    exported: false,
                }],
                consts: vec![ComponentConst {
                    name: "doubled".to_string(),
                    rhs: "count * 2".to_string(),
                }],
                console_logs: vec!["body log".to_string()],
            }],
            shared_bindings: Vec::new(),
            event_bindings: Vec::new(),
            state_vars: Vec::new(),
            effects: Vec::new(),
            inner_functions: Vec::new(),
            function_declarations: vec![InnerFunction {
                name: "helper".to_string(),
                source: "function helper() { return 1; }".to_string(),
                exported: false,
            }],
            class_declarations: Vec::new(),
            exported_vars: Vec::new(),
            named_exports: Vec::new(),
            default_export: None,
            re_exports: Vec::new(),
            global_vars: vec!["const API_URL = \"https://api.test\";".to_string()],
            console_logs: vec!["module log".to_string()],
            extra_headers: Vec::new(),
            cpp_imports: Vec::new(),
        };
        let windows = IRBuilder::new().build(&source, &[], &HashMap::new());
        assert_eq!(windows.len(), 1);
        let win = &windows[0];
        let premain = win.premain_functions.join("\n");
        // Raw JS must never reach the app TU: functions are transpiled and
        // stripped of internal linkage, consts become reactive lambdas.
        assert!(premain.contains("void doLogin()"), "handler transpiled: {premain}");
        assert!(premain.contains("auto helper"), "module fn transpiled: {premain}");
        assert!(premain.contains("API_URL"), "global transpiled: {premain}");
        assert!(!premain.contains("function "), "no raw JS: {premain}");
        assert!(
            premain.contains("auto doubled = []() { return ("),
            "const is reactive lambda: {premain}"
        );
        assert!(premain.contains("__st_count.get()"), "ambient state mapped: {premain}");
        assert!(!premain.contains("static "), "external linkage: {premain}");
        assert_eq!(win.reactive_consts, vec!["doubled".to_string()]);
        // Effects carry transpiled lambdas, not JS callbacks.
        assert_eq!(win.effect_decls.len(), 2);
        assert!(win.effect_decls[0].get("lambda").unwrap().starts_with('['));
        assert!(!win.effect_decls[0].get("lambda").unwrap().contains("=>"));
        assert_eq!(win.effect_decls[0].get("deps").unwrap(), "[]");
        assert_eq!(win.effect_decls[1].get("deps").unwrap(), "[count]");
        // Snippet headers (e.g. <print> for console.log) merge upward.
        assert!(win.extra_headers.iter().any(|h| h.contains("print")), "{:?}", win.extra_headers);
        // Logs merge: module first, then component body.
        assert_eq!(win.startup_logs, vec!["module log".to_string(), "body log".to_string()]);
    }

    #[test]
    fn box_shorthands_expand_per_side() {
        assert_eq!(parse_box_sides("18px"), Some([18.0, 18.0, 18.0, 18.0]));
        assert_eq!(parse_box_sides("10px 6px"), Some([10.0, 6.0, 10.0, 6.0]));
        assert_eq!(parse_box_sides("36px 32px 28px 32px"), Some([36.0, 32.0, 28.0, 32.0]));
        assert_eq!(parse_box_sides("1px 2px 3px"), Some([1.0, 2.0, 3.0, 2.0]));
        assert_eq!(parse_box_sides("10px auto"), None);
        assert_eq!(parse_box_sides(""), None);
    }

    #[test]
    fn flex_and_border_shorthands_map() {
        let builder = IRBuilder::new();
        let mut style = IRStyle::default();
        apply_css_prop(&mut style, "flex-grow", "1");
        apply_css_prop(&mut style, "flex-shrink", "0");
        assert_eq!(style.flex_grow, 1.0);
        assert_eq!(style.flex_shrink, 0.0);
        apply_css_prop(&mut style, "flex", "2");
        assert_eq!(style.flex_grow, 2.0);
        assert_eq!(style.flex_shrink, 1.0);
        assert_eq!(style.flex_basis, "0%");
        apply_css_prop(&mut style, "border", "1px solid #232b3d");
        assert_eq!(style.border_width, 1.0);
        assert_eq!(style.border_style, "solid");
        assert!(style.border_color[0] > 0.1 && style.border_color[0] < 0.2);
        apply_css_prop(&mut style, "margin-top", "50px");
        apply_css_prop(&mut style, "padding-left", "6px");
        assert_eq!(style.margin[0], 50.0);
        assert_eq!(style.padding[3], 6.0);
        let _ = builder;
    }

    #[test]
    fn static_class_names_stay_out_of_reactive_class() {
        // `key op` with an `op` signal must survive verbatim; only
        // className={...} becomes a reactive expression.
        let builder = IRBuilder::new();
        let mut props = HashMap::new();
        props.insert("className".to_string(), JsxPropValue::String("key op".to_string()));
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "button".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
        );
        assert!(node.reactive_class.is_empty());
        let mut props = HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::Expr("op === 1 ? \"a\" : \"b\"".to_string()),
        );
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "button".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
        );
        assert!(!node.reactive_class.is_empty());
    }

    #[test]
    fn template_className_analyzes_ternary_branches() {
        use morph_parser::{MxComponent, MxSource, StateVar};
        let mut props = HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::Template(
                "`header ${theme == \"light\" ? \"bg-white\" : \"bg-gray-900\"}`".to_string(),
            ),
        );
        let source = MxSource {
            filename: "app.mx".to_string(),
            imports: Vec::new(),
            window_config: None,
            components: vec![MxComponent {
                name: "App".to_string(),
                exported: true,
                event_subs: Vec::new(),
                is_default: true,
                props_param: String::new(),
                props: Vec::new(),
                params: Vec::new(),
                jsx: morph_parser::JsxNode::Element {
                    tag: "div".to_string(),
                    props,
                    children: Vec::new(),
                    self_closing: true,
                    line: 0,
                    col: 0,
                },
                state_vars: vec![StateVar {
                    getter: "theme".to_string(),
                    setter: "setTheme".to_string(),
                    init: "\"light\"".to_string(),
                    type_arg: None,
                }],
                effects: Vec::new(),
                inner_functions: Vec::new(),
                consts: Vec::new(),
                console_logs: Vec::new(),
            }],
            shared_bindings: Vec::new(),
            event_bindings: Vec::new(),
            state_vars: Vec::new(),
            effects: Vec::new(),
            inner_functions: Vec::new(),
            function_declarations: Vec::new(),
            class_declarations: Vec::new(),
            exported_vars: Vec::new(),
            named_exports: Vec::new(),
            default_export: None,
            re_exports: Vec::new(),
            global_vars: Vec::new(),
            console_logs: Vec::new(),
            extra_headers: Vec::new(),
            cpp_imports: Vec::new(),
        };
        let windows = IRBuilder::new().build(&source, &[], &HashMap::new());
        let node = &windows[0].nodes[0];
        assert_eq!(node.class_conditional_effects.len(), 1);
        let fx = &node.class_conditional_effects[0];
        assert!(fx.condition.contains("__st_theme"), "cond mapped: {}", fx.condition);
        assert_eq!(fx.on_styles.get("background-color").map(String::as_str), Some("#ffffff"));
        assert_eq!(fx.off_styles.get("background-color").map(String::as_str), Some("#111827"));
        assert!(!node.reactive_class.is_empty());
    }

    #[test]
    fn animation_shorthand_parses_and_filters_unknown_keyframes() {
        use morph_parser::{CssKeyframe, CssRule};
        let rules = vec![(
            ".pulse".to_string(),
            CssRule {
                selector: ".pulse".to_string(),
                properties: [(
                    "animation".to_string(),
                    "pulse 2s ease-in-out infinite".to_string(),
                )]
                .into_iter()
                .collect(),
            },
        )];
        let mut keyframes = HashMap::new();
        keyframes.insert("pulse".to_string(), Vec::<CssKeyframe>::new());
        let mut props = HashMap::new();
        props.insert("className".to_string(), JsxPropValue::String("pulse".to_string()));
        let jsx = morph_parser::JsxNode::Element {
            tag: "div".to_string(),
            props,
            children: Vec::new(),
            self_closing: true,
            line: 0,
            col: 0,
        };
        let builder = IRBuilder::new();
        let node = builder.build_node(
            &jsx,
            &rules,
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &keyframes,
        );
        assert_eq!(node.animations.len(), 1);
        let anim = &node.animations[0];
        assert_eq!(anim.name, "pulse");
        assert_eq!(anim.duration, 2.0);
        assert_eq!(anim.easing, "ease-in-out");
        assert_eq!(anim.iterations, -1.0);
        // Unknown keyframe names are dropped like browsers do.
        let rules = vec![(
            ".ghost".to_string(),
            CssRule {
                selector: ".ghost".to_string(),
                properties: [("animation".to_string(), "nope 1s linear infinite".to_string())]
                    .into_iter()
                    .collect(),
            },
        )];
        let mut props = HashMap::new();
        props.insert("className".to_string(), JsxPropValue::String("ghost".to_string()));
        let jsx = morph_parser::JsxNode::Element {
            tag: "div".to_string(),
            props,
            children: Vec::new(),
            self_closing: true,
            line: 0,
            col: 0,
        };
        let node = builder.build_node(
            &jsx,
            &rules,
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &keyframes,
        );
        assert!(node.animations.is_empty());
    }

    #[test]
    fn tailwind_class_names_flow_into_style() {
        let builder = IRBuilder::new();
        let mut props = HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::String("bg-red-500 text-lg".to_string()),
        );
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "div".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
        );
        assert!((node.style.bg_color[0] - 0xef as f32 / 255.0).abs() < 0.001);
        assert!((node.style.bg_color[1] - 0x44 as f32 / 255.0).abs() < 0.001);
        assert_eq!(node.style.bg_color[3], 1.0);
        assert_eq!(node.style.font_size, 18.0);
    }
}

#[cfg(test)]
mod component_tests {
    use super::*;
    use std::fmt::Write as _;

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("morph_ir_comp_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_file(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = String::new();
        write!(f, "{content}").unwrap();
        std::fs::write(p, f).unwrap();
    }

    const APP: &str = r#"
import { morphState } from 'morph'
import Counter from './Counter.mx'
import { Badge } from './Badge.mx'

export const windowConfig = { title: "Comp", width: 400, height: 300 }

export default function App() {
  const [total, setTotal] = morphState(0)
  function handleReset() { setTotal(0) }
  return (
    <body>
      <div>{total}</div>
      <Counter label="A" step={1} onStep={setTotal} />
      <Counter label="B" step={2} onStep={setTotal} />
      <Badge text={total} />
      <button onClick={() => handleReset()}>reset</button>
    </body>
  )
}
"#;

    const COUNTER: &str = r"
import { morphState } from 'morph'

export default function Counter(props: { label: string, step?: number, onStep: (v: number) => void }) {
  const [count, setCount] = morphState(0)
  function bump() { setCount(count + props.step) }
  return (
    <div>
      <span>{props.label}: {count}</span>
      <button onClick={() => bump()}>+</button>
      <button onClick={() => props.onStep(count)}>send</button>
    </div>
  )
}
";

    const BADGE: &str = r"
export function Badge(props: { text: number }) {
  return <span>total={props.text}</span>
}
";

    fn fixture_root(name: &str) -> PathBuf {
        let root = scratch(name);
        write_file(&root, "App.mx", APP);
        write_file(&root, "Counter.mx", COUNTER);
        write_file(&root, "Badge.mx", BADGE);
        root
    }

    fn build_root(root: &Path) -> Vec<IRWindow> {
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), root).unwrap();
        IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).expect("build_with_graph")
    }

    fn route_entry(id: &str) -> morph_parser::routes::RouteEntry {
        morph_parser::routes::RouteEntry {
            id: id.to_string(),
            rid: 0,
            file: PathBuf::from(format!("/src{id}/route.mx")),
            const_name: "kTest".to_string(),
            title: None,
            width: None,
            height: None,
            parent: String::new(),
            modal: false,
            role: String::new(),
            has_default_export: true,
        }
    }

    const ROUTE: &str = r#"
export default function SettingsPage(props: { userId: number }) {
  const [tab, setTab] = morphState("general")
  return (
    <body>
      <div>{props.userId}</div>
      <div>{tab}</div>
      <button onClick={() => setTab("advanced")}>adv</button>
    </body>
  )
}
"#;

    #[test]
    fn route_build_allows_props_and_seeds_prop_locals() {
        let root = scratch("route_props");
        write_file(&root, "settings/route.mx", ROUTE);
        let graph = morph_parser::resolve_graph(&root.join("settings/route.mx"), &root).unwrap();
        let win = IRBuilder::new()
            .build_route(&graph, &route_entry("/settings"), &[], &HashMap::new())
            .expect("build_route");
        assert_eq!(win.window_id, "route:/settings");
        assert_eq!(win.route_props.len(), 1);
        assert_eq!(win.route_props[0]["name"], "userId");
        assert_eq!(win.route_props[0]["class"], "double");
        // State records identically (codegen interprets as context members).
        assert_eq!(getters(&win), vec!["tab"]);
        // Prop reads lower to the mount-prologue local, not a global.
        fn texts(node: &IRNode, out: &mut Vec<String>) {
            if !node.reactive_text.is_empty() {
                out.push(node.reactive_text.clone());
            }
            for ev in &node.events {
                out.push(ev.action.clone());
                out.push(ev.target.clone());
            }
            for child in node.children.iter().chain(node.then_nodes.iter()) {
                texts(child, out);
            }
        }
        let mut found = Vec::new();
        for node in &win.nodes {
            texts(node, &mut found);
        }
        for ed in &win.effect_decls {
            if let Some(lambda) = ed.get("lambda") {
                found.push(lambda.clone());
            }
        }
        for sub in &win.channel_subs {
            if let Some(body) = sub.get("body") {
                found.push(body.clone());
            }
        }
        found.extend(win.premain_functions.iter().cloned());
        assert!(found.iter().any(|t| t.contains("ctx->userId")), "{found:?}");
        // Root state reads stay bare in node IR (like entry IR keeps
        // `total`); the mount emission's state_map rewrites them to
        // `ctx->` — only build-time translations (premain, effects)
        // carry baked signal expressions.
        assert!(found.iter().any(|t| t == "tab"), "{found:?}");
        assert!(
            !found.iter().any(|t| t.contains("__st_")),
            "no entry globals in route IR: {found:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn route_build_rejects_missing_default_export() {
        let root = scratch("route_noexport");
        write_file(&root, "settings/route.mx", "export const x = 1;\n");
        let graph = morph_parser::resolve_graph(&root.join("settings/route.mx"), &root).unwrap();
        let err = IRBuilder::new()
            .build_route(&graph, &route_entry("/settings"), &[], &HashMap::new())
            .unwrap_err();
        assert!(err.to_string().contains("mx-route-no-export"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    const LINK_APP: &str = r#"
export default function App() {
  return (
    <body>
      <a href="/settings">plain</a>
      <a href="/settings" target="_blank" width={500}>popup</a>
      <a href="/settings" target="_blank" data={{ theme: "dark" }}>with data</a>
      <a href="https://example.com/help">help</a>
      <a>no href</a>
    </body>
  )
}
"#;

    #[test]
    fn link_href_desugars_to_placeholders() {
        let root = scratch("link_desugar");
        write_file(&root, "App.mx", LINK_APP);
        let win = &build_root(&root)[0];
        let mut targets = Vec::new();
        for node in &win.nodes {
            event_targets(node, &mut targets);
        }
        assert_eq!(targets.len(), 4, "{targets:?}");
        assert!(targets[0].contains("__w.navigate(\"/settings\")"), "{targets:?}");
        assert!(targets[1].contains("new Window(\"/settings\", {width: 500})"), "{targets:?}");
        assert!(targets[2].contains("data: { theme: \"dark\" }"), "{targets:?}");
        assert!(
            targets[3].contains("__morph_open_browser(\"https://example.com/help\")"),
            "{targets:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn link_dynamic_href_is_hard_error() {
        let root = scratch("link_dyn");
        write_file(&root, "App.mx", "export default function App() { const u = \"/x\"; return (<body><a href={u}>x</a></body>) }\n");
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("must be a string literal"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    fn getters(win: &IRWindow) -> Vec<String> {
        win.state_vars.iter().map(|m| m.get("getter").cloned().unwrap_or_default()).collect()
    }

    fn reactive_texts(node: &IRNode, out: &mut Vec<String>) {
        if !node.reactive_text.is_empty() {
            out.push(node.reactive_text.clone());
        }
        for child in
            node.children.iter().chain(node.then_nodes.iter()).chain(node.else_nodes.iter())
        {
            reactive_texts(child, out);
        }
        if let Some(tmpl) = node.item_template.as_deref() {
            reactive_texts(tmpl, out);
        }
    }

    fn event_targets(node: &IRNode, out: &mut Vec<String>) {
        for ev in &node.events {
            out.push(ev.target.clone());
        }
        for child in
            node.children.iter().chain(node.then_nodes.iter()).chain(node.else_nodes.iter())
        {
            event_targets(child, out);
        }
        if let Some(tmpl) = node.item_template.as_deref() {
            event_targets(tmpl, out);
        }
    }

    #[test]
    fn instances_get_independent_state() {
        let root = fixture_root("basic_state");
        let windows = build_root(&root);
        assert_eq!(windows.len(), 1);
        let win = &windows[0];
        assert_eq!(win.title, "Comp");
        // Root state keeps its legacy name; each stateful instance is mangled.
        // (Badge is stateless — props don't create signals.)
        let mut g = getters(win);
        g.sort();
        assert_eq!(g, vec!["inst0_count", "inst1_count", "total"]);
        // Same init everywhere, distinct signals.
        for name in ["inst0_count", "inst1_count"] {
            let sv = win.state_vars.iter().find(|m| m["getter"] == name).unwrap();
            assert_eq!(sv["init"], "0");
        }
        // Per-instance helpers are mangled in premain; root helper is flat.
        let premain = win.premain_functions.join("\n");
        assert!(premain.contains("inst0_bump"), "{premain}");
        assert!(premain.contains("inst1_bump"), "{premain}");
        assert!(premain.contains("void handleReset()"), "{premain}");
        assert!(!premain.contains("void bump()"), "no unmangled bump: {premain}");
        // Instance bodies read their own (renamed) signals, not each other's
        // and never the bare name (the emitter maps `instN_count` to the
        // `__st_instN_count` signal from `window.state_vars`).
        let mut texts = Vec::new();
        for n in &win.nodes {
            reactive_texts(n, &mut texts);
        }
        assert!(texts.contains(&"inst0_count".to_string()), "{texts:?}");
        assert!(texts.contains(&"inst1_count".to_string()), "{texts:?}");
        assert!(!texts.contains(&"count".to_string()), "no bare count: {texts:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn state_and_fn_props_bind_reactively() {
        let root = fixture_root("basic_props");
        let win = &build_root(&root)[0];
        let mut texts = Vec::new();
        let mut targets = Vec::new();
        for n in &win.nodes {
            reactive_texts(n, &mut texts);
            event_targets(n, &mut targets);
        }
        // `text={total}` inlines the parent signal read into the child.
        assert!(
            texts.iter().any(|t| t.contains("__st_total.get()")),
            "parent signal read inlined: {texts:?}"
        );
        // `onStep={setTotal}` inlines the parent setter into the child handler.
        assert!(
            targets.iter().any(|t| t.contains("__st_total.set")),
            "parent setter inlined: {targets:?}"
        );
        // String literal props inline quoted.
        assert!(texts.iter().any(|t| t.contains("\"A\"") || t.contains("\"B\"")), "{texts:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_component_expands_without_import() {
        let root = scratch("local");
        write_file(
            &root,
            "App.mx",
            r#"
export default function App() {
  return (
    <body>
      <Helper msg="hi" />
    </body>
  )
}

function Helper(props: { msg: string }) {
  return <span>{props.msg}</span>
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let win = &IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap()[0];
        // One window, one root; Helper is not a second root.
        assert_eq!(win.nodes.len(), 1);
        let mut texts = Vec::new();
        reactive_texts(&win.nodes[0], &mut texts);
        assert!(texts.iter().any(|t| t.contains("\"hi\"")), "{texts:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn optional_prop_omitted_gets_zero_default() {
        let root = scratch("optional");
        write_file(
            &root,
            "App.mx",
            r#"
import Counter from './Counter.mx'
export default function App() {
  const [total, setTotal] = morphState(0)
  return (
    <body>
      <Counter label="C" onStep={setTotal} />
    </body>
  )
}
"#,
        );
        write_file(&root, "Counter.mx", COUNTER);
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let win = &IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap()[0];
        // bump() body references the defaulted step.
        let premain = win.premain_functions.join("\n");
        assert!(premain.contains("0.0"), "defaulted step inlined: {premain}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_required_prop_is_an_error() {
        let root = scratch("missingreq");
        write_file(
            &root,
            "App.mx",
            "import Counter from './Counter.mx'\nexport default function App() { return (<body><Counter label=\"A\" /></body>) }",
        );
        write_file(&root, "Counter.mx", COUNTER);
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("onStep"), "{err}");
        assert!(err.to_string().contains("required"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn inner_function_colliding_with_prop_is_an_error() {
        let root = scratch("membercollision");
        write_file(
            &root,
            "App.mx",
            "import Counter from './Counter.mx'\nexport default function App() { return (<body><Counter label=\"A\" step={1} onStep={() => 1} /></body>) }",
        );
        write_file(
            &root,
            "Counter.mx",
            "export default function Counter(props: { label: string, step: number, onStep: () => void }) { function step() { return 1 } return (<span>{props.label}</span>) }",
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("collides"), "{err}");
        assert!(err.to_string().contains("step"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_prop_is_an_error() {
        let root = scratch("unknownprop");
        write_file(
            &root,
            "App.mx",
            "import { Badge } from './Badge.mx'\nexport default function App() { return (<body><Badge text={1} bogus={2} /></body>) }",
        );
        write_file(&root, "Badge.mx", BADGE);
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("bogus"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_component_is_an_error() {
        let root = scratch("unknowncomp");
        write_file(
            &root,
            "App.mx",
            "export default function App() { return (<body><Nope /></body>) }",
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("Nope"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn component_cycles_are_rejected() {
        let root = scratch("cycle");
        write_file(
            &root,
            "A.mx",
            "import { B } from './B.mx'\nexport default function A() { return (<div><B /></div>) }",
        );
        write_file(
            &root,
            "B.mx",
            "import A from './A.mx'\nexport function B() { return (<span><A /></span>) }",
        );
        let graph = morph_parser::resolve_graph(&root.join("A.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("cyclic"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mid_tags_collect_assignments_with_constants() {
        let root = scratch("midtags");
        write_file(&root, "Counter.mx", COUNTER);
        write_file(
            &root,
            "App.mx",
            r#"
import Counter from './Counter.mx'
export default function App() {
  return (
    <body>
      <Counter label="A" step={1} onStep={(v: number) => {}} mid="hero" />
      <Counter label="B" step={2} onStep={(v: number) => {}} />
      <Counter label="C" step={3} onStep={(v: number) => {}} mid="Fives" />
    </body>
  )
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let wins =
            IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).expect("mid build");
        assert_eq!(wins.len(), 1);
        let assigns = &wins[0].mid_assignments;
        // One entry per (tagged instance × state slot); Counter has 1 state.
        assert_eq!(assigns.len(), 2, "{assigns:?}");
        assert_eq!(assigns[0].get("mid").map(String::as_str), Some("hero"));
        assert_eq!(assigns[0].get("const").map(String::as_str), Some("MID_HERO"));
        assert_eq!(assigns[0].get("index").map(String::as_str), Some("0"));
        assert_eq!(assigns[1].get("mid").map(String::as_str), Some("fives"));
        assert_eq!(assigns[1].get("const").map(String::as_str), Some("MID_FIVES"));
        assert_eq!(assigns[1].get("index").map(String::as_str), Some("1"));
        // Untagged middle instance never shifts ordinals.
        assert!(assigns[0].get("signal").unwrap().ends_with("_count"), "{assigns:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mid_use_site_rule() {
        // `mid` inside a component definition (declared prop) is rejected
        // even when the use-site is valid: identity lives only at reuse.
        let root = scratch("middef");
        write_file(
            &root,
            "Counter.mx",
            r"
import { morphState } from 'morph'
export default function Counter(props: { label: string, mid: string }) {
  const [count, setCount] = morphState(0)
  return (<div><span>{props.label}: {count}</span></div>)
}
",
        );
        write_file(
            &root,
            "App.mx",
            r#"
import Counter from './Counter.mx'
export default function App() {
  return (<body><Counter label="A" mid="hero" /></body>)
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("reserved"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mid_rejects_duplicates_digits_dynamic_lists_and_native_tags() {
        // Duplicate mid on the same component type.
        let root = scratch("middup");
        write_file(&root, "Counter.mx", COUNTER);
        write_file(
            &root,
            "App.mx",
            r#"
import Counter from './Counter.mx'
export default function App() {
  return (
    <body>
      <Counter label="A" step={1} onStep={(v: number) => {}} mid="hero" />
      <Counter label="B" step={2} onStep={(v: number) => {}} mid="Hero" />
    </body>
  )
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("duplicate `mid=\"hero\"`"), "{err}");

        // Digits and symbols are banned (letters only).
        write_file(
            &root,
            "App2.mx",
            r#"
import Counter from './Counter.mx'
export default function App() {
  return (<body><Counter label="A" step={1} onStep={(v: number) => {}} mid="item1" /></body>)
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App2.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("letters only"), "{err}");

        // Dynamic values rejected.
        write_file(
            &root,
            "App3.mx",
            r#"
import Counter from './Counter.mx'
const tag = "hero"
export default function App() {
  return (<body><Counter label="A" step={1} onStep={(v: number) => {}} mid={tag} /></body>)
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App3.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("must be a string literal"), "{err}");

        // mid inside .map() item templates.
        write_file(
            &root,
            "App4.mx",
            r#"
import Counter from './Counter.mx'
export default function App() {
  const items = [1, 2, 3]
  return (<body>{items.map((it) => <Counter label="A" step={1} onStep={(v: number) => {}} mid="hero" />)}</body>)
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App4.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("share one state slot"), "{err}");

        // mid on native elements is rejected: frontend identity is `id`,
        // `mid` is C++-side component-instance identity only.
        write_file(
            &root,
            "App5.mx",
            r#"export default function App() { return (<body><div mid="hero">x</div></body>) }"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App5.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("only for component use-sites"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cross_file_functions_resolve_to_defining_namespace() {
        let root = scratch("xfunc");
        write_file(&root, "network.ts", "export function fetchUserData(): int { return 2 }\n");
        write_file(
            &root,
            "utility.ts",
            "import { fetchUserData } from './network.ts'\nexport function loadData(): int { return fetchUserData() }\n",
        );
        write_file(
            &root,
            "Navbar.mx",
            r#"
import { loadData } from './utility.ts'
export function fetchUserData(): int { return 3 }
export function Navbar() {
  function refresh() { loadData() }
  function sync() { fetchUserData() }
  return (<div><button onClick={() => refresh()}>go</button><button onClick={() => sync()}>sync</button></div>)
}
"#,
        );
        write_file(
            &root,
            "App.mx",
            "import { Navbar } from './Navbar.mx'\nexport default function App() { return (<body><Navbar /></body>) }",
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let wins =
            IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).expect("xfunc build");
        assert_eq!(wins.len(), 1);
        let win = &wins[0];
        // Same-name fetchUserData in network.ts and Navbar.mx coexist, and
        // utility.ts re-uses network's through its import.
        let keys: Vec<&str> = win
            .module_bindings
            .iter()
            .map(|m| m.get("key").map(String::as_str).unwrap_or(""))
            .collect();
        assert!(keys.iter().any(|k| k.ends_with("network.ts::fetchUserData")), "{keys:?}");
        assert!(keys.iter().any(|k| k.ends_with("Navbar.mx::fetchUserData")), "{keys:?}");
        assert!(keys.iter().any(|k| k.ends_with("utility.ts::loadData")), "{keys:?}");
        // Calls rewrite to the defining namespace, never the importer's.
        let premain = win.premain_functions.join("\n");
        assert!(premain.contains("::app::utility::loadData()"), "{premain}");
        assert!(!premain.contains("::app::navbar::loadData"), "{premain}");
        assert!(premain.contains("::app::network::fetchUserData()"), "{premain}");
        assert!(premain.contains("::app::navbar::fetchUserData()"), "{premain}");
        // Definitions live namespaced in premain.
        assert!(premain.contains("namespace utility"), "{premain}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_and_ambiguous_imports_are_hard_errors() {
        let root = scratch("ximporterr");
        write_file(&root, "utility.ts", "export function loadData(): int { return 1 }\n");
        write_file(&root, "other.ts", "export function loadData(): int { return 2 }\n");
        write_file(
            &root,
            "App.mx",
            r#"
import { loadData } from './utility.ts'
import { missing } from './utility.ts'
export default function App() {
  function refresh() { loadData() }
  return (<body><button onClick={() => refresh()}>go</button></body>)
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("unknown import `missing`"), "{err}");

        write_file(
            &root,
            "App2.mx",
            r#"
import { loadData } from './utility.ts'
import { loadData as loadData2 } from './other.ts'
export default function App() {
  function refresh() { loadData() }
  function refresh2() { loadData2() }
  return (<body><button onClick={() => refresh()}>go</button><button onClick={() => refresh2()}>go2</button></body>)
}
"#,
        );
        // Distinct locals: both resolve to their defining namespaces.
        let graph = morph_parser::resolve_graph(&root.join("App2.mx"), &root).unwrap();
        let wins =
            IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).expect("aliased");
        let premain = wins[0].premain_functions.join("\n");
        assert!(premain.contains("::app::utility::loadData()"), "{premain}");
        assert!(premain.contains("::app::other::loadData()"), "{premain}");

        write_file(
            &root,
            "App3.mx",
            r#"
import { loadData } from './utility.ts'
import { loadData } from './other.ts'
export default function App() {
  return (<body><div>hi</div></body>)
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App3.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("ambiguous import of `loadData`"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reexports_resolve_to_ultimate_and_record_aliases() {
        let root = scratch("reexport");
        write_file(&root, "utility.ts", "export function loadData(): int { return 1 }\n");
        write_file(&root, "mid.ts", "export { loadData } from './utility.ts'\n");
        write_file(
            &root,
            "App.mx",
            r#"
import { loadData } from './mid.ts'
export default function App() {
  function refresh() { loadData() }
  return (<body><button onClick={() => refresh()}>go</button></body>)
}
"#,
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let wins =
            IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).expect("reexport");
        let win = &wins[0];
        // Import through the re-exporter lands on the defining namespace.
        let premain = win.premain_functions.join("\n");
        assert!(premain.contains("::app::utility::loadData()"), "{premain}");
        assert!(!premain.contains("::app::mid::loadData"), "{premain}");
        // The re-export itself is recorded as an alias entry for C++.
        let aliases: Vec<&HashMap<String, String>> = win
            .module_bindings
            .iter()
            .filter(|m| m.get("kind").map(String::as_str) == Some("alias"))
            .collect();
        assert_eq!(aliases.len(), 1, "{:?}", win.module_bindings);
        assert_eq!(aliases[0].get("ns").map(String::as_str), Some("mid"));
        assert_eq!(aliases[0].get("target_ns").map(String::as_str), Some("utility"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mx_naming_collision_is_a_hard_error() {
        let root = scratch("namingcollide");
        write_file(
            &root,
            "App.mx",
            r#"
import { Store } from './sub/Store.mx'
import { Store2 } from './Sub/Store.mx'
export default function App() { return (<body><Store /><Store2 /></body>) }
"#,
        );
        write_file(&root, "sub/Store.mx", "export function Store() { return (<div/>) }");
        // Same normalized namespace `sub::store` with different casing.
        write_file(&root, "Sub/Store.mx", "export function Store2() { return (<div/>) }");
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let lints = morph_parser::linter::lint_graph(&graph);
        assert!(
            lints.iter().any(|l| l.code == "mx-naming" && l.message.contains("already claimed")),
            "{lints:?}"
        );
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("already claimed"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn entry_props_and_children_are_rejected() {
        let root = scratch("entryprops");
        write_file(
            &root,
            "App.mx",
            "export default function App(props: { x: string }) { return (<div/>) }",
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("must not declare props"), "{err}");

        write_file(
            &root,
            "App2.mx",
            "import { Badge } from './Badge.mx'\nexport default function App() { return (<body><Badge text={1}>hi</Badge></body>) }",
        );
        write_file(&root, "Badge.mx", BADGE);
        let graph = morph_parser::resolve_graph(&root.join("App2.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("children"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn inline_arrow_props_become_typed_adapters() {
        let root = scratch("adapter");
        write_file(
            &root,
            "App.mx",
            r#"
import Counter from './Counter.mx'
export default function App() {
  const [total, setTotal] = morphState(0)
  return (
    <body>
      <Counter label="D" step={1} onStep={(v) => setTotal(v + 1)} />
    </body>
  )
}
"#,
        );
        write_file(&root, "Counter.mx", COUNTER);
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let win = &IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap()[0];
        let premain = win.premain_functions.join("\n");
        // Typed adapter in premain, parent state correctly referenced.
        assert!(premain.contains("inst0_prop_onStep"), "{premain}");
        assert!(premain.contains("__st_total.set"), "{premain}");
        assert!(!premain.contains("=>"), "no raw JS arrows: {premain}");
        // Child calls the adapter with its own (renamed) signal — the
        // emitter maps `inst0_count` to `__st_inst0_count` from state_vars.
        let mut targets = Vec::new();
        for n in &win.nodes {
            event_targets(n, &mut targets);
        }
        assert!(
            targets.iter().any(|t| t.contains("inst0_prop_onStep") && t.contains("inst0_count")),
            "{targets:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn block_body_arrow_adapter() {
        let root = scratch("adapterblock");
        write_file(
            &root,
            "App.mx",
            r"
import { Badge } from './Badge.mx'
export default function App() {
  const [total, setTotal] = morphState(0)
  return (
    <body>
      <Badge text={total} />
    </body>
  )
}
",
        );
        write_file(&root, "Badge.mx", BADGE);
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        // Badge takes no function props — sanity: no adapters, still builds.
        let win = &IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap()[0];
        assert!(win.premain_functions.iter().all(|p| !p.contains("_prop_")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bad_function_props_are_clear_errors() {
        // Arity mismatch: callback declares 2 params, type takes 1.
        let root = scratch("adapterarity");
        write_file(
            &root,
            "App.mx",
            "import Counter from './Counter.mx'\nexport default function App() { return (<body><Counter label=\"A\" step={1} onStep={(a, b) => a} /></body>) }",
        );
        write_file(&root, "Counter.mx", COUNTER);
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("parameter"), "{err}");
        // Arrow on a non-function prop.
        write_file(
            &root,
            "App2.mx",
            "import { Badge } from './Badge.mx'\nexport default function App() { return (<body><Badge text={() => 1} /></body>) }",
        );
        write_file(&root, "Badge.mx", BADGE);
        let graph = morph_parser::resolve_graph(&root.join("App2.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("non-function prop"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fn_type_and_callable_parsers() {
        let (params, ret) =
            parse_fn_type("(v: number, cb: (x: string) => void) => boolean").unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], ("v".to_string(), "number".to_string(), false));
        assert_eq!(params[1].0, "cb");
        assert!(params[1].1.contains("=>"));
        assert_eq!(ret, "boolean");
        assert!(parse_fn_type("<T>(x: T) => T").is_err());
        assert!(parse_fn_type("(a?: number) => void").is_err());
        assert!(parse_fn_type("string").is_err());
        let (names, body, block) = parse_callable_source("(a, b) => a + b").unwrap();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(body, "a + b");
        assert!(!block);
        let (names, body, block) = parse_callable_source("x => { foo(x); }").unwrap();
        assert_eq!(names, vec!["x"]);
        assert!(block);
        assert!(body.contains("foo(x)"));
        let (names, _, _) = parse_callable_source("() => 1").unwrap();
        assert!(names.is_empty());
        assert!(parse_callable_source("async (x) => x").is_err());
        assert!(parse_callable_source("(a, {b}) => a").is_err());
    }

    #[test]
    fn shared_store_is_namespaced_per_file() {
        let root = scratch("shared");
        write_file(
            &root,
            "store.mx",
            r"
export const [count, setCount] = morphShared<number>(0)
",
        );
        write_file(
            &root,
            "Panel.mx",
            r"
import { count, setCount } from './store.mx'
import { morphState } from 'morph'
export default function Panel() {
  const [open, setOpen] = morphState(true)
  return (
    <div>
      <span>{count}</span>
      {open && <button onClick={() => setCount(0)}>clear</button>}
    </div>
  )
}
",
        );
        write_file(
            &root,
            "App.mx",
            r"
import Panel from './Panel.mx'
import { count, setCount } from './store.mx'
export default function App() {
  return (
    <body>
      <div>{count}</div>
      <button onClick={() => setCount(count + 1)}>+</button>
      <Panel />
    </body>
  )
}
",
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let win = &IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap()[0];
        // Both files import the same store binding; they dedupe to one entry
        // keyed by the canonical store module path + getter name.
        assert_eq!(win.shared_vars.len(), 1);
        let e = &win.shared_vars[0];
        assert_eq!(e["accessor"], "shared_count");
        assert_eq!(e["type"], "int");
        assert_eq!(e["init"], "0");
        assert_eq!(e["getter"], "count");
        // The fully qualified accessor uses a human-computable namespace
        // derived from the store module path (entry-relative, lowercased),
        // not a global name: store.mx next to App.mx → `store`.
        assert_eq!(e["ns"], "store");
        // Panel's local state is mangled alongside.
        assert!(getters(win).contains(&"inst0_open".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn shared_import_ambiguous_is_error() {
        let root = scratch("ambig");
        write_file(
            &root,
            "a.mx",
            "export const [count, setCount] = morphShared<number>(1)\nexport default function A() { return (<div>{count}</div>) }\n",
        );
        write_file(
            &root,
            "b.mx",
            "export const [count, setCount] = morphShared<number>(2)\nexport default function B() { return (<div>{count}</div>) }\n",
        );
        // App imports `count` from both a and b — ambiguous.
        write_file(
            &root,
            "App.mx",
            "import { count } from './a.mx'\nimport { count } from './b.mx'\nexport default function App() { return (<div>{count}</div>) }\n",
        );
        let graph = morph_parser::resolve_graph(&root.join("App.mx"), &root).unwrap();
        let err = IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).unwrap_err();
        assert!(err.to_string().contains("ambiguous import"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn helpers_rename_is_member_and_string_safe() {
        // `item.count` (member) and "count" (string) must survive; bare
        // `count` renames.
        let mut renames = HashMap::new();
        renames.insert("count".to_string(), "inst0_count".to_string());
        let frame = InstanceFrame {
            module: PathBuf::new(),
            vars: HashMap::new(),
            types: HashMap::new(),
            events: HashMap::new(),
            props_param: String::new(),
            renames,
            prop_binds: HashMap::new(),
        };
        let out = capture_raw(r#"item.count + count + "count" + f(count)"#, &frame);
        assert_eq!(out, r#"item.count + inst0_count + "count" + f(inst0_count)"#);
        // props.x rewriting.
        let frame2 = InstanceFrame {
            module: PathBuf::new(),
            vars: HashMap::new(),
            types: HashMap::new(),
            events: HashMap::new(),
            props_param: "props".to_string(),
            renames: HashMap::new(),
            prop_binds: HashMap::new(),
        };
        assert_eq!(subst_props_refs("props.title + y.props.z", &frame2), "title + y.props.z");
    }
}
