//! `rail` v2 — three grouped Powerline rows (identity · context · quota).
//!
//! Each row is a two-cluster `render_bar`: a left identity/pressure cluster on
//! the gray ink ramp, and a right **headline** cluster — the one value that
//! row is about, always filled (`Tint`) and band-coloured. The headline left
//! edges align across rows (shared axis, like `budgets`).
//!
//! Two colour channels, deliberately asymmetric (the anti-rainbow gate):
//! - **Headlines** (cost on row 1, 7d on row 3) are *always* filled and
//!   band-coloured — the row's hero value, not an alarm.
//! - **Left `ink` flags** (effort / ctx / git-dirty) light *only* past
//!   threshold. A calm session is a near-monochrome bar with one or two filled
//!   headlines and zero left flags.
//!
//! Height ladder (reuses `max_total_lines`): 3 rows → 2 (drop quota) → the v1
//! single fused bar (`build_fused_row`), so the dense one-liner never bit-rots
//! and the shortest-footprint user still gets it. Quota drops when
//! `!quota.has_data()`. See `designs/rail-anchor-grouped-rows.md`.

use crate::config::RenderConfig;
use crate::render::color::{extract_ansi_code, visible_width, ThemePalette};
use crate::render::fmt::{burn_rate_per_hour, format_number, format_reset_duration};
use crate::render::frames::powerline::{self, RampLevel, Segment};
use crate::render::icons::{
    ICON_CONTEXT, ICON_EFFORT, ICON_GIT, ICON_MODEL, ICON_PROJECT, ICON_QUOTA, ICON_TOKEN_OUTPUT,
    ICON_VERSION,
};
use crate::render::layout;
use crate::render::pane::LayoutStyle;
use crate::types::{Line1Metrics, RenderFrame};

const CTX_TINT_AT: u64 = 55; // first ctx_marks() mark
const QUOTA_TINT_AT: f64 = 50.0; // first quota mark

/// Resolve a palette role escape to its 256 index (for `ink` / `Tint` codes).
fn code(escape: &str) -> u8 {
    extract_ansi_code(escape).unwrap_or(0)
}

/// Droppable identity-row cells, lowest priority first. model never drops
/// (the head); ctx and the headlines on rows 2/3 are never on this list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row1Cell {
    Version,
    Cwd,
    Git,
    Effort,
}

const ROW1_DROP: [Row1Cell; 4] = [
    Row1Cell::Version,
    Row1Cell::Cwd,
    Row1Cell::Git,
    Row1Cell::Effort,
];

