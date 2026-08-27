use anyhow::Result;
use colored::Colorize;

pub fn run(
    entry: Option<String>,
    output: Option<String>,
    static_: bool,
    upx: Option<bool>,
    no_upx: bool,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry = entry.unwrap_or(config.entry.clone());
    let output_raw = output.unwrap_or(config.output.clone());
    // Clean app name for binary (spaces/special → _)
    let clean_name = morph_config::clean_app_name(&config.name);

    crate::logger::log_banner(&format!("Morph Build — {}", config.name));

    crate::logger::log_step("Verifying runtime");
    crate::commands::install::ensure_runtime(&cwd)?;
    crate::logger::log_success(&format!(
        "Runtime {} v{}",
        config.runtime.runtime_type.cyan(),
        config.runtime.version.dimmed()
    ));

    crate::logger::log_step("Configuration");
    crate::logger::log_key("Entry", &entry);
    crate::logger::log_key("Output", &output_raw);
    crate::logger::log_key("Binary", &clean_name);
    crate::logger::log_key(
        "Runtime",
        &format!("{} v{}", config.runtime.runtime_type, config.runtime.version),
    );
    if static_ {
        crate::logger::log_key("Static", "enabled");
    }
    if let Some(u) = upx {
        crate::logger::log_key("UPX", &u.to_string());
    }
    if no_upx {
        crate::logger::log_key("UPX", "disabled");
    }

    // ── Parse .mx files ──
    crate::logger::log_step("Parsing");

    let pb = crate::logger::spinner("Parsing .mx files...");
    let entry_path = cwd.join(&entry);
    if !entry_path.exists() {
        pb.finish_and_clear();
        anyhow::bail!("Entry file not found: {}", entry_path.display());
    }

    let source = std::fs::read_to_string(&entry_path)?;
    let parsed = morph_parser::parse_mx_str(&source, &entry)?;
    pb.finish_and_clear();
    crate::logger::log_success(&format!("Parsed {}", entry));

    // Report what we found
    if let Some(ref wc) = parsed.window_config {
        crate::logger::log_key("Window", &format!("\"{}\" {}x{}", wc.title, wc.width, wc.height));
    }
    let total_state: usize = parsed.components.iter().map(|c| c.state_vars.len()).sum();
    crate::logger::log_key("Components", &parsed.components.len().to_string());
    crate::logger::log_key("State vars", &total_state.to_string());
    crate::logger::log_key("Imports", &parsed.imports.len().to_string());

    // ── Build IR (Phase 3: morph-ir builder) ──
    let pb = crate::logger::spinner("Building IR...");
    // Collect CSS rules from imports
    let mut css_rules: std::collections::HashMap<String, morph_parser::CssRule> = std::collections::HashMap::new();
    for imp in &parsed.imports {
        if let morph_parser::MxImportKind::CssLocal { path } = &imp.kind {
            let candidates = [
                entry_path.parent().map(|p| p.join(path)).unwrap_or_else(|| cwd.join(path)),
                cwd.join(path),
            ];
            for cand in &candidates {
                if cand.exists() {
                    if let Ok(text) = std::fs::read_to_string(cand) {
                        if let Ok(rules) = morph_parser::parse_css(&text) {
                            css_rules.extend(rules);
                        }
                    }
                    break;
                }
            }
        }
    }
    let builder = morph_ir::IRBuilder::new();
    let windows = builder.build(&parsed, &css_rules);
    pb.finish_and_clear();
    crate::logger::log_success(&format!("IR built — {} window(s)", windows.len()));

    // ── Generate C++ ──
    let output_dir = cwd.join(&output_raw);
    // Ensure output is treated as directory (clean name handles file case)
    let output_dir = if output_raw.ends_with('/') || std::path::Path::new(&output_raw).extension().is_none() {
        output_dir
    } else {
        output_dir.parent().map(|p| p.to_path_buf()).unwrap_or(output_dir)
    };
    let pb = crate::logger::spinner("Generating C++...");
    let emitter = morph_codegen::CppEmitter::new(&windows);
    emitter.emit(&output_dir)?;
    pb.finish_and_clear();
    crate::logger::log_success(&format!("C++ generated → {}", output_dir.display()));

    // ── Compile ──
    let compiler_name = morph_build::detect_compiler();
    let pb = crate::logger::spinner(&format!("Compiling with {}...", compiler_name));
    // Find runtime dir (for headers)
    let runtime_dir = {
        let mut candidates = vec![
            cwd.join("runtime").join("cpp"),
            cwd.join("../runtime").join("cpp"),
            cwd.join("../../runtime").join("cpp"),
            std::path::PathBuf::from("runtime/cpp"),
        ];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("../runtime/cpp"));
            }
        }
        candidates.into_iter().find(|p| p.join("core/window.h").exists() || p.join("include").exists() || p.exists()).unwrap_or_else(|| cwd.join("runtime/cpp"))
    };
    let compiler = morph_build::Compiler::new(None);
    let binary_path = output_dir.join(format!("{}{}", clean_name, morph_build::exe_suffix()));
    // Feature defines for flex, etc.
    let mut fs = morph_codegen::feature_set::FeatureSet::new();
    fs.scan(&windows);
    let defines = fs.required_defines();
    // Ensure output dir exists (already created by emitter)
    if let Err(e) = compiler.compile(&output_dir.join("app.cpp"), &binary_path, &runtime_dir, &defines) {
        pb.finish_and_clear();
        crate::logger::log_error(&format!("Compile failed: {}", e));
        anyhow::bail!("build failed");
    }
    pb.finish_and_clear();
    crate::logger::log_success(&format!("Compiled → {}", binary_path.display()));
    // UPX compression if requested (stub)
    if upx.unwrap_or(false) && !no_upx {
        crate::logger::log_dim("UPX compression requested (not yet implemented in Rust, skipping)");
    }
    println!();

    Ok(())
}

