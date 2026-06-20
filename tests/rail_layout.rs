//! `rail` v3 — grouped rows (方案 A) + the `color_budget` / `headline` dials.
//!
//! Covers the `color_budget` / `headline` dials (see `docs/layouts.md`),
//! patching the v3 grouped rows of `designs/rail-anchor-grouped-rows.md`:
//! - layout 方案 A: row1 `model·effort·cwd·git | version`, row2
//!   `ctx·tokens·cache | $cost`, row3 `5H | 7D`
//! - under the `signal` default, the three per-line **headlines** (model · cost
//!   · 7d) fill as reverse-video `Tint`s; **flags** (effort · ctx · 5h · git ·
//!   cache) add letter colour only past threshold; everything else is gray
//! - `vivid` fills every headline + lit flag (context rides a raised ramp);
//!   `mono` drops all fills for role-coloured text + `\u{e0b1}` ticks
//! - `headline` = `column` (right-hug) / `inline` (trails left); model always left
//! - git lights only the dirty marks; anti-rainbow calm fixture; height ladder;
//!   blocks / ascii degradation
//!
//! Colour detection is precise, and must be **seam-aware**: under `signal`/
//! `vivid` a `Tint` headline's seam emits its band as an *fg* (`38;5;<band>`),
//! which can spoof a naive `contains(role_fg)` check across rows. So fills are
//! pinned by their reverse-video **background** (`tint_bg` → `48;5;<band>`, which
//! seams never emit) or by the seam-proof `TINT_FG_ESC` (`38;5;16`) count; flag
//! inks are checked **per row** to dodge a neighbouring row's headline seam. ctx
//! /5h light the WHOLE value via `ramp_ink` (`whole_lit`); git stays a **partial**
//! cell (`{role}marks{secondary}` — neutral branch, lit dirty marks).

use cc_pulseline::config::{ColorBudget, GlyphMode, Headline, LayoutSeams, RenderConfig};
use cc_pulseline::render::color::{extract_ansi_code, strip_ansi, visible_width};
use cc_pulseline::render::fmt::burn_rate_per_hour;
use cc_pulseline::render::icons::PL_TICK;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{QuotaMetrics, RenderFrame, StdinPayload};
use serde_json::json;

const SEAM_R: char = '\u{e0b0}';
const HALF_R: char = '\u{2590}';
const HALF_L: char = '\u{258c}';
const TINT_FG_ESC: &str = "\x1b[38;5;16m"; // reverse-video tint text (any headline fill)

/// The reverse-video background of a `Tint` headline: `48;5;<band>`. A fill
/// emits this on its own body; seams emit the band as an *fg* (`38;5;`), so this
/// bg marker pins a fill to a specific band without seam spoofing.
fn tint_bg(escape: &str) -> String {
    format!("\x1b[48;5;{}m", extract_ansi_code(escape).unwrap())
}

fn payload() -> StdinPayload {
    serde_json::from_str(&json!({"session_id": "rail-v3"}).to_string()).unwrap()
}

fn frame(effort: &str, ctx: u64, q5: f64, q7: f64, dirty: bool, quota: bool) -> RenderFrame {
    let mut f = RenderFrame::from_payload(&payload());
    f.line1.model = "Opus 4.6".into();
    f.line1.claude_code_version = "2.1.153".into();
    f.line1.project_path = "/home/me/cc-pulseline".into();
    f.line1.git_branch = "main".into();
    f.line1.effort_level = Some(effort.into());
    f.line1.git_dirty = dirty;
    if dirty {
        f.line1.git_modified = 2;
    }
    f.line3.context_used_percentage = Some(ctx);
    f.line3.context_window_size = Some(200_000);
    f.line3.total_cost_usd = Some(3.47);
    f.line3.total_duration_ms = Some(2_700_000);
    f.line3.input_tokens = Some(12_800);
    f.line3.output_tokens = Some(24_600);
    f.line3.cache_read_tokens = Some(68_200);
    if quota {
        f.quota = QuotaMetrics {
            five_hour_pct: Some(q5),
            five_hour_reset_minutes: Some(119),
            seven_day_pct: Some(q7),
            seven_day_reset_minutes: Some(5_760),
        };
    }
    f
}