pub fn render(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> Vec<String> {
    // Below min_width a connected bar can't read — bypass to flat `none`.
    if let Some(w) = config.terminal_width {
        if w < config.pane_min_width {
            return fallback_to_none(frame, config);
        }
    }

    let tier = powerline::tier(config);
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    // Height ladder. `None` → unlimited → 3 rows; the bottom rung is the v1 bar.
    let budget = config.max_total_lines.unwrap_or(3).max(1);
    if budget == 1 {
        return vec![build_fused_row(frame, config, palette)];
    }

    // Active groups (a filterable list, so a future `rail_groups` trim stays
    // possible). Identity is rebuilt inside its width-fit loop; context/quota
    // are fixed (their left cell — ctx / 5h — never drops). Quota drops lazily
    // when there's no Pro/Max data.
    let (_, id_right) = identity_clusters(frame, palette, &[]);
    let (ctx_l, ctx_r) = context_clusters(frame, palette);
    let quota = (budget >= 3 && frame.quota.has_data()).then(|| quota_clusters(frame, palette));

    // Shared headline axis: derived max right-cluster width (never a literal),
    // so every row's headline left-edge lands at `target - headline_col`.
    let mut headline_col = powerline::right_cluster_width(&id_right, tier, mode, palette)
        .max(powerline::right_cluster_width(&ctx_r, tier, mode, palette));
    if let Some((_, q_r)) = &quota {
        headline_col = headline_col.max(powerline::right_cluster_width(q_r, tier, mode, palette));
    }
    let bar = |left: &[Segment], right: &[Segment]| {
        powerline::render_bar(
            left,
            right,
            config.terminal_width,
            Some(headline_col),
            tier,
            mode,
            color,
            palette,
        )
    };

    let mut out: Vec<String> = vec![
        fit_identity(frame, config, palette, headline_col),
        bar(&ctx_l, &ctx_r),
    ];
    if let Some((q_l, q_r)) = &quota {
        out.push(bar(q_l, q_r));
    }
    // Drop blank rows (e.g. the context row before the first API call) — lazy,
    // like quota; never emit an empty line.
    out.retain(|row| visible_width(row) > 0);
    out
}

/// Render the identity row, dropping low-priority left cells under width
/// pressure until it fits the terminal (model + the cost headline survive).
fn fit_identity(
    frame: &RenderFrame,
    config: &RenderConfig,
    palette: &ThemePalette,
    headline_col: usize,
) -> String {
    let tier = powerline::tier(config);
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let mut dropped: Vec<Row1Cell> = Vec::new();
    loop {
        let (left, right) = identity_clusters(frame, palette, &dropped);
        let row = powerline::render_bar(
            &left,
            &right,
            config.terminal_width,
            Some(headline_col),
            tier,
            mode,
            color,
            palette,
        );
        match config.terminal_width {
            None => return row,
            Some(w) if visible_width(&row) <= w => return row,
            Some(_) => match ROW1_DROP.iter().find(|c| !dropped.contains(c)) {
                Some(next) => dropped.push(*next),
                None => return row, // model + headline only; render as-is
            },
        }
    }
}

// ── Row 1 · identity ──────────────────────────────────────────────────────
// model is the bar's head (a `Tint`); effort / git carry `ink` flags only
// past threshold; cost is the always-filled headline, banded by burn rate.
fn identity_clusters(
    frame: &RenderFrame,
    palette: &ThemePalette,
    dropped: &[Row1Cell],
) -> (Vec<Segment>, Vec<Segment>) {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let mut left: Vec<Segment> = Vec::new();

    if !l1.model.is_empty() {
        left.push(Segment::tint(
            ICON_MODEL,
            "M:",
            l1.model.clone(),
            code(&palette.stable_blue),
        ));
    }
    if !dropped.contains(&Row1Cell::Effort) {
        if let Some(level) = &l1.effort_level {
            if powerline::effort_tints(level) {
                left.push(Segment::ramp_ink(
                    ICON_EFFORT,
                    "E:",
                    level.clone(),
                    RampLevel::Base,
                    code(palette.color_for_effort_level(level)),
                ));
            } else {
                left.push(Segment::ramp(
                    ICON_EFFORT,
                    "E:",
                    level.clone(),
                    RampLevel::Base,
                ));
            }
        }
    }
    if !dropped.contains(&Row1Cell::Version) && !l1.claude_code_version.is_empty() {
        left.push(Segment::ramp(
            ICON_VERSION,
            "v",
            format!("v{}", l1.claude_code_version),
            RampLevel::Base,
        ));
    }
    if !dropped.contains(&Row1Cell::Cwd) && !l1.project_path.is_empty() {
        left.push(Segment::ramp(
            ICON_PROJECT,
            "",
            basename(&l1.project_path),
            RampLevel::Base,
        ));
    }
    if !dropped.contains(&Row1Cell::Git) && l1.has_git_branch() {
        let text = git_text(l1);
        if l1.git_dirty {
            left.push(Segment::ramp_ink(
                ICON_GIT,
                "G:",
                text,
                RampLevel::Base,
                code(&palette.alert_orange),
            ));
        } else {
            left.push(Segment::ramp(ICON_GIT, "G:", text, RampLevel::Base));
        }
    }

    // Headline: cost, always filled, banded by burn rate.
    let mut right: Vec<Segment> = Vec::new();
    if let Some(cost) = l3.total_cost_usd {
        let band =
            code(palette.color_for_burn_rate(burn_rate_per_hour(cost, l3.total_duration_ms)));
        right.push(Segment::tint("", "", format!("${cost:.2}"), band));
    }
    (left, right)
}

// ── Row 2 · context ─────────────────────────────────────────────────────────
// ctx carries the ink flag (≥55); the token headline has no band, so it stays
// on the ramp (tokens aren't a threshold value — don't invent one).
fn context_clusters(frame: &RenderFrame, palette: &ThemePalette) -> (Vec<Segment>, Vec<Segment>) {
    let l3 = &frame.line3;
    let mut left: Vec<Segment> = Vec::new();

    if let Some(pct) = l3.context_used_percentage {
        let text = match l3.context_window_size {
            Some(size) => {
                let used = size.saturating_mul(pct) / 100;
                format!("{pct}% {}/{}", format_number(used), format_number(size))
            }
            None => format!("{pct}%"),
        };
        if pct >= CTX_TINT_AT {
            left.push(Segment::ramp_ink(
                ICON_CONTEXT,
                "CTX",
                text,
                RampLevel::Base,
                code(palette.color_for_ctx_pct(pct)),
            ));
        } else {
            left.push(Segment::ramp(ICON_CONTEXT, "CTX", text, RampLevel::Base));
        }
    }

    // Headline: tokens (no band — stays ramp) + cache read total.
    let mut right: Vec<Segment> = Vec::new();
    if l3.input_tokens.is_some() || l3.output_tokens.is_some() {
        let in_v = l3
            .input_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into());
        let out_v = l3
            .output_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into());
        right.push(Segment::ramp(
            ICON_TOKEN_OUTPUT,
            "TOK",
            format!("↓{in_v} ↑{out_v}"),
            RampLevel::Raised,
        ));
    }
    if let Some(cache) = l3.cache_read_tokens {
        right.push(Segment::ramp(
            "",
            "",
            format!("CACHE {}", format_number(cache)),
            RampLevel::Base,
        ));
    }
    (left, right)
}

