# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 1.x     | Yes                |

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Use [GitHub Security Advisories](https://github.com/GregoryHo/cc-pulseline/security/advisories/new) to report the vulnerability privately
3. Include steps to reproduce, impact assessment, and any suggested fixes

### What to Expect

- Acknowledgment within 48 hours
- Status update within 7 days
- Fix release for confirmed vulnerabilities as soon as practical

## Scope

cc-pulseline reads JSON from stdin and outputs formatted text to stdout. It also:

- Reads local config files (`~/.claude/pulseline/config.toml`, `.claude/pulseline.toml`)
- Reads local `.claude/` directory structure for metric collection
- Shells out to `git` for branch/status information
- Reads JSONL transcript files for activity tracking
- Writes session cache files to the system temp directory

Security concerns may include:
- Path traversal in config or transcript file handling
- Command injection via git operations
- Denial of service via malformed input
- Information disclosure through error messages
