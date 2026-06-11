export const meta = {
  name: 'release-bump',
  description: 'cc-pulseline release routine: parallel audit → sequential apply (version sites, CHANGELOG rewrite, doc-rot fixes, dead code/doc removal) → gates + completeness critic. Never commits.',
  whenToUse: 'Cutting a cc-pulseline release ("bump version", "release X.Y.Z", "prepare release"). args: { version: "X.Y.Z" } required; { prev, date } auto-discovered when omitted. Replaces the old /release-bump skill routine — the version-site table, doc-rot audit table, and CHANGELOG rules now live in this file. If a new version site or doc-rot pattern appears, update the prompts here.',
  phases: [
    { title: 'Audit', detail: 'scout + 4 parallel read-only auditors on the pristine tree' },
    { title: 'Apply', detail: 'sequential editors: version bump → CHANGELOG → doc fixes → dead removal' },
    { title: 'Verify', detail: 'fmt/clippy/test/build gates (one repair retry) + completeness critic' },
  ],
}

// ───────────────────────── args ─────────────────────────
// Normalize: tolerate args arriving as a JSON-encoded string.
let A = args
if (typeof A === 'string') {
  try { A = JSON.parse(A) } catch (e) { A = null }
}
const version = (A && A.version) || ''
if (!version) {
  throw new Error('release-bump requires args.version, e.g. Workflow({name:"release-bump", args:{version:"1.2.0"}})')
}

const RULES = [
  'Repo root = your working directory (cc-pulseline, a Rust crate).',
  'NEVER run `git commit`, `git push`, or `git tag` — this routine stops at ready-for-review; the user owns commit/tag timing.',
  'Project conventions: no .unwrap() on external data; colorize() only (no raw ANSI); files stay under 800 lines.',
  'Your final message is consumed by an orchestrator — return only what the task asks for, no preamble.',
].join('\n')

// ──────────────────────── schemas ────────────────────────
const CTX_SCHEMA = {
  type: 'object',
  properties: {
    prev_version: { type: 'string', description: 'latest release tag without leading v, e.g. 1.1.5' },
    today: { type: 'string', description: 'YYYY-MM-DD from `date +%F`' },
    branch: { type: 'string' },
    commit_count: { type: 'number', description: 'commits since the latest release tag' },
    dirty: { type: 'boolean', description: 'true if git status --porcelain is non-empty' },
    branch_summary: { type: 'array', items: { type: 'string' }, description: '4-6 thematic bullets describing the release content' },
  },
  required: ['prev_version', 'today', 'branch', 'commit_count', 'dirty', 'branch_summary'],
}

const SITES_SCHEMA = {
  type: 'object',
  properties: {
    sites: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          file: { type: 'string' },
          line: { type: 'number' },
          snippet: { type: 'string' },
          action: { type: 'string', enum: ['bump', 'historical', 'skip'] },
          note: { type: 'string' },
        },
        required: ['file', 'line', 'snippet', 'action'],
      },
    },
  },
  required: ['sites'],
}

const DOCROT_SCHEMA = {
  type: 'object',
  properties: {
    mismatches: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          file: { type: 'string' },
          line: { type: 'number' },
          claim: { type: 'string', description: 'what the doc currently says' },
          actual: { type: 'string', description: 'what the code measures' },
          fix_hint: { type: 'string', description: 'suggested rewrite — prefer doc-rot-immune prose' },
        },
        required: ['file', 'claim', 'actual'],
      },
    },
  },
  required: ['mismatches'],
}

const DEAD_SCHEMA = {
  type: 'object',
  properties: {
    candidates: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          path: { type: 'string' },
          kind: { type: 'string', enum: ['code', 'doc', 'design', 'fixture', 'script'] },
          evidence: { type: 'string' },
          confidence: { type: 'string', enum: ['high', 'medium', 'low'] },
          recommendation: { type: 'string' },
        },
        required: ['path', 'kind', 'evidence', 'confidence', 'recommendation'],
      },
    },
  },
  required: ['candidates'],
}

const CHANGELOG_SCHEMA = {
  type: 'object',
  properties: {
    markdown: { type: 'string', description: 'full body of the new version section (without the ## header line)' },
    breaking: { type: 'boolean' },
    highlights: { type: 'array', items: { type: 'string' } },
  },
  required: ['markdown', 'breaking', 'highlights'],
}

