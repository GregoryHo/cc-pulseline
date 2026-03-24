use std::io::{self, Read};

use cc_pulseline::{
    config::{
        build_render_config, check_configs, config_path, default_config_toml,
        default_project_config_toml, load_merged_config, project_config_path, ColorsConfig,
    },
    render::color::{available_presets, colorize, resolve_palette, RESET},
    types::StdinPayload,
    PulseLineRunner,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("cc-pulseline {VERSION}");
        return;
    }

    let has_init = args.iter().any(|a| a == "--init");
    let has_project = args.iter().any(|a| a == "--project");
    let has_check = args.iter().any(|a| a == "--check");
    let has_print = args.iter().any(|a| a == "--print");
    let has_preview = args.iter().any(|a| a == "--preview");

    if has_init {
        if has_project {
            init_project_config();
        } else {
            init_config();
        }
        return;
    }

    // For --check and --print, determine project root from cwd
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(String::from));

    if has_check {
        check_config(cwd.as_deref());
        return;
    }

    if has_print {
        print_config(cwd.as_deref());
        return;
    }

    if has_preview {
        let theme_args: Vec<&str> = args
            .iter()
            .skip_while(|a| a.as_str() != "--preview")
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .map(|s| s.as_str())
            .collect();
        preview_themes(&theme_args);
        return;
    }

    let mut input = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut input) {
        eprintln!("failed to read stdin: {err}");
        std::process::exit(1);
    }

    if input.trim().is_empty() {
        input = "{}".to_string();
    }

    // Deserialize once, extract project root for config, then render
    let payload: StdinPayload = match serde_json::from_str(&input) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("invalid stdin JSON: {err}");
            std::process::exit(1);
        }
    };

    let project_root = payload.resolve_project_path();
    let pulseline_config = load_merged_config(project_root.as_deref());
    let render_config = build_render_config(&pulseline_config);

    let lines = match PulseLineRunner::default().run_from_payload(&payload, render_config) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    println!("{}", lines.join("\n"));
}

fn init_config() {
    write_init_file(&config_path(), default_config_toml());
}

fn init_project_config() {
    let cwd = std::env::current_dir().unwrap_or_else(|err| {
        eprintln!("failed to get current directory: {err}");
        std::process::exit(1);
    });
    let cwd_str = cwd.to_str().unwrap_or(".");
    write_init_file(&project_config_path(cwd_str), default_project_config_toml());
}

fn write_init_file(path: &std::path::Path, content: &str) {
    if path.exists() {
        eprintln!("config already exists: {}", path.display());
        std::process::exit(1);
    }

    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!("failed to create directory {}: {err}", parent.display());
            std::process::exit(1);
        }
    }

    if let Err(err) = std::fs::write(path, content) {
        eprintln!("failed to write {}: {err}", path.display());
        std::process::exit(1);
    }

    println!("created {}", path.display());
}

fn check_config(project_root: Option<&str>) {
    let errors = check_configs(project_root);

    let mut paths_to_check = vec![config_path()];
    if let Some(root) = project_root {
        paths_to_check.push(cc_pulseline::config::project_config_path(root));
    }

    for check_path in &paths_to_check {
        if !check_path.exists() {
            println!("SKIP {} (not found)", check_path.display());
            continue;
        }
        let path_errors: Vec<_> = errors.iter().filter(|(p, _)| p == check_path).collect();
        if path_errors.is_empty() {
            println!("OK   {}", check_path.display());
        } else {
            for (path, err) in path_errors {
                eprintln!("FAIL {}: {err}", path.display());
            }
        }
    }

    if !errors.is_empty() {
        std::process::exit(1);
    }
}