fn build_ir(parsed: &morph_parser::MxSource) -> Result<Vec<morph_ir::IRWindow>> {
    use morph_ir::{IRWindow, IRNode, IRStyle};
    use std::collections::HashMap;

    // Aggregate state/effects from all components
    let all_state: Vec<HashMap<String, String>> = parsed.components.iter().flat_map(|c| {
        c.state_vars.iter().map(|sv| {
            let mut m = HashMap::new();
            m.insert("getter".to_string(), sv.getter.clone());
            m.insert("setter".to_string(), sv.setter.clone());
            m.insert("init".to_string(), sv.init.clone());
            m
        })
    }).collect();
    let all_effects: Vec<HashMap<String, String>> = parsed.components.iter().flat_map(|c| {
        c.effects.iter().map(|e| {
            let mut m = HashMap::new();
            m.insert("callback".to_string(), e.callback.clone());
            m.insert("deps".to_string(), e.deps.clone());
            m
        })
    }).collect();

    let wc = parsed.window_config.as_ref().map(|wc| {
        morph_ir::IRWindow {
            window_id: "w0".to_string(),
            title: wc.title.clone(),
            width: wc.width,
            height: wc.height,
            visible: true,
            min_width: wc.min_width,
            max_width: wc.max_width,
            min_height: wc.min_height,
            max_height: wc.max_height,
            modal: wc.modal,
            renderer: "flash".into(),
            nodes: vec![],
            startup_logs: parsed.console_logs.clone(),
            premain_functions: vec![],
            extra_headers: vec![],
            state_vars: all_state.clone(),
            effect_decls: all_effects.clone(),
            cpp_imports: parsed.cpp_imports.iter().map(|ci| {
                let mut m = HashMap::new();
                m.insert("path".to_string(), ci.path.clone());
                m.insert("specifiers".to_string(), ci.specifiers.join(", "));
                m
            }).collect(),
            keyframes: HashMap::new(),
        }
    }).unwrap_or_else(|| {
        IRWindow {
            window_id: "w0".to_string(),
            title: "Morph App".to_string(),
            width: 800,
            height: 600,
            visible: true,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            modal: false,
            renderer: "flash".into(),
            nodes: vec![],
            startup_logs: parsed.console_logs.clone(),
            premain_functions: vec![],
            extra_headers: vec![],
            state_vars: all_state.clone(),
            effect_decls: all_effects.clone(),
            cpp_imports: parsed.cpp_imports.iter().map(|ci| {
                let mut m = HashMap::new();
                m.insert("path".to_string(), ci.path.clone());
                m.insert("specifiers".to_string(), ci.specifiers.join(", "));
                m
            }).collect(),
            keyframes: HashMap::new(),
        }
    });

    let mut windows = vec![wc];

    // Build IR nodes from parsed components
    for comp in &parsed.components {
        let root = jsx_to_ir_node(&comp.jsx, "root");
        windows[0].nodes.push(root);
    }

    Ok(windows)
}

