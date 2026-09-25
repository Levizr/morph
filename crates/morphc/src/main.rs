mod cache;
mod commands;
mod logger;
mod versions;

use clap::{Parser, Subcommand};
use colored::Colorize;
use morpher::TypeMode;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "morphc",
    disable_version_flag = true,
    about = "Build native OpenGL Applications from TypeScript, TSX, and Morph sources",
    long_about = "Morph — Build native OpenGL Applications\n\nCompile strict TypeScript (.ts/.tsx) and Morph (.mx) projects to native OpenGL\nbinaries. No browser. No Electron. No WebView.\n\nMorph only supports strict .ts / .tsx / .mx source. .js and .jsx are rejected.\n\nDirect file morphing (.ts only):\n  morph app.ts              → morph to C++ (default)\n  morph app.ts --to cpp     → morph to C++\n  morph app.ts --to rust    → morph to Rust\n\nProject commands:\n  morph new                 → scaffold a new .mx or .tsx project\n  morph dev                 → start dev mode with hot reload\n  morph build               → compile to production binary\n  morph run                 → build and run\n  morph install             → download runtime sources\n  morph update              → update runtime or morphc\n  morph check               → lint source files\n  morph doctor              → verify system dependencies\n  morph cache               → manage cache"
)]
struct Cli {
    /// Show version
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Direct file morphing: morph <file> [--to cpp|rust] [--type strict|infer]
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

    /// Target for file morphing (cpp, c++, rust)
    #[arg(long = "to", value_name = "TARGET", alias = "target")]
    to: Option<String>,

