//! Lint: every shipped built-in theme must hand-author all three aurora
//! pulse fields (`aurora_low`, `aurora_mid`, `aurora_high`) for its dark and
//! light variants, with sufficient spread between adjacent stops to read as
//! a 3-tier gradient.
//!
//! See `designs/statusline-v2-redesign.md` § "Pulseline Aurora" for the design
//! contract and per-theme seed values.
//!
//! Failure modes caught:
//! - missing field in a theme JSON (would silently fall back to
//!   completed_check / active_cyan / active_coral, defeating the purpose for
//!   built-in themes that ship the v2 widget vocabulary)
//! - typo'd value identical to its neighbour (collapses 3 stops to 2)
//! - adjacent stops too close to read as distinct gradient tiers
//!
//! Note: ANSI 256 codes are *not* perceptually monotonic on the numeric
//! scale (e.g. 51 is bright cyan, 60 is dim violet). So this lint enforces
//! **spread between adjacent stops**, not numeric ordering. Theme authors
//! pick perceptually-graduated values; the lint catches collapse, not
//! mis-ordering.

use cc_pulseline::config::ColorTheme;
use cc_pulseline::render::color::{builtin_aurora_triple, builtin_theme_names};

const MIN_ADJACENT_DELTA: i32 = 4;
const VARIANTS: &[(&str, ColorTheme)] = &[("dark", ColorTheme::Dark), ("light", ColorTheme::Light)];

/// Walk every (theme, variant) pair once with the authored aurora values.
fn for_each_authored<F: FnMut(&str, &str, Option<u8>, Option<u8>, Option<u8>)>(mut f: F) {
    for name in builtin_theme_names() {
        for (variant_label, variant) in VARIANTS {
            let (low, mid, high) = builtin_aurora_triple(name, *variant)
                .unwrap_or_else(|| panic!("built-in theme \"{name}\" not loadable"));
            f(name, variant_label, low, mid, high);
        }
    }
}

#[test]
fn every_builtin_theme_authors_aurora_triple_dark_and_light() {
    let mut missing: Vec<String> = Vec::new();
    for_each_authored(|name, variant, low, mid, high| {
        if low.is_none() || mid.is_none() || high.is_none() {
            missing.push(format!(
                "{name} ({variant}): aurora_low={low:?} aurora_mid={mid:?} aurora_high={high:?}"
            ));
        }
    });

    assert!(
        missing.is_empty(),
        "built-in themes must hand-author all 3 aurora fields (dark + light). Missing:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn every_builtin_theme_meets_min_aurora_adjacent_spread() {
    let mut violations: Vec<String> = Vec::new();
    for_each_authored(|name, variant, low, mid, high| {
        if let (Some(low), Some(mid), Some(high)) = (low, mid, high) {
            let lm = (low as i32 - mid as i32).abs();
            let mh = (mid as i32 - high as i32).abs();
            if lm < MIN_ADJACENT_DELTA || mh < MIN_ADJACENT_DELTA {
                violations.push(format!(
                    "{name} ({variant}): low={low} mid={mid} high={high} \
                     |low-mid|={lm} |mid-high|={mh} (need both ≥ {MIN_ADJACENT_DELTA})"
                ));
            }
        }
    });

    assert!(
        violations.is_empty(),
        "aurora gradient spread floor violated. Each adjacent pair must keep |Δ|≥{MIN_ADJACENT_DELTA} on the ansi256 scale:\n  {}",
        violations.join("\n  ")
    );
}