fn rail_config() -> RenderConfig {
    RenderConfig {
        pane_style: LayoutStyle::Rail,
        glyph_mode: GlyphMode::Icon,
        color_enabled: true,
        pane_seams: LayoutSeams::Powerline,
        pane_max_width: 140,
        show_model: true,
        show_effort: true,
        show_project: true,
        show_git: true,
        show_context: true,
        show_cost: true,
        show_version: true,
        terminal_width: None,
        ..Default::default()
    }
}

fn render(f: &RenderFrame, c: &RenderConfig) -> Vec<String> {
    cc_pulseline::render::layout::render_frame(f, c)
}

fn has_pua(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0xE000..=0xF8FF).contains(&u)
            || (0xF0000..=0xFFFFD).contains(&u)
            || (0x10_0000..=0x10_FFFD).contains(&u)
    })
}

/// The byte pattern of a **partial** "letter"-lit value: role fg on `value`,
/// then a restore to the neutral base (secondary). Proves value-lit AND
/// base-restored. Used for git (neutral branch + lit dirty marks); ctx/quota
/// light the whole value via `ramp_ink` — see `whole_lit`.
fn lit(role: &str, value: &str, base: &str) -> String {
    format!("{role}{value}{base}")
}

/// A **whole-value** letter-lit cell (`ramp_ink`): the role colour escape is
/// present in `row` AND the full `value` is contiguous (no ANSI escape splits
/// it). The old per-`%` splice would break that contiguity, so this guards both
/// the new mode and a regression back to partial colouring.
fn whole_lit(row: &str, role: &str, value: &str) -> bool {
    row.contains(role) && row.contains(value)
}

#[test]
fn default_renders_three_rows_layout_a() {
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &rail_config());
    assert_eq!(lines.len(), 3);
    let p: Vec<String> = lines.iter().map(|l| strip_ansi(l)).collect();
    assert!(
        p[0].contains("Opus 4.6") && p[0].contains("v2.1.153"),
        "row1 id+version: {}",
        p[0]
    );
    assert!(
        p[1].contains("43%") && p[1].contains("↓12.8k") && p[1].contains("$3.47"),
        "row2 usage+cost: {}",
        p[1]
    );
    assert!(
        p[2].contains("5H") && p[2].contains("7D"),
        "row3 quota: {}",
        p[2]
    );
}

#[test]
fn signal_fills_the_three_headlines() {
    // Under the `signal` default, exactly the three per-line headlines fill as
    // reverse-video Tints — model (r1) · cost (r2) · 7d (r3) — and nothing else.
    let config = rail_config();
    let p = &config.palette;
    let s = render(&frame("high", 72, 62.0, 90.0, true, true), &config).join("\n");
    assert_eq!(
        s.matches(TINT_FG_ESC).count(),
        3,
        "model + cost + 7d fill; no left cell tints: {s}"
    );
    // each headline carries its own band as the reverse-video background.
    assert!(
        s.contains(&tint_bg(&p.stable_blue)),
        "model fills (stable_blue)"
    );
    let rate = burn_rate_per_hour(3.47, Some(2_700_000));
    assert!(
        s.contains(&tint_bg(p.color_for_burn_rate(rate))),
        "cost fills (burn band)"
    );
    assert!(
        s.contains(&tint_bg(p.color_for_quota_pct(90.0))),
        "7d fills (quota band)"
    );
}