const APPLY_SCHEMA = {
  type: 'object',
  properties: {
    files_changed: { type: 'array', items: { type: 'string' } },
    notes: { type: 'string' },
  },
  required: ['files_changed', 'notes'],
}

const GATES_SCHEMA = {
  type: 'object',
  properties: {
    gates: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          name: { type: 'string' },
          pass: { type: 'boolean' },
          output: { type: 'string', description: 'short evidence: counts, version line, or failure excerpt' },
        },
        required: ['name', 'pass', 'output'],
      },
    },
    all_green: { type: 'boolean' },
  },
  required: ['gates', 'all_green'],
}

const CRITIC_SCHEMA = {
  type: 'object',
  properties: {
    issues: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          severity: { type: 'string', enum: ['blocker', 'warning', 'note'] },
          description: { type: 'string' },
        },
        required: ['severity', 'description'],
      },
    },
    clean: { type: 'boolean' },
  },
  required: ['issues', 'clean'],
}

// ──────────────────────── Phase: Audit ────────────────────────
phase('Audit')

const ctx = await agent(
  `${RULES}

Scout step of the release-bump workflow. Run and report:
1. \`git tag --sort=-v:refname | head -1\` → strip leading "v" → prev_version
2. \`date +%F\` → today
3. \`git rev-parse --abbrev-ref HEAD\` → branch
4. \`git log --oneline v<prev_version>..HEAD | wc -l\` → commit_count
5. \`git status --porcelain\` → dirty = (non-empty)
6. Read \`git log --oneline v<prev_version>..HEAD\` and \`git diff --stat v<prev_version>..HEAD | tail -20\`, then summarize the release content in 4-6 thematic bullets (new layouts? widget rewrites? schema changes? breaking renames?).`,
  { schema: CTX_SCHEMA, label: 'scout', phase: 'Audit' }
)
if (!ctx) throw new Error('scout failed — re-run with resumeFromRunId')

const prev = (A && A.prev) || ctx.prev_version
const date = (A && A.date) || ctx.today
log(`bump ${prev} → ${version} on ${ctx.branch} (${ctx.commit_count} commits since v${prev})`)
if (ctx.dirty) log('⚠ working tree is dirty — release edits will mix with pre-existing changes')

