use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

fn main() {
    let source = r#"function greet(name: string = "hi"): string { return name; }"#;
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, SourceType::typescript()).parse();
    for stmt in &ret.program.body {
        println!("{:#?}", stmt);
    }
}
