use anyhow::Result;
use colored::Colorize;

pub fn run(
    binary: Option<String>,
    entry: Option<String>,
    output: Option<String>,
    static_: bool,
) -> Result<()> {
    crate::logger::log_banner("Morph Run");

    crate::logger::log_step("Building");
    crate::commands::build::run(entry, output, static_, None, false)?;

    if let Some(bin) = binary {
        crate::logger::log_step("Executing");
        let pb = crate::logger::spinner(&format!("Running {}...", bin.cyan()));
        let status = std::process::Command::new(&bin).status()?;
        pb.finish_and_clear();

        if status.success() {
            crate::logger::log_success("Process exited successfully");
        } else {
            crate::logger::log_error(&format!("Process exited with {}", status));
        }
    } else {
        crate::logger::log_warn("No binary specified. Use `morph run <binary>`.");
    }

    println!();
    Ok(())
}
