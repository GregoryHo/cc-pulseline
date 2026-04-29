# Local Development

This repo's Claude Code session runs the binary produced by its own build.
`.claude/settings.json` pins `statusLine.command` to an absolute path under
the worktree:

```json
{
  "statusLine": {
    "type": "command",
    "command": "/Users/gregho/GitHub/AI/cc-pulseline/target/release/cc-pulseline"
  }
}
```

So the dev loop is:

1. Edit source
2. `cargo build --release`
3. Next statusline tick already uses the new binary — no install step
4. For a controlled input without a live session, pipe a fixture directly:
   `echo '{...}' | target/release/cc-pulseline` (fixtures in `tests/fixtures/`)
5. Before shipping: `cargo test`

## Do NOT

- **Do NOT `cp target/release/cc-pulseline ~/.claude/pulseline/cc-pulseline`**.
  That path is for end users who installed via `scripts/install.sh`. It has
  zero effect on this repo's session because the project-level
  `statusLine.command` overrides it.
- **Do NOT run `scripts/install.sh` to "test changes"** — same reason, it's
  a user-facing installer.
- **Do NOT edit `~/.claude/pulseline/config.toml` to toggle a feature**.
  Edit `.claude/pulseline.toml` (this repo's project config) instead —
  project config deep-merges over user config, and the user's other projects
  aren't affected.
