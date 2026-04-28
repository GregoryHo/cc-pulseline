//! Per-tool cell construction shared between the flat-row activity
//! builder and the cluster-layout tape widget.
//!
//! Both render paths describe the same data — a `ToolSummary` for a
//! currently-running or just-completed tool — so they should produce
//! visually identical cells. Differences in *layout* (single-line packed
//! tape vs multi-row Cell-budgeted list) live in the call sites; the
//! per-tool building blocks live here.
//!
//! Public surface:
//! - `target_strategy_for(name)` — single source of truth for the
//!   per-tool truncation strategy + ideal target width
//! - `build_recent_tool_cell(tool, mode, palette, color)` — produces a
//!   `Cell` with `min_width = 8`, `ideal_width = strategy.ideal`, and
//!   `Required` priority. Consumed by both the activity row builder and
//!   the tape widget; both then hand the cell to
//!   `pack_with_separator(cells, available_width, sep, sep_w, color)`.
//! - `format_recent_tool_inline(tool, mode, palette, color)` —
//!   transitional convenience that emits a finished string without going
//!   through the budgeter. Used by tape's detail mode pending Step 2 of
//!   the cluster-layout cell migration; kept here so the brief-mode tape
//!   and any other inline consumer can share one formatter without
//!   pulling in cell machinery.
//!
//! Icon and colour conventions are identical across all three:
//! - `glyph(ICON_TOOL, "T:")` — Nerd Font in Icon mode, literal `T:` in Ascii
//! - `tool_blue` for prefix + name
//! - `secondary` for target text
//! - `": "` separator between name and target

use crate::config::GlyphMode;
use crate::render::activity::cell::{Cell, CellBody, CellPriority};
use crate::render::activity::truncate::{self, TruncationStrategy};
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::sanitize_single_line;
use crate::render::icons::{glyph, ICON_TOOL};
use crate::types::ToolSummary;

/// Per-tool truncation strategy + ideal target width. Single source of
/// truth that replaced the per-tool `truncate_str(_, N)` constants
/// scattered across `providers/transcript.rs::extract_target`.
pub fn target_strategy_for(tool_name: &str) -> (TruncationStrategy, usize) {
    use TruncationStrategy::*;
    match tool_name {
        "Read" | "Write" | "Edit" | "NotebookEdit" => (KeepTail, 40),
        // KeepHead so the verb (`grep`, `cargo`, `sed`, …) is always at the
        // start of the rendered cell. The previous CommandSmart prioritised
        // regex payloads over the verb, producing rows that were impossible
        // to match to a command at a glance.
        "Bash" | "PowerShell" => (KeepHead, 40),
        "Glob" | "Grep" => (KeepHead, 30),
        "WebFetch" => (KeepMiddle, 40),
        "WebSearch" | "Skill" | "Advisor" | "MCPSearch" | "AskUserQuestion" => (Sentence, 50),
        "SendMessage" | "LSP" | "Monitor" | "PushNotification" => (KeepHead, 30),
        _ => (KeepHead, 30),
    }
}

/// Build a width-budgeter-ready `Cell` for one running/recent tool.
///
/// The body uses `min_width = 8` so the per-tool target can compress to a
/// few-character preview when the row is tight, but never disappears
/// entirely (cell priority is `Required`). `ideal_width` comes from the
/// per-tool strategy so e.g. Bash rows get a 40-char budget while Glob
/// rows get 30. Callers feed the resulting cells through
/// `pack_with_separator` to honour the row's actual width budget.
pub fn build_recent_tool_cell(
    t: &ToolSummary,
    mode: GlyphMode,
    p: &ThemePalette,
    color: bool,
) -> Cell {
    let prefix_glyph = glyph(mode, ICON_TOOL, "T:");
    let prefix = colorize(&prefix_glyph, p.tool_blue(), color);
    let name = colorize(&t.name, p.tool_blue(), color);
    let head = match &t.target {
        Some(_) => format!("{prefix}{name}: "),
        None => format!("{prefix}{name}"),
    };
    let head_w = visible_width(&prefix_glyph)
        + t.name.chars().count()
        + if t.target.is_some() { 2 } else { 0 };
    let body = t.target.as_ref().map(|raw| {
        let (truncator, ideal) = target_strategy_for(&t.name);
        // Defensive `sanitize_single_line` — `extract_target` already
        // sanitises new tool events, but session caches written before
        // the sanitiser landed can carry literal `\n` in `target`. CC's
        // statusline parser splits stdout by `\n` into separate rows;
        // an unsanitised target would leak an extra row into our frame.
        let safe = sanitize_single_line(raw).into_owned();
        CellBody {
            raw: safe,
            truncator,
            min_width: 8,
            ideal_width: ideal,
            color: p.secondary.clone(),
        }
    });
    Cell {
        head,
        head_w,
        body,
        tail: vec![],
        priority: CellPriority::Required,
    }
}

