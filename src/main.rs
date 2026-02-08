use std::io::{self, Read};

use cc_pulseline::{
    config::{build_render_config, config_path, default_config_toml, load_config},
    run_from_str,
};

fn main() {
    // Handle --init flag: create default config file and exit
    if std::env::args().any(|arg| arg == "--init") {
        init_config();
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

    let pulseline_config = load_config();
    let render_config = build_render_config(&pulseline_config);

    let lines = match run_from_str(&input, render_config) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    for line in lines {
        println!("{line}");
    }
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