#[test]
fn cost_headline_fills_with_its_burn_band() {
    // cost is the usage-row headline → under `signal` it fills as a reverse-video
    // Tint whose background is the burn-rate band. (It was `ramp_ink` letter in
    // v3; the signal default flips it to a fill.)
    let config = rail_config();
    let p = &config.palette;
    let rate = burn_rate_per_hour(3.47, Some(2_700_000));
    let s = render(&frame("low", 40, 20.0, 20.0, false, true), &config).join("\n");
    assert!(
        s.contains(&tint_bg(p.color_for_burn_rate(rate))),
        "cost fills with its burn band bg: {s}"
    );
    assert!(strip_ansi(&s).contains("$3.47"), "cost value present");
}

#[test]
fn no_color_renders_plain_with_no_escapes() {
    // Colour off → the ASCII floor: must emit ZERO ANSI escapes (a leaked fg with
    // no RESET would corrupt the line). Exercises ramp_ink + lit_value, color off.
    let mut config = rail_config();
    config.color_enabled = false;
    let s = render(&frame("high", 72, 62.0, 90.0, true, true), &config).join("\n");
    assert!(!s.contains('\x1b'), "NO_COLOR emits zero ANSI escapes");
    assert!(
        s.contains("72%") && s.contains("$3.47"),
        "content still renders plain"
    );
}

#[test]
fn git_staged_count_is_neutral_only_dirty_marks_light() {
    let mut f = frame("low", 40, 20.0, 20.0, true, true);
    f.line1.git_added = 3;
    f.line1.git_modified = 0; // staged only, dirty tree
    let config = rail_config();
    let p = &config.palette;
    let s = render(&f, &config).join("\n");
    assert!(
        strip_ansi(&s).contains("main +3"),
        "branch + staged rendered"
    );
    // only the ` *` dirty mark lights; the `+3` staged count stays neutral.
    assert!(
        s.contains(&lit(&p.alert_orange, " *", &p.secondary)),
        "dirty mark lights orange"
    );
    assert!(
        !s.contains(&format!("{} +3", p.alert_orange)),
        "staged count is NOT orange"
    );
}

#[test]
fn ctx_lights_the_whole_value_past_threshold() {
    let config = rail_config();
    let p = &config.palette;
    let lines = render(&frame("low", 72, 20.0, 20.0, false, true), &config);
    let usage = &lines[1];
    // 72% ≥ 55 → the WHOLE value (incl. denominator) lights in the ctx colour,
    // contiguous (no mid-value restore). effort=low / quota=20 keep the rest calm.
    assert!(
        whole_lit(usage, p.color_for_ctx_pct(72), "72% 144.0k/200.0k"),
        "ctx whole value lit + contiguous: {usage}"
    );
}

#[test]
fn calm_fixture_lights_only_the_headlines() {
    // effort=low, ctx=40, quota 20/20, clean tree, cold cache. The anti-rainbow
    // guard: under `signal` the only fills are the three headlines; no left
    // FLAG inks (effort/ctx/5h below threshold, clean git, cold cache).
    let config = rail_config();
    let p = &config.palette;
    let mut f = frame("low", 40, 20.0, 20.0, false, true);
    f.line3.cache_read_tokens = None; // cold cache → no efficiency flare
    let lines = render(&f, &config);
    let s = lines.join("\n");
    // exactly the 3 headlines fill — no left cell crossed into a Tint.
    assert_eq!(
        s.matches(TINT_FG_ESC).count(),
        3,
        "only the 3 headlines fill: {s}"
    );
    assert!(
        s.contains(&tint_bg(&p.stable_blue)),
        "model is one of the fills"
    );
    // effort=low → no high (amber) flare; clean tree → no git orange. Both bands
    // are absent everywhere (they collide with no headline band in this fixture).
    assert!(
        !s.contains(p.color_for_effort_level("high")),
        "no effort flare when low"
    );
    assert!(
        !s.contains(p.alert_orange.as_str()),
        "no git signal when clean"
    );
    // ctx 40 < 55 → its usage row carries no ctx band. Checked on row 1 alone:
    // the 7d headline's green Tint seam (quota_pct(20) == ctx_good) lives on the
    // quota row, so a joined-string check would false-positive.
    assert!(
        !lines[1].contains(p.color_for_ctx_pct(40)),
        "ctx below 55 not lit on its row: {}",
        lines[1]
    );
    // A wrongly-inked unlit flag is a ramp_ink (letter fg), NOT a Tint — so the
    // count==3 guard above can't see it. Pin each below-threshold left flag with
    // a per-row negative check. effort=low → structural (103); it appears nowhere
    // on the identity row when the cell stays neutral.
    assert!(
        !lines[0].contains(p.color_for_effort_level("low")),
        "effort below high not inked on its row: {}",
        lines[0]
    );
    // 5h=20 → quota band == ctx_good (71). The 7d(20%) headline Tint's seam emits
    // that band as an fg exactly once on the quota row; a wrongly-inked 5h would
    // make it appear twice. So exactly one occurrence proves 5h stayed neutral.
    let green = p.color_for_quota_pct(20.0);
    assert_eq!(
        lines[2].matches(green).count(),
        1,
        "5h below 50 not inked — the lone green is the 7d headline seam: {}",
        lines[2]
    );
}

