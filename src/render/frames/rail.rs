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
//! Width: when a terminal width is known the bar is capped at `pane_max_width`
//! (won't spread across an ultra-wide terminal); with no width it is
//! content-sized. Height ladder
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
    fail_mark, glyph, ICON_COMPACT, ICON_CONTEXT, ICON_EFFORT, ICON_GIT, ICON_MODEL, ICON_PROJECT,
    ICON_QUOTA, ICON_TODO, ICON_TOKEN_OUTPUT, ICON_TOOL, ICON_VERSION, PL_TICK,
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

/// Per-row cell vocabulary — also the built-in default order (left→right) when
/// the user's `rail_*_order` is empty. The built-in hero per row is the first
/// hero-capable cell: identity → `model`, usage → `cost`, quota → `7d`.
const IDENTITY_CELLS: [&str; 5] = ["model", "effort", "cwd", "git", "version"];
// `todo` / `tools` are the traceability cells — quiet counts that ride the
// inter-cluster gap (left of the `cost` hero) and shed first under width
// pressure. They render only when their data exists AND the segment toggle is
// on; the order filter in `render()` drops them when `show_todo`/`show_tools`
// is off (build_cell has no config handle).
const USAGE_CELLS: [&str; 7] = ["ctx", "compact", "tokens", "cache", "todo", "tools", "cost"];
const QUOTA_CELLS: [&str; 2] = ["5h", "7d"];

/// Global cell vocabulary — every buildable cell name. A configured
/// `rail_*_order` is validated against THIS (not the per-row default), so any
/// cell may be placed on any row (e.g. traceability on the quota row). The
/// per-row consts above remain the built-in DEFAULT arrangement.
const ALL_CELLS: [&str; 14] = [
    "model", "effort", "cwd", "git", "version", "ctx", "compact", "tokens", "cache", "todo",
    "tools", "cost", "5h", "7d",
];

/// Cluster-split marker for `rail_*_order`: cells before it form the left
/// cluster, cells after it the right-hugged cluster (e.g.
/// `["5h", "7d", "|", "todo", "tools"]`). With no marker the LAST cell
/// right-hugs (the built-in behaviour). Not a cell — never rendered; the first
/// occurrence splits. Right-cluster cells still honour `drop_priority`, so
/// volatile cells (traceability) shed first even on the right.
const CLUSTER_SPLIT: &str = "|";

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

    /// Promote a natural cell to its row's **hero** (the filled reverse-video
    /// headline), keeping its band. A prebaked (git) cell can't fill, so it
    /// renders unchanged. For model/cost/7d this reproduces today's `headline()`.
    fn into_hero(mut self) -> Self {
        self.kind = CellKind::Headline;
        self.lit = true;
        self
    }
}

