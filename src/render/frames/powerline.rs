//! Powerline seam/cap emit — shared by the `rail` and `anchor` layouts.
//!
//! This is the terminal-side port of the HTML prototype's coloured-span +
//! fg-glyph seam. The classic ANSI Powerline trick: a right-seam between
//! segment A and B is the glyph `\u{e0b0}` printed with **fg = A.bg, bg =
//! B.bg** — the triangle is A-coloured on its left, B-coloured on its right,
//! so the fill looks continuous. (See `designs/powerline-rail-anchor.md`.)
//!
//! Three capability tiers, picked by [`tier`]:
//!
//! | tier        | seam                       | cell glyphs | when |
//! |-------------|----------------------------|-------------|------|
//! | `Powerline` | PUA `\u{e0b0}`/`\u{e0b2}`   | MD glyphs   | color + Icon + `seams=powerline` |
//! | `Blocks`    | unicode half-block `▐`/`▌` | MD glyphs   | color + Icon + `seams=blocks`    |
//! | `AsciiFloor`| ` \| ` separator, no fill  | ASCII prefix| `NO_COLOR`, or `display.icons=false` |
//!
//! The renderer is 256-colour, foreground-only everywhere else; this module
//! and `color::bg_code` are the only place a background fill is emitted. The
//! design specifies a truecolor RGB ramp blended from `term_bg`; the renderer
//! has no `term_bg` source (not in the payload, not detectable), so the bed is
//! a fixed 256 grayscale ramp instead — the standard vim-airline / tmux
//! approach. Truecolor is a later backend swap, gated on a `term_bg` source.

use crate::config::{GlyphMode, LayoutSeams, RenderConfig};
use crate::render::color::{
    bg_code, extract_ansi_code, fg_code, visible_width, ThemePalette, RESET,
};
use crate::render::icons::{glyph, CAP_ROUND_L, CAP_ROUND_R, SEAM_L, SEAM_R};

/// Reset only the background to the terminal default — used for the pointed
/// outer edge of a cluster (the seam fades into whatever the terminal bg is,
/// so we never need to know `term_bg`).
const DEFAULT_BG: &str = "\x1b[49m";

/// Unicode half-block seams for the `Blocks` tier (no patched font needed).
const HALF_R: &str = "\u{2590}"; // ▐ right half block
const HALF_L: &str = "\u{258c}"; // ▌ left half block

// ── Gray ink ramp (256 grayscale) — the monochrome bed. Three quiet steps,
// darkest → brightest, standing in for the design's term_bg→structural blend
// at 18 / 32 / 46%. Module-local (experimental), mirroring `frames/badge.rs`;
// promote to ThemePalette fields if these layouts graduate. ──
const RAMP_BASE: u8 = 235; // cwd, cost, version
const RAMP_RAISED: u8 = 238; // effort/git/ctx when calm
const RAMP_HIGH: u8 = 241; // the model anchor segment (brightest gray)
/// Reverse-video text colour on a tinted (signal) segment — near-black, the
/// `term_bg` stand-in. Tints are always bright (warn/crit/alert) so black reads.
const TINT_FG: u8 = 16;

/// Which seam vocabulary a render uses. Picked from `(color, glyph_mode,
/// seams)` — independent axes collapsed into one decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamTier {
    Powerline,
    Blocks,
    AsciiFloor,
}

/// Resolve the seam tier. `color` is the hard gate (a bar can't read without
/// background fills); `GlyphMode::Ascii` is the user asserting "no fancy
/// glyphs", which drops to the ASCII floor (no PUA, no half-blocks). Only a
/// colour + Icon terminal reaches the `seams` choice.
pub fn tier(config: &RenderConfig) -> SeamTier {
    if !config.color_enabled {
        return SeamTier::AsciiFloor;
    }
    match config.glyph_mode {
        GlyphMode::Ascii => SeamTier::AsciiFloor,
        GlyphMode::Icon => match config.pane_seams {
            LayoutSeams::Powerline => SeamTier::Powerline,
            LayoutSeams::Blocks => SeamTier::Blocks,
        },
    }
}

