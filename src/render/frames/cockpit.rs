//! Cockpit — v2 default layout (3 rows, instrument cluster).
//!
//! See `designs/statusline-v2-redesign.md` for the visual contract.
//!
//! Output rows:
//!   L1  identity headline + right-justified CTX pill
//!   L2  *optional* config row — only when `config_row_enabled(...)` is true
//!   L3  cluster: CTX gauge | sparkline | TOK rate | cost arc | quota
//!   L4  activity ticker (tools / agents / todos)
//!
//! Width handling within the cluster (one row, several cells):
//!   ≥ 120 cols  full cluster
//!   100..120    drop quota text
//!   80..100     drop sparkline + cost arc → gauge + pct + cost text only
//!   < 80        collapse to single CTX line; activity row dropped too

use crate::config::RenderConfig;
use crate::render::color::ThemePalette;
use crate::render::pane::LayoutStyle;

use super::shared;

pub fn render(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<String> {
    // Unknown width → assume 120 (the cockpit's design midpoint). usize::MAX
    // here would blow up `" ".repeat(pad)` in headline_with_pill below.
    // Clamp to `pane_max_width` so the cluster row never grows past the
    // user's chosen ceiling on wide terminals.
    let width = config
        .terminal_width
        .unwrap_or(120)
        .min(config.pane_max_width);

    if width < 80 {
        return vec![shared::degraded_single_row(frame, config, p)];
    }

    let mut lines: Vec<String> = Vec::with_capacity(4);
    lines.push(headline_with_pill(frame, config, p, width));

    if shared::config_row_enabled(config) {
        let row = shared::config_row(frame, config, p, width);
        if !row.is_empty() {
            lines.push(row);
        }
    }

    lines.push(cluster_row(frame, config, p, width));

    let ticker = shared::activity_ticker(frame, config, p, width);
    if !ticker.is_empty() {
        lines.push(ticker);
    }

    lines
}

fn headline_with_pill(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    width: usize,
) -> String {
    let head = shared::identity_headline(&frame.line1, config, p);
    let pill = shared::ctx_pill(&frame.line3, p, config.color_enabled);
    if pill.is_empty() {
        return head;
    }

    // Skip the pill when right-alignment would crowd it against the headline;
    // the cluster row below still shows the same pct.
    use crate::render::color::visible_width;
    let head_w = visible_width(&head);
    let pill_w = visible_width(&pill);
    if head_w + pill_w + 4 > width {
        return head;
    }
    let pad = width.saturating_sub(head_w + pill_w);
    format!("{head}{}{pill}", " ".repeat(pad.max(2)))
}

fn cluster_row(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    width: usize,
) -> String {
    let color = config.color_enabled;
    let mut cells: Vec<String> = Vec::new();

    // Width-keyed gauge sizing for both CTX and quota — see
    // `shared::gauge_widths_for` for the full breakpoint table. Keeping the
    // mapping in one place is the whole point.
    let (gauge_w, quota_bar_w) = shared::gauge_widths_for(LayoutStyle::Cockpit, width);
    // Dispatch CTX rendering through the visual spec — the layout supplies
    // its preferred sizing (gauge_w) but the user's `context_visual` config
    // chooses which widgets to actually render.
    let mut ctx_spec = config.effective_context_visual().to_string();
    if width < 100 {
        // Below the cluster's full width budget, drop sparkline if it was
        // requested — the eye needs cells for the gauge first. Replicates
        // the previous hardcoded `width >= 100` gate.
        ctx_spec = ctx_spec
            .split('+')
            .filter(|w| w.trim() != "sparkline")
            .collect::<Vec<_>>()
            .join("+");
    }
    let ctx_cell = shared::render_context_visual(
        &ctx_spec,
        &frame.line3,
        &frame.ctx_history,
        gauge_w,
        config.glyph_mode,
        p,
        color,
    );
    if !ctx_cell.is_empty() {
        cells.push(ctx_cell);
    }

    if config.show_tokens {
        cells.push(shared::token_rate_cell(
            &frame.line3,
            frame.line3.output_speed_toks_per_sec,
            p,
            color,
        ));
    }

    if config.show_cost {
        // Below the cluster's full-width budget, force "text" — the arc adds
        // a glyph that's not worth its column on narrow renders. Otherwise
        // honour the user's `cost_visual` (defaults to "text+arc").
        let cost_spec = if width >= 100 {
            config.effective_cost_visual().to_string()
        } else {
            "text".to_string()
        };
        let cell =
            shared::render_cost_visual(&cost_spec, &frame.line3, config.glyph_mode, p, color);
        if !cell.is_empty() {
            cells.push(cell);
        }
    }

    if width >= 120 && config.show_quota {
        let quota_spec = config.effective_quota_visual();
        if config.show_quota_five_hour {
            let q = shared::render_quota_visual(
                quota_spec,
                "5h ",
                frame.quota.five_hour_pct,
                frame.quota.five_hour_reset_minutes,
                quota_bar_w,
                config.glyph_mode,
                p,
                color,
            );
            if !q.is_empty() {
                cells.push(q);
            }
        }
        if config.show_quota_seven_day {
            let q = shared::render_quota_visual(
                quota_spec,
                "7d ",
                frame.quota.seven_day_pct,
                frame.quota.seven_day_reset_minutes,
                quota_bar_w,
                config.glyph_mode,
                p,
                color,
            );
            if !q.is_empty() {
                cells.push(q);
            }
        }
    }

    cells.join("   ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RenderConfig;
    use crate::render::color::resolve_palette;
    use crate::types::RenderFrame;

    fn frame_canonical() -> RenderFrame {
        let mut f = RenderFrame::default();
        f.line1.model = "Opus 4.7".to_string();
        f.line1.git_branch = "feat/status-pane".to_string();
        f.line1.git_dirty = true;
        f.line1.git_ahead = 3;
        f.line1.project_path = "~/cc-pulseline".to_string();
        f.line3.context_window_size = Some(200_000);
        f.line3.context_used_percentage = Some(43);
        f.line3.input_tokens = Some(1000);
        f.line3.output_tokens = Some(2000);
        f.line3.total_cost_usd = Some(3.50);
        f.line3.total_duration_ms = Some(60_000 * 30);
        f.line3.output_speed_toks_per_sec = Some(1200.0);
        f.ctx_history = vec![10, 20, 30, 35, 40, 43];
        f
    }

    fn cfg() -> RenderConfig {
        // v2 default: hide config row unless caller flips one of these on.
        RenderConfig {
            color_enabled: false,
            terminal_width: Some(140),
            palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
            show_claude_md: false,
            show_rules: false,
            show_memory: false,
            show_hooks: false,
            show_mcp: false,
            show_skills: false,
            show_plugins: false,
            ..RenderConfig::default()
        }
    }

    #[test]
    fn cockpit_renders_three_rows_when_no_config_toggles() {
        let f = frame_canonical();
        let mut c = cfg();
        c.show_claude_md = false;
        c.show_rules = false;
        c.show_memory = false;
        c.show_hooks = false;
        c.show_mcp = false;
        c.show_skills = false;
        c.show_plugins = false;
        let lines = render(&f, &c, &c.palette.clone());
        // identity + cluster (no activity, no config row)
        assert!(
            lines.len() >= 2,
            "expected >=2 rows in default cockpit, got {lines:?}"
        );
        assert!(lines[0].contains("Opus 4.7"));
        assert!(lines[0].contains("feat/status-pane"));
    }

    #[test]
    fn cockpit_inserts_config_row_when_any_toggle_enabled() {
        let f = frame_canonical();
        let mut c = cfg();
        c.show_claude_md = true; // any one suffices
        c.show_rules = false;
        c.show_memory = false;
        c.show_hooks = false;
        c.show_mcp = false;
        c.show_skills = false;
        c.show_plugins = false;
        let lines = render(&f, &c, &c.palette.clone());
        // L2 row no longer carries a `CFG ` prefix (each segment has its
        // own icon + count + noun, so the label was redundant). The row
        // is identified by its content — `CLAUDE.md` here.
        assert!(
            lines.iter().any(|l| l.contains("CLAUDE.md")),
            "expected config row when a toggle is on, got {lines:?}"
        );
    }

    #[test]
    fn cockpit_collapses_to_single_row_below_80_cols() {
        let f = frame_canonical();
        let mut c = cfg();
        c.terminal_width = Some(70);
        let lines = render(&f, &c, &c.palette.clone());
        assert_eq!(lines.len(), 1, "expected single-row degraded mode");
        assert!(lines[0].contains("43%"));
        assert!(lines[0].contains("$3.50"));
    }

    #[test]
    fn cockpit_drops_quota_below_120_cols() {
        let mut f = frame_canonical();
        f.quota = crate::types::QuotaMetrics {
            five_hour_pct: Some(75.0),
            five_hour_reset_minutes: Some(120),
            seven_day_pct: None,
            seven_day_reset_minutes: None,
        };
        let mut c = cfg();
        c.terminal_width = Some(110);
        let lines = render(&f, &c, &c.palette.clone());
        let cluster = &lines[1]; // L1 identity, L2 cluster (no config row)
        assert!(
            !cluster.contains("5h "),
            "quota should be dropped at 110 cols: {cluster:?}"
        );
    }

    #[test]
    fn cockpit_includes_quota_at_120_cols() {
        let mut f = frame_canonical();
        f.quota = crate::types::QuotaMetrics {
            five_hour_pct: Some(75.0),
            five_hour_reset_minutes: Some(120),
            seven_day_pct: None,
            seven_day_reset_minutes: None,
        };
        let mut c = cfg();
        c.terminal_width = Some(140);
        c.show_quota = true; // not on by default in RenderConfig
        let lines = render(&f, &c, &c.palette.clone());
        let cluster = &lines[1];
        assert!(
            cluster.contains("5h "),
            "quota should appear at >=120 cols: {cluster:?}"
        );
    }
}
