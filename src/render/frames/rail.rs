//! `rail` v3 — three grouped Powerline rows (identity · usage · quota) with two
//! configurable dials: **colour budget** and **headline placement**.
//!
//! ```text
//!  identity │ model · effort · cwd · git              │ version
//!  usage    │ CTX% · tokens · cache                   │ $cost
//!  quota    │ 5H%                                     │ 7D%
//! ```
//!
//! Every cell is first classified by *kind* — `Headline` (the value the line is
//! about: model · cost · 7d), `Flag` (a live state that lights past threshold:
//! effort · ctx · 5h · git · cache), or `Context` (quiet structure: version ·
//! cwd · tokens). One classifier feeds the 3-row form *and* the fused bar, so
//! both honour the dials.
//!
//! **Colour budget** (`[layout] color_budget`) is a transform over those kinds:
//! - `signal` (default) — one reverse-video `Tint` per line (the headline);
//!   flags `ink` only past threshold; nothing else lit. The anti-rainbow stance.
//! - `vivid` — every headline + lit flag fills; context (and below-threshold
//!   flags) ride a raised ramp.
//! - `mono` — no fills at all: role-coloured text joined by thin `\u{e0b1}`
//!   ticks (`emit_mono_line`, bypasses `render_bar`'s seams).
//!
//! **Headline placement** (`[layout] headline`) routes a watch-value headline:
//! `column` (default) right-hugs it (rows share a right-edge axis); `inline`
//! folds it onto the end of the left cluster. The model is always left-anchored.
//! `mono` and the ASCII floor ignore placement (one flat run per row).
//!
//! Width: the bar is capped at `pane_max_width`. Height ladder
//! (`max_total_lines`): 3 → 2 (drop quota) → the single fused bar, which honours
//! `color_budget` too. Lazy-drops any empty row. Bands route through the
//! polarity-correct `color_for_*` helpers — no new palette fields or colour fns.
//! See `docs/layouts.md` (the `rail` section + the `color_budget` / `headline`
//! dials).

use crate::config::{ColorBudget, GlyphMode, Headline, RenderConfig};
use crate::render::color::{colorize, extract_ansi_code, fg_code, visible_width, ThemePalette};
use crate::render::fmt::{burn_rate_per_hour, format_number, format_reset_duration};
use crate::render::frames::powerline::{self, RampLevel, SeamTier, Segment};
use crate::render::icons::{
    glyph, ICON_CONTEXT, ICON_EFFORT, ICON_GIT, ICON_MODEL, ICON_PROJECT, ICON_QUOTA,
    ICON_TOKEN_OUTPUT, ICON_VERSION, PL_TICK,
};
use crate::render::layout;
use crate::render::pane::LayoutStyle;
use crate::types::{Line1Metrics, Line3Metrics, RenderFrame};

const CTX_TINT_AT: u64 = 55; // first ctx_marks() mark
const QUOTA_TINT_AT: f64 = 50.0; // first quota mark
/// Cache lights its EFFICIENCY band only on genuinely good reuse — the same
/// boundary `color_for_cache_hit_pct` uses for its green band. Below it the
/// cache cell stays a quiet Context ramp (a cold cache is never lit, never red).
const CACHE_FLAG_AT: f64 = 80.0;

/// Resolve a palette role escape to its 256 index (for `Tint` / ink codes).
fn code(escape: &str) -> u8 {
    extract_ansi_code(escape).unwrap_or(0)
}

// ── cell model ──────────────────────────────────────────────────────────────

/// What a rail cell *is*, independent of how the colour budget paints it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CellKind {
    /// The one value the line is about (model · cost · 7d). Always fills under
    /// signal/vivid; role-coloured text under mono.
    Headline,
    /// A live state that lights only past its threshold (effort · ctx · 5h ·
    /// git · cache). Inks past threshold under signal; fills past threshold
    /// under vivid; neutral otherwise.
    Flag,
    /// Quiet structure that never carries a band (version · cwd · tokens).
    Context,
}

