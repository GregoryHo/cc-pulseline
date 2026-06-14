//! Badge — discrete two-tone "pill" badges with background fill.
//!
//! The renderer's only background-filled layout. Each data domain is a row
//! of filled, capped chips floating in whitespace, so the eye reads discrete
//! objects rather than a pipe-delimited line of text. Three tiers are picked
//! automatically from `(color_enabled, glyph_mode)` — one coherent design,
//! three faces:
//!
//! ```text
//!   color + Icon   → DRIFT    rounded powerline caps  +  bg fill
//!   color + Ascii  → STENCIL  capless slabs fused by the 236→238 bg step
//!   no color       → ASCII    [label value] plain brackets, no fill
//! ```
//!
//! Three rows, one domain each (each its own visible group):
//!
//! ```text
//!    MDL opus 4.8   EFT high   CC 2.1.119      ~/proj    main *↑2
//!    TOK ↓ 12.4k   ↑ 3.1k     CACHE 87%      CTX 43% · 86k/200k
//!    5H 62% · resets 2h08m       7D 19% · resets 5d
//! ```
//!
//! Color is restrained: grayscale everywhere, a single deep-teal accent
//! (ANSI 30) on exactly the budget + quota value halves, warm (173/130) only
//! when a window nears its limit. The scheme is module-local rather than a
//! `ThemePalette` tier — this layout is experimental; promote the codes to
//! palette fields if it graduates.
//!
//! Owns its full pipeline (dispatched directly from `layout::render_frame`,
//! like Ledger) because the bg-run / cap rhythm doesn't compose via
//! `apply_pane`. Width degradation is a 4-rung ladder (drop CTX used/total →
//! drop quota reset → cwd basename) that drops to `Compact` if even the
//! shrunk render overflows a known terminal width.

use crate::config::{GlyphMode, RenderConfig};
use crate::render::color::{bg_code, fg_code, visible_width, ThemePalette, RESET};
use crate::render::fmt::{format_number, format_reset_duration};
use crate::render::icons::ICON_GIT;
use crate::render::layout;
use crate::render::pane::LayoutStyle;
use crate::types::RenderFrame;

// ── Deep-teal badge scheme (ANSI 256, dark-bg). Module-local. ──
const LABEL_BG: u8 = 236;
const VALUE_BG: u8 = 238;
const LABEL_FG: u8 = 245;
const VALUE_FG: u8 = 252;
const ACCENT_BG: u8 = 30; // deep teal — the single cool accent
const ACCENT_FG: u8 = 231;
const WARN_BG: u8 = 173; // terracotta — alarm only
const CRIT_BG: u8 = 130;
const WARN_FG: u8 = 236;
const FLECK_FG: u8 = 173; // git dirty / ahead-behind, inline in a neutral pill

const CAP_L: &str = "\u{e0b6}"; // rounded left half-circle (Nerd Font)
const CAP_R: &str = "\u{e0b4}"; // rounded right half-circle

const GUTTER: &str = "  "; // between badges within a group
const MOAT: &str = "    "; // between sub-groups in a row

#[derive(Clone, Copy, PartialEq)]
enum Tier {
    Drift,
    Stencil,
    Ascii,
}

fn tier_for(config: &RenderConfig) -> Tier {
    if !config.color_enabled {
        Tier::Ascii
    } else if config.glyph_mode == GlyphMode::Icon {
        Tier::Drift
    } else {
        Tier::Stencil
    }
}

/// A value run: text plus its foreground color. All runs in one badge share
/// the badge's value background.
struct Run {
    text: String,
    fg: u8,
}

fn run(text: impl Into<String>, fg: u8) -> Run {
    Run {
        text: text.into(),
        fg,
    }
}

/// A single value run in the default value color.
fn plain(text: impl Into<String>) -> Vec<Run> {
    vec![run(text, VALUE_FG)]
}

/// Render one badge. Empty `label` → single-value pill (no label half).
fn badge(tier: Tier, label: &str, value: &[Run], value_bg: u8) -> String {
    if tier == Tier::Ascii {
        let mut s = String::from("[");
        if !label.is_empty() {
            s.push_str(label);
            s.push(' ');
        }
        for r in value {
            s.push_str(&r.text);
        }
        s.push(']');
        return s;
    }

    let caps = tier == Tier::Drift;
    let left_bg = if label.is_empty() { value_bg } else { LABEL_BG };
    let mut s = String::new();
    if caps {
        s.push_str(&fg_code(left_bg));
        s.push_str(CAP_L);
    }
    if !label.is_empty() {
        s.push_str(&bg_code(LABEL_BG));
        s.push_str(&fg_code(LABEL_FG));
        s.push(' ');
        s.push_str(label);
        s.push(' ');
    }
    s.push_str(&bg_code(value_bg));
    s.push(' ');
    for r in value {
        s.push_str(&fg_code(r.fg));
        s.push_str(&r.text);
    }
    s.push(' '); // trailing pad, still in value bg
    s.push_str(RESET);
    if caps {
        s.push_str(&fg_code(value_bg));
        s.push_str(CAP_R);
        s.push_str(RESET);
    }
    s
}

