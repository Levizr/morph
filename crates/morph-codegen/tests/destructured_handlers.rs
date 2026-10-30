//! Advanced destructuring reaches both callable lowering paths:
//! JSX event payloads and function-prop adapters. The pattern is kept as a
//! synthetic `const <pattern> = ...;` declaration, so this test verifies the
//! builder/codegen pipeline does not reject or textually corrupt it before
//! morpher gets a chance to lower defaults, rest, and computed keys.

use std::collections::HashMap;
use std::path::PathBuf;

const APP: &str = r#"
import { morphState } from 'morph'

export const windowConfig = { title: "Destr", width: 400, height: 300 }

const key = "value"

function pick({ value = 1, ...rest }: any): number {
  return value + rest.extra
}

function Child(props: { onPick: (p: any) => void }) {
  return <button onClick={() => props.onPick({ extra: 4 })}>child</button>
}

export default function App() {
  const [n, setN] = morphState(0)
  return (
    <body>
      <button onClick={({ [key]: v = 0, ...rest }: any = {}) => setN(v + rest.clientY)}>x</button>
      <text>{pick({ extra: 3 })}</text>
      <Child onPick={({ [key]: value = 1, ...rest }: any = {}) => setN(value + rest.extra)} />
      <text>{n}</text>
    </body>
  )
}
"#;

fn project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("morph_destr_handlers_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("mkdir src");
    std::fs::write(src.join("App.mx"), APP.trim_start()).expect("write App.mx");
    root
}

#[test]
fn advanced_destructuring_in_event_and_function_prop_handlers() {
    let root = project();
    let entry = root.join("src/App.mx");
    let graph = morph_parser::resolve_graph(&entry, &root).expect("resolve graph");
    let lints = morph_parser::linter::lint_graph(&graph);
    assert!(lints.is_empty(), "{lints:?}");

    let windows = morph_ir::IRBuilder::new()
        .build_with_graph(&graph, &[], &HashMap::new())
        .expect("build IR");
    assert_eq!(windows.len(), 1);

    let out_dir = root.join("out");
    morph_codegen::CppEmitter::new(&windows).emit(&out_dir).expect("emit C++");
    let app_cpp = std::fs::read_to_string(out_dir.join("app.cpp")).expect("read app.cpp");

    assert!(!app_cpp.contains("/* destructuring not supported */"), "{app_cpp}");
    assert!(app_cpp.contains("morph::js_object_rest"), "missing object rest:\n{app_cpp}");
    assert!(app_cpp.contains("morph::js_to_string"), "missing computed key:\n{app_cpp}");
    assert!(app_cpp.contains(".is_undefined()"), "missing default guard:\n{app_cpp}");
    assert!(app_cpp.contains("inst0_prop_onPick"), "missing function-prop adapter:\n{app_cpp}");

    let logic = morph_codegen::logic_emitter::emit_logic(&windows);
    assert!(!logic.source.contains("/* destructuring not supported */"), "{}", logic.source);
    assert!(logic.source.contains("morph::js_object_rest"), "missing dev-path rest");
    assert!(logic.source.contains(".is_undefined()"), "missing dev-path default");

    let _ = std::fs::remove_dir_all(&root);
}