/// A rail cell *before* the colour budget decides how it paints. `band` is the
/// resolved 256 role code (`None` = no band). `lit` = the flag is past its
/// threshold this frame. `prebaked` text already carries its own colour splices
/// (git's partial `~N`/`*` marks) and must never be re-coloured by the
/// transform — it always renders on a plain ramp / neutral mono base.
struct RailCell {
    icon: &'static str,
    ascii: &'static str,
    text: String,
    kind: CellKind,
    band: Option<u8>,
    lit: bool,
    prebaked: bool,
}

impl RailCell {
    fn headline(
        icon: &'static str,
        ascii: &'static str,
        text: impl Into<String>,
        band: u8,
    ) -> Self {
        Self {
            icon,
            ascii,
            text: text.into(),
            kind: CellKind::Headline,
            band: Some(band),
            lit: true,
            prebaked: false,
        }
    }

    fn flag(
        icon: &'static str,
        ascii: &'static str,
        text: impl Into<String>,
        band: u8,
        lit: bool,
    ) -> Self {
        Self {
            icon,
            ascii,
            text: text.into(),
            kind: CellKind::Flag,
            band: Some(band),
            lit,
            prebaked: false,
        }
    }

    fn context(icon: &'static str, ascii: &'static str, text: impl Into<String>) -> Self {
        Self {
            icon,
            ascii,
            text: text.into(),
            kind: CellKind::Context,
            band: None,
            lit: false,
            prebaked: false,
        }
    }

    /// A partial cell (git) whose `text` already carries its own splices.
    fn prebaked(icon: &'static str, ascii: &'static str, text: impl Into<String>) -> Self {
        Self {
            icon,
            ascii,
            text: text.into(),
            kind: CellKind::Flag,
            band: None,
            lit: false,
            prebaked: true,
        }
    }
}

/// The colour-budget transform: a classified cell → a `powerline::Segment`.
/// `mono` never reaches here (it uses `emit_mono_line`).
fn to_segment(c: &RailCell, budget: ColorBudget) -> Segment {
    // git-style partial cells carry their own splices — render plain, never
    // re-colour the whole cell.
    if c.prebaked {
        return Segment::ramp(c.icon, c.ascii, c.text.clone(), RampLevel::Base);
    }
    match budget {
        ColorBudget::Signal => match c.kind {
            CellKind::Headline => match c.band {
                Some(code) => Segment::tint(c.icon, c.ascii, c.text.clone(), code),
                None => Segment::ramp(c.icon, c.ascii, c.text.clone(), RampLevel::Base),
            },
            CellKind::Flag if c.lit => Segment::ramp_ink(
                c.icon,
                c.ascii,
                c.text.clone(),
                RampLevel::Base,
                c.band.unwrap_or(0),
            ),
            _ => Segment::ramp(c.icon, c.ascii, c.text.clone(), RampLevel::Base),
        },
        ColorBudget::Vivid => {
            // Headlines + lit flags fill; context + below-threshold flags ride
            // the raised ramp.
            let fills = matches!(c.kind, CellKind::Headline) || c.lit;
            match c.band.filter(|_| fills) {
                Some(code) => Segment::tint(c.icon, c.ascii, c.text.clone(), code),
                None => Segment::ramp(c.icon, c.ascii, c.text.clone(), RampLevel::Raised),
            }
        }
        ColorBudget::Mono => unreachable!("mono uses emit_mono_line, not render_bar"),
    }
}

fn to_segments(cells: &[RailCell], budget: ColorBudget) -> Vec<Segment> {
    cells.iter().map(|c| to_segment(c, budget)).collect()
}

/// Route a row's `(left, right)` cells per the headline dial. `column` keeps the
/// split (the headline right-hugs); `inline` folds the right cluster onto the
/// end of the left one (`render_bar` returns left-only when right is empty).
fn place(
    mut left: Vec<RailCell>,
    right: Vec<RailCell>,
    headline: Headline,
) -> (Vec<RailCell>, Vec<RailCell>) {
    match headline {
        Headline::Column => (left, right),
        Headline::Inline => {
            left.extend(right);
            (left, Vec::new())
        }
    }
}

/// Per-render constants shared by every grouped row (kept in one struct so the
/// row renderer stays under the argument-count lint).
struct RailCtx<'a> {
    budget: ColorBudget,
    headline: Headline,
    /// `true` only when `budget == Mono` AND the tier can carry fills (the ASCII
    /// floor has none, so mono there is identical to the floor).
    mono: bool,
    target: Option<usize>,
    tier: SeamTier,
    mode: GlyphMode,
    color: bool,
    palette: &'a ThemePalette,
}