fn print_config(project_root: Option<&str>) {
    let config = load_merged_config(project_root);
    println!("[display]");
    println!("theme = {:?}", config.display.theme);
    if let Some(ref variant) = config.display.variant {
        println!("variant = {:?}", variant);
    }
    println!("icons = {}", config.display.icons);
    println!();
    println!("[segments.identity]");
    println!("show_model = {}", config.segments.identity.show_model);
    println!("show_style = {}", config.segments.identity.show_style);
    println!("show_version = {}", config.segments.identity.show_version);
    println!("show_project = {}", config.segments.identity.show_project);
    println!("show_git = {}", config.segments.identity.show_git);
    println!(
        "show_git_stats = {}",
        config.segments.identity.show_git_stats
    );
    println!("show_agent = {}", config.segments.identity.show_agent);
    println!("show_worktree = {}", config.segments.identity.show_worktree);
    println!();
    println!("[segments.config]");
    println!("show_claude_md = {}", config.segments.config.show_claude_md);
    println!("show_rules = {}", config.segments.config.show_rules);
    println!("show_memory = {}", config.segments.config.show_memory);
    println!("show_hooks = {}", config.segments.config.show_hooks);
    println!("show_mcp = {}", config.segments.config.show_mcp);
    println!("show_skills = {}", config.segments.config.show_skills);
    println!("show_duration = {}", config.segments.config.show_duration);
    println!();
    println!("[segments.budget]");
    println!("show_context = {}", config.segments.budget.show_context);
    println!("show_tokens = {}", config.segments.budget.show_tokens);
    println!("show_cost = {}", config.segments.budget.show_cost);
    println!("show_speed = {}", config.segments.budget.show_speed);
    println!();
    println!("[segments.quota]");
    println!("enabled = {}", config.segments.quota.enabled);
    println!("show_five_hour = {}", config.segments.quota.show_five_hour);
    println!("show_seven_day = {}", config.segments.quota.show_seven_day);
    println!();
    println!("[segments.tools]");
    println!("enabled = {}", config.segments.tools.enabled);
    println!("max_lines = {}", config.segments.tools.max_lines);
    println!("max_completed = {}", config.segments.tools.max_completed);
    println!("tools_per_line = {}", config.segments.tools.tools_per_line);
    println!();
    println!("[segments.agents]");
    println!("enabled = {}", config.segments.agents.enabled);
    println!("max_lines = {}", config.segments.agents.max_lines);
    println!();
    println!("[segments.todo]");
    println!("enabled = {}", config.segments.todo.enabled);
    println!("max_lines = {}", config.segments.todo.max_lines);
}

