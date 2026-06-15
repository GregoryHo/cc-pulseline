//! Architectural guard for the Visual Dispatch Hub Pattern.
//!
//! Iron rule (`.claude/rules/patterns.md`): layouts never call
//! `widgets::*::render` directly — they go through the per-segment
//! hub in `frames/shared.rs`. Direct calls bypass user override
//! capability for that segment.
//!
//! Allowlist:
//! - `shared.rs` IS the hub.
//! - `ledger.rs` owns its full pipeline (sparkline aurora fill is
//!   chosen by the layout from CTX consumption velocity; the gauge
//!   cell is composed inline with bespoke ITEM_GAP spacing). See the
//!   ledger note in `CLAUDE.md`.
//!
//! Anything else that calls `widgets::gauge::render` or
//! `widgets::sparkline::render` is breaking the rule and should be
//! refactored to dispatch through `render_context_visual` /
//! `render_quota_visual`.

use std::fs;

const FRAMES_DIR: &str = "src/render/frames";
const ALLOWED_DIRECT_WIDGET_CALLERS: &[&str] = &["shared.rs", "ledger.rs"];
const WIDGETS_TO_GUARD: &[&str] = &["gauge", "sparkline", "effort", "plot"];

#[test]
fn no_direct_widget_render_outside_dispatch_hub() {
    for entry in fs::read_dir(FRAMES_DIR).expect("frames dir exists") {
        let path = entry.expect("readable entry").path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if !name.ends_with(".rs") || ALLOWED_DIRECT_WIDGET_CALLERS.contains(&name) {
            continue;
        }

        let src = fs::read_to_string(&path).expect("readable file");
        for widget in WIDGETS_TO_GUARD {
            let needle = format!("widgets::{widget}::render");
            assert!(
                !src.contains(&needle),
                "{}: direct `{}` call detected — must dispatch through \
                 `render_context_visual` / `render_quota_visual` in \
                 `frames/shared.rs`. See `.claude/rules/patterns.md` \
                 \"Visual Dispatch Hub Pattern\".",
                path.display(),
                needle,
            );
        }
    }
}