/// Emit a row as role-coloured TEXT only — no fills, no seams. Cells join with a
/// thin powerline tick (` \u{e0b1} ` on the Powerline tier, ` · ` on Blocks, to
/// match `anchor`). Headlines + lit flags take their band colour; everything
/// else is neutral; git keeps its own `~N`/`*` splice. Only ever called on the
/// fill-capable tiers (the ASCII floor uses `ascii_bar`).
fn emit_mono_line(
    cells: &[RailCell],
    tier: SeamTier,
    mode: GlyphMode,
    color: bool,
    palette: &ThemePalette,
) -> String {
    let tick = match tier {
        SeamTier::Powerline => PL_TICK,
        _ => "·",
    };
    let sep = format!(" {} ", colorize(tick, &palette.separator, color));
    cells
        .iter()
        .map(|c| {
            let prefix = if c.icon.is_empty() && c.ascii.is_empty() {
                String::new()
            } else {
                glyph(mode, c.icon, c.ascii)
            };
            let body = format!("{prefix}{}", c.text);
            // git's prebaked text already colours its marks — wrap it in the
            // neutral base. Otherwise headlines + lit flags take their band fg.
            let role = if !c.prebaked && (matches!(c.kind, CellKind::Headline) || c.lit) {
                c.band
                    .map(fg_code)
                    .unwrap_or_else(|| palette.secondary.clone())
            } else {
                palette.secondary.clone()
            };
            colorize(&body, &role, color)
        })
        .collect::<Vec<_>>()
        .join(&sep)
}

