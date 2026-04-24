use std::ops::Range;

use crate::config::GlyphMode;

use super::color::{colorize, visible_width};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneStyle {
    None,
    Rail,
    Box,
    /// Per-line 2-col Nerd Font icon gutter (e.g. `  Opus 4.7 ...`).
    Gutter,
    /// Per-line 3-letter text label + middot (e.g. `id  · Opus 4.7 ...`).
    Labeled,
    /// Per-line single-char bullet (e.g. `●  Opus 4.7 ...`).
    Bullet,
    /// Per-line reverse-video "tag" pill (e.g. ` ID  Opus 4.7 ...`).
    Pill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneWidth {
    Auto,
    Terminal,
    Fixed(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineKind {
    Identity,
    Config,
    Budget,
    Activity,
}

#[derive(Debug, Clone)]
pub struct PaneGroup {
    pub label: String,
    pub kinds: Vec<LineKind>,
}

/// Cols subtracted from `terminal_width` in `PaneWidth::Terminal` mode.
/// Claude Code allocates the statusline a sub-region that is ~1-4 cols
/// narrower than the raw terminal; lines at exactly the raw width trigger
/// wrap and collapse multi-line rendering to a single visible line. 4 is
/// the empirically verified safe default on CC 2.1.119.
pub const DEFAULT_PANE_CC_MARGIN: usize = 4;

#[derive(Debug, Clone)]
pub struct PaneConfig {
    pub style: PaneStyle,
    pub width_mode: PaneWidth,
    pub min_width: usize,
    pub max_width: usize,
    pub groups: Vec<PaneGroup>,
    pub glyph_mode: GlyphMode,
    pub terminal_width: Option<usize>,
    pub cc_margin: usize,
    /// ANSI fg-color escapes per group, indexed by `LineKind as usize`:
    /// [Identity, Config, Budget, Activity]. Used by Gutter / Labeled /
    /// Bullet / Pill styles to tier-color their prefixes. Empty strings
    /// mean "don't emit color" — degrades cleanly under `NO_COLOR`.
    pub group_colors: [String; 4],
    /// When true, the prefix styles (Gutter/Labeled/Bullet/Pill) also
    /// emit colored prefixes. Matches the semantics of `color_enabled`
    /// elsewhere in `RenderConfig`.
    pub color_enabled: bool,
    /// When true, emit a blank row after the Identity group. Orthogonal
    /// to `style` — combines with any non-None style.
    pub identity_spacing: bool,
}

/// Insert group separators around `lines`:
/// - Box: labelled horizontal dividers (`─── Config ──────`) above each group,
///   including the first. No side or bottom borders.
/// - Rail: left-side `│` guide with `├ Label` shoulder between groups.
///
/// Returns `lines` unchanged when `cfg.style == PaneStyle::None` or when the
/// terminal can't fit `cfg.min_width`.
pub fn apply_pane(
    lines: Vec<String>,
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
) -> Vec<String> {
    if matches!(cfg.style, PaneStyle::None) {
        return lines;
    }
    if lines.is_empty() || cfg.groups.is_empty() {
        return lines;
    }

    // Safety gate: skip framing when the terminal can't fit min_width. The old
    // `+ 4` was for side-border cost; the current box mode has none and rail
    // eats only 2 cols, so gate on min_width alone (conservative enough).
    if let Some(term) = cfg.terminal_width {
        if term < cfg.min_width {
            return lines;
        }
    }

    let grouped = collect_grouped_lines(&lines, groups, &cfg.groups);
    if grouped.is_empty() {
        return lines;
    }

    let g = glyphs(cfg.glyph_mode);
    match cfg.style {
        PaneStyle::Box => render_box(&grouped, &lines, cfg, g),
        PaneStyle::Rail => render_rail(&grouped, g),
        PaneStyle::Gutter | PaneStyle::Labeled | PaneStyle::Bullet | PaneStyle::Pill => {
            render_prefixed(&lines, groups, cfg)
        }
        PaneStyle::None => lines,
    }
}

/// Per-line prefix styles: Gutter / Labeled / Bullet / Pill. All share the
/// same shape — prepend a group-specific prefix to every content line. No
/// extra rows introduced (except the optional `identity_spacing` blank).
fn render_prefixed(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
) -> Vec<String> {
    let extra = if cfg.identity_spacing { 1 } else { 0 };
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + extra);

    for (kind, range) in groups {
        let prefix = build_prefix(*kind, cfg);
        for idx in range.start..range.end {
            if let Some(line) = lines.get(idx) {
                out.push(format!("{prefix}{line}"));
            }
        }
        if cfg.identity_spacing && matches!(kind, LineKind::Identity) {
            out.push(String::new());
        }
    }
    out
}

fn build_prefix(kind: LineKind, cfg: &PaneConfig) -> String {
    let glyph = prefix_glyph(cfg.style, kind, cfg.glyph_mode);
    let color = &cfg.group_colors[group_idx(kind)];
    let tint = cfg.color_enabled && !color.is_empty();
    let colored = colorize(&glyph, color, tint);
    // Pill wraps the colored prefix in reverse video. The `\x1b[7m` must come
    // before the fg-color escape so the terminal swaps fg↔bg using the tinted
    // color as bg; the RESET inside `colored` closes both attributes at once.
    if matches!(cfg.style, PaneStyle::Pill) && tint {
        format!("\x1b[7m{colored}")
    } else {
        colored
    }
}

const fn group_idx(kind: LineKind) -> usize {
    match kind {
        LineKind::Identity => 0,
        LineKind::Config => 1,
        LineKind::Budget => 2,
        LineKind::Activity => 3,
    }
}

/// Returns `(icon, ascii)` base glyphs for `(style, kind)`. Callers choose
/// the mode-appropriate string and apply style-specific framing (spacing,
/// middot, brackets) in `prefix_glyph`.
fn glyph_pair(style: PaneStyle, kind: LineKind) -> (&'static str, &'static str) {
    use LineKind::{Activity, Budget, Config, Identity};
    use PaneStyle::{Bullet, Gutter, Labeled, Pill};
    match (style, kind) {
        // Gutter — Nerd Font icons: user / cog / dollar / bolt.
        (Gutter, Identity) => ("\u{f007}", "ID"),
        (Gutter, Config) => ("\u{f013}", "CF"),
        (Gutter, Budget) => ("\u{f155}", "$$"),
        (Gutter, Activity) => ("\u{f0e7}", "AC"),
        // Labeled — mode-agnostic 3-letter tags.
        (Labeled, Identity) => ("id ", "id "),
        (Labeled, Config) => ("cfg", "cfg"),
        (Labeled, Budget) => ("bdg", "bdg"),
        (Labeled, Activity) => ("act", "act"),
        // Bullet — solid/hollow/diamond/triangle with ASCII fallback.
        (Bullet, Identity) => ("●", "*"),
        (Bullet, Config) => ("○", "o"),
        (Bullet, Budget) => ("◆", "#"),
        (Bullet, Activity) => ("▸", ">"),
        // Pill — mode-agnostic 2-char tags (wrapped in spaces at frame time).
        (Pill, Identity) => ("ID", "ID"),
        (Pill, Config) => ("CF", "CF"),
        (Pill, Budget) => ("$$", "$$"),
        (Pill, Activity) => ("AC", "AC"),
        _ => ("", ""),
    }
}

fn prefix_glyph(style: PaneStyle, kind: LineKind, mode: GlyphMode) -> String {
    let (icon, ascii) = glyph_pair(style, kind);
    let base = match mode {
        GlyphMode::Icon => icon,
        GlyphMode::Ascii => ascii,
    };
    match style {
        PaneStyle::Gutter | PaneStyle::Bullet => format!("{base}  "),
        PaneStyle::Labeled => format!("{base} · "),
        PaneStyle::Pill => format!(" {base}  "),
        _ => String::new(),
    }
}

type Grouped<'a> = [(&'a str, Vec<&'a str>)];

fn render_box(
    grouped: &Grouped<'_>,
    lines: &[String],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    let ruler_width = resolve_inner_width(grouped, cfg);
    if ruler_width < cfg.min_width {
        return lines.to_vec();
    }

    let mut out: Vec<String> = Vec::with_capacity(grouped.len() * 2 + lines.len());
    for (label, group_lines) in grouped.iter() {
        out.push(render_section_divider(label, ruler_width, g));
        for line in group_lines {
            out.push((*line).to_string());
        }
    }
    out
}

fn render_rail(grouped: &Grouped<'_>, g: &FrameGlyphs) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(grouped.len() * 2 + 1);
    for (i, (label, group_lines)) in grouped.iter().enumerate() {
        let shoulder = if i == 0 { g.tl } else { g.tee_l };
        out.push(format!("{shoulder} {label}"));
        for line in group_lines {
            out.push(format!("{v} {line}", v = g.v));
        }
    }
    out.push(g.bl.to_string());
    out
}

fn collect_grouped_lines<'a>(
    lines: &'a [String],
    groups: &[(LineKind, Range<usize>)],
    configured: &'a [PaneGroup],
) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut grouped: Vec<(&str, Vec<&str>)> = Vec::with_capacity(configured.len());
    for pane_group in configured {
        let mut collected: Vec<&str> = Vec::new();
        for (kind, range) in groups {
            if pane_group.kinds.contains(kind) {
                for idx in range.start..range.end {
                    if let Some(line) = lines.get(idx) {
                        collected.push(line.as_str());
                    }
                }
            }
        }
        if !collected.is_empty() {
            grouped.push((pane_group.label.as_str(), collected));
        }
    }
    grouped
}