// ── Row 3 · quota ───────────────────────────────────────────────────────────
// 5h carries the ink flag (≥50); 7d is the always-filled headline, banded.
fn quota_clusters(frame: &RenderFrame, palette: &ThemePalette) -> (Vec<Segment>, Vec<Segment>) {
    let q = &frame.quota;
    let mut left: Vec<Segment> = Vec::new();

    if let Some(pct) = q.five_hour_pct {
        let pct_u = pct.round() as u64;
        let reset = q
            .five_hour_reset_minutes
            .map(|m| format!(" {}", format_reset_duration(m)))
            .unwrap_or_default();
        let text = format!("5H {pct_u}%{reset}");
        if pct >= QUOTA_TINT_AT {
            left.push(Segment::ramp_ink(
                ICON_QUOTA,
                "5H",
                text,
                RampLevel::Base,
                code(palette.color_for_quota_pct(pct)),
            ));
        } else {
            left.push(Segment::ramp(ICON_QUOTA, "5H", text, RampLevel::Base));
        }
    }

    let mut right: Vec<Segment> = Vec::new();
    if let Some(pct) = q.seven_day_pct {
        let pct_u = pct.round() as u64;
        let reset = q
            .seven_day_reset_minutes
            .map(|m| format!(" {}", format_reset_duration(m)))
            .unwrap_or_default();
        right.push(Segment::tint(
            ICON_QUOTA,
            "7D",
            format!("7D {pct_u}%{reset}"),
            code(palette.color_for_quota_pct(pct)),
        ));
    }
    (left, right)
}

// ── Bottom rung: the v1 single fused bar ────────────────────────────────────
// Identity → pressure on one line, `model · effort · cwd · git · ctx | cost ·
// version`. Preserved as the height-1 rung so the dense one-liner survives.
// (The git `~N` flag now rides `ink` instead of v1's spliced escape — for a
// clean tree this is byte-identical to v1.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FusedCell {
    Version,
    Cost,
    Cwd,
    Git,
    Effort,
}

const FUSED_DROP_ORDER: [FusedCell; 5] = [
    FusedCell::Version,
    FusedCell::Cost,
    FusedCell::Cwd,
    FusedCell::Git,
    FusedCell::Effort,
];