#[test]
fn high_fixture_lights_each_crossing_signal() {
    let config = rail_config();
    let p = &config.palette;
    let lines = render(&frame("high", 72, 62.0, 90.0, true, true), &config);
    // effort lit on the identity row (letter ink); ctx + 5h are lit FLAGS (whole
    // value letter-lit). 7d is the row HEADLINE → a reverse-video Tint, not a
    // flag, so it's checked by its band background, not an fg escape.
    assert!(
        lines[0].contains(p.color_for_effort_level("high")),
        "effort word lit ≥high: {}",
        lines[0]
    );
    assert!(
        whole_lit(&lines[1], p.color_for_ctx_pct(72), "72% 144.0k/200.0k"),
        "ctx whole value lit: {}",
        lines[1]
    );
    assert!(
        whole_lit(&lines[2], p.color_for_quota_pct(62.0), "5H 62%"),
        "5h whole value lit ≥50: {}",
        lines[2]
    );
    assert!(
        lines[2].contains(&tint_bg(p.color_for_quota_pct(90.0))),
        "7d headline fills with its quota band: {}",
        lines[2]
    );
    assert!(
        strip_ansi(&lines[2]).contains("7D 90%"),
        "7d value present: {}",
        lines[2]
    );
}

#[test]
fn vivid_fills_every_banded_or_lit_cell() {
    let mut config = rail_config();
    config.pane_color_budget = ColorBudget::Vivid;
    let p = config.palette.clone();
    // hot fixture: headlines + lit flags all fill (model · effort · ctx · cost ·
    // 5h · 7d). Context cells (version/tokens) ride the RAISED ramp (238).
    let s = render(&frame("high", 72, 62.0, 90.0, true, true), &config).join("\n");
    assert!(s.contains(&tint_bg(&p.stable_blue)), "model fills");
    assert!(
        s.contains(&tint_bg(p.color_for_effort_level("high"))),
        "lit effort flag fills under vivid"
    );
    assert!(
        s.contains(&tint_bg(p.color_for_ctx_pct(72))),
        "lit ctx flag fills under vivid"
    );
    assert!(
        s.contains(&tint_bg(p.color_for_quota_pct(62.0))),
        "lit 5h flag fills under vivid"
    );
    assert!(
        s.contains("\x1b[48;5;238m"),
        "context cells ride the raised ramp (238): {s}"
    );
    // Q1: a BELOW-threshold flag does NOT fill — it rides raised, like context.
    let mut calm = frame("low", 40, 20.0, 20.0, false, true);
    calm.line3.cache_read_tokens = None;
    let c = render(&calm, &config).join("\n");
    assert!(
        !c.contains(&tint_bg(p.color_for_effort_level("low"))),
        "unlit effort does not fill in vivid (rides raised): {c}"
    );
}