fn resolve_inner_width(grouped: &Grouped<'_>, cfg: &PaneConfig) -> usize {
    let content_max = grouped
        .iter()
        .flat_map(|(_, ls)| ls.iter())
        .map(|s| visible_width(s))
        .max()
        .unwrap_or(0);
    let label_max = grouped
        .iter()
        .map(|(label, _)| visible_width(label) + 2)
        .max()
        .unwrap_or(0);

    let max_width = cfg.max_width.max(cfg.min_width); // guard against misconfig (min > max)
    let raw = match cfg.width_mode {
        PaneWidth::Auto => content_max.max(label_max),
        PaneWidth::Fixed(w) => w.max(label_max),
        PaneWidth::Terminal => {
            // Span the detected terminal width, minus `cc_margin` cols for
            // Claude Code's statusline padding. CC allocates the statusline a
            // sub-region of the raw terminal — empirically ~1-4 cols narrower;
            // lines at exactly the raw width trigger wrap and collapse the
            // whole multi-line render. Defaults to 4 cols (verified safe on
            // CC 2.1.119); configurable via `pane.cc_margin` for other hosts.
            //
            // When detection failed (`terminal_width = None`), fall back to
            // content-fit (Auto behavior) — NOT to max_width.
            match cfg.terminal_width {
                Some(term) => term.saturating_sub(cfg.cc_margin).max(label_max),
                None => content_max.max(label_max),
            }
        }
    };
    raw.clamp(cfg.min_width, max_width)
}