/// The three ramp steps a ramp segment can sit on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampLevel {
    Base,
    Raised,
    High,
}

/// A segment's fill: a monochrome ramp step, or a render-role tint (the lone
/// live signal). `Tint` carries the 256 colour index of the role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Ramp(RampLevel),
    Tint(u8),
}

/// One bar cell. `icon` is the MD glyph (Icon tiers); `ascii` is the
/// floor-mode prefix (e.g. `"M:"`); `text` is the value.
///
/// `fill` is the background (ramp gray or reverse-video tint). `ink` is an
/// independent **text** channel: a ramp segment can paint its glyphs a
/// render-role colour (a mid-bar state flag — dirty branch, effort band, ctx
/// threshold) while keeping its quiet gray bg. `ink` is ignored on a `Tint`
/// (a fill already owns its fg). This generalises v1's hand-spliced orange
/// `~N` escape into a first-class field.
#[derive(Debug, Clone)]
pub struct Segment {
    pub icon: &'static str,
    pub ascii: &'static str,
    pub text: String,
    pub fill: Fill,
    pub ink: Option<u8>,
}

impl Segment {
    pub fn ramp(
        icon: &'static str,
        ascii: &'static str,
        text: impl Into<String>,
        lvl: RampLevel,
    ) -> Self {
        Segment {
            icon,
            ascii,
            text: text.into(),
            fill: Fill::Ramp(lvl),
            ink: None,
        }
    }
    /// A ramp segment that flags its text in a render-role colour while
    /// keeping the gray bg (the `ink` channel). `ink_code` is a 256 index.
    pub fn ramp_ink(
        icon: &'static str,
        ascii: &'static str,
        text: impl Into<String>,
        lvl: RampLevel,
        ink_code: u8,
    ) -> Self {
        Segment {
            icon,
            ascii,
            text: text.into(),
            fill: Fill::Ramp(lvl),
            ink: Some(ink_code),
        }
    }
    pub fn tint(
        icon: &'static str,
        ascii: &'static str,
        text: impl Into<String>,
        code: u8,
    ) -> Self {
        Segment {
            icon,
            ascii,
            text: text.into(),
            fill: Fill::Tint(code),
            ink: None,
        }
    }
    pub fn is_tinted(&self) -> bool {
        matches!(self.fill, Fill::Tint(_))
    }
}

/// Cap geometry for the `anchor` capsule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapStyle {
    /// Angled `\u{e0b2}`/`\u{e0b0}` — rhymes with the rail seam. Anchor default.
    Angle,
    /// Rounded `\u{e0b6}`/`\u{e0b4}` — the canonical capsule.
    Round,
}

fn idx(escape: &str, fallback: u8) -> u8 {
    extract_ansi_code(escape).unwrap_or(fallback)
}

/// The shared tinting threshold for effort: tints from `high` upward
/// (`color_for_effort_level` runs warn→crit across high/xhigh/max). Used by
/// both `rail` (segment leaves the ramp) and `anchor` (trail item lights up).
pub fn effort_tints(level: &str) -> bool {
    matches!(level, "high" | "xhigh" | "max")
}

fn seg_bg(seg: &Segment) -> u8 {
    match seg.fill {
        Fill::Ramp(RampLevel::Base) => RAMP_BASE,
        Fill::Ramp(RampLevel::Raised) => RAMP_RAISED,
        Fill::Ramp(RampLevel::High) => RAMP_HIGH,
        Fill::Tint(c) => c,
    }
}