/// The colour-budget transform: a classified cell → a `powerline::Segment`.
/// `mono` never reaches here (it uses `emit_mono_line`).
fn to_segment(c: &RailCell, budget: ColorBudget) -> Segment {
    // git-style partial cells carry their own splices — skip the colour
    // transform, but still ride the budget's ramp tier so the cell matches its
    // row-mates (context rides Raised under vivid, Base otherwise; mono never
    // reaches here).
    if c.prebaked {
        let level = match budget {
            ColorBudget::Vivid => RampLevel::Raised,
            _ => RampLevel::Base,
        };
        return Segment::ramp(c.icon, c.ascii, c.text.clone(), level);
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

    // Resolve each row's order + hero once (validate names, warn on unknown,
    // empty → built-in default) — kept out of the fit loop so a bad name warns
    // at most once per render, like the `parse_layout_*` parsers.
    let id_order = resolve_order(&config.rail_identity_order, &IDENTITY_CELLS, "identity");
    let mut us_order = resolve_order(&config.rail_usage_order, &USAGE_CELLS, "usage");
    // Segment toggles gate the traceability cells (build_cell has no config
    // handle); removing them here keeps `cost` the last/right-hug cell.
    us_order.retain(|c| match c.as_str() {
        "todo" => config.show_todo,
        "tools" => config.show_tools,
        _ => true,
    });
    let id_hero = effective_hero(&config.rail_identity_hero, "model", &id_order, "identity");
    let us_hero = effective_hero(&config.rail_usage_hero, "cost", &us_order, "usage");

    let mut out = vec![
        render_row(
            |n| {
                assemble(
                    &id_order,
                    id_hero,
                    false,
                    frame,
                    palette,
                    color,
                    config.glyph_mode,
                    n,
                )
            },
            max_drops(&id_order),
            &ctx,
        ),
        // usage row drops the lone hero when there's no detail (pre-API `$0.00`).
        render_row(
            |n| {
                assemble(
                    &us_order,
                    us_hero,
                    true,
                    frame,
                    palette,
                    color,
                    config.glyph_mode,
                    n,
                )
            },
            max_drops(&us_order),
            &ctx,
        ),
    ];
    if max_lines >= 3 && frame.quota.has_data() {
        let qu_order = resolve_order(&config.rail_quota_order, &QUOTA_CELLS, "quota");
        let qu_hero = effective_hero(&config.rail_quota_hero, "7d", &qu_order, "quota");
        out.push(render_row(
            |n| {
                assemble(
                    &qu_order,
                    qu_hero,
                    false,
                    frame,
                    palette,
                    color,
                    config.glyph_mode,
                    n,
                )
            },
            max_drops(&qu_order),
            &ctx,
        ));
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

// ── Cell registry + order-driven assembly ──────────────────────────────────
// Each cell builds with its NATURAL kind; the row's hero is promoted to a fill
// via `into_hero`. model/cost/7d are always-banded heroes (a `Flag(lit=true)`
// when displaced, so they ink their band instead of going gray); effort/ctx/5h/
// cache are threshold flags; cwd/version/tokens are context; git is prebaked.

/// Build a single named cell with its natural kind, or `None` if its data is
/// absent. Unknown names never reach here (filtered by `resolve_order`).
fn build_cell(
    name: &str,
    frame: &RenderFrame,
    palette: &ThemePalette,
    color: bool,
    mode: GlyphMode,
) -> Option<RailCell> {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let q = &frame.quota;
    match name {
        "model" => (!l1.model.is_empty()).then(|| {
            RailCell::flag(
                ICON_MODEL,
                "M:",
                l1.model.clone(),
                code(&palette.stable_blue),
                true,
            )
        }),
        "effort" => l1.effort_level.as_ref().map(|level| {
            RailCell::flag(
                ICON_EFFORT,
                "E:",
                level.clone(),
                code(palette.color_for_effort_level(level)),
                powerline::effort_tints(level),
            )
        }),
        "cwd" => (!l1.project_path.is_empty())
            .then(|| RailCell::context(ICON_PROJECT, "P:", basename(&l1.project_path))),
        "git" => l1.has_git_branch().then(|| {
            let text = git_cell_text(l1, &palette.alert_orange, &palette.secondary, color);
            RailCell::prebaked(ICON_GIT, "G:", text)
        }),
        "version" => (!l1.claude_code_version.is_empty())
            .then(|| RailCell::context(ICON_VERSION, "", format!("v{}", l1.claude_code_version))),
        "ctx" => l3.context_used_percentage.map(|pct| {
            let denom = match l3.context_window_size {
                Some(size) => {
                    let used = size.saturating_mul(pct) / 100;
                    format!(" {}/{}", format_number(used), format_number(size))
                }
                None => String::new(),
            };
            ctx_cell(ICON_CONTEXT, format!("{pct}%{denom}"), pct, palette)
        }),
        "tokens" => (l3.input_tokens.is_some() || l3.output_tokens.is_some()).then(|| {
            let in_v = l3
                .input_tokens
                .map(format_number)
                .unwrap_or_else(|| "--".into());
            let out_v = l3
                .output_tokens
                .map(format_number)
                .unwrap_or_else(|| "--".into());
            RailCell::context(ICON_TOKEN_OUTPUT, "TOK", format!("↓{in_v} ↑{out_v}"))
        }),
        "cache" => l3
            .cache_read_tokens
            .is_some()
            .then(|| cache_cell(l3, palette)),
        // Context-compaction marker `⟳N` — only once a compaction has happened.
        "compact" => (frame.compact_count > 0)
            .then(|| RailCell::context(ICON_COMPACT, "~", frame.compact_count.to_string())),
        // Task progress `c/t` — a quiet Context count (progress is never an
        // alert, so it never lights). Absent when there's no todo state.
        "todo" => {
            frame.todo.as_ref().filter(|t| t.total > 0).map(|t| {
                RailCell::context(ICON_TODO, "TODO", format!("{}/{}", t.completed, t.total))
            })
        }
        // Tool-use volume `N` (honest uncapped total) — a quiet Flag that lights
        // its alert band only when failures occurred this session (`✘M`).
        "tools" => (frame.completed_tool_total > 0).then(|| {
            let failed = frame.failed_tool_total;
            let text = if failed > 0 {
                format!(
                    "{} {}{}",
                    frame.completed_tool_total,
                    fail_mark(mode),
                    failed
                )
            } else {
                frame.completed_tool_total.to_string()
            };
            RailCell::flag(ICON_TOOL, "T:", text, code(&palette.alert_red), failed > 0)
        }),
        "cost" => l3.total_cost_usd.map(|cost| {
            let band =
                code(palette.color_for_burn_rate(burn_rate_per_hour(cost, l3.total_duration_ms)));
            RailCell::flag("", "", format!("${cost:.2}"), band, true)
        }),
        "5h" => q.five_hour_pct.map(|pct| {
            RailCell::flag(
                ICON_QUOTA,
                "",
                quota_text("5H", pct, q.five_hour_reset_minutes),
                code(palette.color_for_quota_pct(pct)),
                pct >= QUOTA_TINT_AT,
            )
        }),
        "7d" => q.seven_day_pct.map(|pct| {
            RailCell::flag(
                ICON_QUOTA,
                "",
                quota_text("7D", pct, q.seven_day_reset_minutes),
                code(palette.color_for_quota_pct(pct)),
                pct >= QUOTA_TINT_AT,
            )
        }),
        _ => None,
    }
}

/// Intrinsic width-fit drop priority (lower = drops first). Reproduces the v3
/// `drops < N` gates regardless of display order; cells not listed never drop.
fn drop_priority(name: &str) -> Option<usize> {
    match name {
        "cwd" => Some(1),
        "git" => Some(2),
        "effort" => Some(3),
        "cache" => Some(1),
        "compact" => Some(1),
        "tokens" => Some(2),
        // Traceability sheds first (lowest tier) — core identity/usage survives.
        "todo" => Some(1),
        "tools" => Some(1),
        _ => None,
    }
}

/// The fit ladder's depth for a row = its deepest droppable cell.
fn max_drops(order: &[String]) -> usize {
    order
        .iter()
        .filter_map(|n| drop_priority(n))
        .max()
        .unwrap_or(0)
}

/// Validate a configured order against the GLOBAL vocabulary (`ALL_CELLS`) plus
/// the `|` split marker: empty → built-in default; unknown names warn (once per
/// render) and are skipped; an order with no real cell falls back to the
/// default. Any valid cell may be placed on any row.
fn resolve_order(raw: &[String], default: &[&'static str], label: &str) -> Vec<String> {
    if raw.is_empty() {
        return default.iter().map(|s| s.to_string()).collect();
    }
    let kept: Vec<String> = raw
        .iter()
        .filter(|name| {
            let ok = name.as_str() == CLUSTER_SPLIT || ALL_CELLS.contains(&name.as_str());
            if !ok {
                eprintln!(
                    "warning: unknown layout.rail_{label}_order cell {name:?}; skipping \
                     (valid: {} — or {CLUSTER_SPLIT:?} to split left|right)",
                    ALL_CELLS.join(" ")
                );
            }
            ok
        })
        .cloned()
        .collect();
    // A marker alone isn't a cell — fall back if nothing real survived.
    if kept.iter().all(|n| n == CLUSTER_SPLIT) {
        return default.iter().map(|s| s.to_string()).collect();
    }
    kept
}

/// The effective hero name: empty → built-in default; a non-empty hero that
/// isn't in the resolved order warns (the row then has no filled headline).
fn effective_hero<'a>(raw: &'a str, default: &'a str, order: &[String], label: &str) -> &'a str {
    if raw.is_empty() {
        return default;
    }
    if !order.iter().any(|c| c == raw) {
        eprintln!(
            "warning: layout.rail_{label}_hero {raw:?} is not in rail_{label}_order; \
             the row will have no filled headline"
        );
    }
    raw
}

/// Build a row's `(left, right)` clusters from a resolved order. The cluster
/// split is either an explicit `|` marker (cells before → left, after → right)
/// or, with no marker, the LAST cell right-hugs (the built-in behaviour). The
/// hero is promoted to a fill. The hero — and, in the no-marker form, the
/// right-hug cell — never drop for width; an explicit right cluster still
/// honours `drop_priority` (so traceability sheds first even on the right).
/// `drop_if_left_empty` (usage only) drops a lone hero so the pre-API `$0.00`
/// row falls away.
#[allow(clippy::too_many_arguments)]
fn assemble(
    order: &[String],
    hero: &str,
    drop_if_left_empty: bool,
    frame: &RenderFrame,
    palette: &ThemePalette,
    color: bool,
    mode: GlyphMode,
    drops: usize,
) -> (Vec<RailCell>, Vec<RailCell>) {
    let split = order.iter().position(|s| s == CLUSTER_SPLIT);
    // No-marker right-hug: the last real (non-marker) cell.
    let last = order.iter().rposition(|s| s != CLUSTER_SPLIT);
    let mut left: Vec<RailCell> = Vec::new();
    let mut right: Vec<RailCell> = Vec::new();
    for (i, name) in order.iter().enumerate() {
        if name == CLUSTER_SPLIT {
            continue;
        }
        let to_right = match split {
            Some(idx) => i > idx,
            None => Some(i) == last,
        };
        let is_hero = name.as_str() == hero;
        // The hero never drops; the no-marker right-hug cell never drops. Cells
        // in an explicit right cluster still honour drop_priority.
        let protected = is_hero || (split.is_none() && to_right);
        if !protected {
            if let Some(p) = drop_priority(name) {
                if drops >= p {
                    continue;
                }
            }
        }
        let Some(mut cell) = build_cell(name, frame, palette, color, mode) else {
            continue;
        };
        if is_hero {
            cell = cell.into_hero();
        }
        if to_right {
            right.push(cell);
        } else {
            left.push(cell);
        }
    }
    if drop_if_left_empty && left.is_empty() {
        return (Vec::new(), Vec::new());
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

/// Cache as an EFFICIENCY Flag, shown as the **hit %** (`CACHE 84%`, same number
/// as L3's `C:%`). Lights its band only on good reuse (hit ≥ 80, the helper's
/// green boundary); a cold/absent hit-rate (`CACHE --%`) stays a quiet Context
/// ramp and is never red. Band via the canonical `cache_hit_pct` +
/// `cache_creation_share` pairing (same as `layout.rs`).
fn cache_cell(l3: &Line3Metrics, palette: &ThemePalette) -> RailCell {
    match l3.cache_hit_pct() {
        Some(hit) => RailCell::flag(
            "",
            "",
            format!("CACHE {}%", hit.round() as u64),
            code(palette.color_for_cache_hit_pct(hit, l3.cache_creation_share())),
            hit >= CACHE_FLAG_AT,
        ),
        None => RailCell::context("", "", "CACHE --%".to_string()),
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
    // The fused bar keeps its own built-in arrangement (NOT the order-driven
    // vocab): traceability (todo/tools) is intentionally absent here — the
    // single most-compressed rung sheds volatile activity first.
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
