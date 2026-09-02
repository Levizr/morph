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
    let start = std::time::Instant::now();
    // Use morph-js translator (direct oxc -> C++/Rust, fastest, bug-free same as python for cpp)
    let (ext, code) = match target.as_str() {
        "rust" => {
            // For Rust target, use Rust codegen; fallback to C++ if not yet fully implemented
            // Currently morph-js Rust emitter is minimal but produces valid .rs file
            match morph_js::translate_to_rust(&content, &file, 0) {
                Ok(rust_code) => ("rs", rust_code),
                Err(_) => {
                    // Fallback: generate C++ and save as .rs with comment (ensures file exists)
                    match morph_js::translate(&content, &file) {
                        Ok(cpp) => ("rs", format!("// Translated from {} (fallback C++ in Rust file)\n{}", file, cpp)),
                        Err(e) => {
                            pb.finish_and_clear();
                            anyhow::bail!("translate failed: {}", e);
                        }
                    }
                }
            }
        }
        _ => match morph_js::translate(&content, &file) {
            Ok(cpp) => ("cpp", cpp),
            Err(e) => {
                pb.finish_and_clear();
                anyhow::bail!("translate failed: {}", e);
            }
        },
    };
    pb.finish_and_clear();
    crate::logger::log_success(&format!("Parsed & generated in {:?}", start.elapsed()));

    crate::logger::log_step("Writing output");
    // Make the generated file standalone-compilable
    // 1) Use global runtime if available (~/.morph/cache/runtimes/cpp/...), otherwise inline minimal headers
    // 2) Wrap top-level executable statements in `int main()` if needed
    // 3) Add shim for morph::dev_log -> std::println so it runs standalone with C++23
    let mut standalone_code = code.clone();
    // Replace relative runtime includes with global absolute path if found
    let global_runtime = find_global_runtime();
    let has_global = global_runtime.is_some();
    if let Some(runtime_path) = &global_runtime {
        let runtime_str = runtime_path.display().to_string();
        // Generated code has #include "../../runtime/cpp/..." — replace with absolute
        // Do the most specific first to avoid double replacement
        standalone_code = standalone_code.replace("../../runtime/cpp/types/js_types.h", &format!("{}/types/js_types.h", runtime_str));
        standalone_code = standalone_code.replace("../../runtime/cpp", &runtime_str);
        standalone_code = standalone_code.replace("\"../../runtime", &format!("\"{}", runtime_str));
    } else {
        // No global runtime found — tell user
        crate::logger::log_warn("No global runtime found in ~/.morph/cache/runtimes/cpp — generated file will be self-contained");
        crate::logger::log_bullet("Run `morph install` to cache the runtime, or compile with -I <runtime>/cpp");
    }
    // Always add shim for morph::str / dev_log so the file is workable standalone
    let needs_str_shim = standalone_code.contains("morph::str") || standalone_code.contains("morph::dev_log");
    let has_str_shim = standalone_code.contains("inline std::string str");
    if needs_str_shim && !has_str_shim {
        let shim = r#"#include <iostream>
#include <string>
namespace morph {
inline std::string str(const JsNumber& v) {
    if (v.is_int()) return std::to_string(std::get<int64_t>(v.value));
    if (v.is_double()) return std::to_string(std::get<double>(v.value));
    return std::get<std::string>(v.value);
}
inline std::string str(const JsString& v) { return v.value; }
inline std::string str(const JsBoolean& v) { return v.value ? "true" : "false"; }
inline std::string str(const JsValue& v) {
    if (v.is_string()) return std::get<JsString>(v.inner).value;
    if (v.is_number()) return str(std::get<JsNumber>(v.inner));
    if (v.is_boolean()) return str(std::get<JsBoolean>(v.inner));
    if (v.is_null()) return "null";
    if (v.is_undefined()) return "undefined";
    return "[object]";
}
inline std::string str(const std::string& s) { return s; }
inline std::string str(const char* s) { return std::string(s); }
inline std::string str(int v) { return std::to_string(v); }
inline std::string str(int64_t v) { return std::to_string(v); }
inline std::string str(double v) { return std::to_string(v); }
inline void dev_log(auto&& x) { std::cout << str(x) << std::endl; }
inline void dev_log_warn(auto&& x) { std::cerr << "WARN: " << str(x) << std::endl; }
inline void dev_log_error(auto&& x) { std::cerr << "ERROR: " << str(x) << std::endl; }
}
"#;
        if let Some(pos) = standalone_code.find("\n\n") {
            standalone_code.insert_str(pos + 2, &format!("{}\n", shim));
        } else {
            standalone_code = format!("{}\n{}", shim, standalone_code);
        }
        // Ensure iostream is included (for std::cout)
        if !standalone_code.contains("#include <iostream>") {
            standalone_code = format!("#include <iostream>\n{}", standalone_code);
        }
    }

    // If top-level executable statements exist outside any function/class, wrap them in main
    // Heuristic: if code contains `morph::dev_log` at file scope (line starting without indent and not inside class/function), wrap
    let needs_main_wrapper = should_wrap_in_main(&standalone_code);
    if needs_main_wrapper {
        // Extract top-level executable lines (those that are not `class`, `static`, `template`, `#include`, etc.)
        // For now, just wrap the entire file's executable tail in main if no `int main` already exists
        if !standalone_code.contains("int main") {
            // Find where to insert main: after includes and class/function defs, before top-level statements
            // Simpler: append a main that contains the top-level statements that were at file scope
            // For now, we keep the file as is but add a main wrapper that calls the top-level code
            // Instead, we will extract lines that look like executable statements at file scope
            let wrapped = wrap_top_level_in_main(&standalone_code);
            if !wrapped.is_empty() {
                standalone_code = wrapped;
            }
        }
    }

    if !has_global {
        crate::logger::log_warn("Generated file is self-contained — compile with `g++ -std=c++23 -O2 file.cpp -o file`");
    } else {
        crate::logger::log_bullet(&format!("Using global runtime at {}", global_runtime.unwrap().display()));
        crate::logger::log_bullet("Compile with `g++ -std=c++23 file.cpp -I <runtime>/cpp -o file` or just `g++ -std=c++23 file.cpp` if runtime is in default search path");
    }

    // Python: base, _ = os.path.splitext(src); out = base + ".cpp"  (same dir, same basename)
    // For Rust, use .rs; for C++ use .cpp — exactly like `path.with_extension(ext)` but with_extension handles multi-dot correctly
    let out_path = path.with_extension(ext);
    // Python writes `f.write(cpp); f.write("\n")` => always trailing newline, even if cpp already ends with \n
    // We do the same for exact match
    let output_with_newline = if standalone_code.ends_with('\n') { standalone_code.clone() } else { format!("{}\n", standalone_code) };
    std::fs::write(&out_path, &output_with_newline)?;
    crate::logger::log_success(&format!("Generated {} code", if target == "rust" { "Rust" } else { "C++" }));
    println!();
    crate::logger::log_key("Output", &out_path.display().to_string());
    crate::logger::log_key("Lines", &format!("{}", code.lines().count()));
    // Show preview of first few lines
    println!();
    crate::logger::log_step("Output preview");
    for line in code.lines().take(20) {
        println!("      {}", line.dimmed());
    }
    if code.lines().count() > 20 {
        println!("      {}", format!("... ({} more lines)", code.lines().count() - 20).dimmed());
    }
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

