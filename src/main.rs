use std::io::{self, Read};

use cc_pulseline::{
    config::{ColorTheme, GlyphMode, RenderConfig},
    run_from_str,
};

fn main() {
    let mut input = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut input) {
        eprintln!("failed to read stdin: {err}");
        std::process::exit(1);
    }

    if input.trim().is_empty() {
        input = "{}".to_string();
    }

    let lines = match run_from_str(&input, build_config()) {
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

fn build_config() -> RenderConfig {
    let color_enabled = std::env::var("NO_COLOR").is_err();
    let glyph_mode = match std::env::var("PULSELINE_ICONS").as_deref() {
        Ok("0" | "false" | "ascii") => GlyphMode::Ascii,
        _ => GlyphMode::Icon,
    };
    let color_theme = match std::env::var("PULSELINE_THEME").as_deref() {
        Ok("light") => ColorTheme::Light,
        _ => ColorTheme::Dark,
    };
    let terminal_width = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse().ok());

    RenderConfig {
        color_enabled,
        color_theme,
        glyph_mode,
        terminal_width,
        ..RenderConfig::default()
    }
}
