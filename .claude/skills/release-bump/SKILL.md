---
name: release-bump
description: Use this skill when the user wants to bump the cc-pulseline crate version — triggers on "bump version", "release X.Y.Z", "prepare release", "ship 1.X", "tag a release", or any phrasing that signals a version is about to be cut. Always invoke this skill before touching Cargo.toml or CHANGELOG.md, even if the user names a specific file directly. Do NOT use for npm-package version bumps in `npm/*/package.json` (CI-driven `0.0.0` placeholders) or unrelated dependency upgrades.
---

# Release bump — launcher

The release routine lives in the project workflow
**`.claude/workflows/release-bump.js`** (audit → apply → verify, multi-agent,
never commits). This skill is a thin launcher; the version-site table, the
doc-rot audit table, the CHANGELOG rewrite rules, and the pitfall list all
live in the workflow script — maintain them THERE, not here.

## How to run

1. **Confirm the target version** with the user if not stated
   (`MAJOR.MINOR.PATCH`; check `git tag --sort=-v:refname | head -3` and the
   nature of branch changes — breaking changes mean surfacing the
   MINOR-with-BREAKING-callout vs MAJOR choice; the user picks).

2. Launch the workflow:

   ```
   Workflow({ name: "release-bump", args: { version: "<X.Y.Z>" } })
   ```

   Optional args: `prev` (previous version) and `date` (YYYY-MM-DD) — both
   auto-discovered (latest git tag / `date +%F`) when omitted.

3. When the workflow returns, relay its report to the user: files changed by
   group (version sites / CHANGELOG / doc-rot fixes / dead removals +
   skipped medium/low-confidence candidates), gate results, critic issues.

## Hard rules

- **Never auto-commit, push, or tag** — the workflow doesn't, and neither do
  you. Tag pushes trigger irreversible CI publish. The user owns commit
  message split, push/merge sequencing, and `v<X.Y.Z>` tagging.
- If the workflow reports stray version sites or new doc-rot patterns, update
  the prompts in `.claude/workflows/release-bump.js` before ending the
  routine — an outdated checklist is worse than no checklist.

End with: "Ready for review — let me know if you want me to commit, or if
anything in the diff needs adjusting."