#[test]
fn mono_emits_no_fills_and_uses_ticks() {
    let mut config = rail_config();
    config.pane_color_budget = ColorBudget::Mono;
    let p = config.palette.clone();
    let s = render(&frame("high", 72, 62.0, 90.0, true, true), &config).join("\n");
    // no fills anywhere — zero background bytes.
    assert!(!s.contains("\x1b[48;5;"), "mono emits no bg fills: {s}");
    // thin powerline ticks join the cells (Powerline tier).
    assert!(s.contains(PL_TICK), "mono uses PL_TICK ticks");
    // headlines + lit flags still carry their role colour as fg.
    let rate = burn_rate_per_hour(3.47, Some(2_700_000));
    assert!(
        s.contains(p.color_for_burn_rate(rate)),
        "cost role fg present"
    );
    assert!(s.contains(p.color_for_ctx_pct(72)), "ctx role fg present");
    // headline placement makes NO difference to mono bytes.
    let mut col = config.clone();
    col.pane_headline = Headline::Column;
    let mut inl = config.clone();
    inl.pane_headline = Headline::Inline;
    let f = frame("high", 72, 62.0, 90.0, true, true);
    assert_eq!(
        render(&f, &col).join("\n"),
        render(&f, &inl).join("\n"),
        "mono ignores headline placement"
    );
}

#[test]
fn headline_column_shares_right_edge_inline_trails() {
    let mut config = rail_config();
    config.terminal_width = Some(120); // a target is required for right-hug.
    let f = frame("high", 43, 62.0, 41.0, false, true);

    config.pane_headline = Headline::Column;
    let col = render(&f, &config);
    // column right-hugs every row to the same target → all rows share a width.
    assert_eq!(
        visible_width(&col[0]),
        visible_width(&col[1]),
        "column rows share the right edge"
    );
    assert_eq!(visible_width(&col[1]), visible_width(&col[2]));

    config.pane_headline = Headline::Inline;
    let inl = render(&f, &config);
    // inline is content-width: the usage row is narrower than its padded column form.
    assert!(
        visible_width(&inl[1]) < visible_width(&col[1]),
        "inline is content-width, not padded to the target"
    );

    // model leads the identity row in BOTH placements (never moves right).
    for row in [&col[0], &inl[0]] {
        let r = strip_ansi(row);
        assert!(
            r.find("Opus 4.6").unwrap() < r.find("v2.1.153").unwrap(),
            "model leads identity: {r}"
        );
    }
}

#[test]
fn fused_row_honours_color_budget() {
    let mut config = rail_config();
    config.max_total_lines = Some(1);
    let f = frame("high", 72, 62.0, 90.0, true, true);

    // signal: the fused bar fills its two headlines (model + cost); 7d isn't in it.
    let sig = render(&f, &config).join("\n");
    assert_eq!(
        sig.matches(TINT_FG_ESC).count(),
        2,
        "fused signal: model + cost fill: {sig}"
    );

    // vivid: lit flags fill and context rides the raised ramp, on the one row.
    config.pane_color_budget = ColorBudget::Vivid;
    let p = config.palette.clone();
    let vivid = render(&f, &config).join("\n");
    assert!(
        vivid.contains(&tint_bg(p.color_for_effort_level("high"))),
        "fused vivid: lit effort fills: {vivid}"
    );
    assert!(
        vivid.contains("\x1b[48;5;238m"),
        "fused vivid: context rides raised ramp: {vivid}"
    );

    // mono: no fills, ticks present.
    config.pane_color_budget = ColorBudget::Mono;
    let mono = render(&f, &config).join("\n");
    assert!(!mono.contains("\x1b[48;5;"), "fused mono: no fills: {mono}");
    assert!(mono.contains(PL_TICK), "fused mono: ticks present");
}

