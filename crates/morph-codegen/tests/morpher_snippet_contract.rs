#[test]
fn probe_snippet_shapes() {
    let mut state_vars = std::collections::HashMap::new();
    state_vars.insert("acc".to_string(), "__st_acc.get()".to_string());
    state_vars.insert("setAcc".to_string(), "__st_acc.set".to_string());
    let mut state_types = std::collections::HashMap::new();
    state_types.insert("acc".to_string(), "std::string".to_string());
    let options = morpher::TranslateOptions { state_vars, state_types, ..Default::default() };
    for src in ["acc", "() => pressClear()", "() => pressDigit(7)", "cur", "opSym"] {
        let out = morpher::translate_snippet(src, "snippet.ts", options.clone());
        match out {
            Ok(o) => println!("SRC {:?}\n  includes {:?}\n  body {:?}", src, o.includes, o.body),
            Err(e) => println!("SRC {:?}\n  ERROR {:?}", src, e),
        }
    }
}
