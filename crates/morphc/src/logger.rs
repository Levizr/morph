use colored::Colorize;

// ═══════════════════════════════════════════════════════════════
//  MORPH TUI — Better than Python version
// ═══════════════════════════════════════════════════════════════

pub fn print_logo() {
    // Animated fade-in effect (fast sequential prints)
    let lines = [
        "  ███╗   ███╗ ██████╗ ██████╗ ██████╗ ██╗  ██╗",
        "  ████╗ ████║██╔═══██╗██╔══██╗██╔══██╗██║  ██║",
        "  ██╔████╔██║██║   ██║██████╔╝██████╔╝███████║",
        "  ██║╚██╔╝██║██║   ██║██╔══██╗██╔═══╝ ██╔══██║",
        "  ██║ ╚═╝ ██║╚██████╔╝██║  ██║██║     ██║  ██║",
        "  ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝     ╚═╝  ╚═╝",
    ];
    println!();
    for line in &lines {
        println!("{}", line.cyan().bold());
    }
    println!();
    println!("  {}  {}", "Build native OpenGL Applications with HTML, CSS, and JavaScript".white().bold(), "");
    println!("  {}", "No browser. No Electron. No WebView.".dimmed());
    println!("  {}", format!("v{} — morphc (Rust)", env!("CARGO_PKG_VERSION")).dimmed());
    println!();
}

pub fn print_features() {
    let features = [
        ("Build native UIs",  "Compile .mx files → native OpenGL binary (no browser, no Electron)"),
        ("Hot Reload Dev",    "Edit code → instant window update via Unix socket IPC"),
        ("Tailwind CSS",      "500+ built-in utility classes + arbitrary values"),
        ("Custom C++ Nodes",  "Drop into C++ for full OpenGL control"),
        ("Package System",    "Install community packages via `morph pkg add`"),
        ("Viewports",         "Embed raw OpenGL canvases inside your UI layout"),
        ("Oxc & LightningCSS","3× faster parsing, typed CSS, parallel builds"),
    ];
    for (title, desc) in features {
        println!("    {} {}", "▸".green().bold(), title.bold());
        println!("      {}", desc.dimmed());
    }
    println!();
}

// ── Core log helpers (match Python) ──

pub fn log_info(msg: &str) {
    println!("    {}  {}", "◆".cyan().bold(), msg);
}

pub fn log_success(msg: &str) {
    println!("    {}  {}", "✓".green().bold(), msg);
}

pub fn log_warn(msg: &str) {
    println!("    {}  {}", "⚠".yellow().bold(), msg);
}

pub fn log_error(msg: &str) {
    eprintln!("    {}  {}", "✗".red().bold(), msg);
}

pub fn log_step(msg: &str) {
    println!("\n  {} {}", "→".blue().bold(), msg.bold().white());
}

pub fn log_header(msg: &str) {
    println!("\n  {}", msg.bold().white());
}

pub fn log_banner(title: &str) {
    let width = 56;
    let line = "─".repeat(width);
    println!();
    println!("  {}", line.dimmed());
    println!("  {} {}", "  ◆".cyan().bold(), title.bold().white());
    println!("  {}", line.dimmed());
}

pub fn log_banner_simple(title: &str) {
    println!();
    println!("  {}", title.bold().white());
    println!();
}

pub fn log_bullet(msg: &str) {
    println!("      {} {}", "•".cyan().bold(), msg);
}

pub fn log_muted(msg: &str) {
    println!("      {}", msg.dimmed());
}

pub fn log_dim(msg: &str) {
    println!("    {} {}", "·".dimmed(), msg.dimmed());
}

pub fn log_key(label: &str, val: &str) {
    println!("      {}: {}", label.dimmed().bold(), val.bold().white());
}

pub fn log_section(msg: &str) {
    println!("\n  {}", msg.bold().white());
}

pub fn log_version(label: &str, current: &str, latest: &str) {
    let up_to_date = current == latest;
    let status = if up_to_date {
        "✓ up to date".green().bold().to_string()
    } else {
        format!("↑ {} available", "update".yellow().bold())
    };
    println!(
        "    {:<14} {} {} {}",
        label.bold(),
        format!("v{}", current).cyan(),
        format!("(latest: v{})", latest).dimmed(),
        status,
    );
}

// ── Visual dividers ──

pub fn divider() {
    println!("  {}", "─".repeat(56).dimmed());
}

pub fn divider_thick() {
    println!("  {}", "━".repeat(56).dimmed());
}

// ── Indicatif wrappers ──

pub fn spinner(msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("    {spinner:.cyan.bold} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

pub fn progress_bar(len: u64, msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(len);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("    {spinner:.cyan.bold} {msg} [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ "),
    );
    pb.set_message(msg.to_string());
    pb
}

// ── Suggestion helper ──

pub fn suggest_command(input: &str) -> Option<String> {
    let commands = [
        "new", "install", "update", "dev", "build", "run",
        "check", "doctor", "cache",
    ];
    let mut best: Option<(String, f64)> = None;
    for cmd in &commands {
        let dist = strsim::levenshtein(input, cmd) as f64;
        let max_len = input.len().max(cmd.len()) as f64;
        let similarity = 1.0 - (dist / max_len);
        if similarity > 0.4 {
            if let Some((_, best_sim)) = &best {
                if similarity > *best_sim {
                    best = Some((cmd.to_string(), similarity));
                }
            } else {
                best = Some((cmd.to_string(), similarity));
            }
        }
    }
    best.map(|(cmd, _)| cmd)
}

pub fn did_you_mean(input: &str) {
    if let Some(suggestion) = suggest_command(input) {
        println!(
            "\n    {} Did you mean `{}`?",
            "?".yellow().bold(),
            suggestion.cyan().bold(),
        );
        println!(
            "    {} Run `morph {} --help` for usage.\n",
            "→".dimmed(),
            suggestion,
        );
    } else {
        println!(
            "\n    {} Unknown command `{}`. Run `morph --help` for usage.\n",
            "✗".red().bold(),
            input.red(),
        );
    }
}
