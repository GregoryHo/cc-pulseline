//! Truncation strategies. See `designs/activity-width-budget.md` §2.2 / §2.3.
//! Char-safe throughout: counts use `chars()`, never byte indexing.

const ELLIPSIS: char = '\u{2026}';

/// Dispatch enum so cell descriptors can pick a strategy without holding
/// a function pointer (keeps `Cell` cheaply `Clone`-able and serializable
/// later if needed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationStrategy {
    KeepHead,
    KeepTail,
    KeepMiddle,
    Sentence,
    CommandSmart,
}

/// Apply the named strategy. Pure dispatch — no allocations beyond the
/// chosen strategy's own.
pub fn apply(strategy: TruncationStrategy, raw: &str, max_chars: usize) -> String {
    match strategy {
        TruncationStrategy::KeepHead => keep_head(raw, max_chars),
        TruncationStrategy::KeepTail => keep_tail(raw, max_chars),
        TruncationStrategy::KeepMiddle => keep_middle(raw, max_chars),
        TruncationStrategy::Sentence => sentence(raw, max_chars),
        TruncationStrategy::CommandSmart => command_smart(raw, max_chars),
    }
}

/// Take the first `max_chars` chars; append `…` when truncated.
pub fn keep_head(raw: &str, max_chars: usize) -> String {
    let count = raw.chars().count();
    if count <= max_chars {
        return raw.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return ELLIPSIS.to_string();
    }
    let take = max_chars.saturating_sub(1);
    let head: String = raw.chars().take(take).collect();
    format!("{head}{ELLIPSIS}")
}