#[test]
fn floor_ignores_headline_placement() {
    // The ASCII floor (GlyphMode::Ascii) and NO_COLOR flatten left+right into one
    // ` | `-joined run, so `column` and `inline` must be byte-identical there —
    // placement never leaks into the floor.
    let f = frame("high", 72, 62.0, 90.0, true, true);
    for set_floor in [
        (|c: &mut RenderConfig| c.glyph_mode = GlyphMode::Ascii) as fn(&mut RenderConfig),
        |c: &mut RenderConfig| c.color_enabled = false,
    ] {
        let mut col = rail_config();
        set_floor(&mut col);
        col.pane_headline = Headline::Column;
        let mut inl = rail_config();
        set_floor(&mut inl);
        inl.pane_headline = Headline::Inline;
        assert_eq!(
            render(&f, &col).join("\n"),
            render(&f, &inl).join("\n"),
            "floor ignores headline placement"
        );
    }
}

#[test]
fn git_lights_only_the_dirty_marks() {
    let config = rail_config();
    let p = &config.palette;
    let s = render(&frame("low", 40, 20.0, 20.0, true, true), &config).join("\n");
    // dirty (modified=2) → " ~2 *" lit orange; branch + staged stay neutral.
    assert!(
        s.contains(&lit(&p.alert_orange, " ~2 *", &p.secondary)),
        "only dirty marks lit: {s}"
    );
    assert!(
        strip_ansi(&s).contains("main"),
        "branch text present (neutral)"
    );
}

#[test]
fn max_width_caps_the_bar() {
    let mut config = rail_config();
    config.terminal_width = Some(200); // → 196 after margin, capped to 140
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    for l in &lines {
        assert_eq!(
            visible_width(l),
            140,
            "bar capped at max_width, not spread to 196"
        );
    }
}

#[test]
fn max_total_lines_one_is_fused_bar() {
    let mut config = rail_config();
    config.max_total_lines = Some(1);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    assert_eq!(lines.len(), 1);
    let bar = strip_ansi(&lines[0]);
    for needle in ["Opus 4.6", "43%", "$3.47", "v2.1.153"] {
        assert!(bar.contains(needle), "fused bar missing {needle}: {bar}");
    }
}

#[test]
fn max_total_lines_two_drops_quota() {
    let mut config = rail_config();
    config.max_total_lines = Some(2);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    assert_eq!(lines.len(), 2);
    let joined: String = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !joined.contains("5H") && !joined.contains("7D"),
        "quota is the dropped row"
    );
}

#[test]
fn quota_less_renders_two_rows() {
    let lines = render(&frame("high", 43, 0.0, 0.0, false, false), &rail_config());
    assert_eq!(lines.len(), 2);
}

#[test]
fn usage_row_drops_cleanly_when_no_api_data() {
    let mut f = frame("high", 43, 62.0, 41.0, false, true);
    f.line3.context_used_percentage = None;
    f.line3.input_tokens = None;
    f.line3.output_tokens = None;
    f.line3.cache_read_tokens = None;
    f.line3.total_cost_usd = None;
    let lines = render(&f, &rail_config());
    assert!(
        lines.iter().all(|l| visible_width(l) > 0),
        "no blank rows: {lines:?}"
    );
    assert!(lines.len() < 3, "the empty usage row dropped: {lines:?}");
}

#[test]
fn blocks_tier_uses_half_block_not_seam_glyph() {
    let mut config = rail_config();
    config.pane_seams = LayoutSeams::Blocks;
    let s = render(&frame("high", 72, 62.0, 90.0, true, true), &config).join("\n");
    assert!(
        s.contains(HALF_R) || s.contains(HALF_L),
        "blocks emits half-blocks"
    );
    assert!(!s.contains(SEAM_R), "blocks emits no PUA seam glyph");
}