    /// Type resolution mode: strict (respect user annotations) or infer (analyze code, default)
    #[arg(long = "types", value_name = "MODE", alias = "type", default_value = "infer")]
    type_mode: TypeMode,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new project
    New {
        /// Project name (use '.' for current directory)
        name: Option<String>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        entry: Option<String>,
        /// Source extension for the entry file: mx | tsx (default: mx)
        #[arg(long)]
        ext: Option<String>,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Download runtime sources
    Install,
    /// Update runtime or morphc
    Update {
        #[arg(long)]
        runtime: bool,
        #[arg(long = "self")]
        self_update: bool,
    },
    /// Start dev mode with live reload of .ts/.tsx/.mx source
    Dev {
        #[arg(long)]
        entry: Option<String>,
    },
    /// Build optimized production binary from .ts/.tsx/.mx source
    Build {
        #[arg(long)]
        entry: Option<String>,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        static_: bool,
        #[arg(long)]
        upx: Option<bool>,
        #[arg(long = "no-upx")]
        no_upx: bool,
        /// Pin a UPX release (overrides build.upx_version in config)
        #[arg(long = "upx-version", value_name = "VERSION")]
        upx_version: Option<String>,
        /// Type resolution mode for translated logic: strict or infer (default, from morph.config.json)
        #[arg(long = "types", value_name = "MODE")]
        type_mode: Option<String>,
        /// Compile in the headless runtime self-test (`binary --morph-self-test`).
        /// Off by default: the test body is dead weight in shipped binaries.
        #[arg(long = "self-test")]
        self_test: bool,
    },
    /// Build and run production binary
    Run {
        binary: Option<String>,
        #[arg(long)]
        entry: Option<String>,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        static_: bool,
        #[arg(long)]
        upx: Option<bool>,
        #[arg(long = "no-upx")]
        no_upx: bool,
        /// Pin a UPX release (overrides build.upx_version in config)
        #[arg(long = "upx-version", value_name = "VERSION")]
        upx_version: Option<String>,
        /// Type resolution mode for translated logic: strict or infer (default, from morph.config.json)
        #[arg(long = "types", value_name = "MODE")]
        type_mode: Option<String>,
    },
    /// Lint source files (.ts/.tsx/.mx) for framework rules
    Check {
        /// File or directory to check (overrides morph.config.json)
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
        #[arg(long)]
        entry: Option<String>,
        #[arg(long)]
        migrate: bool,
    },
    /// Verify system dependencies (toolchain + mandatory GLFW/FreeType/HarfBuzz)
    Doctor {
        /// Show detailed version info for every check
        #[arg(short, long)]
        verbose: bool,
        /// Auto-install missing packages without prompting
        #[arg(short, long)]
        yes: bool,
    },
    /// Manage fetched CSS cache
    Cache,
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            // Let clap handle --help / --version display
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => e.exit(),
                _ => {}
            }
            // Check for typo in subcommand (e.g. `cheack` -> `check`)
            let args: Vec<String> = std::env::args().collect();
            if args.len() > 1 {
                let input = &args[1];
                if !input.starts_with('-') {
                    if let Some(suggestion) = logger::suggest_command(input) {
                        // Only suggest if it's actually a typo (not exact match)
                        if suggestion != *input {
                            logger::did_you_mean(input);
                            std::process::exit(1);
                        }
                    }
                }
            }
            e.exit();
        }
    };

    // ── Version flag ──
    if cli.version {
        println!("morphc {} (CLI)", env!("CARGO_PKG_VERSION"));
        return;
    }

    // ── Top-level file morphing: morph <file> [--to cpp|rust] ──
    if let Some(file) = cli.file {
        if cli.command.is_some() {
            eprintln!("\n    {} Cannot use <FILE> and <COMMAND> together.", "error:".red().bold());
            eprintln!("    {} Use `morph <file> --to cpp` or `morph <command>`.", "hint:".dimmed());
            eprintln!();
            std::process::exit(1);
        }

        // Validate: direct file morphing accepts strict TypeScript only.
        // .tsx/.jsx/.mx use morph build / morph run instead.
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file_str = file.display().to_string();

        if ext == "tsx" || ext == "jsx" || ext == "mx" {
            eprintln!("\n    {} `morph <file>` only works with .ts files.", "error:".red().bold());
            eprintln!(
                "    {} Use `morph build` or `morph run` for .mx/.tsx files.",
                "hint:".dimmed(),
            );
            eprintln!();
            std::process::exit(1);
        }

        if ext != "ts" {
            // Not a valid morph file — treat as a mistyped command
            logger::did_you_mean(&file_str);
            std::process::exit(1);
        }

        let target = cli.to.unwrap_or_else(|| "cpp".to_string());
        let res = commands::translate::run(file.display().to_string(), target, cli.type_mode);
        if let Err(e) = res {
            eprintln!("\n    {} {}", "error:".red().bold(), e);
            for cause in e.chain().skip(1) {
                eprintln!("      {} {}", "caused by:".dimmed(), cause);
            }
            eprintln!();
            std::process::exit(1);
        }
        return;
    }

    // ── Subcommand dispatch ──
    let result = match cli.command {
        Some(Commands::New { name, width, height, title, entry, ext, yes }) => {
            commands::init::run(name, width, height, title, entry, ext, yes)
        }
        Some(Commands::Install) => commands::install::run(),
        Some(Commands::Update { runtime, self_update }) => {
            commands::update::run(runtime, self_update)
        }
        Some(Commands::Dev { entry }) => commands::dev::run(entry),
        Some(Commands::Build {
            entry,
            output,
            static_,
            upx,
            no_upx,
            upx_version,
            type_mode,
            self_test,
        }) => commands::build::run(
            entry,
            output,
            static_,
            upx,
            no_upx,
            false,
            type_mode,
            upx_version,
            self_test,
        )
        .map(|_| ()),
        Some(Commands::Run {
            binary,
            entry,
            output,
            static_,
            upx,
            no_upx,
            upx_version,
            type_mode,
        }) => {
            commands::run::run(binary, entry, output, static_, type_mode, upx, no_upx, upx_version)
        }
        Some(Commands::Check { path, entry, migrate }) => {
            commands::check::run(path.as_ref(), entry, migrate)
        }
        Some(Commands::Doctor { verbose, yes }) => commands::doctor::run(verbose, yes),
        Some(Commands::Cache) => commands::cache::run(),
        None => {
            print_welcome();
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("\n    {} {}", "error:".red().bold(), e);
        for cause in e.chain().skip(1) {
            eprintln!("      {} {}", "caused by:".dimmed(), cause);
        }
        eprintln!();
        std::process::exit(1);
    }
}