pub fn render(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> Vec<String> {
    // Below min_width a connected bar can't read — bypass to flat `none`.
    if let Some(w) = config.terminal_width {
        if w < config.pane_min_width {
            return fallback_to_none(frame, config);
        }
    }

    let tier = powerline::tier(config);
    let color = config.color_enabled;
    // Cap the right-flush target at max_width so the bar doesn't spread across
    // an ultra-wide terminal (the rest stays terminal background).
    let target = config.terminal_width.map(|w| w.min(config.pane_max_width));

    // Height ladder. `None` → 3 rows; the bottom rung is the single fused bar.
    let max_lines = config.max_total_lines.unwrap_or(3).max(1);
    if max_lines == 1 {
        return vec![build_fused_row(frame, config, palette, target)];
    }

    let ctx = RailCtx {
        budget: config.pane_color_budget,
        headline: config.pane_headline,
        mono: config.pane_color_budget == ColorBudget::Mono && tier != SeamTier::AsciiFloor,
        target,
        tier,
        mode: config.glyph_mode,
        color,
        palette,
    };

    let mut out = vec![
        render_row(|n| identity_cells(frame, palette, color, n), 3, &ctx),
        render_row(|n| context_cells(frame, palette, n), 2, &ctx),
    ];
    if max_lines >= 3 && frame.quota.has_data() {
        out.push(render_row(|_| quota_cells(frame, palette), 0, &ctx));
    }
    // Drop blank rows (e.g. the usage row before the first API call) — lazy,
    // like quota; never emit an empty line.
    out.retain(|row| visible_width(row) > 0);
    out
}

/// Render one grouped row. `mono` emits the flat (undropped) cells directly;
/// signal/vivid run the width-fit → placement → `render_bar` path. Under the
/// ASCII floor the budget is moot (no fills), so segments are mapped as `signal`
/// — which keeps `to_segment`'s `mono` arm unreachable.
fn render_row(
    build: impl Fn(usize) -> (Vec<RailCell>, Vec<RailCell>),
    max_drops: usize,
    ctx: &RailCtx,
) -> String {
    if ctx.mono {
        let (mut cells, right) = build(0);
        cells.extend(right); // mono is one flat run — no headline column.
        return emit_mono_line(&cells, ctx.tier, ctx.mode, ctx.color, ctx.palette);
    }
    let seg_budget = if ctx.tier == SeamTier::AsciiFloor {
        ColorBudget::Signal
    } else {
        ctx.budget
    };
    fit_row(
        |n| {
            let (left, right) = build(n);
            let (left, right) = place(left, right, ctx.headline);
            (
                to_segments(&left, seg_budget),
                to_segments(&right, seg_budget),
            )
        },
        max_drops,
        ctx.target,
        ctx.tier,
        ctx.mode,
        ctx.color,
        ctx.palette,
    )
}

/// Render a row, dropping the lowest-priority left cells (via `build(n)`, which
/// skips the first `n` droppable cells) until it fits the capped target.
fn fit_row(
    build: impl Fn(usize) -> (Vec<Segment>, Vec<Segment>),
    max_drops: usize,
    target: Option<usize>,
    tier: SeamTier,
    mode: GlyphMode,
    color: bool,
    palette: &ThemePalette,
) -> String {
    for n in 0..max_drops {
        let (l, r) = build(n);
        let row = powerline::render_bar(&l, &r, target, tier, mode, color, palette);
        match target {
            None => return row,
            Some(w) if visible_width(&row) <= w => return row,
            _ => {}
        }
    }
    let (l, r) = build(max_drops);
    powerline::render_bar(&l, &r, target, tier, mode, color, palette)
}

// ── Row 1 · identity ──────────────────────────────────────────────────────
// Headline = model (the only constant Tint, always left). effort flags its word
// ≥high; git flags its dirty marks; cwd neutral. version is a Context cell (the
// right-cluster occupant under `column`). Drop order (left): cwd → git → effort.
fn identity_cells(
    frame: &RenderFrame,
    palette: &ThemePalette,
    color: bool,
    drops: usize,
) -> (Vec<RailCell>, Vec<RailCell>) {
    let l1 = &frame.line1;
    let mut left: Vec<RailCell> = Vec::new();

    if !l1.model.is_empty() {
        left.push(RailCell::headline(
            ICON_MODEL,
            "M:",
            l1.model.clone(),
            code(&palette.stable_blue),
        ));
    }
    if drops < 3 {
        if let Some(level) = &l1.effort_level {
            left.push(RailCell::flag(
                ICON_EFFORT,
                "E:",
                level.clone(),
                code(palette.color_for_effort_level(level)),
                powerline::effort_tints(level),
            ));
        }
    }
    if drops < 1 && !l1.project_path.is_empty() {
        left.push(RailCell::context(
            ICON_PROJECT,
            "P:",
            basename(&l1.project_path),
        ));
    }
    if drops < 2 && l1.has_git_branch() {
        let text = git_cell_text(l1, &palette.alert_orange, &palette.secondary, color);
        left.push(RailCell::prebaked(ICON_GIT, "G:", text));
    }

    let mut right: Vec<RailCell> = Vec::new();
    if !l1.claude_code_version.is_empty() {
        right.push(RailCell::context(
            ICON_VERSION,
            "",
            format!("v{}", l1.claude_code_version),
        ));
    }
    (left, right)
}

// ── Row 2 · usage ──────────────────────────────────────────────────────────
// ctx flags the whole value past 55; tokens neutral; cache is an EFFICIENCY flag
// (lights green only on good reuse). Headline = $cost (burn-rate band). Drop
// order: cache → tokens.
fn context_cells(
    frame: &RenderFrame,
    palette: &ThemePalette,
    drops: usize,
) -> (Vec<RailCell>, Vec<RailCell>) {
    let l3 = &frame.line3;
    let mut left: Vec<RailCell> = Vec::new();

    if let Some(pct) = l3.context_used_percentage {
        let denom = match l3.context_window_size {
            Some(size) => {
                let used = size.saturating_mul(pct) / 100;
                format!(" {}/{}", format_number(used), format_number(size))
            }
            None => String::new(),
        };
        left.push(ctx_cell(
            ICON_CONTEXT,
            format!("{pct}%{denom}"),
            pct,
            palette,
        ));
    }
    if drops < 2 && (l3.input_tokens.is_some() || l3.output_tokens.is_some()) {
        let in_v = l3
            .input_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into());
        let out_v = l3
            .output_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into());
        left.push(RailCell::context(
            ICON_TOKEN_OUTPUT,
            "TOK",
            format!("↓{in_v} ↑{out_v}"),
        ));
    }
    if drops < 1 {
        if let Some(cache) = l3.cache_read_tokens {
            left.push(cache_cell(l3, cache, palette));
        }
    }

    let mut right: Vec<RailCell> = Vec::new();
    if let Some(cost) = l3.total_cost_usd {
        let band =
            code(palette.color_for_burn_rate(burn_rate_per_hour(cost, l3.total_duration_ms)));
        right.push(RailCell::headline("", "", format!("${cost:.2}"), band));
    }
    (left, right)
}

