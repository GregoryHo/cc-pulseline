//! Flightstrip — v2 dense 2-row layout for narrow IDE statuslines.
//!
//! Output rows:
//!   L1  identity headline + CTX% + gauge + total cost
//!   L2  sparkline + activity ticker (tools tape, agents, todos, quota)
//!
//! Width handling:
//!   ≥ 110   full
//!   90..110 drop quota cluster
//!   70..90  drop sparkline; gauge → 6 cells; drop cost from L1
//!   < 70    single row only (identity + pct + cost)

use crate::config::RenderConfig;
use crate::render::color::{colorize, ThemePalette};

use super::shared;

const FULL_GAUGE_WIDTH: usize = 12;
const NARROW_GAUGE_WIDTH: usize = 6;

pub fn render(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<String> {
    // Unknown width → assume 100 (flightstrip's intended bracket midpoint).
    let width = config.terminal_width.unwrap_or(100);

    if width < 70 {
        return vec![shared::degraded_single_row(frame, config, p)];
    }

    let mut lines: Vec<String> = Vec::with_capacity(2);
    lines.push(strip_l1(frame, config, p, width));
    let l2 = strip_l2(frame, config, p, width);
    if !l2.is_empty() {
        lines.push(l2);
    }
    lines
}

fn strip_l1(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    width: usize,
) -> String {
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();
    parts.push(shared::identity_headline(&frame.line1, config, p));

    let pct = frame.line3.context_used_percentage.unwrap_or(0);
    let pct_color = p.color_for_ctx_pct(pct, frame.line3.context_window_size);
    let pct_str = colorize(&format!("{pct}%"), pct_color, color);
    let gauge_w = if width >= 90 {
        FULL_GAUGE_WIDTH
    } else {
        NARROW_GAUGE_WIDTH
    };
    let gauge = crate::render::widgets::gauge::render(pct, gauge_w, config.glyph_mode, p, color);
    parts.push(format!("{pct_str} {gauge}"));

    if width >= 90 && config.show_cost {
        parts.push(shared::cost_text_only(&frame.line3, p, color));
    }

    parts.join("  ")
}

fn strip_l2(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    width: usize,
) -> String {
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();

    if width >= 90 && shared::sparkline_enabled(config) {
        let spark = shared::ctx_sparkline(&frame.ctx_history, config.glyph_mode, p, color);
        parts.push(spark);
    }

    let ticker = shared::activity_ticker(frame, config, p);
    if !ticker.is_empty() {
        parts.push(ticker);
    }

    if width >= 110 && config.show_quota && config.show_quota_five_hour {
        let q = shared::quota_text_cell(&frame.quota, p, color);
        if !q.is_empty() {
            parts.push(q);
        }
    }

    parts.join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RenderConfig;
    use crate::render::color::resolve_palette;
    use crate::types::RenderFrame;

    fn frame_basic() -> RenderFrame {
        let mut f = RenderFrame::default();
        f.line1.model = "Opus 4.7".to_string();
        f.line1.git_branch = "main".to_string();
        f.line1.project_path = "~/cc-pulseline".to_string();
        f.line3.context_window_size = Some(200_000);
        f.line3.context_used_percentage = Some(43);
        f.line3.total_cost_usd = Some(3.50);
        f.ctx_history = vec![20, 30, 40, 43];
        f
    }

    fn cfg(width: usize) -> RenderConfig {
        RenderConfig {
            color_enabled: false,
            terminal_width: Some(width),
            palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
            ..RenderConfig::default()
        }
    }

    #[test]
    fn flightstrip_two_rows_at_full_width() {
        let f = frame_basic();
        let c = cfg(120);
        let lines = render(&f, &c, &c.palette.clone());
        assert!(!lines.is_empty());
        assert!(lines[0].contains("Opus 4.7"));
        assert!(lines[0].contains("43%"));
        assert!(lines[0].contains("$3.50"));
    }

    #[test]
    fn flightstrip_drops_cost_below_90_cols() {
        let f = frame_basic();
        let c = cfg(80);
        let lines = render(&f, &c, &c.palette.clone());
        assert!(!lines[0].contains("$3.50"));
        assert!(lines[0].contains("43%"));
    }

    #[test]
    fn flightstrip_collapses_below_70_cols() {
        let f = frame_basic();
        let c = cfg(60);
        let lines = render(&f, &c, &c.palette.clone());
        assert_eq!(lines.len(), 1);
    }
}