fn seg_fg(seg: &Segment, palette: &ThemePalette) -> u8 {
    match seg.fill {
        // A fill owns its fg (reverse-video) — `ink` is ignored on a Tint.
        Fill::Tint(_) => TINT_FG,
        // Ramp: an `ink` flag (if present) wins over the plain text tier.
        Fill::Ramp(level) => seg.ink.unwrap_or(match level {
            RampLevel::High => idx(&palette.primary, 252),
            _ => idx(&palette.secondary, 250),
        }),
    }
}

/// Foreground escape for a cell in the ASCII floor (no fills): tints and `ink`
/// flags show in the role colour, plain ramp cells in their text tier.
fn ascii_fg(seg: &Segment, palette: &ThemePalette) -> String {
    match seg.fill {
        Fill::Tint(c) => fg_code(c),
        Fill::Ramp(level) => match seg.ink {
            Some(c) => fg_code(c),
            None => match level {
                RampLevel::High => palette.primary.clone(),
                _ => palette.secondary.clone(),
            },
        },
    }
}

/// The cell prefix (icon in Icon tiers, ASCII label in the floor). An
/// icon-less, label-less cell (e.g. cost, whose `$` lives in the text) gets
/// no prefix — `glyph()` would otherwise emit a stray space in Icon mode.
fn cell_prefix(seg: &Segment, mode: GlyphMode) -> String {
    if seg.icon.is_empty() && seg.ascii.is_empty() {
        String::new()
    } else {
        glyph(mode, seg.icon, seg.ascii)
    }
}

/// Emit a filled segment body: ` <icon> <text> ` on the segment bg.
fn emit_body(s: &mut String, seg: &Segment, mode: GlyphMode, palette: &ThemePalette) {
    s.push_str(&bg_code(seg_bg(seg)));
    s.push_str(&fg_code(seg_fg(seg, palette)));
    s.push(' ');
    s.push_str(&cell_prefix(seg, mode));
    s.push_str(&seg.text);
    s.push(' ');
}

/// Emit a right-pointing seam after a left-cluster segment. `next` is the
/// following segment's bg (None = the cluster's pointed outer edge → default
/// terminal bg). In `Blocks` mode an inner boundary is a half-block; the outer
/// edge is flush (handled by the caller's trailing RESET).
fn emit_seam_r(s: &mut String, this_bg: u8, next: Option<u8>, tier: SeamTier) {
    match tier {
        SeamTier::Powerline => {
            s.push_str(&fg_code(this_bg));
            match next {
                Some(nb) => s.push_str(&bg_code(nb)),
                None => s.push_str(DEFAULT_BG),
            }
            s.push_str(SEAM_R);
        }
        SeamTier::Blocks => {
            if let Some(nb) = next {
                // left half = this, right half = next.
                s.push_str(&fg_code(nb));
                s.push_str(&bg_code(this_bg));
                s.push_str(HALF_R);
            }
        }
        SeamTier::AsciiFloor => {}
    }
}

/// Emit a left-pointing seam before a right-cluster segment. `prev` is the
/// preceding bg (None = the cluster's leading outer edge → default).
fn emit_seam_l(s: &mut String, this_bg: u8, prev: Option<u8>, tier: SeamTier) {
    match tier {
        SeamTier::Powerline => {
            s.push_str(&fg_code(this_bg));
            match prev {
                Some(pb) => s.push_str(&bg_code(pb)),
                None => s.push_str(DEFAULT_BG),
            }
            s.push_str(SEAM_L);
        }
        SeamTier::Blocks => {
            if let Some(pb) = prev {
                // left half = prev, right half = this.
                s.push_str(&fg_code(pb));
                s.push_str(&bg_code(this_bg));
                s.push_str(HALF_L);
            }
        }
        SeamTier::AsciiFloor => {}
    }
}

fn emit_left_cluster(
    segs: &[Segment],
    tier: SeamTier,
    mode: GlyphMode,
    palette: &ThemePalette,
) -> String {
    let mut s = String::new();
    for (i, seg) in segs.iter().enumerate() {
        emit_body(&mut s, seg, mode, palette);
        let next = segs.get(i + 1).map(seg_bg);
        emit_seam_r(&mut s, seg_bg(seg), next, tier);
    }
    if !segs.is_empty() {
        s.push_str(RESET);
    }
    s
}