#[test]
fn ascii_floor_no_pua_and_letter_survives() {
    let mut config = rail_config();
    config.glyph_mode = GlyphMode::Ascii;
    let p = config.palette.clone();
    let s = render(&frame("high", 72, 62.0, 90.0, true, true), &config).join("\n");
    assert!(!has_pua(&s), "ASCII floor has zero PUA: {s:?}");
    assert!(
        !s.contains("\x1b[48;5;"),
        "no background fills in the floor"
    );
    assert!(
        s.contains(p.color_for_ctx_pct(72)),
        "ctx letter colour survives as fg"
    );
}

#[test]
fn below_min_width_falls_back_to_flat() {
    let mut config = rail_config();
    config.terminal_width = Some(40);
    let s = render(&frame("high", 43, 62.0, 41.0, false, true), &config).join("\n");
    assert!(
        !s.contains("\x1b[48;5;"),
        "flat fallback has no powerline bg fills"
    );
}

#[test]
fn fit_row_drops_low_priority_cells_under_width_pressure() {
    // A width above min_width but below the identity row's natural width forces
    // fit_row's per-cell drop ladder: cwd sheds first (drops<1), model + version
    // never drop. (Guards the newly-written drop loop the old narrow_width test
    // used to cover.)
    let mut config = rail_config();
    config.terminal_width = Some(84);
    let mut f = frame("high", 43, 62.0, 41.0, false, true);
    f.line1.project_path = "/home/me/a-very-long-project-directory-name".into();
    f.line1.git_branch = "feature/some-long-branch-name".into();
    let lines = render(&f, &config);
    let id = strip_ansi(&lines[0]);
    assert!(id.contains("Opus 4.6"), "model survives the drop: {id}");
    assert!(
        !id.contains("a-very-long-project-directory-name"),
        "cwd shed first under width pressure: {id}"
    );
    assert!(
        visible_width(&lines[0]) <= 84,
        "identity row fit to the capped target: {}",
        visible_width(&lines[0])
    );
}

#[test]
fn pre_api_cost_only_usage_row_is_dropped() {
    // Session start: cost is present (0.0) but no API usage yet (ctx/tokens/cache
    // all None). The usage row must NOT render a lone right-flushed `$0.00` — it
    // drops like any blank row.
    let mut config = rail_config();
    config.terminal_width = Some(120); // a target, so column would right-flush.
    let mut f = frame("high", 43, 62.0, 41.0, false, true);
    f.line3.context_used_percentage = None;
    f.line3.input_tokens = None;
    f.line3.output_tokens = None;
    f.line3.cache_read_tokens = None;
    f.line3.total_cost_usd = Some(0.0);
    let joined: String = render(&f, &config)
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !joined.contains("$0.00"),
        "no lone floating $0.00 pre-API: {joined:?}"
    );
    assert!(joined.contains("Opus 4.6"), "identity row still renders");
}

// ── arrangement: rail_*_order / rail_*_hero ─────────────────────────────────