/// Render one running-tool cell as a complete inline string (already
/// colourised, target already truncated to its strategy's `ideal` width).
///
/// **Transitional**: this bypasses the row-width budgeter, so callers
/// must guarantee the assembled row fits their pane (cluster layouts
/// currently violate this on long Bash commands — see Step 2 of the
/// cell migration). Brief-mode tape uses this safely because the cells
/// are tiny; detail-mode tape exposes the bug. New callers should prefer
/// `build_recent_tool_cell` + `pack_with_separator`.
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
    fn strategy_table_known_tools() {
        assert_eq!(target_strategy_for("Read").0, TruncationStrategy::KeepTail);
        assert_eq!(target_strategy_for("Bash").0, TruncationStrategy::KeepHead);
        assert_eq!(target_strategy_for("Glob").0, TruncationStrategy::KeepHead);
        assert_eq!(
            target_strategy_for("WebFetch").0,
            TruncationStrategy::KeepMiddle
        );
        assert_eq!(
            target_strategy_for("UnknownTool").0,
            TruncationStrategy::KeepHead
        );
    }

    #[test]
    fn cell_with_target_has_required_priority_and_min_8() {
        let p = aurora_marker_palette();
        let cell =
            build_recent_tool_cell(&t("Bash", Some("cargo test")), GlyphMode::Ascii, &p, false);
        assert_eq!(cell.priority, CellPriority::Required);
        let body = cell.body.as_ref().expect("body present");
        assert_eq!(body.min_width, 8);
        // Bash strategy is KeepHead at 40 → ideal_width should match.
        assert_eq!(body.ideal_width, 40);
    }

    #[test]
    fn cell_without_target_has_no_body() {
        let p = aurora_marker_palette();
        let cell = build_recent_tool_cell(&t("EnterPlanMode", None), GlyphMode::Ascii, &p, false);
        assert!(cell.body.is_none(), "no target → no body");
        // Head ends with the bare name, no trailing ": ".
        assert!(cell.head.ends_with("EnterPlanMode"));
    }

    #[test]
    fn cell_body_sanitises_multiline_targets() {
        // Defensive: stale session caches may carry literal `\n` in target.
        let p = aurora_marker_palette();
        let multi = "rg --multiline\n  --type rust\n  'pattern' src/";
        let cell = build_recent_tool_cell(&t("Bash", Some(multi)), GlyphMode::Ascii, &p, false);
        let body = cell.body.expect("body present");
        assert!(
            !body.raw.contains('\n'),
            "raw should be single-line: {:?}",
            body.raw
        );
        assert!(!body.raw.contains('\r'));
        assert!(!body.raw.contains('\t'));
    }

    #[test]
    fn inline_with_target_uses_colon_separator() {
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
    fn inline_no_target_drops_separator_and_target() {
        let p = aurora_marker_palette();
        let s = format_recent_tool_inline(&t("EnterPlanMode", None), GlyphMode::Ascii, &p, false);
        assert!(
            s.ends_with("EnterPlanMode"),
            "no trailing colon/target: {s:?}"
        );
        assert!(!s.contains(": "));
    }

    #[test]
    fn inline_long_bash_target_uses_keep_head_truncation() {
        let p = aurora_marker_palette();
        let cmd = "cargo test --release --no-default-features --features experimental_quota";
        let s = format_recent_tool_inline(&t("Bash", Some(cmd)), GlyphMode::Ascii, &p, false);
        assert!(s.contains("Bash: cargo test"), "verb survived: {s:?}");
        assert!(s.contains('…'), "long tail truncated: {s:?}");
    }

    #[test]
    fn inline_multi_line_target_collapsed_to_single_line() {
        let p = aurora_marker_palette();
        let multi = "rg --multiline\\\n  --type rust\\\n  'pattern' src/";
        let s = format_recent_tool_inline(&t("Bash", Some(multi)), GlyphMode::Ascii, &p, false);
        assert!(!s.contains('\n'), "leak newline: {s:?}");
        assert!(!s.contains('\r'), "leak CR: {s:?}");
        assert!(!s.contains('\t'), "leak tab: {s:?}");
    }
}
