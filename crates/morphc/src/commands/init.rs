use anyhow::Result;
use colored::Colorize;
use morph_config::MorphConfig;
use std::path::{Path, PathBuf};

pub fn run(
    name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    title: Option<String>,
    entry: Option<String>,
    ext: Option<String>,
    yes: bool,
) -> Result<()> {
    // Source extension for the scaffolded entry. Defaults to `.mx`. UI entries
    // need JSX, so only `mx`/`tsx` are scaffoldable — `.ts` is rejected with a
    // hint, and any JS-family extension is a hard error.
    let ext = ext.unwrap_or_else(|| "mx".to_string());
    let ext = ext.trim_start_matches('.').to_string();
    match ext.as_str() {
        "mx" | "tsx" => {}
        "ts" => anyhow::bail!(
            "`.ts` entry files can't hold JSX — use `--ext tsx` for an entry file, or import `.ts` support modules."
        ),
        e if morph_config::is_disallowed_js_ext(e) => anyhow::bail!(
            "`.{}` files are not supported — Morph only supports strict `.ts`, `.tsx`, and `.mx`. Use `--ext tsx`.",
            e
        ),
        e => anyhow::bail!("unsupported extension `{}` — use `--ext mx` or `--ext tsx`.", e),
    }

    let target = match name.as_deref() {
        Some(".") | None => PathBuf::from("."),
        Some(n) => PathBuf::from(n),
    };

    let project_name = if target == Path::new(".") {
        std::env::current_dir()?
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-app")
            .to_string()
    } else {
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-app")
            .to_string()
    };

    if target != Path::new(".") {
        if target.exists() {
            anyhow::bail!("directory '{}' already exists", target.display());
        }
        std::fs::create_dir_all(&target)?;
    }

    let project_dir = if target == Path::new(".") {
        std::env::current_dir()?
    } else {
        std::env::current_dir()?.join(&target)
    };

    // ── Banner ──
    crate::logger::log_banner(&format!("Morph New — Creating project: {}", project_name));

    let mut config = MorphConfig::default();
    config.name = project_name.clone();
    if let Some(w) = width {
        config.window.width = w;
    }
    if let Some(h) = height {
        config.window.height = h;
    }
    if let Some(t) = title.clone() {
        config.window.title = t;
    }
    if let Some(e) = entry.clone() {
        // Validate provided --entry extension (hard error on .js/.jsx, etc.)
        morph_config::validate_entry_ext(std::path::Path::new(&e))
            .map_err(|msg| anyhow::anyhow!(msg))?;
        config.entry = e;
    } else {
        config.entry = format!("src/App.{}", ext);
    }

    // ── Interactive wizard ──
    if !yes {
        if width.is_none() {
            let w: String = dialoguer::Input::new()
                .with_prompt(format!("  {} Window width", "→".blue().bold()))
                .default(config.window.width.to_string())
                .interact_text()?;
            config.window.width = w.parse().unwrap_or(800);
        }
        if height.is_none() {
            let h: String = dialoguer::Input::new()
                .with_prompt(format!("  {} Window height", "→".blue().bold()))
                .default(config.window.height.to_string())
                .interact_text()?;
            config.window.height = h.parse().unwrap_or(600);
        }
        if title.is_none() {
            let t: String = dialoguer::Input::new()
                .with_prompt(format!("  {} Window title", "→".blue().bold()))
                .default(project_name.clone())
                .interact_text()?;
            config.window.title = t;
        }
    } else if config.window.title == "Morph App" {
        config.window.title = project_name.clone();
    }

    // ── Scaffold ──
    crate::logger::log_step("Scaffolding project");
    let pb = crate::logger::spinner("Creating project structure...");
    std::thread::sleep(std::time::Duration::from_millis(200));
    scaffold_project(&project_dir, &config, &ext)?;
    pb.finish_and_clear();

    crate::logger::log_success(&format!("Created {} {}", "src/".cyan().dimmed(), format!("App.{ext}").cyan()));
    crate::logger::log_success(&format!("Created {}", "morph.config.json".cyan()));
    crate::logger::log_success(&format!("Created {}", ".gitignore".cyan()));
    crate::logger::log_success(&format!("Created {}", ".morph/ directory".cyan()));

    crate::logger::log_step("Project configuration");
    crate::logger::log_key("Name", &config.name);
    crate::logger::log_key("Entry", &config.entry);
    crate::logger::log_key(
        "Window",
        &format!(
            "{}×{} — \"{}\"",
            config.window.width, config.window.height, config.window.title
        ),
    );
    crate::logger::log_key("Output", &config.output);
    crate::logger::log_key(
        "Runtime",
        &format!("{} v{}", config.runtime.runtime_type, config.runtime.version),
    );

    println!();
    crate::logger::log_success(&format!(
        "Project {} created!",
        project_name.green().bold()
    ));

    if target == Path::new(".") {
        println!("\n  {}", "Next steps:".bold().white());
        println!("      {} {}", "▸".cyan().bold(), "morph dev".cyan().bold());
    } else {
        println!("\n  {}", "Next steps:".bold().white());
        println!(
            "      {} {}",
            "▸".cyan().bold(),
            format!("cd {}", project_name).cyan()
        );
        println!("      {} {}", "▸".cyan().bold(), "morph dev".cyan().bold());
    }
    println!();

    // ── Prompt to install ──
    let install_now = if yes {
        false
    } else {
        dialoguer::Confirm::new()
            .with_prompt(format!("  {} Install runtime sources now?", "→".blue().bold()))
            .default(true)
            .interact()?
    };

    if install_now {
        println!();
        crate::commands::install::run_with_dir(&project_dir)?;
    } else if !yes {
        println!(
            "    {} {} {}",
            "Ok, run".dimmed(),
            "morph install".yellow().bold(),
            "later when ready.".dimmed()
        );
        println!(
            "      {} {}",
            "$".dimmed(),
            format!("cd {} && morph install", project_name).cyan().dimmed()
        );
        println!();
    } else {
        println!(
            "    {} Run {} to download runtime sources.",
            "→".dimmed(),
            "morph install".cyan().bold()
        );
        println!();
    }

    Ok(())
}