/// Accent badge: single value run in `afg` on `abg`.
fn accent(tier: Tier, label: &str, value: impl Into<String>, abg: u8, afg: u8) -> String {
    badge(tier, label, &[run(value, afg)], abg)
}

/// Join sub-groups: badges within a sub-group by GUTTER, sub-groups by MOAT.
fn join_row(subgroups: &[Vec<String>]) -> String {
    subgroups
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.join(GUTTER))
        .collect::<Vec<_>>()
        .join(MOAT)
}

/// CTX value-half colors: teal until critical (≥70%), then warm.
fn ctx_colors(pct: u64) -> (u8, u8) {
    if pct >= 70 {
        (WARN_BG, WARN_FG)
    } else {
        (ACCENT_BG, ACCENT_FG)
    }
}

/// Quota value-half colors: teal < 50, warn ≥ 50, critical ≥ 85.
fn quota_colors(pct: f64) -> (u8, u8) {
    if pct >= 85.0 {
        (CRIT_BG, WARN_FG)
    } else if pct >= 50.0 {
        (WARN_BG, WARN_FG)
    } else {
        (ACCENT_BG, ACCENT_FG)
    }
}

/// Width-degradation rungs, cheapest first. Each rung sheds one detail; the
/// last truncates the cwd to its basename. If even the final rung overflows
/// a known width, the layout drops to `Compact` (content-sized, never
/// overflows) — mirroring Ledger's narrow-terminal fallback.
#[derive(Clone, Copy)]
struct Detail {
    ctx_total: bool,   // CTX shows `· used/total`
    quota_reset: bool, // quota shows `· resets …`
    cwd_full: bool,    // cwd shows the full path vs basename
}

const DETAIL_LADDER: [Detail; 4] = [
    Detail {
        ctx_total: true,
        quota_reset: true,
        cwd_full: true,
    },
    Detail {
        ctx_total: false,
        quota_reset: true,
        cwd_full: true,
    },
    Detail {
        ctx_total: false,
        quota_reset: false,
        cwd_full: true,
    },
    Detail {
        ctx_total: false,
        quota_reset: false,
        cwd_full: false,
    },
];

pub fn render(frame: &RenderFrame, config: &RenderConfig, _p: &ThemePalette) -> Vec<String> {
    let t = tier_for(config);
    let icon = config.glyph_mode == GlyphMode::Icon;

    for detail in DETAIL_LADDER {
        let rows = build_rows(frame, config, t, icon, detail);
        match config.terminal_width {
            None => return rows,
            Some(w) if rows.iter().all(|r| visible_width(r) <= w) => return rows,
            _ => {}
        }
    }
    // Every rung still overflows the known width → drop to Compact.
    fallback_to_compact(frame, config)
}

