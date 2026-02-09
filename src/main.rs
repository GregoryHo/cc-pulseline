use std::io::{self, Read};

use cc_pulseline::{
    config::{
        build_render_config, check_configs, config_path, default_config_toml,
        default_project_config_toml, load_merged_config, project_config_path,
    },
    run_from_str,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has_init = args.iter().any(|a| a == "--init");
    let has_project = args.iter().any(|a| a == "--project");
    let has_check = args.iter().any(|a| a == "--check");
    let has_print = args.iter().any(|a| a == "--print");

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

    let mut input = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut input) {
        eprintln!("failed to read stdin: {err}");
        std::process::exit(1);
    }

    if input.trim().is_empty() {
        input = "{}".to_string();
    }

    // Extract project root from stdin payload for config scoping
    let project_root = extract_project_root(&input);
    let pulseline_config = load_merged_config(project_root.as_deref());
    let render_config = build_render_config(&pulseline_config);

    let lines = match run_from_str(&input, render_config) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    println!("{}", lines.join("\n"));
}

/// Extract project root from stdin JSON without full deserialization.
fn extract_project_root(input: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    value["workspace"]["current_dir"]
        .as_str()
        .map(String::from)
        .or_else(|| value["cwd"].as_str().map(String::from))
}

fn init_config() {
    let path = config_path();

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

    if let Err(err) = std::fs::write(&path, default_config_toml()) {
        eprintln!("failed to write config {}: {err}", path.display());
        std::process::exit(1);
    }

    println!("created {}", path.display());
}

fn init_project_config() {
    let cwd = std::env::current_dir().unwrap_or_else(|err| {
        eprintln!("failed to get current directory: {err}");
        std::process::exit(1);
    });
    let cwd_str = cwd.to_str().unwrap_or(".");
    let path = project_config_path(cwd_str);

    if path.exists() {
        eprintln!("project config already exists: {}", path.display());
        std::process::exit(1);
    }

    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!("failed to create directory {}: {err}", parent.display());
            std::process::exit(1);
        }
    }

    if let Err(err) = std::fs::write(&path, default_project_config_toml()) {
        eprintln!("failed to write project config {}: {err}", path.display());
        std::process::exit(1);
    }

    println!("created {}", path.display());
}

fn check_config(project_root: Option<&str>) {
    let errors = check_configs(project_root);

    let user_path = config_path();
    if user_path.exists() {
        if errors.iter().any(|(p, _)| p == &user_path) {
            for (path, err) in &errors {
                if path == &user_path {
                    eprintln!("FAIL {}: {err}", path.display());
                }
            }
        } else {
            println!("OK   {}", user_path.display());
        }
    } else {
        println!("SKIP {} (not found)", user_path.display());
    }

    if let Some(root) = project_root {
        let project_path = cc_pulseline::config::project_config_path(root);
        if project_path.exists() {
            if errors.iter().any(|(p, _)| p == &project_path) {
                for (path, err) in &errors {
                    if path == &project_path {
                        eprintln!("FAIL {}: {err}", path.display());
                    }
                }
            } else {
                println!("OK   {}", project_path.display());
            }
        } else {
            println!("SKIP {} (not found)", project_path.display());
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
    println!("icons = {}", config.display.icons);
    println!("tokyo_bg = {}", config.display.tokyo_bg);
    println!();
    println!("[segments.identity]");
    println!("show_model = {}", config.segments.identity.show_model);
    println!("show_style = {}", config.segments.identity.show_style);
    println!("show_version = {}", config.segments.identity.show_version);
    println!("show_project = {}", config.segments.identity.show_project);
    println!("show_git = {}", config.segments.identity.show_git);
    println!();
    println!("[segments.config]");
    println!(
        "show_claude_md = {}",
        config.segments.config.show_claude_md
    );
    println!("show_rules = {}", config.segments.config.show_rules);
    println!("show_hooks = {}", config.segments.config.show_hooks);
    println!("show_mcp = {}", config.segments.config.show_mcp);
    println!("show_skills = {}", config.segments.config.show_skills);
    println!("show_duration = {}", config.segments.config.show_duration);
    println!();
    println!("[segments.budget]");
    println!(
        "show_context = {}",
        config.segments.budget.show_context
    );
    println!("show_tokens = {}", config.segments.budget.show_tokens);
    println!("show_cost = {}", config.segments.budget.show_cost);
    println!();
    println!("[segments.tools]");
    println!("enabled = {}", config.segments.tools.enabled);
    println!("max_lines = {}", config.segments.tools.max_lines);
    println!("max_completed = {}", config.segments.tools.max_completed);
    println!();
    println!("[segments.agents]");
    println!("enabled = {}", config.segments.agents.enabled);
    println!("max_lines = {}", config.segments.agents.max_lines);
    println!();
    println!("[segments.todo]");
    println!("enabled = {}", config.segments.todo.enabled);
    println!("max_lines = {}", config.segments.todo.max_lines);
}
