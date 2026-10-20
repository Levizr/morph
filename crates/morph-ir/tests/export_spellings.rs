//! Export/import spelling equivalence.
//!
//! The same binding must resolve however the user spells it: inline
//! `export`, a separate `export { … }` list (aliased or not), default
//! exports by reference, function expressions instead of arrows, and
//! barrel re-exports.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

const CONFIG: &str = "export const windowConfig = { title: \"T\", width: 400, height: 300 }\n";

fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "morph_exports_{}_{}",
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

fn build(root: &Path, entry: &str) -> Result<usize, String> {
    let graph = morph_parser::resolve_graph(&root.join(entry), root).map_err(|e| e.to_string())?;
    let windows = morph_ir::IRBuilder::new()
        .build_with_graph(&graph, &[], &HashMap::new())
        .map_err(|e| e.to_string())?;
    Ok(windows.len())
}

fn assert_builds(name: &str, entry: &str, files: &[(&str, &str)]) {
    let root = project(name, files);
    let result = build(&root, entry);
    let _ = std::fs::remove_dir_all(&root);
    match result {
        Ok(n) => assert_eq!(n, 1, "{name}: expected one window"),
        Err(e) => panic!("{name}: {e}"),
    }
}

fn app(body: &str) -> String {
    format!("{CONFIG}{body}")
}

#[test]
fn component_declaration_and_export_spellings() {
    let body = "return (<body><text>hi</text></body>)";
    for (name, src) in [
        ("arrow_ref", format!("const App = () => {{ {body} }}\nexport default App")),
        ("fn_expr_ref", format!("const App = function() {{ {body} }}\nexport default App")),
        (
            "named_fn_expr_ref",
            format!("const App = function Inner() {{ {body} }}\nexport default App"),
        ),
        ("anon_fn_default", format!("export default function() {{ {body} }}")),
        ("anon_fn_expr_default", format!("export default (function() {{ {body} }})")),
        ("paren_arrow_default", format!("export default (() => {{ {body} }})")),
        ("list_default", format!("const App = () => {{ {body} }}\nexport {{ App as default }}")),
    ] {
        assert_builds(name, "src/App.mx", &[("src/App.mx", &app(&src))]);
    }
}

#[test]
fn named_export_lists_and_aliases_resolve() {
    let cases = [
        (
            "fn_alias",
            "function greet() { return \"hi\" }\nexport { greet as hello }",
            "hello",
            "{hello()}",
        ),
        (
            "arrow_alias",
            "const greet = () => \"hi\"\nexport { greet as hello }",
            "hello",
            "{hello()}",
        ),
        (
            "value_alias",
            "const token = \"abc\"\nexport { token as t }",
            "t",
            "{t}",
        ),
        (
            "shared_alias",
            "import { morphShared } from 'morph'\nconst [theme, setTheme] = morphShared(\"light\")\nexport { theme as th, setTheme as sh }",
            "th",
            "{th}",
        ),
    ];
    for (name, lib, import_name, usage) in cases {
        let entry = app(&format!(
            "import {{ {import_name} }} from './lib.ts'\nconst App = () => (<body><text>{usage}</text></body>)\nexport default App"
        ));
        assert_builds(name, "src/App.mx", &[("src/App.mx", &entry), ("src/lib.ts", lib)]);
    }
}

#[test]
fn default_helpers_by_reference_and_combined_imports() {
    let entry = app(
        "import shout, { greet, token } from './lib.ts'\nconst App = () => (<body><text>{greet()} {shout(token)}</text></body>)\nexport default App",
    );
    assert_builds(
        "default_arrow_helper",
        "src/App.mx",
        &[
            ("src/App.mx", &entry),
            (
                "src/lib.ts",
                "export function greet() { return \"hi\" }\nexport const token = \"abc\"\nconst shout = (s: string): string => s + \"!\"\nexport default shout",
            ),
        ],
    );

    let entry = app(
        "import token from './lib.ts'\nconst App = () => (<body><text>{token}</text></body>)\nexport default App",
    );
    assert_builds(
        "default_value_by_reference",
        "src/App.mx",
        &[("src/App.mx", &entry), ("src/lib.ts", "const token = \"abc\"\nexport default token")],
    );
}

#[test]
fn barrels_re_export_components_and_values() {
    let card_named = "export function Card() { return (<view><text>card</text></view>) }";
    let card_default = "const Card = () => (<view><text>card</text></view>)\nexport default Card";
    for (name, card, barrel) in [
        ("component_named", card_named, "export { Card } from './Card.mx'"),
        ("component_default", card_default, "export { default as Card } from './Card.mx'"),
        ("component_star", card_named, "export * from './Card.mx'"),
    ] {
        let entry = app(
            "import { Card } from './components.ts'\nconst App = () => (<body><Card /></body>)\nexport default App",
        );
        assert_builds(
            name,
            "src/App.mx",
            &[("src/App.mx", &entry), ("src/components.ts", barrel), ("src/Card.mx", card)],
        );
    }

    let entry = app(
        "import { hello, Token } from './barrel.ts'\nconst App = () => (<body><text>{hello()} {Token}</text></body>)\nexport default App",
    );
    assert_builds(
        "value_aliases_through_star_and_default",
        "src/App.mx",
        &[
            ("src/App.mx", &entry),
            (
                "src/barrel.ts",
                "export { hello } from './lib.ts'\nexport { default as Token } from './lib.ts'",
            ),
            (
                "src/lib.ts",
                "function greet() { return \"hi\" }\nexport { greet as hello }\nconst token = \"abc\"\nexport default token",
            ),
        ],
    );

    let entry = app(
        "import { hello } from './barrel.ts'\nconst App = () => (<body><text>{hello()}</text></body>)\nexport default App",
    );
    assert_builds(
        "alias_through_star",
        "src/App.mx",
        &[
            ("src/App.mx", &entry),
            ("src/barrel.ts", "export * from './lib.ts'"),
            ("src/lib.ts", "function greet() { return \"hi\" }\nexport { greet as hello }"),
        ],
    );
}