fn scaffold_project(project_dir: &Path, config: &MorphConfig, ext: &str) -> Result<()> {
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // src/App.{mx,tsx}
    let app_template = format!(
        r##"import {{ morphState }} from "morph";

export default function App() {{
  const [count, setCount] = morphState(0);

  return (
    <div style={{{{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", width: "100%", height: "100%", gap: "16px" }}}}>
      <text style={{{{ fontSize: "24px", fontWeight: "bold" }}}}>Hello, Morph!</text>
      <text>Count: {{count}}</text>
      <button
        style={{{{ padding: "10px 20px", backgroundColor: "#4F46E5", color: "white", borderRadius: "8px" }}}}
        onClick={{() => setCount(count + 1)}}
      >
        Increment
      </button>
    </div>
  );
}}
"##
    );
    let app_path = src_dir.join(format!("App.{}", ext));
    if !app_path.exists() {
        std::fs::write(&app_path, app_template)?;
    }

    // morph.config.json
    let config_path = project_dir.join("morph.config.json");
    if !config_path.exists() {
        let json = config.to_json_pretty()?;
        std::fs::write(&config_path, json)?;
    }

    // .gitignore
    let gitignore = r#".morph/build/
.morph/cache/
.morph/runtime/
node_modules/
"#;
    let gi_path = project_dir.join(".gitignore");
    if !gi_path.exists() {
        std::fs::write(&gi_path, gitignore)?;
    } else {
        let existing = std::fs::read_to_string(&gi_path).unwrap_or_default();
        if !existing.contains(".morph/") {
            let mut f = std::fs::OpenOptions::new().append(true).open(&gi_path)?;
            use std::io::Write;
            writeln!(f, "\n.morph/build/\n.morph/cache/")?;
        }
    }

    // .morph/
    let morph_dir = project_dir.join(".morph");
    std::fs::create_dir_all(morph_dir.join("build"))?;
    std::fs::create_dir_all(morph_dir.join("cache"))?;
    std::fs::create_dir_all(morph_dir.join("cache").join("css"))?;

    Ok(())
}