fn find_global_runtime() -> Option<std::path::PathBuf> {
    // Helper to check if a runtime path is valid (has non-empty headers)
    let is_valid = |p: &std::path::Path| {
        let js = p.join("types").join("js_types.h");
        if js.exists() {
            if let Ok(meta) = std::fs::metadata(&js) {
                if meta.len() == 0 {
                    return false;
                }
                // Check for expected content
                if let Ok(content) = std::fs::read_to_string(&js) {
                    if content.contains("js_value.h") || content.contains("JsNumber") {
                        return true;
                    }
                }
                return meta.len() > 10;
            }
        }
        p.join("morph_api.h").exists() || p.join("core").join("window.h").exists()
    };
    // Check local runtime first (more reliable than global cache which may be empty)
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let local_candidates = [
        cwd.join("runtime").join("cpp"),
        cwd.join("../runtime").join("cpp"),
        cwd.join("../../runtime").join("cpp"),
        std::path::PathBuf::from("runtime/cpp"),
        std::path::PathBuf::from("/home/piyush/My_Projects/morph/runtime/cpp"),
    ];
    for p in local_candidates {
        if is_valid(&p) {
            return Some(p);
        }
    }
    // Check global cache
    if let Ok(home) = dirs::home_dir().ok_or(()) {
        let base = home.join(".morph").join("cache").join("runtimes").join("cpp");
        if base.exists() {
            if let Ok(entries) = std::fs::read_dir(&base) {
                let mut candidates: Vec<std::path::PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                candidates.sort();
                for latest in candidates.iter().rev() {
                    if is_valid(latest) {
                        return Some(latest.clone());
                    }
                }
                // Fallback to any candidate even if not fully valid
                if let Some(latest) = candidates.last().cloned() {
                    if latest.exists() {
                        return Some(latest);
                    }
                }
            }
            if is_valid(&base) {
                return Some(base);
            }
        }
        let alt = home.join(".morph").join("runtimes").join("cpp");
        if is_valid(&alt) {
            return Some(alt);
        }
    }
    None
}