// ── Row 3 · quota ───────────────────────────────────────────────────────────
// 5h is a SATURATION flag (lights its whole value ≥50). Headline = 7d (always a
// fill — the row's hero, not an alarm).
fn quota_cells(frame: &RenderFrame, palette: &ThemePalette) -> (Vec<RailCell>, Vec<RailCell>) {
    let q = &frame.quota;
    let mut left: Vec<RailCell> = Vec::new();
    if let Some(pct) = q.five_hour_pct {
        left.push(RailCell::flag(
            ICON_QUOTA,
            "",
            quota_text("5H", pct, q.five_hour_reset_minutes),
            code(palette.color_for_quota_pct(pct)),
            pct >= QUOTA_TINT_AT,
        ));
    }
    let mut right: Vec<RailCell> = Vec::new();
    if let Some(pct) = q.seven_day_pct {
        right.push(RailCell::headline(
            ICON_QUOTA,
            "",
            quota_text("7D", pct, q.seven_day_reset_minutes),
            code(palette.color_for_quota_pct(pct)),
        ));
    }
    (left, right)
}

fn quota_text(label: &str, pct: f64, reset_min: Option<u64>) -> String {
    let pct_u = pct.round() as u64;
    let reset = reset_min
        .map(|m| format!(" {}", format_reset_duration(m)))
        .unwrap_or_default();
    format!("{label} {pct_u}%{reset}")
}

/// CTX cell as a SATURATION Flag: the *whole* value lights `color_for_ctx_pct`
/// past the warn threshold, neutral below it. Shared by the 3-row and fused
/// forms.
fn ctx_cell(icon: &'static str, text: String, pct: u64, palette: &ThemePalette) -> RailCell {
    RailCell::flag(
        icon,
        "CTX:",
        text,
        code(palette.color_for_ctx_pct(pct)),
        pct >= CTX_TINT_AT,
    )
}

/// Cache as an EFFICIENCY Flag: lights its band only on good reuse (hit ≥ 80,
/// the helper's green boundary); a cold/absent hit-rate stays a quiet Context
/// ramp and is never red. Band via the canonical `cache_hit_pct` +
/// `cache_creation_share` pairing (same as `layout.rs`).
fn cache_cell(l3: &Line3Metrics, cache: u64, palette: &ThemePalette) -> RailCell {
    let text = format!("CACHE {}", format_number(cache));
    match l3.cache_hit_pct() {
        Some(hit) => RailCell::flag(
            "",
            "",
            text,
            code(palette.color_for_cache_hit_pct(hit, l3.cache_creation_share())),
            hit >= CACHE_FLAG_AT,
        ),
        None => RailCell::context("", "", text),
    }
}

// ── Bottom rung: single fused bar (max_total_lines = 1) ─────────────────────
// The dense one-liner: `model · effort · cwd · git · ctx | cost · version`,
// in the same classifier + colour-budget language as the 3-row form.
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

fn build_fused_row(
    frame: &RenderFrame,
    config: &RenderConfig,
    palette: &ThemePalette,
    target: Option<usize>,
) -> String {
    let tier = powerline::tier(config);
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let budget = config.pane_color_budget;

    // mono fused: one flat role-coloured run (no seams). The ASCII floor falls
    // through to the cluster path (no fills to drop).
    if budget == ColorBudget::Mono && tier != SeamTier::AsciiFloor {
        let (mut cells, right) = build_fused_cells(frame, palette, color, &[]);
        cells.extend(right);
        return emit_mono_line(&cells, tier, mode, color, palette);
    }

    let seg_budget = if tier == SeamTier::AsciiFloor {
        ColorBudget::Signal
    } else {
        budget
    };
    let mut dropped: Vec<FusedCell> = Vec::new();
    loop {
        let (left, right) = build_fused_cells(frame, palette, color, &dropped);
        let row = powerline::render_bar(
            &to_segments(&left, seg_budget),
            &to_segments(&right, seg_budget),
            target,
            tier,
            mode,
            color,
            palette,
        );
        match target {
            None => return row,
            Some(w) if visible_width(&row) <= w => return row,
            Some(_) => match FUSED_DROP_ORDER.iter().find(|c| !dropped.contains(c)) {
                Some(next) => dropped.push(*next),
                None => return row, // model + ctx only; render as-is
            },
        }
    }
}

