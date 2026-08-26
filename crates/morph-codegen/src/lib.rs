use anyhow::Result;
use morph_ir::IRPage;

/// Stub codegen — will use Tera templates
pub struct Emitter;

impl Emitter {
    pub fn emit_cpp(ir: &IRPage, output_dir: &std::path::Path) -> Result<()> {
        std::fs::create_dir_all(output_dir)?;
        // TODO: Use Tera templates to generate C++ source
        let stub = format!(
            "// Generated C++ stub for Morph app\n// Window: {}x{} \"{}\"\n#include \"morph_api.h\"\nint main() {{ return 0; }}\n",
            ir.window.width, ir.window.height, ir.window.title
        );
        std::fs::write(output_dir.join("app_main.cpp"), stub)?;
        Ok(())
    }
}
