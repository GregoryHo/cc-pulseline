//! `Cell` data shape consumed by `super::budget::pack`. See
//! `designs/activity-width-budget.md` §2.1.

use crate::render::activity::truncate::{self, TruncationStrategy};
use crate::render::color::colorize;

#[derive(Debug, Clone)]
pub struct Cell {
    /// Unbreakable prefix — icon, label, structural punctuation. Always
    /// rendered exactly. Counted toward the row budget.
    pub head: String,
    /// Variable-width content. None when the cell is purely a label
    /// (e.g. `✓ Bash ×163`).
    pub body: Option<CellBody>,
    /// Suffix fragments. Pinned ones survive any non-drop adjustment;
    /// Slack ones get dropped first under width pressure.
    pub tail: Vec<TailFragment>,
    /// Determines what happens when the cell can't fit its `min_width`.
    pub priority: CellPriority,
    /// Visible width of `head` (precomputed at construction; ANSI-stripped
    /// upstream). Stored so the allocator doesn't re-strip per pass.
    pub head_w: usize,
}

#[derive(Debug, Clone)]
pub struct CellBody {
    /// Plain text (already ANSI-stripped, sanitized to single-line).
    pub raw: String,
    /// How to truncate when budget < ideal.
    pub truncator: TruncationStrategy,
    /// Shortest still-meaningful representation. Below this the allocator
    /// either drops the cell (if `Optional`) or accepts visible clipping.
    pub min_width: usize,
    /// Full content; the allocator never gives more than this even when
    /// the row has slack (avoids absurdly stretched single cells).
    pub ideal_width: usize,
    /// Palette color code applied to the body after truncation. Caller
    /// passes a palette field clone (e.g. `p.secondary.clone()`); the cell
    /// invokes `crate::render::color::colorize(&truncated, &color, enabled)`
    /// at render time so truncation operates on plain text and never slices
    /// an ANSI escape sequence.
    pub color: String,
}

#[derive(Debug, Clone)]
pub enum TailFragment {
    /// Always shown if the cell is shown (e.g. elapsed time for running agents).
    /// Stored as final ANSI-wrapped string + visible width.
    Pinned { text: String, width: usize },
    /// Shown if budget permits; first to drop under pressure
    /// (e.g. model tag `[haiku]`).
    Slack { text: String, width: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellPriority {
    /// Drop the entire cell when overflowing.
    Optional,
    /// Show at least `head` even if `body` collapses; never dropped wholesale.
    Required,
}

impl Cell {
    /// Construct a label-only cell (no body, fixed-width head + tail).
    pub fn label(head: impl Into<String>, head_w: usize, priority: CellPriority) -> Self {
        Cell {
            head: head.into(),
            body: None,
            tail: Vec::new(),
            priority,
            head_w,
        }
    }

    /// Width of head + every Pinned tail fragment (the cell's "irreducible"
    /// width). Used by the allocator to decide whether a cell can survive
    /// before allocating any body width to it.
    pub fn min_required_width(&self) -> usize {
        let pinned: usize = self
            .tail
            .iter()
            .map(|f| match f {
                TailFragment::Pinned { width, .. } => *width,
                TailFragment::Slack { .. } => 0,
            })
            .sum();
        let body_min = match &self.body {
            Some(b) => b.min_width,
            None => 0,
        };
        self.head_w + body_min + pinned
    }

    /// Width of head + body's `ideal_width` + every tail fragment.
    /// Used by the allocator to know when to stop allocating slack.
    pub fn ideal_total_width(&self) -> usize {
        let tail: usize = self
            .tail
            .iter()
            .map(|f| match f {
                TailFragment::Pinned { width, .. } | TailFragment::Slack { width, .. } => *width,
            })
            .sum();
        let body = match &self.body {
            Some(b) => b.ideal_width,
            None => 0,
        };
        self.head_w + body + tail
    }

    /// Render the cell at a specific body width. Body is truncated to
    /// `body_budget` chars (or omitted if `body_budget == 0`); `include_slack`
    /// controls whether Slack tail fragments are included; `color_enabled`
    /// is forwarded to `colorize()` for the body — head/tail are already
    /// pre-colorized at construction.
    pub fn render(&self, body_budget: usize, include_slack: bool, color_enabled: bool) -> String {
        let mut out = self.head.clone();
        if let Some(body) = &self.body {
            if body_budget > 0 {
                let truncated = truncate::apply(body.truncator, &body.raw, body_budget);
                out.push_str(&colorize(&truncated, &body.color, color_enabled));
            }
        }
        for frag in &self.tail {
            match frag {
                TailFragment::Pinned { text, .. } => out.push_str(text),
                TailFragment::Slack { text, .. } => {
                    if include_slack {
                        out.push_str(text);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_cell_min_eq_head() {
        let c = Cell::label("✓ Bash ×163", 11, CellPriority::Optional);
        assert_eq!(c.min_required_width(), 11);
        assert_eq!(c.ideal_total_width(), 11);
        assert_eq!(c.render(0, false, false), "✓ Bash ×163");
    }

    #[test]
    fn cell_with_body_renders_truncated() {
        let c = Cell {
            head: "T:Bash: ".to_string(),
            head_w: 8,
            body: Some(CellBody {
                raw: "very long command that needs truncation".to_string(),
                truncator: TruncationStrategy::Sentence,
                min_width: 6,
                ideal_width: 30,
                color: String::new(),
            }),
            tail: vec![],
            priority: CellPriority::Required,
        };
        let out = c.render(15, false, false);
        assert!(out.starts_with("T:Bash: "));
        // body_budget=15 → sentence truncates at last word boundary ≤ 14
        assert!(out.chars().count() <= 8 + 15);
        assert!(out.contains('\u{2026}'));
    }

    #[test]
    fn slack_dropped_when_not_included() {
        let c = Cell {
            head: "A:".to_string(),
            head_w: 2,
            body: None,
            tail: vec![
                TailFragment::Pinned {
                    text: " (1m)".to_string(),
                    width: 5,
                },
                TailFragment::Slack {
                    text: " [haiku]".to_string(),
                    width: 8,
                },
            ],
            priority: CellPriority::Required,
        };
        assert_eq!(c.render(0, false, false), "A: (1m)");
        assert_eq!(c.render(0, true, false), "A: (1m) [haiku]");
    }

    #[test]
    fn min_width_sums_head_plus_body_min_plus_pinned() {
        let c = Cell {
            head: "A:".to_string(),
            head_w: 2,
            body: Some(CellBody {
                raw: "x".to_string(),
                truncator: TruncationStrategy::KeepHead,
                min_width: 4,
                ideal_width: 20,
                color: String::new(),
            }),
            tail: vec![
                TailFragment::Pinned {
                    text: "p".to_string(),
                    width: 1,
                },
                TailFragment::Slack {
                    text: "s".to_string(),
                    width: 3,
                },
            ],
            priority: CellPriority::Required,
        };
        assert_eq!(c.min_required_width(), 2 + 4 + 1);
        assert_eq!(c.ideal_total_width(), 2 + 20 + 1 + 3);
    }
}
