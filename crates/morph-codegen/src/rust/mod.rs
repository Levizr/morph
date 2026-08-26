use anyhow::Result;
use morph_ir::IRWindow;
use std::path::Path;

pub struct RustEmitter<'a> {
    windows: &'a [IRWindow],
}

impl<'a> RustEmitter<'a> {
    pub fn new(windows: &'a [IRWindow]) -> Self {
        Self { windows }
    }

    pub fn emit(&self, _output_dir: &Path) -> Result<()> {
        // TODO: Rust codegen — will be implemented in a future phase
        // Will generate Rust code using morph-runtime-rs crate
        todo!("Rust codegen not yet implemented")
    }
}
