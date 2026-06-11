use std::io::{self, Read};

use cc_pulseline::{
    config::{
        build_render_config, check_configs, config_path, default_config_toml,
        default_project_config_toml, load_merged_config, project_config_path,
        update_theme_in_config, ColorsConfig,
    },
    preview::preview_layouts,
    render::color::{
        available_presets, colorize, extract_ansi_code, light_contrast_failures, resolve_palette,
        theme_descriptions, ThemePalette, RESET,
    },
    types::StdinPayload,
    PulseLineRunner,
};

fn is_color_on() -> bool {
    std::env::var("NO_COLOR").is_err()
}

fn bold(text: &str, color_on: bool) -> String {
    if color_on {
        format!("\x1b[1m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

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
    // Exact-equality match, checked before "--preview" below, so the two
    // flags can't collide.
    let has_preview_layouts = args.iter().any(|a| a == "--preview-layouts");
    let has_preview = args.iter().any(|a| a == "--preview");
    let has_select_theme = args.iter().any(|a| a == "--select-theme");
    let has_palette_map = args.iter().any(|a| a == "--palette-map");

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

    if has_preview_layouts {
        let mut widths: Vec<usize> = Vec::new();
        for arg in args
            .iter()
            .skip_while(|a| a.as_str() != "--preview-layouts")
            .skip(1)
            .filter(|a| !a.starts_with('-'))
        {
            match arg.parse::<usize>() {
                Ok(w) => widths.push(w),
                Err(_) => eprintln!("warning: ignoring invalid width {arg:?}"),
            }
        }
        preview_layouts(&widths);
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

    if has_select_theme {
        select_theme(has_project, cwd.as_deref());
        return;
    }

    if has_palette_map {
        let theme_arg = args
            .iter()
            .skip_while(|a| a.as_str() != "--palette-map")
            .nth(1)
            .filter(|a| !a.starts_with('-'))
            .map(|s| s.as_str());
        print_palette_map(theme_arg);
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
    println!(
        "max_completed_lines = {}",
        config.segments.tools.max_completed_lines
    );
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

    let color_on = is_color_on();
    let c = |text: &str, color: &str| colorize(text, color, color_on);

    for (idx, theme_name) in themes.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        let p = resolve_palette(theme_name, None, &ColorsConfig::default());
        let sep = c("|", &p.separator);

        println!("{}\n", bold(&format!("═══ {theme_name} ═══"), color_on));

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

        // Warn on stderr if the light variant of this theme has high-luminance value fields.
        // Built-in themes ship with these fixed; this warning exists for custom themes.
        let light_p = resolve_palette(theme_name, Some("light"), &ColorsConfig::default());
        let contrast_failures = light_contrast_failures(&light_p, 0.75);
        if !contrast_failures.is_empty() {
            let field_list: Vec<String> = contrast_failures
                .iter()
                .map(|(f, code, lum)| format!("{f}={code}(lum={lum:.2})"))
                .collect();
            eprintln!(
                "warning: theme \"{theme_name}\" light variant has high-luminance fields \
                 that may be illegible on light backgrounds: {}",
                field_list.join(", ")
            );
        }
    }
}

fn select_theme(project: bool, project_root: Option<&str>) {
    let theme_data = theme_descriptions();
    let color_on = is_color_on();

    println!("Available themes:\n");
    for (idx, (name, desc)) in theme_data.iter().enumerate() {
        let p = resolve_palette(name, None, &ColorsConfig::default());
        let swatch = format_mini_swatch(&p, color_on);
        println!("  {:>2}) {:<22} {}  {}", idx + 1, name, swatch, desc);
    }
    println!();

    eprint!("Select theme [1-{}]: ", theme_data.len());
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(0) | Err(_) => {
            eprintln!("\nCancelled.");
            return;
        }
        Ok(_) => {}
    }
    let input = input.trim();
    if input.is_empty() {
        eprintln!("Cancelled.");
        return;
    }
    let choice: usize = match input.parse::<usize>() {
        Ok(n) if n >= 1 && n <= theme_data.len() => n,
        _ => {
            eprintln!("Invalid selection: {input}");
            std::process::exit(1);
        }
    };

    let selected = &theme_data[choice - 1].0;

    println!();
    preview_themes(&[selected.as_str()]);
    println!();

    let (path, template) = if project {
        let root = project_root.unwrap_or(".");
        (project_config_path(root), default_project_config_toml())
    } else {
        (config_path(), default_config_toml())
    };

    match update_theme_in_config(&path, template, selected) {
        Ok(()) => println!("Theme set to \"{selected}\" in {}", path.display()),
        Err(e) => {
            eprintln!("Failed to save theme: {e}");
            std::process::exit(1);
        }
    }
}

fn format_mini_swatch(p: &ThemePalette, color_on: bool) -> String {
    if !color_on {
        return "██████".to_string();
    }
    format!(
        "{}█{RESET}{}█{RESET}{}█{RESET}{}█{RESET}{}█{RESET}{}█{RESET}",
        p.primary, p.active_cyan, p.alert_red, p.stable_green, p.completed_check, p.separator,
    )
}

fn print_palette_map(theme_arg: Option<&str>) {
    let color_on = is_color_on();

    let theme_name = match theme_arg {
        Some(name) => name.to_string(),
        None => {
            let cwd = std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(String::from));
            let config = load_merged_config(cwd.as_deref());
            config.display.theme
        }
    };

    let p = resolve_palette(&theme_name, None, &ColorsConfig::default());
    let c = |text: &str, color: &str| colorize(text, color, color_on);
    let code = |escape: &str| extract_ansi_code(escape).unwrap_or(0);
    let sep = c("|", &p.separator);

    // ── Anatomy: rendered lines with per-segment palette annotations ──

    println!(
        "{}\n",
        bold(&format!("═══ {theme_name} ═══  Anatomy"), color_on)
    );

    // L1: Identity
    println!("  L1: Identity");
    println!(
        "  {} {sep} {} {sep} {} {sep} {} {sep} {} {sep} {}{} {}",
        c("M:Opus 4.6", &p.stable_blue),
        c("AG:greg-bot", &p.head_agent),
        c("[T]", &p.head_thinking),
        c("S:explanatory", &p.secondary),
        c("CC:2.1.80", &p.secondary),
        c("G:main", p.git_green()),
        c("*", p.git_modified()),
        c("↑2", p.git_ahead()),
    );
    print_anatomy_entries(
        &[
            (
                "M:Opus 4.6",
                "stable_blue",
                &p.stable_blue,
                code(&p.stable_blue),
            ),
            (
                "AG:greg-bot",
                "head_agent",
                &p.head_agent,
                code(&p.head_agent),
            ),
            (
                "[T]",
                "head_thinking",
                &p.head_thinking,
                code(&p.head_thinking),
            ),
            ("|", "emphasis_separator", &p.separator, code(&p.separator)),
            (
                "S:  CC:  P:",
                "emphasis_secondary",
                &p.secondary,
                code(&p.secondary),
            ),
            ("G:main", "stable_green", p.git_green(), code(p.git_green())),
            (
                "*",
                "alert_orange",
                p.git_modified(),
                code(p.git_modified()),
            ),
            ("↑2 ↓1", "active_coral", p.git_ahead(), code(p.git_ahead())),
        ],
        color_on,
    );

    // L2: Config Counts
    println!("  L2: Config Counts");
    println!(
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
    print_anatomy_entries(
        &[
            (
                "icons",
                "indicator_*",
                &p.indicator_claude_md,
                code(&p.indicator_claude_md),
            ),
            (
                "2  9  1  32",
                "emphasis_primary",
                &p.primary,
                code(&p.primary),
            ),
            (
                "CLAUDE.md  rules ...",
                "emphasis_structural",
                &p.structural,
                code(&p.structural),
            ),
            ("|", "emphasis_separator", &p.separator, code(&p.separator)),
        ],
        color_on,
    );

    // L3: Budget (3 stages)
    println!("  L3: Budget");
    for (pct, label, cost) in [(43u64, "good", 3.5), (60, "warn", 25.0), (82, "crit", 85.0)] {
        let pct_color = p.color_for_ctx_pct(pct);
        let rate_color = p.color_for_burn_rate(cost);
        let cost_s = format!("${cost:.2}");
        let rate_s = format!("${cost:.2}/h");
        println!(
            "  {:<5} {} {sep} {} {} {sep} {} {}{}{}",
            label,
            c(&format!("CTX:{pct}%"), pct_color),
            c("TOK I:", &p.structural),
            c("86.0k", &p.primary),
            c(&cost_s, &p.cost_base),
            c("(", &p.separator),
            c(&rate_s, rate_color),
            c(")", &p.separator),
        );
    }
    print_anatomy_entries(
        &[
            ("CTX:<55%", "stable_green", p.ctx_good(), code(p.ctx_good())),
            (
                "CTX:55-69%",
                "active_amber",
                p.ctx_warn(),
                code(p.ctx_warn()),
            ),
            (
                "CTX:≥70%",
                "alert_red",
                p.ctx_critical(),
                code(p.ctx_critical()),
            ),
            (
                "TOK I: (labels)",
                "emphasis_structural",
                &p.structural,
                code(&p.structural),
            ),
            (
                "86.0k (values)",
                "emphasis_primary",
                &p.primary,
                code(&p.primary),
            ),
            ("$3.50", "cost_base", &p.cost_base, code(&p.cost_base)),
            (
                "<$10/h",
                "cost_low_rate",
                &p.cost_low_rate,
                code(&p.cost_low_rate),
            ),
            (
                "$10-50/h",
                "cost_med_rate",
                &p.cost_med_rate,
                code(&p.cost_med_rate),
            ),
            (
                ">$50/h",
                "cost_high_rate",
                &p.cost_high_rate,
                code(&p.cost_high_rate),
            ),
        ],
        color_on,
    );

    // Activity
    println!("  Activity");
    println!(
        "  {} {} {sep} {} {}",
        c("✓ Read", &p.completed_check),
        c("×12", &p.secondary),
        c("T:Read:", p.tool_blue()),
        c("main.rs", &p.secondary),
    );
    println!(
        "  {}{}{} {} {}{}{}",
        c("A:Explore", p.agent_purple()),
        c(" [haiku]", &p.structural),
        c(":", p.agent_purple()),
        c("Investigating auth", &p.secondary),
        c("(", &p.separator),
        c("2m", &p.structural),
        c(")", &p.separator),
    );
    println!(
        "  {} {}",
        c("TODO:", p.todo_teal()),
        c("Fixing auth bug", p.todo_teal())
    );
    println!("  {}", c("✓ All todos complete (3/3)", &p.completed_check));
    print_anatomy_entries(
        &[
            (
                "✓ Read",
                "completed_check",
                &p.completed_check,
                code(&p.completed_check),
            ),
            (
                "×12  main.rs",
                "emphasis_secondary",
                &p.secondary,
                code(&p.secondary),
            ),
            ("T:Read:", "active_cyan", p.tool_blue(), code(p.tool_blue())),
            (
                "A:Explore",
                "active_purple",
                p.agent_purple(),
                code(p.agent_purple()),
            ),
            ("TODO:", "active_teal", p.todo_teal(), code(p.todo_teal())),
            (
                "[haiku] (2m)",
                "emphasis_structural",
                &p.structural,
                code(&p.structural),
            ),
            (
                "✓ All todos",
                "completed_check",
                &p.completed_check,
                code(&p.completed_check),
            ),
        ],
        color_on,
    );

    // ── Field Reference ──

    println!(
        "{}\n",
        bold(&format!("═══ {theme_name} ═══  Field Reference"), color_on)
    );

    print_field_section(
        "Emphasis",
        &[
            (code(&p.primary), "emphasis_primary", &p.primary),
            (code(&p.secondary), "emphasis_secondary", &p.secondary),
            (code(&p.structural), "emphasis_structural", &p.structural),
            (code(&p.separator), "emphasis_separator", &p.separator),
        ],
        color_on,
    );
    print_field_section(
        "Alert",
        &[
            (code(&p.alert_red), "alert_red", &p.alert_red),
            (code(&p.alert_orange), "alert_orange", &p.alert_orange),
            (code(&p.alert_magenta), "alert_magenta", &p.alert_magenta),
        ],
        color_on,
    );
    print_field_section(
        "Active",
        &[
            (code(&p.active_cyan), "active_cyan", &p.active_cyan),
            (code(&p.active_purple), "active_purple", &p.active_purple),
            (code(&p.active_teal), "active_teal", &p.active_teal),
            (code(&p.active_amber), "active_amber", &p.active_amber),
            (code(&p.active_coral), "active_coral", &p.active_coral),
        ],
        color_on,
    );
    print_field_section(
        "Stable",
        &[
            (code(&p.stable_blue), "stable_blue", &p.stable_blue),
            (code(p.git_green()), "stable_green", p.git_green()),
        ],
        color_on,
    );
    print_field_section(
        "Indicator",
        &[
            (
                code(&p.indicator_claude_md),
                "indicator_claude_md",
                &p.indicator_claude_md,
            ),
            (
                code(&p.indicator_rules),
                "indicator_rules",
                &p.indicator_rules,
            ),
            (
                code(&p.indicator_memory),
                "indicator_memory",
                &p.indicator_memory,
            ),
            (
                code(&p.indicator_hooks),
                "indicator_hooks",
                &p.indicator_hooks,
            ),
            (code(&p.indicator_mcp), "indicator_mcp", &p.indicator_mcp),
            (
                code(&p.indicator_skills),
                "indicator_skills",
                &p.indicator_skills,
            ),
            (
                code(&p.indicator_duration),
                "indicator_duration",
                &p.indicator_duration,
            ),
        ],
        color_on,
    );
    print_field_section(
        "Completed",
        &[(
            code(&p.completed_check),
            "completed_check",
            &p.completed_check,
        )],
        color_on,
    );
    print_field_section(
        "Cost",
        &[
            (code(&p.cost_base), "cost_base", &p.cost_base),
            (code(&p.cost_low_rate), "cost_low_rate", &p.cost_low_rate),
            (code(&p.cost_med_rate), "cost_med_rate", &p.cost_med_rate),
            (code(&p.cost_high_rate), "cost_high_rate", &p.cost_high_rate),
        ],
        color_on,
    );
    print_field_section(
        "Strata",
        &[
            (code(&p.strata_state), "strata_state", &p.strata_state),
            (
                code(&p.strata_activity),
                "strata_activity",
                &p.strata_activity,
            ),
        ],
        color_on,
    );
    print_field_section(
        "Aurora",
        &[
            (code(&p.aurora_low), "aurora_low", &p.aurora_low),
            (code(&p.aurora_mid), "aurora_mid", &p.aurora_mid),
            (code(&p.aurora_high), "aurora_high", &p.aurora_high),
        ],
        color_on,
    );
    print_field_section(
        "Heads & Tag",
        &[
            (code(&p.tag_label), "tag_label", &p.tag_label),
            (code(&p.head_agent), "head_agent", &p.head_agent),
            (code(&p.head_thinking), "head_thinking", &p.head_thinking),
        ],
        color_on,
    );

    // ── Helper functions ──

    fn print_anatomy_entries(entries: &[(&str, &str, &str, u8)], color_on: bool) {
        let c = |text: &str, color: &str| colorize(text, color, color_on);
        let last = entries.len().saturating_sub(1);
        for (i, (segment, field, color, code)) in entries.iter().enumerate() {
            let branch = if i == last { "└─" } else { "├─" };
            let dots = "·".repeat(28_usize.saturating_sub(segment.len()));
            println!(
                "  {branch} {} {dots} {} {}",
                c(segment, color),
                c(field, color),
                c(&format!("[{code}]"), color),
            );
        }
        println!();
    }

    fn print_field_section(title: &str, entries: &[(u8, &str, &str)], color_on: bool) {
        let c = |text: &str, color: &str| colorize(text, color, color_on);
        let fields: Vec<String> = entries
            .iter()
            .map(|(code, name, color)| c(&format!("[{code:>3}] {name}"), color))
            .collect();
        println!("  {title:<12}{}", fields.join("  "));
    }
}

fn print_help() {
    // Generated from theme_descriptions() so new themes can't be forgotten.
    let themes = theme_descriptions()
        .into_iter()
        .map(|(name, desc)| {
            let tag = if name == "tokyo-night" {
                " (default)"
            } else {
                ""
            };
            format!("    {name:<20}{desc}{tag}").trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
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
                     Example: --preview tokyo-night echo-sub-zero
    --preview-layouts [WIDTH ...] Preview every layout at the given width(s). Default: 140
    --select-theme   Interactively select and apply a theme
    --select-theme --project  Select theme for project config
    --palette-map [THEME]  Show palette field → UI element mapping
                     Example: --palette-map echo-sub-zero

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
{themes}"
    );
}