/// Path-aware tail keep: prefer `.../{leaf}` if the leaf fits; otherwise
/// truncate the leaf with `keep_head`. For non-path content (no `/`),
/// falls back to a leading `…` plus the last `max_chars - 1` chars.
pub fn keep_tail(raw: &str, max_chars: usize) -> String {
    let count = raw.chars().count();
    if count <= max_chars {
        return raw.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if let Some(leaf) = raw.rsplit('/').next() {
        if leaf != raw {
            // It's a path. Try `.../{leaf}`.
            const PREFIX: &str = ".../";
            let leaf_w = leaf.chars().count();
            if leaf_w + PREFIX.chars().count() <= max_chars {
                return format!("{PREFIX}{leaf}");
            }
            // Leaf alone doesn't fit either; truncate the leaf.
            return keep_head(leaf, max_chars);
        }
    }
    // No path separator — show the tail with a leading ellipsis.
    if max_chars == 1 {
        return ELLIPSIS.to_string();
    }
    let take = max_chars.saturating_sub(1);
    let skip = count.saturating_sub(take);
    let tail: String = raw.chars().skip(skip).collect();
    format!("{ELLIPSIS}{tail}")
}

/// Show prefix + `…` + suffix, splitting the budget evenly. Useful for URLs
/// where both the host and the leaf path matter.
pub fn keep_middle(raw: &str, max_chars: usize) -> String {
    let count = raw.chars().count();
    if count <= max_chars {
        return raw.to_string();
    }
    if max_chars < 3 {
        return keep_head(raw, max_chars);
    }
    // 1 char for `…`; split the rest, prefer head when odd.
    let body = max_chars - 1;
    let head_w = body.div_ceil(2);
    let tail_w = body / 2;
    let tail_start = count - tail_w;
    // Single walk: capture byte index after the head_w-th char and the byte
    // index of the (count - tail_w)-th char.
    let mut head_byte_end = raw.len();
    let mut tail_byte_start = raw.len();
    for (i, (byte_idx, _)) in raw.char_indices().enumerate() {
        if i == head_w {
            head_byte_end = byte_idx;
        }
        if i == tail_start {
            tail_byte_start = byte_idx;
            break;
        }
    }
    format!(
        "{head}{ELLIPSIS}{tail}",
        head = &raw[..head_byte_end],
        tail = &raw[tail_byte_start..]
    )
}

/// Segment-aware path compression for `/`-delimited paths.
///
/// Preserves the first and last segments (most informative), replacing
/// middle segments with a single `…`. Three-stage cascade:
///   1. Try `{first}/…/{last}`.
///   2. If too wide, drop the first segment: `…/{last}`.
///   3. If the leaf alone is too wide, truncate it with `keep_tail`.
///
/// Falls back to `keep_tail` for paths with 0 or 1 segments (no
/// middle to elide). Char-safe — uses `chars()` for width math.
///
/// Examples (max_width=24):
///   `~/Workspace/Paradise/Frontend/platform-1.0/platform-web`
///     → `~/…/platform-web` (16 chars) when middle elision suffices
///   `/very/deep/nested/path/extremely_long_filename.rs`
///     → `…/extremely_long_filena…` when leaf needs truncation too
pub fn compress_path_segments(path: &str, max_width: usize) -> String {
    let count = path.chars().count();
    if count <= max_width {
        return path.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 3 {
        // 0, 1, or 2 segments: no middle to elide. Fall back to keep_tail.
        return keep_tail(path, max_width);
    }
    let first = segments.first().copied().unwrap_or("");
    let last = segments.last().copied().unwrap_or("");
    let first_w = first.chars().count();
    let last_w = last.chars().count();
    // Stage 1: `{first}/…/{last}` — needs first + 3 (for `/…/`) + last chars.
    let stage1_w = first_w + 3 + last_w;
    if stage1_w <= max_width {
        return format!("{first}/{ELLIPSIS}/{last}");
    }
    // Stage 2: `…/{last}` — needs 2 + last chars.
    let stage2_w = 2 + last_w;
    if stage2_w <= max_width {
        return format!("{ELLIPSIS}/{last}");
    }
    // Stage 3: leaf alone is too long; truncate it.
    keep_tail(last, max_width)
}

/// Truncate at a word boundary: never cut mid-word. If the first word is
/// itself longer than `max_chars`, falls back to `keep_head` so we still
/// emit something. Words are space-separated.
pub fn sentence(raw: &str, max_chars: usize) -> String {
    let count = raw.chars().count();
    if count <= max_chars {
        return raw.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    // We need 1 char for `…` after the last kept word.
    let target = max_chars.saturating_sub(1);
    let mut end = 0usize;
    // Inspect chars at positions 0..=target — record the rightmost space at
    // a position ≤ target so a space sitting exactly there still counts.
    for (chars_so_far, (i, ch)) in raw.char_indices().enumerate() {
        if chars_so_far > target {
            break;
        }
        if ch == ' ' {
            end = i;
        }
    }
    if end == 0 {
        // First word is longer than the budget; punt to keep_head.
        return keep_head(raw, max_chars);
    }
    let head = &raw[..end];
    format!("{head}{ELLIPSIS}")
}

/// Shell-command-aware truncation. Strips the verb and any leading flags,
/// then surfaces the first "meaningful payload" — a quoted string, a path
/// argument, or just the next bare token. Falls back to `keep_head` when
/// nothing meaningful can be extracted.
///
/// Examples (target ≈ 40 chars):
///   `cargo test --all-features`              → `cargo test --all-features` (fits, no strip)
///   `git commit -m "feat: add foo"`          → `feat: add foo`
///   `find . -name "*.tmp" -delete`           → `*.tmp`
///   `sed 's/x/y/' file.txt`                  → `s/x/y/  file.txt`
///   `node scripts/build.js --watch`          → `scripts/build.js`
///   `sed -i '' 's/^name = ".*"$/...` (long)  → `s/^name = ".*"$/name = "cards"/  ...toml`
pub fn command_smart(raw: &str, max_chars: usize) -> String {
    let count = raw.chars().count();
    if count <= max_chars {
        return raw.to_string();
    }
    let payload = extract_command_payload(raw);
    if payload.is_empty() {
        return keep_head(raw, max_chars);
    }
    if payload.chars().count() <= max_chars {
        return payload;
    }
    sentence(&payload, max_chars)
}

/// Internal: extract the "meaningful payload" from a shell command.
/// Algorithm:
///   1. Pop the verb (first bare token — typically `git`, `cargo`, `sed`, …).
///   2. Collect "interesting" remaining tokens:
///        - Quoted strings (drop quotes) — most likely the payload
///        - Path-looking bare tokens (contain `/`, are `.` / `..`, start
///          with `~`, or look like `name.ext`)
///   3. If nothing interesting → fall back to all non-flag tokens after
///      the verb (so simple commands like `cargo test --foo` still emit
///      `test`).
///   4. Join with two spaces — caller's `sentence` truncator will shorten.
fn extract_command_payload(raw: &str) -> String {
    let tokens = tokenize_shell(raw);
    if tokens.is_empty() {
        return String::new();
    }
    let mut iter = tokens.into_iter();
    let _verb = iter.next();
    let rest: Vec<ShellToken> = iter.collect();

    // Pass 1: keep only quoted + path-like, dropping empty payload (e.g. the
    // `''` in `sed -i ''` would otherwise render as a doubled separator).
    let interesting: Vec<String> = rest
        .iter()
        .filter(|t| !t.text.is_empty() && (t.was_quoted || looks_like_path(&t.text)))
        .map(|t| t.text.clone())
        .collect();
    if !interesting.is_empty() {
        return interesting.join("  ");
    }

    // Pass 2 — fallback: all non-flag bare tokens.
    let fallback: Vec<String> = rest
        .into_iter()
        .filter(|t| !t.text.starts_with('-') && !t.text.is_empty())
        .map(|t| t.text)
        .collect();
    fallback.join(" ")
}

fn looks_like_path(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s == "." || s == ".." {
        return true;
    }
    if s.starts_with('/') || s.starts_with("./") || s.starts_with("../") || s.starts_with('~') {
        return true;
    }
    if s.contains('/') {
        return true;
    }
    // `name.ext` style — has a dot in a non-leading position followed by
    // 1-4 alphanumerics (typical extension shape).
    if let Some(dot) = s.rfind('.') {
        if dot > 0 && dot < s.len() - 1 {
            let ext = &s[dot + 1..];
            if !ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
                return true;
            }
        }
    }
    false
}

#[derive(Debug)]
struct ShellToken {
    text: String,
    was_quoted: bool,
}

/// Tokenize a shell command into a flat list, honouring single + double quotes.
/// We do not attempt full POSIX (no `$()`, no escape handling, no backticks)
/// — just enough to keep quoted strings intact so `extract_command_payload`
/// can treat them as units.
fn tokenize_shell(raw: &str) -> Vec<ShellToken> {
    let mut out: Vec<ShellToken> = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    let mut was_quoted = false;
    for ch in raw.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => {
                quote = None;
                was_quoted = true;
            }
            (Some(_), c) => buf.push(c),
            (None, '\'') | (None, '"') => quote = Some(ch),
            (None, ' ') | (None, '\t') => {
                if !buf.is_empty() || was_quoted {
                    out.push(ShellToken {
                        text: std::mem::take(&mut buf),
                        was_quoted,
                    });
                    was_quoted = false;
                }
            }
            (None, c) => buf.push(c),
        }
    }
    if !buf.is_empty() || was_quoted {
        out.push(ShellToken {
            text: buf,
            was_quoted,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_head_short() {
        assert_eq!(keep_head("hello", 10), "hello");
    }
    #[test]
    fn keep_head_truncates() {
        assert_eq!(keep_head("hello world", 6), "hello\u{2026}");
    }
    #[test]
    fn keep_head_zero() {
        assert_eq!(keep_head("hi", 0), "");
    }
    #[test]
    fn keep_head_one() {
        assert_eq!(keep_head("hello", 1), "\u{2026}");
    }
    #[test]
    fn keep_head_unicode_safe() {
        // 5 chars, each multibyte
        assert_eq!(keep_head("中文字符測", 3), "中文\u{2026}");
    }

    #[test]
    fn keep_tail_path_fits_with_prefix() {
        assert_eq!(
            keep_tail("/foo/bar/very/deep/path/main.rs", 16),
            ".../main.rs"
        );
    }
    #[test]
    fn keep_tail_leaf_too_long_truncates_leaf() {
        // leaf alone (`extremely_long_filename.rs`) > max → fall back
        let result = keep_tail("/a/b/extremely_long_filename.rs", 12);
        assert!(result.ends_with('\u{2026}'));
        assert!(result.chars().count() <= 12);
    }
    #[test]
    fn keep_tail_no_slash_uses_ellipsis_prefix() {
        // 10 chars: 1 ellipsis + 9 trailing chars
        assert_eq!(keep_tail("hello world example", 10), "\u{2026}d example");
    }
    #[test]
    fn keep_tail_short_returns_intact() {
        assert_eq!(keep_tail("/x/y", 10), "/x/y");
    }

    #[test]
    fn keep_middle_balances() {
        // 15 chars in, max=9: body=8, head=4 (div_ceil), tail=4
        assert_eq!(keep_middle("abcdefghijklmno", 9), "abcd\u{2026}lmno");
    }
    #[test]
    fn keep_middle_short_returns_intact() {
        assert_eq!(keep_middle("hello", 10), "hello");
    }

    #[test]
    fn compress_path_short_returns_intact() {
        assert_eq!(
            compress_path_segments("~/repo/file.rs", 30),
            "~/repo/file.rs"
        );
    }
    #[test]
    fn compress_path_elides_middle_segments() {
        // 55 chars in, max=24, segments=5; stage 1: `~` (1) + `/…/` (3) + `platform-web` (12) = 16
        assert_eq!(
            compress_path_segments(
                "~/Workspace/Paradise/Frontend/platform-1.0/platform-web",
                24
            ),
            "~/\u{2026}/platform-web"
        );
    }
    #[test]
    fn compress_path_drops_first_when_too_long() {
        // first segment alone (`very-long-org-prefix`) is 20 chars, plus `/…/leaf` (8)
        // = 28 > 24. Stage 2: `…/leaf` = 6 chars, fits.
        assert_eq!(
            compress_path_segments("very-long-org-prefix/mid/leaf", 10),
            "\u{2026}/leaf"
        );
    }
    #[test]
    fn compress_path_truncates_leaf_when_alone_too_long() {
        let result = compress_path_segments("a/b/extremely_long_filename_here.rs", 10);
        assert!(result.chars().count() <= 10);
        // Either `.../leaf` (when leaf fits with prefix) or a truncated leaf.
        assert!(result.contains('\u{2026}') || result.starts_with("..."));
    }
    #[test]
    fn compress_path_single_segment_falls_back_to_keep_tail() {
        // No `/` separators — delegates to keep_tail, which uses leading ellipsis.
        let result = compress_path_segments("supercalifragilisticexpialidocious", 10);
        assert!(result.chars().count() <= 10);
        assert!(result.starts_with('\u{2026}'));
    }
    #[test]
    fn compress_path_two_segments_falls_back_to_keep_tail() {
        // 2 segments — keep_tail tries `.../{leaf}`. `~/very-long-leaf` (16) > 10.
        // keep_tail's path branch tries `.../very-long-leaf` (18) > 10, leaf alone
        // (14) > 10, falls back to keep_head(leaf, 10) = "very-long…"
        let result = compress_path_segments("~/very-long-leaf", 10);
        assert!(result.chars().count() <= 10);
    }
    #[test]
    fn compress_path_branch_with_slashes_works() {
        // Branches like `feature/e2e-traceability-rollout-integration` have one
        // `/` — that's 2 segments, falls back to keep_tail.
        let result = compress_path_segments("feature/e2e-traceability-rollout-integration", 16);
        assert!(result.chars().count() <= 16);
    }
    #[test]
    fn compress_path_unicode_safe() {
        // 多段 unicode path
        let result = compress_path_segments("~/工作區/前端/平台/網頁", 8);
        assert!(result.chars().count() <= 8);
    }

    #[test]
    fn sentence_word_boundary() {
        assert_eq!(
            sentence("the quick brown fox jumps", 16),
            "the quick brown\u{2026}"
        );
    }
    #[test]
    fn sentence_no_boundary_falls_back() {
        assert_eq!(sentence("supercalifragilistic", 8), "superca\u{2026}");
    }
    #[test]
    fn sentence_short_returns_intact() {
        assert_eq!(sentence("hello world", 20), "hello world");
    }

    // ── command_smart edge cases (the spec table from the design doc) ──
    #[test]
    fn cmd_smart_short_returns_intact() {
        assert_eq!(
            command_smart("cargo test --all-features", 40),
            "cargo test --all-features"
        );
    }
    #[test]
    fn cmd_smart_quoted_payload_extracted() {
        // input is 29 chars; budget 20 forces extraction
        assert_eq!(
            command_smart("git commit -m \"feat: add foo\"", 20),
            "feat: add foo"
        );
    }
    #[test]
    fn cmd_smart_skips_flags_to_first_quote() {
        // input is 28 chars; budget 15 forces extraction (`.` and `*.tmp` survive)
        assert_eq!(
            command_smart("find . -name \"*.tmp\" -delete", 15),
            ".  *.tmp"
        );
    }
    #[test]
    fn cmd_smart_sed_recovers_regex() {
        let raw = "sed -i '' 's/^name = \".*\"$/name = \"cards\"/' .claude/pulseline.toml";
        let got = command_smart(raw, 60);
        assert!(
            got.contains("s/^name") || got.contains("pulseline.toml"),
            "expected meaningful payload, got {got:?}"
        );
        assert!(!got.starts_with("sed"));
    }
    #[test]
    fn cmd_smart_node_path_arg() {
        assert_eq!(
            command_smart("node scripts/build.js --watch", 25),
            "scripts/build.js"
        );
    }
    #[test]
    fn cmd_smart_falls_back_when_nothing_meaningful() {
        // No quotes, no path-y args, just a long verb chain — fall back to
        // keep_head so we emit *something*.
        let result = command_smart("aaaaaaaaaa", 5);
        assert!(result.chars().count() <= 5);
        assert!(result.ends_with('\u{2026}') || result == "aaaaa");
    }
}
