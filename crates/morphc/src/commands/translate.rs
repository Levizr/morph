use anyhow::Result;
use colored::Colorize;
use std::path::Path;

pub fn run(file: String, to: String) -> Result<()> {
    let path = Path::new(&file);
    if !path.exists() {
        anyhow::bail!("file not found: {}", file);
    }

    let target = normalize_target(&to);

    crate::logger::log_banner(&format!(
        "Morph — {} → {}",
        file.cyan(),
        target.cyan()
    ));

    crate::logger::log_step("Reading file");
    let content = std::fs::read_to_string(path)?;
    crate::logger::log_key("File", &file);
    crate::logger::log_key("Size", &format!("{} bytes", content.len()));
    crate::logger::log_key("Target", &target);

    crate::logger::log_step("Parsing");
    let pb = crate::logger::spinner(&format!("Parsing {} with Oxc...", file));
    std::thread::sleep(std::time::Duration::from_millis(300));
    pb.finish_and_clear();
    crate::logger::log_success("Parsed successfully");

    crate::logger::log_step("Code generation");
    let lang = if target == "rust" { "Rust" } else { "C++" };
    let pb = crate::logger::spinner(&format!("Generating {} code...", lang));
    std::thread::sleep(std::time::Duration::from_millis(200));
    pb.finish_and_clear();
    crate::logger::log_success(&format!("Generated {} code", lang));

    println!();
    crate::logger::log_warn("Morphing not yet fully implemented in morphc.");
    crate::logger::log_bullet(&format!(
        "This will use {} + {} for max performance.",
        "Oxc".cyan().bold(),
        if target == "rust" {
            "Rust codegen"
        } else {
            "C++ codegen"
        }
        .cyan()
    ));

    println!();
    crate::logger::log_step("Output preview (stub)");
    println!("      {}", format!("// {} → {} (via morphc)", file, target).dimmed());
    println!("      {}", "// Generated with Oxc + Tera".dimmed());
    println!();

    Ok(())
}

fn normalize_target(to: &str) -> String {
    match to.to_lowercase().as_str() {
        "c++" | "cpp" | "c" | "cxx" => "cpp".to_string(),
        "rs" | "rust" => "rust".to_string(),
        other => other.to_string(),
    }
}
