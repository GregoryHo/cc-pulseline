---
name: release-bump
description: Use this skill when the user wants to bump the cc-pulseline crate version — triggers on "bump version", "release X.Y.Z", "prepare release", "ship 1.X", "tag a release", or any phrasing that signals a version is about to be cut. Always invoke this skill before touching Cargo.toml or CHANGELOG.md, even if the user names a specific file directly — the routine catches the version sites that are easy to miss (Cargo.lock, plugin manifests, README CLI banner, test assertions) and the doc rot that systematically appears between releases. Do NOT use this skill for npm-package version bumps in `npm/*/package.json` (those are CI-driven `0.0.0` placeholders) or for unrelated dependency upgrades.
---

# Release bump routine — cc-pulseline

This skill codifies the version-bump workflow that produced 1.0.6 → 1.1.0. It exists because version bumps in this repo touch **6 files** in three different formats and the docs need a cross-audit that's easy to skip when you're focused on `Cargo.toml`. The routine is also designed to catch a recurring failure mode: shipping with stale "8 themes" / "28 fields" / "[████▎]" descriptions that no longer match the code.

The routine does NOT auto-commit. It stops at "ready for review" and the user decides commit / push / tag timing.

## When to use this

Trigger on any of:
- "bump version", "bump to 1.X", "release 1.X.Y", "prepare release"
- "tag a release", "ship X.Y.Z", "cut a release"
- The user explicitly references this skill or `/release-bump`

If the user just asks to edit `Cargo.toml` directly, still run this skill — silent edits to `Cargo.toml` without the rest of the routine are how releases ship with mismatched plugin manifests, broken `--version` test assertions, and outdated CHANGELOG entries.

## Phase A — Align before touching anything

Before any file edits, surface the four decisions that block the rest of the routine. Don't proceed until the user answers (or explicitly defers).

1. **Target version.** Confirm `MAJOR.MINOR.PATCH`. Look at the previous tag (`git tag --sort=-v:refname | head -3`) and the nature of changes on the branch. If breaking changes exist (config renames, removed features), surface the choice between MINOR-with-BREAKING-callout and MAJOR — both are valid; the user picks.

2. **Branch state audit.** Run in parallel:
   ```bash
   git log --oneline main..HEAD | wc -l                   # commit count
   git log --oneline main..HEAD                           # commit list
   git diff --stat main..HEAD | tail -20                  # change scale
   git status                                              # untracked / dirty state
   ```
   Summarize the branch in 4-6 thematic bullets (new layout? widget rewrite? schema changes? breaking renames?). If 50+ commits, the existing CHANGELOG `[Unreleased]` section is almost certainly stale and needs a rewrite, not an append (see Phase C).

3. **Untracked files in `designs/`.** Per `designs/README.md` policy, design docs are deleted once their content is absorbed into tracked docs. Read each untracked design doc and ask: "Is this content already in `docs/layouts.md` / `docs/architecture.md` / `docs/theme-palette.md` / `CLAUDE.md`?" If yes, **delete the design doc**, don't commit it. If something is genuinely worth preserving but not yet absorbed, extract it to the right tracked doc first, then delete.

4. **Historical version notes in tracked docs.** Search for the previous version string in tracked docs:
   ```bash
   grep -rn "v<PREV_VERSION>\|<PREV_VERSION>" docs/ README.md CLAUDE.md --include="*.md"
   ```
   Some hits are legitimate history (e.g. `docs/metrics-reference.md` "v1.0.4 cache schema change") — leave those. Others are mis-dated (e.g. `docs/layouts.md` "Removed in v1.0.6" when the removal actually happened on **this** branch). Ask the user which class each hit falls into when ambiguous.

## Phase B — Implementation ↔ docs cross-audit

This is the phase that catches doc rot. **Always read code as the source of truth, never trust the doc number.** The audit pattern: measure from code → diff against docs → list mismatches.

Run this checklist verbatim for cc-pulseline. The rationale column explains *why* each item recurs — keep that in mind so you can extend the audit if the codebase grows new dimensions.