fn jsx_to_ir_node(jsx: &morph_parser::JsxNode, node_id: &str) -> morph_ir::IRNode {
    use morph_ir::{IRNode, IRStyle, IREvent};
    use std::collections::HashMap;

    match jsx {
        morph_parser::JsxNode::Element { tag, props, children, self_closing: _, .. } => {
            let mut node = IRNode::default();
            node.node_id = node_id.to_string();
            node.node_type = tag.clone();

            // Extract static props
            if let Some(morph_parser::JsxPropValue::String(val)) = props.get("className") {
                node.reactive_class = val.clone();
            }
            if let Some(morph_parser::JsxPropValue::String(val)) = props.get("id") {
                node.attrs.insert("id".to_string(), val.clone());
            }
            if let Some(morph_parser::JsxPropValue::String(val)) = props.get("placeholder") {
                node.attrs.insert("placeholder".to_string(), val.clone());
            }
            if let Some(morph_parser::JsxPropValue::String(val)) = props.get("src") {
                node.attrs.insert("src".to_string(), val.clone());
            }
            if let Some(morph_parser::JsxPropValue::String(val)) = props.get("type") {
                node.attrs.insert("type".to_string(), val.clone());
            }

            // Extract text content
            let text_parts: Vec<String> = children.iter().filter_map(|c| {
                if let morph_parser::JsxNode::Text(t) = c {
                    Some(t.clone())
                } else {
                    None
                }
            }).collect();
            if !text_parts.is_empty() {
                node.text_content = text_parts.join("");
            }

            // Events
            if let Some(morph_parser::JsxPropValue::Fn(expr)) = props.get("onClick") {
                node.events.push(IREvent {
                    trigger: "click".to_string(),
                    action: "call".to_string(),
                    target: expr.clone(),
                });
            }
            if let Some(morph_parser::JsxPropValue::Fn(expr)) = props.get("onInput") {
                node.events.push(IREvent {
                    trigger: "input".to_string(),
                    action: "call".to_string(),
                    target: expr.clone(),
                });
            }

            // Children
            for (i, child) in children.iter().enumerate() {
                let child_id = format!("{node_id}_{i}");
                node.children.push(jsx_to_ir_node(child, &child_id));
            }

            node
        }
        morph_parser::JsxNode::Fragment { children, .. } => {
            let mut node = IRNode::default();
            node.node_id = node_id.to_string();
            node.node_type = "__fragment__".to_string();
            for (i, child) in children.iter().enumerate() {
                let child_id = format!("{node_id}_{i}");
                node.children.push(jsx_to_ir_node(child, &child_id));
            }
            node
        }
        morph_parser::JsxNode::Text(text) => {
            let mut node = IRNode::default();
            node.node_id = node_id.to_string();
            node.node_type = "__text__".to_string();
            node.text_content = text.clone();
            node
        }
        morph_parser::JsxNode::Expression(expr) => {
            let mut node = IRNode::default();
            node.node_id = node_id.to_string();
            node.node_type = "__expr__".to_string();
            node.reactive_text = expr.clone();
            node
        }
        morph_parser::JsxNode::Conditional { condition, then_branch, else_branch, .. } => {
            let mut node = IRNode::default();
            node.node_id = node_id.to_string();
            node.node_type = "__conditional__".to_string();
            node.condition_expr = condition.clone();
            for (i, child) in then_branch.iter().enumerate() {
                let child_id = format!("{node_id}_then_{i}");
                node.then_nodes.push(jsx_to_ir_node(child, &child_id));
            }
            for (i, child) in else_branch.iter().enumerate() {
                let child_id = format!("{node_id}_else_{i}");
                node.else_nodes.push(jsx_to_ir_node(child, &child_id));
            }
            node
        }
        morph_parser::JsxNode::List { array_expr, key_expr, item_template, .. } => {
            let mut node = IRNode::default();
            node.node_id = node_id.to_string();
            node.node_type = "__list__".to_string();
            node.list_expr = array_expr.clone();
            node.list_key_expr = key_expr.clone();
            node.item_template = Some(Box::new(jsx_to_ir_node(item_template, &format!("{node_id}_item"))));
            node
        }
    }
}
