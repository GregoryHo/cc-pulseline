# Implementation Plan: Display Improvements

## Phase 1: Path Display & Time Format

### Stage 1: Path Simplification
**Goal**: Convert `/Users/gregho` to `~` in project path display
**Success Criteria**:
- Home directory replaced with `~` in Line 1
- Works across different OS (macOS, Linux, Windows)
- Tests verify path simplification
**Tests**:
- Test path with home directory → shows `~`
- Test path without home directory → shows full path
- Test empty/unknown path → shows "unknown"
**Status**: Complete

### Stage 2: Enhanced Time Format
**Goal**: Add day/hour/minute format (xd xh xm) to elapsed time
**Success Criteria**:
- Format: `<1m`, `Xm`, `Xh Xm`, `Xd Xh` (drop minutes if >24h)
- Matches claude-hud time format logic
- Tests verify all time ranges
**Tests**:
- < 1 min → "<1m"
- 1-59 min → "Xm"
- 1-23 hours → "Xh Xm"
- >= 1 day → "Xd Xh"
**Status**: Complete

## Phase 2: Line 2 Display Improvements

### Stage 3: Line 2 Value-First Format
**Goal**: Change from `C:1` to `1 CLAUDE.md` value-first format
**Success Criteria**:
- Value-first labels: `1 CLAUDE.md | 2 rules | 1 hooks | 2 MCPs | 2 skills`
- Icon mode shows nerd font icon before label
- Tests updated for new format
**Status**: Complete

### Stage 4: Align CLAUDE.md Count with claude-hud
**Goal**: Check 5 locations instead of 2
**Locations to check**:
1. `~/.claude/CLAUDE.md`
2. `{cwd}/CLAUDE.md`
3. `{cwd}/CLAUDE.local.md`
4. `{cwd}/.claude/CLAUDE.md`
5. `{cwd}/.claude/CLAUDE.local.md`
**Success Criteria**: Count matches claude-hud behavior
**Tests**: Test each location
**Status**: Not Started

### Stage 5: Align Rules Count with claude-hud
**Goal**: Only count `.md` files recursively, include user scope
**Success Criteria**:
- Recursively scan `.claude/rules/`
- Only count `.md` files (not all files)
- Check both user (`~/.claude/rules/`) and project scope
**Tests**: Test with .md and non-.md files
**Status**: Complete

### Stage 6: Align Skills Count with claude-hud
**Goal**: Include user scope `~/.claude/skills/` in skills count
**Success Criteria**: Count skill dirs from project + user scope
**Status**: Complete

### Stage 7: Align Hooks Count with claude-hud
**Goal**: Count hooks from `settings.json` `hooks` object
**Locations**:
- `~/.claude/settings.json`
- `{cwd}/.claude/settings.json`
- `{cwd}/.claude/settings.local.json`
**Success Criteria**: Parse JSON and count hooks keys
**Tests**: Test with valid/invalid JSON
**Status**: Not Started

### Stage 8: Align MCP Count with claude-hud
**Goal**: Aggregate MCP servers from multiple sources and exclude disabled
**User scope**:
- `~/.claude/settings.json` (mcpServers)
- `~/.claude.json` (mcpServers)
- Exclude: `~/.claude.json` (disabledMcpServers)
**Project scope**:
- `{cwd}/.mcp.json` (mcpServers)
- `{cwd}/.claude/settings.json` (mcpServers)
- `{cwd}/.claude/settings.local.json` (mcpServers)
- Exclude: `.claude/settings.local.json` (disabledMcpServers)
**Success Criteria**:
- Deduplicate within scope
- Count = user_count + project_count
**Tests**: Test with various config combinations
**Status**: Not Started

## Phase 3: Line 3 Display Improvements

### Stage 9: Context Window Display Format
**Goal**: Change format from `XX%/size` to `used/size (XX%)`
**Success Criteria**:
- Format: `CTX:86.0k/200.0k (43%)` with k/M suffixes
- Colors match thresholds
**Tests**: Test different percentages
**Status**: Complete

### Stage 10: Token Display with Icons
**Goal**: Use nerd font icons for token type prefixes
**Changes**:
- `I:` → upload icon (nf-fa-upload) in icon mode
- `O:` → download icon (nf-fa-download) in icon mode
- `C:` → save icon (nf-fa-save) in icon mode
- `R:` → refresh icon (nf-fa-refresh) in icon mode
- ASCII mode keeps `I:`, `O:`, `C:`, `R:` prefixes
**Tests**: Update format tests
**Status**: Complete

## Notes

- Each stage should compile and pass tests
- Commit after each stage completion
- Update memory.md with learnings