fn order(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn empty_arrangement_equals_built_in_default() {
    // The byte guard for the arrangement refactor: an explicit default order +
    // hero must render byte-for-byte identical to leaving the config empty.
    let f = frame("high", 72, 62.0, 90.0, true, true);
    let default = render(&f, &rail_config());
    let mut explicit = rail_config();
    explicit.rail_identity_order = order(&["model", "effort", "cwd", "git", "version"]);
    explicit.rail_usage_order = order(&["ctx", "compact", "tokens", "cache", "cost"]);
    explicit.rail_quota_order = order(&["5h", "7d"]);
    explicit.rail_identity_hero = "model".into();
    explicit.rail_usage_hero = "cost".into();
    explicit.rail_quota_hero = "7d".into();
    assert_eq!(
        default,
        render(&f, &explicit),
        "explicit default == built-in"
    );
}

#[test]
fn compaction_count_shows_on_usage_row_when_nonzero() {
    let config = rail_config();
    // No compaction → no marker; the default look is unchanged.
    let f0 = frame("high", 43, 62.0, 41.0, false, true);
    assert!(
        !render(&f0, &config).join("\n").contains('\u{27f3}'),
        "no ⟳ marker at compact_count 0"
    );
    // After compactions → ⟳N on the usage row.
    let mut f = frame("high", 43, 62.0, 41.0, false, true);
    f.compact_count = 3;
    let usage = strip_ansi(&render(&f, &config)[1]);
    assert!(usage.contains("\u{27f3} 3"), "⟳ 3 on usage row: {usage}");
}

#[test]
fn order_reorders_cells_within_a_row() {
    // Move cost to the front of the usage row: it now leads (still filled), and
    // the rightmost listed cell (cache) takes the right axis.
    let mut config = rail_config();
    config.rail_usage_order = order(&["cost", "ctx", "tokens", "cache"]);
    let p = config.palette.clone();
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    let usage = strip_ansi(&lines[1]);
    assert!(
        usage.find("$3.47").unwrap() < usage.find("43%").unwrap(),
        "cost moved before ctx: {usage}"
    );
    // cost is still the hero → still fills as a Tint.
    let rate = burn_rate_per_hour(3.47, Some(2_700_000));
    assert!(
        lines[1].contains(&tint_bg(p.color_for_burn_rate(rate))),
        "cost still fills after reorder"
    );
}

#[test]
fn hero_swap_fills_the_chosen_cell_and_demotes_the_old_one() {
    // usage_hero = ctx → ctx fills (Tint); the displaced cost falls back to a
    // letter flag (inks its burn band as fg, no fill).
    let mut config = rail_config();
    config.rail_usage_hero = "ctx".into();
    let p = config.palette.clone();
    // ctx=72 → critical band; 7d=41 → a different (green) band, so the ctx fill
    // bg is unambiguous on the usage row.
    let s = render(&frame("high", 72, 62.0, 41.0, false, true), &config).join("\n");
    assert!(
        s.contains(&tint_bg(p.color_for_ctx_pct(72))),
        "ctx fills as the usage hero: {s}"
    );
    let rate = burn_rate_per_hour(3.47, Some(2_700_000));
    assert!(
        !s.contains(&tint_bg(p.color_for_burn_rate(rate))),
        "displaced cost no longer fills"
    );
    assert!(
        s.contains(p.color_for_burn_rate(rate)),
        "displaced cost still inks its burn band as a flag"
    );
}

#[test]
fn order_omission_hides_a_cell() {
    // Listing only model + version drops effort / cwd / git from the identity row.
    let mut config = rail_config();
    config.rail_identity_order = order(&["model", "version"]);
    let lines = render(&frame("high", 43, 62.0, 41.0, true, true), &config);
    let id = strip_ansi(&lines[0]);
    assert!(
        id.contains("Opus 4.6") && id.contains("v2.1.153"),
        "kept: {id}"
    );
    assert!(
        !id.contains("cc-pulseline") && !id.contains("main"),
        "cwd + git dropped by omission: {id}"
    );
}

#[test]
fn unknown_cell_name_is_skipped_not_fatal() {
    // A typo'd cell name warns (stderr) and is skipped; the rest still render.
    let mut config = rail_config();
    config.rail_usage_order = order(&["ctx", "bogus", "cost"]);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    let usage = strip_ansi(&lines[1]);
    assert!(
        usage.contains("43%") && usage.contains("$3.47"),
        "valid cells render, bogus skipped: {usage}"
    );
    assert!(!usage.contains("bogus"), "unknown name not rendered");
}

#[test]
fn quota_row_renders_with_only_seven_day() {
    // Regression guard: the usage-only "drop when left empty" rule must NOT drop
    // the quota row when only the 7d window is present (5h absent).
    let mut f = frame("high", 43, 62.0, 90.0, true, true);
    f.quota.five_hour_pct = None;
    f.quota.five_hour_reset_minutes = None;
    let lines = render(&f, &rail_config());
    let joined: String = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("7D 90%"), "lone 7d still renders: {joined}");
    assert!(!joined.contains("5H"), "5h absent");
}
