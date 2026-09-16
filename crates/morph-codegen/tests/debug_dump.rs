#[test]
fn dump_total_lines() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/runtime/component-test");
    let entry = root.join("src/App.mx");
    let graph = morph_parser::resolve_graph(&entry, &root).unwrap();
    let windows = morph_ir::IRBuilder::new()
        .build_with_graph(&graph, &[], &std::collections::HashMap::new())
        .unwrap();
    // Dump IR reactive texts + event targets mentioning total.
    fn walk(n: &morph_ir::IRNode) {
        if n.reactive_text.contains("total") || n.reactive_text.contains("Total") {
            println!("REACTIVE {}: {:?}", n.node_id, n.reactive_text);
        }
        for ev in &n.events {
            if ev.target.contains("otal") {
                println!("EVENT {} {}: {:?}", n.node_id, ev.trigger, ev.target);
            }
        }
        for c in n.children.iter().chain(n.then_nodes.iter()).chain(n.else_nodes.iter()) {
            walk(c);
        }
    }
    for w in &windows {
        for n in &w.nodes {
            walk(n);
        }
    }
    let out_dir = std::env::temp_dir().join("morph_debug_dump");
    let _ = std::fs::remove_dir_all(&out_dir);
    morph_codegen::CppEmitter::new(&windows).emit(&out_dir).unwrap();
    let app = std::fs::read_to_string(out_dir.join("app.cpp")).unwrap();
    for line in app.lines().filter(|l| l.contains("total") || l.contains("Total")) {
        println!("CPP: {line}");
    }
    let _ = std::fs::remove_dir_all(&out_dir);
}