fn render_section_divider(label: &str, ruler_width: usize, g: &FrameGlyphs) -> String {
    // Pattern: `─── {label} ` + fill × `─`, targeting `ruler_width` total visible chars.
    const PREFIX_DASHES: usize = 3;
    let label_w = visible_width(label);
    let overhead = PREFIX_DASHES + 1 + label_w + 1; // "─── label "
    let fill = ruler_width.saturating_sub(overhead);
    let head = g.h.repeat(PREFIX_DASHES);
    let tail = g.h.repeat(fill);
    format!("{head} {label} {tail}")
}

struct FrameGlyphs {
    tl: &'static str,
    bl: &'static str,
    h: &'static str,
    v: &'static str,
    tee_l: &'static str,
}

const UNICODE_GLYPHS: FrameGlyphs = FrameGlyphs {
    tl: "╭",
    bl: "╰",
    h: "─",
    v: "│",
    tee_l: "├",
};

const ASCII_GLYPHS: FrameGlyphs = FrameGlyphs {
    tl: "+",
    bl: "+",
    h: "-",
    v: "|",
    tee_l: "+",
};

fn glyphs(mode: GlyphMode) -> &'static FrameGlyphs {
    match mode {
        GlyphMode::Icon => &UNICODE_GLYPHS,
        GlyphMode::Ascii => &ASCII_GLYPHS,
    }
}