fn build_fused_cells(
    frame: &RenderFrame,
    palette: &ThemePalette,
    color: bool,
    dropped: &[FusedCell],
) -> (Vec<RailCell>, Vec<RailCell>) {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let mut left: Vec<RailCell> = Vec::new();
    let mut right: Vec<RailCell> = Vec::new();

    if !l1.model.is_empty() {
        left.push(RailCell::headline(
            ICON_MODEL,
            "M:",
            l1.model.clone(),
            code(&palette.stable_blue),
        ));
    }
    if !dropped.contains(&FusedCell::Effort) {
        if let Some(level) = &l1.effort_level {
            left.push(RailCell::flag(
                ICON_EFFORT,
                "E:",
                level.clone(),
                code(palette.color_for_effort_level(level)),
                powerline::effort_tints(level),
            ));
        }
    }
    if !dropped.contains(&FusedCell::Cwd) && !l1.project_path.is_empty() {
        left.push(RailCell::context(
            ICON_PROJECT,
            "P:",
            basename(&l1.project_path),
        ));
    }
    if !dropped.contains(&FusedCell::Git) && l1.has_git_branch() {
        let text = git_cell_text(l1, &palette.alert_orange, &palette.secondary, color);
        left.push(RailCell::prebaked(ICON_GIT, "G:", text));
    }
    if let Some(pct) = l3.context_used_percentage {
        left.push(ctx_cell(ICON_CONTEXT, format!("{pct}%"), pct, palette));
    }
    if !dropped.contains(&FusedCell::Cost) {
        if let Some(cost) = l3.total_cost_usd {
            let band =
                code(palette.color_for_burn_rate(burn_rate_per_hour(cost, l3.total_duration_ms)));
            right.push(RailCell::headline("", "", format!("${cost:.2}"), band));
        }
    }
    if !dropped.contains(&FusedCell::Version) && !l1.claude_code_version.is_empty() {
        right.push(RailCell::context(
            ICON_VERSION,
            "",
            format!("v{}", l1.claude_code_version),
        ));
    }
    (left, right)
}

// ── shared helpers ──────────────────────────────────────────────────────────

/// A **partial** cell text where only `value` carries the `role` colour;
/// `prefix`/`suffix` stay in the cell's base (neutral) fg. fg-only — no `RESET`,
/// so the segment's ramp bg persists — and emits nothing when colour is off
/// (NO_COLOR-safe). Used only by `git_cell_text`, a genuine two-part cell
/// (neutral `branch`/`+added` identity + the `~N`/`*` dirty signal).
fn lit_value(
    prefix: &str,
    value: &str,
    suffix: &str,
    role: &str,
    base: &str,
    color: bool,
) -> String {
    if color {
        format!("{prefix}{role}{value}{base}{suffix}")
    } else {
        format!("{prefix}{value}{suffix}")
    }
}

/// The git cell: `branch +added` neutral; the dirty marks `~modified`/`*` light
/// alert_orange (letter — only the signal, not the branch). Plain when clean
/// or colour-off.
fn git_cell_text(l1: &Line1Metrics, orange: &str, base: &str, color: bool) -> String {
    let mut prefix = l1.git_branch.clone();
    if l1.git_added > 0 {
        prefix.push_str(&format!(" +{}", l1.git_added));
    }
    let mut sig = String::new();
    if l1.git_modified > 0 {
        sig.push_str(&format!(" ~{}", l1.git_modified));
    }
    if l1.git_dirty {
        sig.push_str(" *");
    }
    if sig.is_empty() {
        return prefix;
    }
    lit_value(&prefix, &sig, "", orange, base, color)
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