fn build_rows(
    frame: &RenderFrame,
    config: &RenderConfig,
    t: Tier,
    icon: bool,
    detail: Detail,
) -> Vec<String> {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let q = &frame.quota;
    let mut rows: Vec<String> = Vec::with_capacity(3);

    // ── Row 1 · IDENTITY ──
    let mut head: Vec<String> = Vec::new();
    if config.show_model && !l1.model.is_empty() {
        head.push(badge(t, "MDL", &plain(&l1.model), VALUE_BG));
    }
    if config.show_effort {
        if let Some(e) = &l1.effort_level {
            head.push(badge(t, "EFT", &plain(e), VALUE_BG));
        }
    }
    if config.show_version && !l1.claude_code_version.is_empty() {
        head.push(badge(t, "CC", &plain(&l1.claude_code_version), VALUE_BG));
    }
    let mut place: Vec<String> = Vec::new();
    if config.show_project && !l1.project_path.is_empty() {
        place.push(badge(
            t,
            "",
            &plain(cwd_text(&l1.project_path, detail.cwd_full)),
            VALUE_BG,
        ));
    }
    if config.show_git && l1.has_git_branch() {
        let mut runs = vec![run(l1.git_branch.clone(), VALUE_FG)];
        let mut fleck = String::new();
        if l1.git_dirty {
            fleck.push('*');
        }
        if l1.git_ahead > 0 {
            fleck.push_str(&format!("↑{}", l1.git_ahead));
        }
        if l1.git_behind > 0 {
            fleck.push_str(&format!("↓{}", l1.git_behind));
        }
        if !fleck.is_empty() {
            runs.push(run(format!(" {fleck}"), FLECK_FG));
        }
        let label = if icon { ICON_GIT } else { "git" };
        place.push(badge(t, label, &runs, VALUE_BG));
    }
    rows.push(join_row(&[head, place]));

    // ── Row 2 · BUDGET (tokens · cache · context) ──
    let mut tok: Vec<String> = Vec::new();
    let mut cache: Vec<String> = Vec::new();
    if config.show_tokens {
        let arrow_in = if icon { "↓ " } else { "in " };
        let arrow_out = if icon { "↑ " } else { "out " };
        let in_v = l3
            .input_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into());
        let out_v = l3
            .output_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into());
        tok.push(badge(
            t,
            "TOK",
            &plain(format!("{arrow_in}{in_v}")),
            VALUE_BG,
        ));
        tok.push(badge(
            t,
            "",
            &plain(format!("{arrow_out}{out_v}")),
            VALUE_BG,
        ));
        match l3.cache_hit_pct() {
            Some(p) => cache.push(accent(
                t,
                "CACHE",
                format!("{}%", p.round() as u64),
                ACCENT_BG,
                ACCENT_FG,
            )),
            None => cache.push(badge(t, "CACHE", &plain("--"), VALUE_BG)),
        }
    }
    let mut ctx: Vec<String> = Vec::new();
    if config.show_context {
        match l3.context_used_percentage {
            Some(pct) => {
                let (abg, afg) = ctx_colors(pct);
                let value = match (detail.ctx_total, l3.context_window_size) {
                    (true, Some(total)) => {
                        let used = total.saturating_mul(pct) / 100;
                        format!("{pct}% · {}/{}", format_number(used), format_number(total))
                    }
                    _ => format!("{pct}%"),
                };
                ctx.push(accent(t, "CTX", value, abg, afg));
            }
            None => ctx.push(badge(t, "CTX", &plain("--"), VALUE_BG)),
        }
    }
    rows.push(join_row(&[tok, cache, ctx]));

    // ── Row 3 · QUOTA (5h · 7d) ──
    if config.show_quota {
        let mut q5: Vec<String> = Vec::new();
        let mut q7: Vec<String> = Vec::new();
        if config.show_quota_five_hour {
            q5.push(quota_badge(
                t,
                "5H",
                q.five_hour_pct,
                q.five_hour_reset_minutes,
                detail.quota_reset,
            ));
        }
        if config.show_quota_seven_day {
            q7.push(quota_badge(
                t,
                "7D",
                q.seven_day_pct,
                q.seven_day_reset_minutes,
                detail.quota_reset,
            ));
        }
        let row = join_row(&[q5, q7]);
        if !row.is_empty() {
            rows.push(row);
        }
    }

    rows
}

/// Full path, or just the last path component when `full` is false.
fn cwd_text(path: &str, full: bool) -> String {
    if full {
        return path.to_string();
    }
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

fn quota_badge(
    tier: Tier,
    label: &str,
    pct: Option<f64>,
    reset_min: Option<u64>,
    show_reset: bool,
) -> String {
    match pct {
        Some(p) => {
            let (abg, afg) = quota_colors(p);
            let reset = match (show_reset, reset_min) {
                (true, Some(m)) => format!(" · resets {}", format_reset_duration(m)),
                _ => String::new(),
            };
            accent(
                tier,
                label,
                format!("{}%{reset}", p.round() as u64),
                abg,
                afg,
            )
        }
        None => badge(tier, label, &plain("--"), VALUE_BG),
    }
}

/// Narrow-terminal escape hatch: Compact is content-sized and never
/// overflows. Restore `pane_cc_margin` first so the re-entrant `render_frame`
/// doesn't subtract it twice (same pattern as Ledger's console fallback).
fn fallback_to_compact(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = LayoutStyle::Compact;
    if let Some(w) = shrunk.terminal_width {
        shrunk.terminal_width = Some(w + shrunk.pane_cc_margin);
    }
    layout::render_frame(frame, &shrunk)
}