fn build_fused_row(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> String {
    let tier = powerline::tier(config);
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let mut dropped: Vec<FusedCell> = Vec::new();
    loop {
        let (left, right) = build_fused_clusters(frame, palette, &dropped);
        let row = powerline::render_bar(
            &left,
            &right,
            config.terminal_width,
            None,
            tier,
            mode,
            color,
            palette,
        );
        match config.terminal_width {
            None => return row,
            Some(w) if visible_width(&row) <= w => return row,
            Some(_) => match FUSED_DROP_ORDER.iter().find(|c| !dropped.contains(c)) {
                Some(next) => dropped.push(*next),
                None => return row, // model + ctx only; render as-is
            },
        }
    }
}

fn build_fused_clusters(
    frame: &RenderFrame,
    palette: &ThemePalette,
    dropped: &[FusedCell],
) -> (Vec<Segment>, Vec<Segment>) {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let mut left: Vec<Segment> = Vec::new();
    let mut right: Vec<Segment> = Vec::new();

    if !l1.model.is_empty() {
        left.push(Segment::ramp(
            ICON_MODEL,
            "M:",
            l1.model.clone(),
            RampLevel::High,
        ));
    }
    if !dropped.contains(&FusedCell::Effort) {
        if let Some(level) = &l1.effort_level {
            if powerline::effort_tints(level) {
                left.push(Segment::tint(
                    ICON_EFFORT,
                    "E:",
                    level.clone(),
                    code(palette.color_for_effort_level(level)),
                ));
            } else {
                left.push(Segment::ramp(
                    ICON_EFFORT,
                    "E:",
                    level.clone(),
                    RampLevel::Raised,
                ));
            }
        }
    }
    if !dropped.contains(&FusedCell::Cwd) && !l1.project_path.is_empty() {
        left.push(Segment::ramp(
            ICON_PROJECT,
            "P:",
            basename(&l1.project_path),
            RampLevel::Base,
        ));
    }
    if !dropped.contains(&FusedCell::Git) && l1.has_git_branch() {
        let text = git_text(l1);
        if l1.git_dirty {
            left.push(Segment::ramp_ink(
                ICON_GIT,
                "G:",
                text,
                RampLevel::Raised,
                code(&palette.alert_orange),
            ));
        } else {
            left.push(Segment::ramp(ICON_GIT, "G:", text, RampLevel::Raised));
        }
    }
    if let Some(pct) = l3.context_used_percentage {
        let text = format!("{pct}%");
        if pct >= CTX_TINT_AT {
            left.push(Segment::tint(
                ICON_CONTEXT,
                "CTX:",
                text,
                code(palette.color_for_ctx_pct(pct)),
            ));
        } else {
            left.push(Segment::ramp(ICON_CONTEXT, "CTX:", text, RampLevel::Raised));
        }
    }
    if !dropped.contains(&FusedCell::Cost) {
        if let Some(cost) = l3.total_cost_usd {
            right.push(Segment::ramp(
                "",
                "",
                format!("${cost:.2}"),
                RampLevel::Base,
            ));
        }
    }
    if !dropped.contains(&FusedCell::Version) && !l1.claude_code_version.is_empty() {
        right.push(Segment::ramp(
            ICON_VERSION,
            "CC:",
            format!("v{}", l1.claude_code_version),
            RampLevel::Base,
        ));
    }
    (left, right)
}

// ── shared helpers ──────────────────────────────────────────────────────────

/// The git cell text: `branch +added ~modified` + ` *` when dirty. Plain text
/// now — the dirty flag rides the segment's `ink` channel (retired the v1
/// spliced-escape hack), so the ramp fill persists and colour is first-class.
fn git_text(l1: &Line1Metrics) -> String {
    let mut t = l1.git_branch.clone();
    if l1.git_added > 0 {
        t.push_str(&format!(" +{}", l1.git_added));
    }
    if l1.git_modified > 0 {
        t.push_str(&format!(" ~{}", l1.git_modified));
    }
    if l1.git_dirty {
        t.push_str(" *");
    }
    t
}

fn basename(path: &str) -> String {
    let base = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path);
    if base.is_empty() {
        path.to_string()
    } else {
        base.to_string()
    }
}

/// Narrow-terminal escape hatch: flat `none` is content-sized and never
/// overflows. `render_frame` already subtracted `pane_cc_margin`; restore it so
/// the re-entrant call doesn't subtract twice.
fn fallback_to_none(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = LayoutStyle::None;
    if let Some(w) = shrunk.terminal_width {
        shrunk.terminal_width = Some(w + shrunk.pane_cc_margin);
    }
    layout::render_frame(frame, &shrunk)
}