fn preview_themes(theme_args: &[&str]) {
    let themes: Vec<String> = if theme_args.is_empty() {
        available_presets()
    } else {
        theme_args.iter().map(|s| s.to_string()).collect()
    };

    let color_on = std::env::var("NO_COLOR").is_err();
    let c = |text: &str, color: &str| colorize(text, color, color_on);
    let bold = |text: &str| {
        if color_on {
            format!("\x1b[1m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };

    for (idx, theme_name) in themes.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        let p = resolve_palette(theme_name, None, &ColorsConfig::default());
        let sep = c("|", &p.separator);

        println!("{}\n", bold(&format!("═══ {theme_name} ═══")));

        // L1: Identity
        let l1 = format!(
            "  {} {sep} {} {sep} {} {sep} {}{} {}",
            c("M:Opus 4.6", &p.stable_blue),
            c("S:explanatory", &p.secondary),
            c("CC:2.1.80", &p.secondary),
            c("G:main", p.git_green()),
            c("*", p.git_modified()),
            c("↑2", p.git_ahead()),
        );
        println!("{l1}");

        // L2: Config counts
        let l2 = format!(
            "  {} {} {sep} {} {} {sep} {} {} {sep} {} {}",
            c("2", &p.primary),
            c("CLAUDE.md", &p.structural),
            c("9", &p.primary),
            c("rules", &p.structural),
            c("1", &p.primary),
            c("memories", &p.structural),
            c("32", &p.primary),
            c("hooks", &p.structural),
        );
        println!("{l2}");

        // L3: Context stages (uses same thresholds as layout.rs via palette helpers)
        for (pct, label, cost) in [(43u64, "good", 3.5), (60, "warn", 25.0), (82, "crit", 85.0)] {
            let pct_color = p.color_for_ctx_pct(pct);
            let rate_color = p.color_for_burn_rate(cost);
            let ctx = c(&format!("CTX:{pct}%"), pct_color);
            let tok_part = format!("{} {}", c("TOK I:", &p.structural), c("86.0k", &p.primary));
            let cost_s = format!("${cost:.2}");
            let rate_s = format!("${cost:.2}/h");
            let cost_part = format!(
                "{} {}{}{}",
                c(&cost_s, &p.cost_base),
                c("(", &p.separator),
                c(&rate_s, rate_color),
                c(")", &p.separator),
            );
            println!("  {label:<5} {ctx} {sep} {tok_part} {sep} {cost_part}");
        }

        // Activity
        let completed_line = format!(
            "  {} {} {sep} {} {}",
            c("✓ Read", &p.completed_check),
            c("×12", &p.secondary),
            c("T:Read:", p.tool_blue()),
            c("main.rs", &p.secondary),
        );
        println!("{completed_line}");

        let agent_line = format!(
            "  {}{}{} {} {}{}{}",
            c("A:Explore", p.agent_purple()),
            c(" [haiku]", &p.structural),
            c(":", p.agent_purple()),
            c("Investigating auth", &p.secondary),
            c("(", &p.separator),
            c("2m", &p.structural),
            c(")", &p.separator),
        );
        println!("{agent_line}");
        println!(
            "  {} {}",
            c("TODO:", p.todo_teal()),
            c("Fixing auth bug", p.todo_teal())
        );
        println!("  {}", c("✓ All todos complete (3/3)", &p.completed_check));

        // Color swatch
        let dim_label = if color_on {
            format!("\x1b[2mPalette:{RESET}")
        } else {
            "Palette:".to_string()
        };
        println!("\n  {dim_label}");
        println!(
            "  {}pri{RESET}  {}sec{RESET}  {}str{RESET}  {}sep{RESET}",
            p.primary, p.secondary, p.structural, p.separator,
        );
        println!(
            "  {}accent{RESET}  {}warn{RESET}  {}alert{RESET}  {}dirty{RESET}  {}done{RESET}  {}model{RESET}",
            p.active_cyan, p.active_amber, p.alert_red, p.alert_orange, p.completed_check, p.stable_blue,
        );
    }
}

fn print_help() {
    println!(
        "cc-pulseline {VERSION} - High-performance Claude Code statusline

USAGE:
    cc-pulseline [OPTIONS]
    echo '{{\"model\":...}}' | cc-pulseline

OPTIONS:
    -h, --help       Show this help message
    -V, --version    Show version
    --init           Create user config (~/.claude/pulseline/config.toml)
    --init --project Create project config (.claude/pulseline.toml)
    --check          Validate config files
    --print          Show effective merged config
    --preview [THEME ...] Preview theme(s). No args = all presets.
                     Examples: --preview tokyo-night echo-sub-zero

RUNTIME:
    Reads Claude Code statusline JSON from stdin, outputs formatted lines.
    Empty stdin defaults to {{}}.

CONFIG FILES:
    User:    ~/.claude/pulseline/config.toml
    Project: {{project}}/.claude/pulseline.toml

ENVIRONMENT:
    NO_COLOR    Disable color output
    COLUMNS     Terminal width for layout degradation

THEMES:
    tokyo-night         Blue-tinted grays, 25+ semantic colors (default)
    echo-sub-zero       Mono-accent minimalist, 3-stage signaling
    titanium-precision  Industrial steel blues, amber warnings, brick reds
    cnc-telemetry       Hardware telemetry: anodized teal, matte copper, rust red
    cyberdeck-hud       Sci-Fi HUD: neon cyan, cyber orange, laser crimson
    stark-hud           Iron Man: Arc Reactor cyan, Armor red, Faceplate gold
    mako-reactor        FFVII: Shinra steel, Mako cyan-green, Materia accents
    aburaya-twilight    Spirited Away: bathhouse red, dragon teal, spirit blues"
    );
}
