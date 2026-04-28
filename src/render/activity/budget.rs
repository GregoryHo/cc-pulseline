//! Row-level width budget allocator.
//!
//! Given a row's `available_width` and a `Vec<Cell>`, decide which cells
//! survive and how much body width each gets. Returns the rendered string
//! (single line, ANSI included).
//!
//! See `designs/activity-width-budget.md` §2.4 for the design.

use super::cell::{Cell, CellPriority};

const SEPARATOR_DEFAULT: &str = " | ";
const SEPARATOR_DEFAULT_W: usize = 3;

/// Pack `cells` into a single row of at most `available` visible chars.
/// Returns the rendered row (cells joined by ` | ` by default), or empty
/// when nothing fits. `color_enabled` is forwarded to per-cell body
/// colorize calls; head/tail are already pre-colorized.
pub fn pack(cells: &[Cell], available: usize, color_enabled: bool) -> String {
    pack_with_separator(
        cells,
        available,
        SEPARATOR_DEFAULT,
        SEPARATOR_DEFAULT_W,
        color_enabled,
    )
}

/// Like `pack`, but with a custom inter-cell separator. The separator's
/// visible width must be passed in (callers typically know it statically).
pub fn pack_with_separator(
    cells: &[Cell],
    available: usize,
    separator: &str,
    sep_w: usize,
    color_enabled: bool,
) -> String {
    if cells.is_empty() || available == 0 {
        return String::new();
    }

    // Pass 1 — survival: drop trailing Optional cells until min totals fit.
    let mut survivors: Vec<&Cell> = cells.iter().collect();
    loop {
        let total_min = survivors
            .iter()
            .map(|c| c.min_required_width())
            .sum::<usize>()
            + separators_width(survivors.len(), sep_w);
        if total_min <= available {
            break;
        }
        // Find rightmost Optional and drop it.
        if let Some(idx) = survivors
            .iter()
            .rposition(|c| c.priority == CellPriority::Optional)
        {
            survivors.remove(idx);
            if survivors.is_empty() {
                return String::new();
            }
            continue;
        }
        // No more Optionals to drop — accept the overflow.
        break;
    }

    // Pass 2 — slack allocation across bodies.
    let total_min: usize = survivors.iter().map(|c| c.min_required_width()).sum();
    let used = total_min + separators_width(survivors.len(), sep_w);
    let mut slack = available.saturating_sub(used);

    // Per-cell "extra" body capacity granted on top of min_width.
    let mut extras: Vec<usize> = vec![0; survivors.len()];
    // Greedy: each cell wants up to ideal-min more chars; give it what's
    // available, going left to right. Required cells get slack before
    // Optional.
    for phase_priority in [CellPriority::Required, CellPriority::Optional] {
        for (i, c) in survivors.iter().enumerate() {
            if slack == 0 {
                break;
            }
            if c.priority != phase_priority {
                continue;
            }
            let want = match &c.body {
                Some(b) => b.ideal_width.saturating_sub(b.min_width),
                None => 0,
            };
            let give = want.min(slack);
            extras[i] += give;
            slack -= give;
        }
    }

    // Pass 3 — Slack tail fragments included only if there's room. We
    // don't make this a separate budget pass; instead each cell decides
    // at render time whether `slack > 0` was given to it. Approximation:
    // include Slack tails whenever the cell received at least its full
    // ideal body width (i.e. the row had room to spare).
    let include_slack: Vec<bool> = survivors
        .iter()
        .enumerate()
        .map(|(i, c)| match &c.body {
            Some(b) => extras[i] + b.min_width >= b.ideal_width,
            None => true,
        })
        .collect();

    // Render survivors.
    let mut parts: Vec<String> = Vec::with_capacity(survivors.len());
    for (i, c) in survivors.iter().enumerate() {
        let body_budget = match &c.body {
            Some(b) => b.min_width + extras[i],
            None => 0,
        };
        parts.push(c.render(body_budget, include_slack[i], color_enabled));
    }
    parts.join(separator)
}

fn separators_width(cells: usize, sep_w: usize) -> usize {
    cells.saturating_sub(1) * sep_w
}