| Measurement | Source of truth (code) | Doc sites that go stale |
|---|---|---|
| Built-in theme count | `ls src/themes/*.json \| wc -l` | `README.md` Features list + THEMES section; `docs/theme-palette.md` "Built-in Themes" table |
| `ThemePalette` field count | `grep -cE "^    pub [a-z_]+:" src/render/color.rs` (count `String` and `u8` fields together) | `docs/theme-palette.md` "Tier Summary" + "REQUIRED — the N ANSI codes" + JSON schema field list |
| Layouts list | `pub enum LayoutStyle` in `src/render/pane.rs` | `README.md` "Layouts & Visual Composition"; `docs/architecture.md` `LayoutStyle` mention; `docs/layouts.md` catalog |
| Widgets list | `ls src/render/widgets/*.rs` (excl. `mod.rs`) | `docs/architecture.md` `widgets/` description; `CLAUDE.md` `widgets/` description |
| Widget visual form | top-of-file doc comment in each `widgets/*.rs` | `docs/architecture.md`, `docs/layouts.md`, `CLAUDE.md` widget descriptions |
| CTX threshold marks | `ThemePalette::ctx_marks()` return value in `src/render/color.rs` | `docs/layouts.md` "Recognized widgets per segment" CTX row |
| Quota threshold marks | `render_quota_visual` body in `src/render/frames/shared.rs` | `docs/layouts.md` quota visual description |
| Config TOML keys | `serde` field names on `*SegmentConfig` structs in `src/config.rs` | `README.md` example config; `docs/layouts.md` example TOML; default-config template strings in `src/main.rs` |
| Stdin payload schema | `StdinPayload` and nested structs in `src/types.rs` | `.claude/rules/integration.md` Schema section |

For each row, surface mismatches in a single report block (file → line → wrong claim → correct value) so the user can scan it. Don't fix yet — present, then fix in Phase D.

**Why specific numbers go stale faster than prose:** if a doc says "8 themes", changing the code to add a 9th theme doesn't mechanically force the writer to update the doc. Prose like "see `src/themes/` for the full list" is doc-rot-immune by construction. When you fix doc rot, prefer rewriting toward this pattern when it doesn't lose useful information.

## Phase C — Rewrite CHANGELOG, don't append

For releases that span 50+ commits, the existing `[Unreleased]` section is almost certainly written from the *intermediate* state of the branch — describing features that were later renamed, deleted, or superseded. Users coming from the previous tag never see those intermediate states; they see only the *end state*. **Rewrite the section from the end-state-vs-prior-tag perspective.**

Concretely:
- A widget that was added in commit 5 and deleted in commit 47 should not appear in the CHANGELOG at all.
- A config field renamed twice on the branch should be described as one final rename (`old_name` → `new_name`), not two.
- A layout added then removed mid-branch is invisible to users; skip it.

Use Keep-a-Changelog sections in this order: **Added**, **Changed**, **Removed**, **Fixed**. If the release contains breaking changes:
- Add a `> **⚠ Breaking changes**` callout block at the top of the version section explaining migration.
- Prefix each breaking entry inside `### Changed` / `### Removed` with `**BREAKING:**` so readers who skim a single section also catch it.

After writing the section, **add the compare link to the bottom** of `CHANGELOG.md`:
```markdown
[NEW]: https://github.com/<org>/<repo>/compare/v<PREV>...v<NEW>
```

## Phase D — Bump versions across the 6 known sites

These are the sites that hold the version string in cc-pulseline. Edit them all in one logical pass, then run `cargo build` to sync `Cargo.lock`.

| # | File | What to change |
|---|---|---|
| 1 | `Cargo.toml` | `version = "<OLD>"` → `version = "<NEW>"` (line 3) |
| 2 | `Cargo.lock` | Auto-synced by `cargo build`; do not hand-edit |
| 3 | `.claude-plugin/plugin.json` | `"version": "<OLD>"` → `"<NEW>"` |
| 4 | `.claude-plugin/marketplace.json` | `"version": "<OLD>"` → `"<NEW>"` (inside `plugins[0]`) |
| 5 | `README.md` | CLI banner line: `cc-pulseline <OLD> - High-performance Claude Code statusline` → new version |
| 6 | `tests/cli_flags.rs` | Two `assert!(stdout.contains("<OLD>")...)` lines (currently L76 + L89) |