const [sites, docrot, dead, clProposal] = await parallel([
  // 1. Version-site scan — classifies every occurrence of the previous version.
  () => agent(
    `${RULES}

READ-ONLY audit. Find every occurrence of the previous version string "${prev}" in tracked files (exclude target/, .claude/logs/, node_modules/). Classify each hit:
- action="bump": must change to ${version}. The 5 KNOWN sites: (1) Cargo.toml \`version = "${prev}"\`; (2) Cargo.lock — auto-synced by cargo build, mark action="skip" with that note; (3) .claude-plugin/plugin.json "version"; (4) .claude-plugin/marketplace.json "version" inside plugins[0]; (5) tests/cli_flags.rs — two assert! lines containing the literal version. (README's hardcoded CLI banner was a 6th site until 1.2.0 — it was replaced with a version-free command list; the runtime banner derives from env!("CARGO_PKG_VERSION"). If a hardcoded version reappears in README, classify it bump and flag the table.)
- action="historical": leave unchanged — e.g. CHANGELOG.md's existing \`## [${prev}]\` section + compare links, docs notes like "v1.0.4 cache schema change".
- action="skip": npm/main/package.json + npm/platforms/*/package.json are CI-rewritten "0.0.0" placeholders — NEVER bump those; Cargo.lock as above.
If you find a bump-class hit OUTSIDE the 6 known sites, still classify it action="bump" and say so in note — that means the known-sites table in .claude/workflows/release-bump.js needs extending.`,
    { schema: SITES_SCHEMA, label: 'audit:version-sites', phase: 'Audit' }
  ),

  // 2. Doc-rot cross-audit — code is the source of truth, never the doc number.
  () => agent(
    `${RULES}

READ-ONLY audit: implementation ↔ docs cross-audit. For each row, MEASURE FROM CODE first, then diff against every listed doc site. Report only mismatches (file, line, wrong claim, correct value, fix hint). Prefer fix hints that rewrite toward doc-rot-immune prose ("see src/themes/ for the full list") over fresh hard-coded numbers, unless the number carries real information.

| Measurement | Source of truth (code) | Doc sites that go stale |
|---|---|---|
| Built-in theme count | \`ls src/themes/*.json | wc -l\` | README.md Features list + THEMES section; docs/theme-palette.md "Built-in Themes" table |
| ThemePalette field count | count \`pub\` fields in the ThemePalette struct in src/render/color.rs | docs/theme-palette.md "Tier Summary" + "REQUIRED — the N ANSI codes" + JSON schema field list; CLAUDE.md color-system blurb |
| Layouts list | \`pub enum LayoutStyle\` in src/render/pane.rs | README.md "Layouts & Visual Composition"; docs/architecture.md; docs/layouts.md catalog; src/config.rs template comments |
| Widgets list | \`ls src/render/widgets/*.rs\` (excl. mod.rs) | docs/architecture.md widgets/ description; CLAUDE.md widgets/ description |
| Widget visual form | top-of-file doc comment in each widgets/*.rs | docs/architecture.md, docs/layouts.md, CLAUDE.md widget descriptions |
| CTX threshold marks | ThemePalette::ctx_marks() in src/render/color.rs | docs/layouts.md "Recognized widgets per segment" CTX row |
| Quota threshold marks | render_quota_visual body in src/render/frames/shared.rs | docs/layouts.md quota visual description |
| Config TOML keys | serde field names on *SegmentConfig structs in src/config.rs | README.md example config; docs/layouts.md example TOML; default-config template strings in src/config.rs |
| Stdin payload schema | StdinPayload + nested structs in src/types.rs | .claude/rules/integration.md Schema section |
| Compact layout row contract | assemble_compact in src/render/layout.rs (2-3 rows, idle = 2) | README.md, docs/layouts.md, CLAUDE.md compact descriptions |
| CLI flags list | flag handling in src/main.rs | README.md CLI section; CLAUDE.md "CLI Flags" block |

Also scan README.md + docs/*.md for any other stale specific numbers or removed-feature mentions you notice along the way.`,
    { schema: DOCROT_SCHEMA, label: 'audit:doc-rot', phase: 'Audit' }
  ),

  // 3. Dead code / dead doc scan.
  () => agent(
    `${RULES}

READ-ONLY audit: find dead code and dead documents. Report candidates with evidence and confidence — do NOT delete anything. Only confidence="high" items (evidence proves zero references AND no intentional-keep marker) will be auto-removed downstream.

Check, in order:
1. designs/ — per designs/README.md policy, design docs are deleted once absorbed into tracked docs (docs/layouts.md, docs/architecture.md, docs/theme-palette.md, CLAUDE.md). For each file in designs/ (tracked or untracked), judge: absorbed → high-confidence delete; partially absorbed → medium with extraction note.
2. docs/ + README.md — sections describing features removed since v${prev}. Check \`git log --oneline v${prev}..HEAD\` for removals (e.g. deleted layouts zones/grid/sections, removed config keys width_mode/fixed_width) and grep docs for leftover mentions. CHANGELOG.md entries and explicit "removed in vX" historical notes are NOT dead — skip those.
3. src/ — \`#[allow(dead_code)]\` items; pub fns/structs with zero references outside their defining module (verify by grep across src/ AND tests/ AND benches/). IMPORTANT intentional keeps: legacy \`pub const\` color values in src/render/color.rs are test-compat by design — skip them.
4. tests/fixtures/ — .json/.jsonl fixtures referenced by no file under tests/.
5. scripts/ — scripts referencing flags, paths, or binaries that no longer exist.
6. Cargo.toml — dependencies with zero \`use\`/path references (grep crate name across src/ tests/ benches/).`,
    { schema: DEAD_SCHEMA, label: 'audit:dead-scan', phase: 'Audit' }
  ),

  // 4. CHANGELOG proposal — rewrite from the end-state-vs-prior-tag perspective.
  () => agent(
    `${RULES}

READ-ONLY audit: draft the CHANGELOG section for ${version} (target date ${date}).

Read: (a) the current \`## [Unreleased]\` section of CHANGELOG.md, (b) \`git log v${prev}..HEAD\` (oneline first; read full messages for commits that look significant), (c) the style of previous sections in CHANGELOG.md.

RULES — rewrite, don't append:
- Users upgrading from v${prev} see only the END STATE. A feature added then deleted on the branch must not appear. A field renamed twice is described as one final rename.
- The existing [Unreleased] content describes mid-branch state — treat it as a hint, not as authoritative.
- Keep-a-Changelog section order: Added, Changed, Removed, Fixed.
- If breaking changes exist (config renames, removed layouts/keys): add a \`> **⚠ Breaking changes**\` callout at the top of the section and prefix each breaking entry with \`**BREAKING:**\`.
- Match the prose style and entry density of the [${prev}] section.

Return the section BODY only (everything that goes under the \`## [${version}] - ${date}\` header, excluding that header line), plus 3-5 one-line highlights for the final report.`,
    { schema: CHANGELOG_SCHEMA, label: 'audit:changelog', phase: 'Audit' }
  ),
])