/// Pack `cells` into as many rows of width `available` as needed.
///
/// Greedy: each row consumes as many leading cells as fit (using the
/// `min_required_width` of each cell + `sep_w` between cells). Then the
/// row is rendered through `pack_with_separator` so survivors get their
/// per-cell slack budget. When `max_rows` is `Some(n)`, additional rows
/// beyond `n` are dropped silently — caller handles overflow framing.
///
/// Returns `(rows, consumed)` where `consumed` is the number of cells
/// rendered into rows so the caller can compute `cells.len() - consumed`
/// for an overflow summary without re-walking the fitting algorithm.
///
/// Used by both flat-row's completed-tool segment and cluster layouts'
/// agent / tool segments.
pub fn pack_multi_row(
    cells: &[Cell],
    available: usize,
    separator: &str,
    sep_w: usize,
    color_enabled: bool,
    max_rows: Option<usize>,
) -> (Vec<String>, usize) {
    let cap = max_rows.unwrap_or(usize::MAX).max(1);
    let mut rows: Vec<String> = Vec::new();
    let mut start = 0usize;
    let mut consumed = 0usize;
    while start < cells.len() && rows.len() < cap {
        let mut used = cells[start].min_required_width();
        let mut end = start + 1;
        while end < cells.len() {
            let advance = sep_w + cells[end].min_required_width();
            if used + advance > available {
                break;
            }
            used += advance;
            end += 1;
        }
        let row = pack_with_separator(
            &cells[start..end],
            available,
            separator,
            sep_w,
            color_enabled,
        );
        if !row.is_empty() {
            rows.push(row);
            consumed = end;
        }
        start = end;
    }
    (rows, consumed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity::cell::{CellBody, TailFragment};
    use crate::render::activity::truncate::TruncationStrategy;

    fn body_cell(head: &str, raw: &str, min: usize, ideal: usize, prio: CellPriority) -> Cell {
        Cell {
            head: head.to_string(),
            head_w: head.chars().count(),
            body: Some(CellBody {
                raw: raw.to_string(),
                truncator: TruncationStrategy::Sentence,
                min_width: min,
                ideal_width: ideal,
                color: String::new(),
            }),
            tail: vec![],
            priority: prio,
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(pack(&[], 80, false), "");
    }

    #[test]
    fn zero_width_returns_empty() {
        let cells = vec![Cell::label("x", 1, CellPriority::Required)];
        assert_eq!(pack(&cells, 0, false), "");
    }

    #[test]
    fn everything_fits_renders_all() {
        let cells = vec![
            Cell::label("a", 1, CellPriority::Required),
            Cell::label("b", 1, CellPriority::Required),
            Cell::label("c", 1, CellPriority::Required),
        ];
        // 3 chars + 2 separators(3 each)=9 → fits in 10
        assert_eq!(pack(&cells, 10, false), "a | b | c");
    }

    #[test]
    fn drops_optional_from_right_under_pressure() {
        let cells = vec![
            Cell::label("required", 8, CellPriority::Required),
            Cell::label("opt1", 4, CellPriority::Optional),
            Cell::label("opt2", 4, CellPriority::Optional),
        ];
        // total need: 8+4+4 + 2*3 = 22; available 14 → drop opt2 (need 16) → drop opt1 (need 8)
        let out = pack(&cells, 9, false);
        assert_eq!(out, "required");
    }

    #[test]
    fn body_cell_gets_slack_up_to_ideal() {
        let cells = vec![body_cell(
            "T:Bash: ",
            "very long command needing room",
            4,
            30,
            CellPriority::Required,
        )];
        // available 50 — body has plenty of slack, gets ideal_width
        let out = pack(&cells, 50, false);
        assert!(out.starts_with("T:Bash: "));
        // body can be up to ideal=30 chars; sentence truncator may shorten
        // if no word boundary fits, but raw fits in 30, so:
        assert!(out.chars().count() <= 8 + 30);
    }

    #[test]
    fn body_cell_collapses_to_min_under_pressure() {
        let cells = vec![body_cell(
            "T:",
            "really really really really really long command",
            6,
            40,
            CellPriority::Required,
        )];
        // available exactly head + min + 0 slack = 8
        let out = pack(&cells, 8, false);
        assert!(out.starts_with("T:"));
        // body at min_width=6, sentence truncator emits 5 chars + ellipsis → ≤ 6
        assert!(out.chars().count() <= 8);
    }

    #[test]
    fn required_survives_when_optional_drops() {
        let cells = vec![
            Cell::label("required", 8, CellPriority::Required),
            Cell::label("optional", 8, CellPriority::Optional),
        ];
        // need 8+8+3 = 19; available 10 → drop optional → fits
        assert_eq!(pack(&cells, 10, false), "required");
    }

    #[test]
    fn slack_tails_included_when_room_permits() {
        let mut c = body_cell("X", "hello", 5, 5, CellPriority::Required);
        c.tail.push(TailFragment::Pinned {
            text: "P".to_string(),
            width: 1,
        });
        c.tail.push(TailFragment::Slack {
            text: "S".to_string(),
            width: 1,
        });
        // body fully satisfied at ideal; slack tail eligible to include
        let out = pack(&[c], 80, false);
        assert!(out.contains('S'), "slack tail should appear: {out:?}");
        assert!(out.contains('P'));
    }
}