After editing, run `cargo build` (not `cargo build --release` — debug is faster and Cargo.lock updates the same way). Verify `Cargo.lock` now shows `name = "cc-pulseline"` followed by `version = "<NEW>"`:

```bash
grep -A1 'name = "cc-pulseline"' Cargo.lock | head
```

### What NOT to bump

- **`npm/main/package.json` and all `npm/platforms/*/package.json`** — these are placeholders pinned at `"0.0.0"` and are rewritten by the CI publish workflow. Editing them by hand creates a CI conflict.
- **Any `version` field in `Cargo.lock` other than the `cc-pulseline` package's own** — those are dependency versions, not the crate version.
- **`docs/metrics-reference.md` references to `v1.0.4` cache schema** (or similar past-version mentions) — those are historical commentary, not current-version claims.

## Phase E — Release-readiness verification

Run the four checks in parallel where possible. The first three must be silent / zero-warning / all-green. The smoke test must emit ANSI output and the new version string.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

After release build:
```bash
./target/release/cc-pulseline --version       # must show "cc-pulseline <NEW>"
echo '{"session_id":"smoke","version":"2.1.119","model":{"id":"sonnet","display_name":"Sonnet"}}' | \
  ./target/release/cc-pulseline                # must render 3 lines of ANSI output
```

If `cargo test` reports any failures, **stop and investigate** — most likely the `tests/cli_flags.rs` version assertion was missed in Phase D. Don't proceed to user handoff until everything is green.

Optionally, run the local preview script to spot visual regressions:
```bash
./scripts/preview-all-layouts.sh 160 110 80
```

## Phase F — Handoff to user

After Phase E passes, present a final summary that lists:
1. Every file modified, grouped by purpose (version sites / CHANGELOG / doc-rot fixes / design-doc deletions).
2. Test totals and key release-readiness check results.
3. Suggested next steps for the user, framed as decisions they own:
   - Commit strategy: single `chore(release): bump to <NEW>` vs. split (`docs(changelog):` + `chore(release):`).
   - Push / merge / tag sequencing (typically: push → PR → merge to main → tag `v<NEW>` on main → push `--tags` to trigger CI publish).

**Do NOT auto-commit, push, or tag.** The user owns those decisions because:
- Commit message conventions are project-specific.
- Tag pushes can trigger irreversible CI publish workflows (npm publish, GitHub release).
- The user may want to do a final review of the diff before any of this.

End the routine with "Ready for review — let me know if you want me to commit, or if anything in the diff needs adjusting."

## Common pitfalls (caught by the routine, not by intuition)

- **Forgetting `tests/cli_flags.rs`.** This file asserts the literal version string. Skipping it means `cargo test` fails after the bump and the failure looks confusingly like a real regression.
- **Editing `npm/*/package.json`.** It looks like a version site but isn't — CI rewrites these. Hand-edits cause publish failures.
- **Treating the existing `[Unreleased]` CHANGELOG as authoritative.** On long-lived branches it describes mid-branch state, not end state. Rewrite, don't append.
- **Missing doc rot because nobody read the docs.** Specific numbers in prose (theme count, field count, threshold values) silently drift. The Phase B audit catches them; eyeballing doesn't.
- **Auto-committing because everything looks ready.** The user may want to review, may want a different commit message, may want to split commits. Always stop at handoff.

## Skill maintenance

If you discover a new doc rot pattern during a release, add it to the Phase B table in this skill so the next release catches it. The same applies if a new version site is added to the codebase (e.g. a new manifest format, a new test that asserts the version string) — add it to the Phase D table. The skill's value is its accuracy; an outdated checklist is worse than no checklist.

The list of file paths in Phase D is the source of truth for "where the version string lives in this repo". If you find yourself updating a version string in a file not listed there, that's a signal the table needs updating before the routine ends.
