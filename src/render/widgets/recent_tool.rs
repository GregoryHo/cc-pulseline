//! Per-tool inline formatter shared between the tape widget and the
//! flat-row activity builder.
//!
//! Both render paths are about the same data — a `ToolSummary` for a
//! currently-running or just-completed tool — so they should produce
//! visually identical cells. Differences in *layout* (single-line packed
//! tape vs multi-row Cell-budgeted list) live in the call sites; the
//! per-tool format lives here.
//!
//! Output:
//! - With target: `<icon> Read: src/main.rs` (target truncated by the
//!   per-tool strategy from `target_strategy_for(name)`)
//! - Without target: `<icon> Read`
//!
//! Icon and colour conventions match
//! `render::activity::builder::build_recent_tool_cell` exactly:
//! - `glyph(ICON_TOOL, "T:")` — Nerd Font in Icon mode, literal `T:` in Ascii
//! - `tool_blue` for prefix + name
//! - `secondary` for target text
//! - `": "` separator between name and target

use crate::config::GlyphMode;
use crate::render::activity::builder::target_strategy_for;
use crate::render::activity::truncate;
use crate::render::color::{colorize, ThemePalette};
use crate::render::fmt::sanitize_single_line;
use crate::render::icons::{glyph, ICON_TOOL};
use crate::types::ToolSummary;

/// Render one running-tool cell as a complete inline string (already
/// colourised, target already truncated to its strategy's `ideal` width).
///
/// Caller composes multiple cells with their own separator (tape uses
/// `· `, but other consumers could use `, ` or `\n` — this fn does not
/// emit any separator).
pub fn format_recent_tool_inline(
    tool: &ToolSummary,
    mode: GlyphMode,
    palette: &ThemePalette,
    color_enabled: bool,
) -> String {
    let prefix_glyph = glyph(mode, ICON_TOOL, "T:");
    let prefix = colorize(&prefix_glyph, palette.tool_blue(), color_enabled);
    let name = colorize(&tool.name, palette.tool_blue(), color_enabled);
    match &tool.target {
        Some(raw) => {
            // Defensive `sanitize_single_line` — `extract_target` already
            // sanitises new tool events from the transcript, but
            // `SessionState::recent_tools` may persist on disk via the
            // session cache, and caches written before the sanitiser
            // landed can carry literal `\n` in their `target` strings.
            // CC's statusline parser splits stdout by `\n` into separate
            // rows; an unsanitised target from a stale cache leaks an
            // extra row into our 6-line frame and pushes everything
            // below the top border off CC's display window.
            let safe = sanitize_single_line(raw);
            let (strategy, ideal) = target_strategy_for(&tool.name);
            let truncated = truncate::apply(strategy, &safe, ideal);
            let target = colorize(&truncated, &palette.secondary, color_enabled);
            format!("{prefix}{name}: {target}")
        }
        None => format!("{prefix}{name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::widgets::test_support::aurora_marker_palette;

    fn t(name: &str, target: Option<&str>) -> ToolSummary {
        ToolSummary {
            id: format!("id-{name}"),
            name: name.to_string(),
            target: target.map(str::to_string),
        }
    }

    #[test]
    fn with_target_uses_colon_separator() {
        let p = aurora_marker_palette();
        let s =
            format_recent_tool_inline(&t("Read", Some("src/main.rs")), GlyphMode::Ascii, &p, false);
        assert!(s.contains("T:"), "ascii prefix expected: {s:?}");
        assert!(
            s.contains("Read: src/main.rs"),
            "name + sep + target: {s:?}"
        );
    }

    #[test]
    fn no_target_drops_separator_and_target() {
        let p = aurora_marker_palette();
        let s = format_recent_tool_inline(&t("EnterPlanMode", None), GlyphMode::Ascii, &p, false);
        assert!(
            s.ends_with("EnterPlanMode"),
            "no trailing colon/target: {s:?}"
        );
        assert!(!s.contains(": "));
    }

    #[test]
    fn long_bash_target_uses_keep_head_truncation() {
        // Bash strategy is KeepHead at ideal=40 — the verb (`cargo`) at the
        // start must survive, the long flags at the end get truncated with `…`.
        let p = aurora_marker_palette();
        let cmd = "cargo test --release --no-default-features --features experimental_quota";
        let s = format_recent_tool_inline(&t("Bash", Some(cmd)), GlyphMode::Ascii, &p, false);
        assert!(s.contains("Bash: cargo test"), "verb survived: {s:?}");
        assert!(s.contains('…'), "long tail truncated: {s:?}");
    }

    #[test]
    fn multi_line_target_collapsed_to_single_line() {
        // CC's statusline parser splits stdout by `\n` into separate rows.
        // If we render a tool whose `target` carries an embedded `\n` (rare
        // but possible — e.g. an old session cache written before the
        // transcript-side `sanitize_single_line` landed, or a pasted
        // multi-line bash command), the leak would push our intended
        // bottom rows off CC's display window. Defensive sanitiser inside
        // `format_recent_tool_inline` collapses `\n` / `\r` / `\t` so the
        // output is always a single statusline row.
        let p = aurora_marker_palette();
        let multi = "rg --multiline\\\n  --type rust\\\n  'pattern' src/";
        let s = format_recent_tool_inline(&t("Bash", Some(multi)), GlyphMode::Ascii, &p, false);
        assert!(
            !s.contains('\n'),
            "rendered cell must not leak newlines from target: {s:?}"
        );
        assert!(
            !s.contains('\r'),
            "rendered cell must not leak carriage returns from target: {s:?}"
        );
        assert!(
            !s.contains('\t'),
            "rendered cell must not leak tabs from target: {s:?}"
        );
    }
}
