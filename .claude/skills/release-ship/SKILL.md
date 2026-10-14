---
name: release-ship
description: Use this skill to LAND a cc-pulseline release that is already prepared — triggers on "merge and tag", "ship it", "cut the release", "push the tag", "publish 1.X", "merge the release PR", or any phrasing that moves a prepared version onto main and out to npm/crates.io. This is the counterpart to release-bump, which prepares the version and deliberately stops before shipping. Do NOT use to decide or write a version — that is release-bump. Pushing the tag is irreversible and publishes publicly, so the user must say go.
---

# Release ship — merge, tag, publish

`release-bump` prepares a release and stops: it edits the version sites, the
CHANGELOG and the docs, and never commits. This skill is what happens next —
gates, merge, tag, and the checks that prove the tag actually shipped.

**The tag is the trigger.** Pushing `v<X.Y.Z>` starts
`.github/workflows/release.yml`: seven platform builds, a GitHub release with
auto-generated notes, `npm publish --provenance` for the main package and
each platform package, and `cargo publish`. None of it is undone by deleting
the tag.

## 1. Gates, before anything irreversible

Run all of these. A release has never been worth skipping one.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                                  # expect 0 failed
npx tsc -p .                                # needs .claude-plugin/types/, see note
command claude plugin test .                # the hook tests, against the engine
command claude plugin validate .            # hooks, $ calls, $.state keys, .catch
cargo package --list | grep -E '^(hooks|types)/|^tsconfig\.json'   # expect no output
cargo publish --dry-run                     # catches a packaging failure before the tag
```

Three of those need explaining:

- **`command claude`, not `claude`.** The user's shell wraps `claude` with
  `--allow-dangerously-skip-permissions`, and `plugin test` rejects an option
  before the subcommand. `validate` tolerates it; `test` does not.
- **`tsc` needs the engine-written contract** at
  `.claude-plugin/types/claude-code/index.d.ts`. There is no command that
  writes it — the engine does, each time it loads the plugin from a folder the
  author owns. A fresh clone has no types until a session has loaded the
  plugin once (`claude --plugin-dir .`). This is by design; do not vendor a
  copy.
- **`cargo publish --dry-run` fails on an untracked file**, because
  `cargo package` includes them. That is why `.arcforge/` is in `.gitignore`.
  If it fails this way, gitignore the file rather than passing `--allow-dirty`.

Then confirm the version is the same everywhere. `release-bump`'s workflow
owns this table; these are the sites as of 1.5.0:

```bash
grep -m1 '^version' Cargo.toml
grep -m1 '"version"' .claude-plugin/plugin.json
grep -n '"version"' .claude-plugin/marketplace.json     # inside plugins[0]
grep -A1 '^name = "cc-pulseline"' Cargo.lock | grep version
```

`npm/main/package.json` and `npm/platforms/*/package.json` stay at their
`0.0.0` placeholders — CI rewrites them from the tag. `tests/cli_flags.rs`
asserts against `env!("CARGO_PKG_VERSION")` and holds no literal.

**The tag and `Cargo.toml` must agree exactly.** npm takes its version from
`GITHUB_REF` and crates.io from `Cargo.toml`; a mismatch publishes two
different versions of the same release.

## 2. Documentation that belongs to shipping

`release-bump` audits doc rot against the code. These four are about the
release itself, so check them here:

| File | What to check |
| --- | --- |
| `CHANGELOG.md` | the `## [X.Y.Z] - YYYY-MM-DD` section exists and is not empty, `## [Unreleased]` is left empty above it, and the compare link `[X.Y.Z]: …/compare/v<prev>...v<X.Y.Z>` is at the bottom |
| `README.md` | the install line and any stated binary floor match this version — the band's gate is a *minimum binary version*, so a release that moves it moves this too |
| `docs/band.md` | the contract version it says it was verified against, and the binary ladder's rungs if they changed |
| `docs/event-coverage.md` | the Claude Code version in its header note, when the contract was re-read for this release |

Anything the release changes about *behaviour* is `release-bump`'s audit, not
this one. If you find behaviour rot here, say so and stop — do not fix it
inside a ship.

## 3. Merge

The convention is **squash**, and every commit on `main` carries `(#NN)`:

```bash
gh pr merge <NN> --squash \
  --subject "<type>: <subject> (#<NN>)" \
  --body "<what landed, and why it is shaped that way>"
```

Write the body yourself. A squash of a long branch concatenates every commit
message by default, which reads as a changelog of a changelog; the PR's own
comments stay on GitHub as the detailed record.

Then verify what landed, on `main`, before tagging it:

```bash
git checkout main && git pull --ff-only origin main
cargo fmt --check && cargo test && command claude plugin test .
git status --short          # expect clean
```

## 4. Tag — the same sitting as the merge

```bash
git tag -a v<X.Y.Z> -m "<name> <X.Y.Z> — <one line>

<two or three lines: what this release is, and the install line>"
git push origin v<X.Y.Z>
```

**Why the same sitting.** `.claude-plugin/marketplace.json` has
`"source": "."`, so the marketplace entry serves whatever `main` holds. The
moment the PR lands, `main` advertises an install whose band needs a binary
that is not on npm until the tag's workflow finishes. Merging and leaving the
tag for later is a window where a new installer gets a plugin it cannot run.

Annotated (`-a`), as `v1.4.0` and `v1.5.0` are.

## 5. Watch it, and say what shipped

```bash
gh run list --workflow Release --limit 1
gh run watch <run-id> --exit-status
```

Four jobs: `build` (seven platforms), `GitHub Release`, `Publish to npm`,
`Publish to crates.io`.

**A green npm job is not proof that npm published.** Every `npm publish` in
it ends `|| echo "::warning::… publish failed (may already exist)"`, so the
job is green whether it published or not. `cargo publish` has no such guard,
so a red crates job is a real failure and a green one did publish.

So check the artifacts rather than the job colours:

```bash
gh release view v<X.Y.Z> --json assets --jq '.assets | length'   # expect 7
npm view @cc-pulseline/cc-pulseline version                      # expect X.Y.Z
cargo search cc-pulseline --limit 1
```

Seven assets, one archive per platform: `.tar.gz` for the six unix targets
and `.zip` for windows.

## 6. What cannot be undone

Say this plainly if the user asks to reverse a release:

- **crates.io**: a version cannot be deleted, only `cargo yank`ed — it stays
  resolvable for existing lockfiles and cannot be republished.
- **npm**: unpublish is allowed only within 72 hours and only if nothing
  depends on it; otherwise it is `npm deprecate`.
- **The tag and the GitHub release** can be deleted, which changes nothing
  about the two above.

The only real remedy is a new patch version.

## Hard rules

- **Never `--no-verify`.** Never skip a gate because the diff looks small.
- **Never push a tag without the user saying go**, in this turn, for this
  version. A prepared release sitting on a branch is not consent.
- **Never bump a version here.** If a gate shows the version is wrong or
  inconsistent, stop and hand it back to `release-bump`.
- **Report what the gates said, with numbers** — test counts, asset count,
  published versions. "All green" is not a report.