fn print_welcome() {
    logger::print_logo();

    // ── Usage ──
    logger::log_section("USAGE");
    println!("      {} {} {}", "$".dimmed(), "morph".cyan().bold(), "<command> [options]".dimmed());
    println!(
        "      {} {} {}",
        "$".dimmed(),
        "morph".cyan().bold(),
        "<file> [--to cpp|rust]".dimmed()
    );
    println!();

    // ── Project commands ──
    logger::log_section("PROJECT");
    let project_cmds = [
        ("new", "Scaffold a new .mx or .tsx project", "◆"),
        ("install", "Download runtime sources", "⬇"),
        ("dev", "Start dev mode with live hot reload", "⚡"),
        ("build", "Compile .ts/.tsx/.mx → native binary", "▣"),
        ("run", "Build and run production binary", "▶"),
        ("check", "Lint source files (.ts/.tsx/.mx)", "✓"),
    ];
    for (cmd, desc, icon) in project_cmds {
        println!("    {} {:<24} {}", icon.cyan().bold(), cmd.green().bold(), desc.dimmed());
    }
    println!();

    // ── Utility commands ──
    logger::log_section("UTILITIES");
    let util_cmds = [
        ("update", "Update runtime (--runtime) or morphc (--self)", "↻"),
        ("doctor", "Verify system dependencies", "♥"),
        ("cache", "Clear fetched CSS cache", "♻"),
    ];
    for (cmd, desc, icon) in util_cmds {
        println!("    {} {:<24} {}", icon.cyan().bold(), cmd.green().bold(), desc.dimmed());
    }
    println!();

    // ── File morphing ──
    logger::log_section("FILE MORPHING (.ts only)");
    println!(
        "    {} {:<24} {}",
        "⇄".cyan().bold(),
        "morph <file>".green().bold(),
        "Morph .ts → C++ (default)".dimmed(),
    );
    println!(
        "    {} {:<24} {}",
        " ".dimmed(),
        "morph <file> --to rust".green().bold(),
        "Morph .ts → Rust".dimmed(),
    );
    println!(
        "    {} {:<24} {}",
        " ".dimmed(),
        "morph <file> --to cpp".green().bold(),
        "Morph .ts → C++ (explicit)".dimmed(),
    );
    println!();
    println!(
        "    {} .tsx/.mx files use {} instead.",
        "Note:".yellow().bold(),
        "morph build / morph run".cyan(),
    );
    println!();

    // ── Options ──
    logger::log_section("OPTIONS");
    println!(
        "      {} {:<24} {}",
        " ".dimmed(),
        "-h, --help".green().bold(),
        "Show this help".dimmed()
    );
    println!(
        "      {} {:<24} {}",
        " ".dimmed(),
        "-v, --version".green().bold(),
        "Show version".dimmed()
    );
    println!(
        "      {} {:<24} {}",
        " ".dimmed(),
        "--to <TARGET>".green().bold(),
        "Target for file morphing".dimmed()
    );
    println!();

    // ── Quick start ──
    logger::log_section("QUICK START");
    println!("      {} {}", "▸".green().bold(), "morph new my-app --yes".cyan().bold());
    println!("      {} {}", "▸".green().bold(), "cd my-app && morph install && morph dev".cyan());
    println!("      {} {}", "▸".green().bold(), "morph app.ts --to cpp".cyan());
    println!();

    // ── Features ──
    logger::log_section("FEATURES");
    logger::print_features();

    // ── Footer ──
    logger::divider();
    println!(
        "    {} {} {}",
        "Run".dimmed(),
        "morph <command> --help".yellow().bold(),
        "for command-specific options".dimmed(),
    );
    println!("    {} {}", "Docs:".dimmed(), "https://morph.levizr.com/docs".cyan());
    println!();
}