if (!sites || !docrot || !dead || !clProposal) {
  throw new Error('audit fan-out incomplete — re-run with resumeFromRunId to reuse finished auditors')
}

const bumpSites = sites.sites.filter(s => s.action === 'bump')
const highDead = dead.candidates.filter(c => c.confidence === 'high')
log(`audit: ${bumpSites.length} bump sites, ${docrot.mismatches.length} doc-rot mismatches, ${dead.candidates.length} dead candidates (${highDead.length} high-confidence), changelog drafted${clProposal.breaking ? ' (BREAKING)' : ''}`)

// ──────────────────────── Phase: Apply ────────────────────────
// Sequential on purpose: the editors overlap on README.md / docs/ / src,
// and the gates expect a settled tree.
phase('Apply')

const bumpResult = await agent(
  `${RULES}

Bump the version ${prev} → ${version}. Edit exactly these classified sites (skip Cargo.lock — cargo syncs it):
${JSON.stringify(bumpSites, null, 2)}

Then run \`cargo build\` (debug — faster, syncs Cargo.lock the same way) and verify with \`grep -A1 'name = "cc-pulseline"' Cargo.lock | head\` that the lock now shows version = "${version}". Do NOT touch npm/*/package.json. Report files changed.`,
  { schema: APPLY_SCHEMA, label: 'apply:bump', phase: 'Apply' }
)

const clResult = await agent(
  `${RULES}

Apply the CHANGELOG for ${version}:
1. In CHANGELOG.md, convert the current \`## [Unreleased]\` content into \`## [${version}] - ${date}\`, using this drafted body (refine wording/ordering if you spot improvements while reading the surrounding file, but keep its substance):
---DRAFT START---
${clProposal.markdown}
---DRAFT END---
2. Keep an empty \`## [Unreleased]\` header above the new section (Keep-a-Changelog convention — match what this file did after previous releases).
3. In the link block at the bottom: add \`[${version}]: https://github.com/GregoryHo/cc-pulseline/compare/v${prev}...v${version}\` and, if an [Unreleased] compare link exists, point it at \`v${version}...HEAD\`.
Report files changed.`,
  { schema: APPLY_SCHEMA, label: 'apply:changelog', phase: 'Apply' }
)

const docfixResult = docrot.mismatches.length
  ? await agent(
      `${RULES}

Fix these doc-rot mismatches found by the release audit (code is the source of truth). Surgical edits only — don't reflow or "improve" surrounding prose. Where the fix hint suggests doc-rot-immune phrasing, prefer it.
${JSON.stringify(docrot.mismatches, null, 2)}

Verify each fix by re-reading the edited lines. Report files changed.`,
      { schema: APPLY_SCHEMA, label: 'apply:doc-fixes', phase: 'Apply' }
    )
  : { files_changed: [], notes: 'no doc-rot mismatches' }

