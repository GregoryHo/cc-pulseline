---
description: Run diagnostic health checks on cc-pulseline installation
allowed-tools: Bash, Read, Glob
---

You are running diagnostics on the cc-pulseline installation. Perform each check and report results in a summary table.

## Diagnostic Checks

### Check 1: Binary Exists

```bash
if [ -x ~/.claude/pulseline/cc-pulseline ]; then
  echo "OK: Binary found at ~/.claude/pulseline/cc-pulseline"
  ~/.claude/pulseline/cc-pulseline --help 2>&1 | head -1
else
  echo "FAIL: Binary not found at ~/.claude/pulseline/cc-pulseline"
fi
```

### Check 2: Settings Registration

Read `~/.claude/settings.json` and verify it contains a `statusLine` entry pointing to the cc-pulseline binary.

**Expected**:
```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/pulseline/cc-pulseline"
  }
}
```

Report OK if the entry exists and points to the correct binary path. Report FAIL with the actual value (or "missing") if not.

### Check 3: Config Validation

```bash
~/.claude/pulseline/cc-pulseline --check 2>&1
```

Report OK if validation passes. Report FAIL with error details if it fails.

### Check 4: Runtime Test

Pipe a sample payload and verify output:

```bash
OUTPUT=$(echo '{"session_id":"diag","version":"1.0.2","model":{"id":"claude-sonnet-4-5-20250929","display_name":"Sonnet 4.5"}}' | ~/.claude/pulseline/cc-pulseline 2>&1)
if [ -n "$OUTPUT" ]; then
  echo "OK: Binary produces output"
  echo "$OUTPUT"
else
  echo "FAIL: Binary produced no output"
fi
```

### Check 5: Config Files

List config file locations and sizes:

```bash
echo "--- User Config ---"
ls -la ~/.claude/pulseline/config.toml 2>/dev/null || echo "Not found"

echo "--- Project Config ---"
ls -la .claude/pulseline.toml 2>/dev/null || echo "Not found"

echo "--- Session Cache ---"
ls -la ${TMPDIR:-/tmp}/cc-pulseline-*.json 2>/dev/null || echo "No cache files"
```

### Check 6: Effective Config

```bash
~/.claude/pulseline/cc-pulseline --print 2>&1
```

## Summary Report

Present results as a table:

```
| Check               | Status | Details                              |
|---------------------|--------|--------------------------------------|
| Binary              | OK/FAIL| Path and version info                |
| Settings            | OK/FAIL| statusLine registration status       |
| Config              | OK/FAIL| Validation result                    |
| Runtime             | OK/FAIL| Test payload result                  |
| Config Files        | INFO   | File locations and sizes             |
| Effective Config    | INFO   | Merged config output                 |
```

## Remediation Hints

For any FAIL results, provide specific remediation steps:

- **Binary missing**: Run `/pulseline:setup` to install
- **Settings not registered**: Run `/pulseline:setup` to register, or manually add the `statusLine` entry to `~/.claude/settings.json`
- **Config invalid**: Run `/pulseline:config reset` to recreate defaults, or `/pulseline:config edit` to fix specific issues
- **Runtime failure**: Check if the binary is the correct architecture for this machine. Try rebuilding with `/pulseline:setup cargo`
