//! End-to-end component pipeline over `tests/runtime/component-test`:
//! resolve the transitive `.mx` graph, lint it, build IR with per-instance
//! expansion, and emit C++ — everything short of invoking the C++ compiler.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/runtime/component-test")
}

fn state_getters(windows: &[morph_ir::IRWindow]) -> Vec<String> {
    windows
        .iter()
        .flat_map(|w| w.state_vars.iter())
        .filter_map(|sv| sv.get("getter").cloned())
        .collect()
}

#[test]
fn component_project_resolves_lints_builds_and_emits() {
    let root = project_root();
    let entry = root.join("src/App.mx");
    assert!(entry.is_file(), "missing {}", entry.display());

    // Transitive graph: App + Hero + Counter + Badge + shared store.
    let graph = morph_parser::resolve_graph(&entry, &root).expect("resolve component-test graph");
    assert_eq!(graph.len(), 5);
    let names: HashSet<String> =
        graph.all_paths().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
    for expected in ["App.mx", "Hero.mx", "Counter.mx", "Badge.mx", "Store.mx"] {
        assert!(names.contains(expected), "{names:?}");
    }

    // Clean: no unknown components, prop mismatches, or shared-key issues.
    let lints = morph_parser::linter::lint_graph(&graph);
    assert!(lints.is_empty(), "{lints:?}");

    // IR: one window; the two Counter instances get independent state while
    // the shared store is identified by its canonical module path and namespaced.
    let css_rules: Vec<(String, morph_parser::CssRule)> = Vec::new();
    let css_keyframes: HashMap<String, Vec<morph_parser::CssKeyframe>> = HashMap::new();
    let windows = morph_ir::IRBuilder::new()
        .build_with_graph(&graph, &css_rules, &css_keyframes)
        .expect("build component-test IR");
    assert_eq!(windows.len(), 1);
    let getters = state_getters(&windows);
    assert!(getters.contains(&"inst0_count".to_string()), "{getters:?}");
    assert!(getters.contains(&"inst1_count".to_string()), "{getters:?}");
    assert_eq!(windows[0].shared_vars.len(), 1);
    let shared = &windows[0].shared_vars[0];
    let shared_key = shared.get("key").cloned().unwrap_or_default();
    let shared_ns = shared.get("ns").cloned().unwrap_or_default();
    let shared_accessor = shared.get("accessor").cloned().unwrap_or_default();
    assert!(shared_key.ends_with("Store.mx::total"), "{shared_key}");
    assert_eq!(shared_accessor, "shared_total");
    assert_eq!(shared_ns, "components::store");
    let mut subscriptions = Vec::new();
    for sub in &windows[0].channel_subs {
        let channel = sub.get("channel").cloned().unwrap_or_default();
        let body = sub.get("body").cloned().unwrap_or_default();
        assert!(channel.ends_with("Store.mx::resetEvent"), "{channel}");
        assert!(body.starts_with("[](const JsValue&"), "{body}");
        assert!(!body.contains("morph::channel"), "{body}");
        subscriptions.push((channel, body));
    }
    assert!(!subscriptions.is_empty());

    // C++ emit: per-instance signals, the static channel subscription,
    // and the function-prop adapter all reach the generated translation
    // unit — with zero string channel lookups in app.cpp.
    let out_dir =
        std::env::temp_dir().join(format!("morph_component_project_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    morph_codegen::CppEmitter::new(&windows).emit(&out_dir).expect("emit component-test C++");
    let app_cpp = std::fs::read_to_string(out_dir.join("app.cpp")).expect("read app.cpp");
    let api_h = std::fs::read_to_string(out_dir.join("morph_api.h")).expect("read morph_api.h");
    assert!(app_cpp.contains("__st_inst0_count"), "missing inst0 signal");
    assert!(app_cpp.contains("__st_inst1_count"), "missing inst1 signal");
    assert!(app_cpp.contains("#include \"morph_api.h\""), "missing api include");
    assert!(
        app_cpp.contains(&format!("::app::{shared_ns}::{shared_accessor}().get()")),
        "missing qualified shared read"
    );
    assert!(
        api_h.contains(&format!("namespace {shared_ns} {{")),
        "missing shared namespace in header: {api_h}"
    );
    assert!(!app_cpp.contains("morph::channel(\"evt:"), "string lookup in app.cpp");
    for (_, body) in &subscriptions {
        assert!(
            app_cpp.contains(&format!("evt_resetEvent().on({body});")),
            "missing lowered subscription"
        );
    }
    assert!(api_h.contains("inline morph::Channel& evt_resetEvent()"), "missing channel static");
    assert!(api_h.contains("inline void emit_resetEvent("), "missing emit wrapper");
    assert!(api_h.contains("inline void notify_resetEvent()"), "missing notify wrapper");
    let _ = std::fs::remove_dir_all(&out_dir);

    // Dev logic TU: the qualified shared accessor matches builder premain, and
    // the channel header is included for subscriptions.
    let logic = morph_codegen::logic_emitter::emit_logic(&windows);
    assert!(
        logic.source.contains(&format!("::app::{shared_ns}::{shared_accessor}()")),
        "missing shared accessor in dev TU"
    );
    assert!(logic.source.contains("reactivity/channel.h"), "missing channel include in dev TU");
    assert!(
        logic.source.contains("Store.mx::resetEvent"),
        "missing channel subscription in dev TU"
    );
    for (channel, body) in &subscriptions {
        assert!(
            logic.source.contains(&format!("morph::channel(\"{channel}\").on({body});")),
            "missing paired subscription in dev TU"
        );
    }
}
