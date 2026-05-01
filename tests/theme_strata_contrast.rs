//! Lint: every shipped built-in theme must hand-author both `strata_state`
//! and `strata_activity` for its dark and light variants, with the chrome
//! values at least Δ ≥ 3 apart on the ansi256 scale.
//!
//! See `designs/tonal-strata-redesign.md` for the design contract.
//!
//! Failure modes caught:
//! - missing field in a theme JSON (would silently fall back to separator/
//!   structural, defeating the purpose for built-ins)
//! - typo'd value identical to its neighbor
//! - value too close to the other tier to read as a chrome lift

use cc_pulseline::config::ColorTheme;
use cc_pulseline::render::color::{builtin_strata_pair, builtin_theme_names};

const MIN_DELTA: i32 = 3;
const VARIANTS: &[(&str, ColorTheme)] = &[("dark", ColorTheme::Dark), ("light", ColorTheme::Light)];

/// Walk every (theme, variant) pair once with the authored strata values.
fn for_each_authored<F: FnMut(&str, &str, Option<u8>, Option<u8>)>(mut f: F) {
    for name in builtin_theme_names() {
        for (variant_label, variant) in VARIANTS {
            let (state, activity) = builtin_strata_pair(name, *variant)
                .unwrap_or_else(|| panic!("built-in theme \"{name}\" not loadable"));
            f(name, variant_label, state, activity);
        }
    }
}

#[test]
fn every_builtin_theme_authors_strata_pair_dark_and_light() {
    let mut missing: Vec<String> = Vec::new();
    for_each_authored(|name, variant, state, activity| {
        if state.is_none() || activity.is_none() {
            missing.push(format!(
                "{name} ({variant}): strata_state={state:?} strata_activity={activity:?}"
            ));
        }
    });

    assert!(
        missing.is_empty(),
        "built-in themes must hand-author both strata fields (dark + light). Missing:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn every_builtin_theme_meets_min_strata_delta() {
    let mut violations: Vec<String> = Vec::new();
    for_each_authored(|name, variant, state, activity| {
        if let (Some(state), Some(activity)) = (state, activity) {
            let delta = (state as i32 - activity as i32).abs();
            if delta < MIN_DELTA {
                violations.push(format!(
                    "{name} ({variant}): strata_state={state} strata_activity={activity} Δ={delta} (need Δ≥{MIN_DELTA})"
                ));
            }
        }
    });

    assert!(
        violations.is_empty(),
        "strata contrast floor violated. Each theme must keep |state−activity|≥{MIN_DELTA} on the ansi256 scale:\n  {}",
        violations.join("\n  ")
    );
}