fn emit_right_cluster(
    segs: &[Segment],
    tier: SeamTier,
    mode: GlyphMode,
    palette: &ThemePalette,
) -> String {
    let mut s = String::new();
    let mut prev: Option<u8> = None;
    for seg in segs {
        emit_seam_l(&mut s, seg_bg(seg), prev, tier);
        emit_body(&mut s, seg, mode, palette);
        prev = Some(seg_bg(seg));
    }
    if !segs.is_empty() {
        s.push_str(RESET);
    }
    s
}

/// Render the ASCII floor: ` <prefix><text> ` cells joined by ` | `, each in
/// its own fg colour. The bar can't read without fills, so this is the
/// honest collapse — `rail` ≈ `none`'s identity row.
fn ascii_bar(
    left: &[Segment],
    right: &[Segment],
    mode: GlyphMode,
    color: bool,
    palette: &ThemePalette,
) -> String {
    left.iter()
        .chain(right.iter())
        .map(|seg| {
            let body = format!("{}{}", cell_prefix(seg, mode), seg.text);
            crate::render::color::colorize(&body, &ascii_fg(seg, palette), color)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Render a two-cluster bar into one row. The left cluster runs identity →
/// pressure and hugs the left edge; the right cluster (the headline) is pushed
/// flush to the right edge at `target_width` — so left-hug / right-hug, like a
/// conventional Powerline/tmux bar. Rows therefore share a clean right edge
/// (all pad to `target_width`) rather than a floating mid-row axis.
pub fn render_bar(
    left: &[Segment],
    right: &[Segment],
    target_width: Option<usize>,
    tier: SeamTier,
    mode: GlyphMode,
    color: bool,
    palette: &ThemePalette,
) -> String {
    if tier == SeamTier::AsciiFloor {
        return ascii_bar(left, right, mode, color, palette);
    }
    let left_str = emit_left_cluster(left, tier, mode, palette);
    let right_str = emit_right_cluster(right, tier, mode, palette);
    if right_str.is_empty() {
        return left_str;
    }
    let used = visible_width(&left_str) + visible_width(&right_str);
    let gap = target_width
        .map(|w| w.saturating_sub(used))
        .filter(|p| *p >= 2)
        .unwrap_or(2);
    format!("{left_str}{}{right_str}", " ".repeat(gap))
}

/// Render the `anchor` hero capsule: a reverse-video chip with caps. `bg` is
/// the model role colour index. In the ASCII floor it degrades to `[text]`.
pub fn render_capsule(
    icon: &'static str,
    text: &str,
    bg: u8,
    tier: SeamTier,
    caps: CapStyle,
    mode: GlyphMode,
    color: bool,
) -> String {
    if tier == SeamTier::AsciiFloor {
        return format!(
            "[{}]",
            crate::render::color::colorize(text, &fg_code(bg), color)
        );
    }
    let (cap_l, cap_r) = match (tier, caps) {
        (SeamTier::Blocks, _) => (HALF_R, HALF_L), // reverse-cap ▐body▌
        (_, CapStyle::Angle) => (SEAM_L, SEAM_R),
        (_, CapStyle::Round) => (CAP_ROUND_L, CAP_ROUND_R),
    };
    let mut s = String::new();
    s.push_str(&fg_code(bg));
    s.push_str(cap_l);
    s.push_str(&bg_code(bg));
    s.push_str(&fg_code(TINT_FG));
    s.push(' ');
    s.push_str(&glyph(mode, icon, ""));
    s.push_str(text);
    s.push(' ');
    s.push_str(RESET);
    s.push_str(&fg_code(bg));
    s.push_str(cap_r);
    s.push_str(RESET);
    s
}
