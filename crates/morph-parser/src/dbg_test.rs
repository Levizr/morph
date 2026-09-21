#[test]
fn debug_data_prop() {
    let src = "export default function App() { return (<body><a href=\"/s\" data={{ theme: \"dark\" }}>x</a></body>) }\n";
    let parsed = crate::parse_mx_str(src, "App.mx").unwrap();
    for c in &parsed.components {
        for n in [&c.jsx] {
            eprintln!("{n:?}");
        }
    }
    panic!("show");
}
