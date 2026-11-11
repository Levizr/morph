//! Module initializer wiring: every graph module gets one guarded init,
//! dependency calls are emitted in import order, and late `var`
//! declarations land in the init prelude/body in source order.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "morph_module_init_{}_{}",
        std::process::id(),
        name.replace(['/', ' '], "_")
    ));
    let _ = std::fs::remove_dir_all(&root);
    for (rel, content) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, content).expect("write fixture");
    }
    root
}

fn build(root: &Path, entry: &str) -> morph_ir::IRWindow {
    let graph = morph_parser::resolve_graph(&root.join(entry), root).expect("resolve graph");
    let mut windows =
        morph_ir::IRBuilder::new().build_with_graph(&graph, &[], &HashMap::new()).expect("build");
    assert_eq!(windows.len(), 1);
    windows.pop().expect("window")
}

#[test]
fn modules_emit_guarded_initializers_in_dependency_order() {
    let root = project(
        "order",
        &[
            (
                "src/App.mx",
                "import { greet } from './logger.ts'\n\
                 console.log(\"entry\")\n\
                 export const windowConfig = { title: \"T\", width: 400, height: 300 }\n\
                 export default function App() { return (<body><text>{greet()}</text></body>) }\n",
            ),
            (
                "src/logger.ts",
                "import { count } from './state.ts'\n\
                 console.log(\"logger\", count())\n\
                 export function greet(): string { return \"hi\" }\n",
            ),
            (
                "src/state.ts",
                "console.log(\"state before\", n)\n\
                 var n = 1\n\
                 console.log(\"state after\", n)\n\
                 export function count(): number { return n }\n",
            ),
        ],
    );
    let win = build(&root, "src/App.mx");
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(
        win.module_init_calls,
        vec![
            "::app::app::__morph_module_init".to_string(),
            "::app::logger::__morph_module_init".to_string(),
            "::app::state::__morph_module_init".to_string(),
        ]
    );

    let premain = win.premain_functions.join("\n");
    // The entry initializer calls its direct import first; that module
    // initializes its own dependency.
    let entry_init = premain
        .split("namespace app {\nnamespace app {\nstatic bool __morph_module_init_done")
        .nth(1)
        .unwrap_or_else(|| panic!("entry init:\n{premain}"));
    assert!(entry_init.contains("::app::logger::__morph_module_init();"), "{entry_init}");

    // The logger initializer initializes the state module it imports
    // before its own body runs.
    let logger_init = premain
        .split("namespace app {\nnamespace logger {\nstatic bool __morph_module_init_done")
        .nth(1)
        .expect("logger init");
    let logger_dep = logger_init.find("::app::state::__morph_module_init();").expect("state dep");
    let logger_log = logger_init.find("\"logger\"").expect("logger log");
    assert!(logger_dep < logger_log, "{logger_init}");

    // Early read widens the hoisted binding to `JsLateValue`; the
    // declaration site assigns after the read, in source order.
    assert!(premain.contains("JsLateValue n{};"), "{premain}");
    let state_init = premain
        .split("namespace app {\nnamespace state {\nstatic bool __morph_module_init_done")
        .nth(1)
        .expect("state init");
    let before = state_init.find("\"state before\"").expect("before log");
    let assign = state_init.find("n = 1;").expect("assignment");
    let after = state_init.find("\"state after\"").expect("after log");
    assert!(before < assign && assign < after, "{state_init}");
}

#[test]
fn cycles_reference_the_guarded_initializer() {
    let root = project(
        "cycle",
        &[
            (
                "src/App.mx",
                "import { a } from './a.ts'\n\
                 export const windowConfig = { title: \"T\", width: 400, height: 300 }\n\
                 export default function App() { return (<body><text>{a()}</text></body>) }\n",
            ),
            (
                "src/a.ts",
                "import { b } from './b.ts'\n\
                 console.log(\"a\", b())\n\
                 export function a(): number { return 1 }\n",
            ),
            (
                "src/b.ts",
                "import { a } from './a.ts'\n\
                 console.log(\"b\", a())\n\
                 export function b(): number { return 2 }\n",
            ),
        ],
    );
    let win = build(&root, "src/App.mx");
    let _ = std::fs::remove_dir_all(&root);
    let premain = win.premain_functions.join("\n");
    assert!(premain.contains("::app::b::__morph_module_init();"), "{premain}");
    assert!(premain.contains("::app::a::__morph_module_init();"), "{premain}");
    assert!(
        premain.contains("if (__morph_module_init_done) return;"),
        "cycle guard missing: {premain}"
    );
}