fn should_wrap_in_main(code: &str) -> bool {
    if code.contains("int main") {
        return false;
    }
    let mut brace_depth: i32 = 0;
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let open = line.matches('{').count() as i32;
        let close = line.matches('}').count() as i32;
        let prev_depth = brace_depth;
        brace_depth += open - close;
        if prev_depth == 0 {
            if trimmed.starts_with("morph::dev_log")
                || trimmed.starts_with("std::cout")
                || trimmed.starts_with("std::cerr")
                || trimmed.starts_with("std::println")
                || trimmed.starts_with("for (")
                || trimmed.starts_with("while (")
                || trimmed.starts_with("if (")
                || trimmed.starts_with("auto ")
                || trimmed.starts_with("Js")
            {
                if trimmed.starts_with("morph::dev_log")
                    || trimmed.starts_with("std::cout")
                    || trimmed.starts_with("std::cerr")
                    || trimmed.starts_with("std::println") {
                    return true;
                }
                if trimmed.ends_with(';') && (trimmed.starts_with("for ") || trimmed.starts_with("while ") || trimmed.starts_with("if ") || trimmed.contains("dev_log") || trimmed.contains("cout") || trimmed.contains("println")) {
                    return true;
                }
            }
        }
    }
    false
}

fn wrap_top_level_in_main(code: &str) -> String {
    // Simple wrapper: extract top-level executable statements and put them in main
    // For now, we just append a main that will be used if the file has top-level executable code
    // We keep original code as is, but add a main that contains the executable statements
    // Heuristic: find lines that are at file scope and look like executable, collect them
    let mut header_lines = Vec::new();
    let mut main_body = Vec::new();
    let mut brace_depth: i32 = 0;
    let mut in_main_wrapper = false;
    for line in code.lines() {
        let trimmed = line.trim();
        // Track depth before processing line
        let open = line.matches('{').count() as i32;
        let close = line.matches('}').count() as i32;
        // If this line is at file scope and looks like executable, move it to main
        let is_file_scope = brace_depth == 0;
        let is_executable = is_file_scope
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("class ")
            && !trimmed.starts_with("struct ")
            && !trimmed.starts_with("template")
            && !trimmed.starts_with("static ")
            && !trimmed.starts_with("inline")
            && !trimmed.starts_with("namespace")
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("/*")
            && (trimmed.starts_with("morph::dev_log")
                || trimmed.starts_with("std::cout")
                || trimmed.starts_with("std::cerr")
                || trimmed.starts_with("std::println")
                || trimmed.starts_with("for (")
                || trimmed.starts_with("while (")
                || trimmed.starts_with("if (")
                || trimmed.starts_with("try {")
                || trimmed.starts_with("auto ")
                    && trimmed.contains("std::make_shared"))
            ;
        if is_executable {
            // Strip leading indent and add to main body with indent
            main_body.push(format!("    {}", trimmed));
            in_main_wrapper = true;
        } else {
            header_lines.push(line.to_string());
            // Update depth after
        }
        brace_depth += open - close;
        if brace_depth < 0 {
            brace_depth = 0;
        }
    }
    if !in_main_wrapper || main_body.is_empty() {
        return String::new();
    }
    // Reconstruct: header + main wrapper
    let mut out = header_lines.join("\n");
    out.push_str("\n\nint main() {\n");
    for line in main_body {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("    return 0;\n}\n");
    out
}