const deadResult = highDead.length
  ? await agent(
      `${RULES}

Remove ONLY these high-confidence dead items (the audit already verified zero references — but re-verify each with a fresh grep before deleting; if your grep disagrees, leave it and say so):
${JSON.stringify(highDead, null, 2)}

For code removals: after deleting, run \`cargo check\` and \`cargo fmt\` — if cargo check fails, revert that specific removal and report it instead.
For doc/design removals: \`git rm\` tracked files, plain delete for untracked.
Do NOT touch medium/low-confidence candidates. Report files changed.`,
      { schema: APPLY_SCHEMA, label: 'apply:dead-removal', phase: 'Apply' }
    )
  : { files_changed: [], notes: 'no high-confidence dead items' }

// ──────────────────────── Phase: Verify ────────────────────────
phase('Verify')

const GATES_PROMPT = `${RULES}

Run the release gates and report each honestly (pass=false with the failure excerpt if anything is off):
1. \`cargo fmt --check\` — must be silent
2. \`cargo clippy --all-targets -- -D warnings\` — zero warnings
3. \`cargo test\` — all green; report total test count
4. \`cargo build --release\` — must succeed
5. \`./target/release/cc-pulseline --version\` — must print "cc-pulseline ${version}"
6. \`echo '{"session_id":"smoke","version":"2.1.119","model":{"id":"sonnet","display_name":"Sonnet"}}' | ./target/release/cc-pulseline\` — must render multiple lines of ANSI output
Do not fix anything — measure and report.`

let gates = await agent(GATES_PROMPT, { schema: GATES_SCHEMA, label: 'verify:gates', phase: 'Verify' })

if (gates && !gates.all_green) {
  const failures = gates.gates.filter(g => !g.pass)
  log(`gates failed (${failures.map(f => f.name).join(', ')}) — one repair attempt`)
  await agent(
    `${RULES}

The release gates failed. Diagnose and fix with the smallest possible change (likely suspects: a missed version assertion in tests/cli_flags.rs, a doc-fix or dead-removal that broke compilation — \`git diff\` shows everything this routine changed; revert a bad removal rather than patching around it):
${JSON.stringify(failures, null, 2)}`,
    { label: 'verify:repair', phase: 'Verify' }
  )
  gates = await agent(GATES_PROMPT, { schema: GATES_SCHEMA, label: 'verify:gates-retry', phase: 'Verify' })
}

const critic = await agent(
  `${RULES}

Completeness critic for the ${version} release prep (read-only). Check and report issues:
1. \`grep -rn "${prev}" --include="*.toml" --include="*.md" --include="*.rs" --include="*.json" . | grep -v target | grep -v .claude/logs\` — every remaining hit must be legitimately historical (CHANGELOG [${prev}] section + compare links, past-version doc notes). Stray bump sites = blocker.
2. CHANGELOG.md has \`## [${version}] - ${date}\`, a bottom compare link for [${version}], and an [Unreleased] header above it.
3. \`git status --porcelain\` + \`git diff --stat\` — npm/ must be untouched; nothing committed (this routine never commits); summarize the change footprint.
4. .claude-plugin/plugin.json AND marketplace.json both show "${version}".
5. Docs don't reference features removed this release outside historical/CHANGELOG context.
6. README CLI banner shows ${version}.`,
  { schema: CRITIC_SCHEMA, label: 'verify:critic', phase: 'Verify' }
)

// ──────────────────────── Report ────────────────────────
return {
  version,
  prev,
  date,
  branch: ctx.branch,
  commit_count: ctx.commit_count,
  branch_summary: ctx.branch_summary,
  highlights: clProposal.highlights,
  breaking: clProposal.breaking,
  applied: {
    bump: bumpResult ? bumpResult.files_changed : [],
    changelog: clResult ? clResult.files_changed : [],
    doc_fixes: docfixResult ? docfixResult.files_changed : [],
    dead_removed: deadResult ? deadResult.files_changed : [],
    dead_skipped: dead.candidates.filter(c => c.confidence !== 'high'),
  },
  gates: gates ? gates.gates : [],
  all_green: gates ? gates.all_green : false,
  critic: critic ? critic.issues : [{ severity: 'warning', description: 'critic agent did not return' }],
  committed: false,
  handoff: 'No commits made. Review the diff, then decide: commit message split (docs(changelog) + chore(release) vs single), and push → merge → tag v' + version + ' → push --tags (tag push triggers CI publish).',
}
