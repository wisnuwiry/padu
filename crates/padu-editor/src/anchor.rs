//! Positions that outlive the edits under them.
//!
//! A comment or a highlight is anchored to a range of text, and that range has
//! to move when the text around it does.
//!
//! Mapping follows the left-sticky rule: text typed at the end of a range joins
//! it, text typed at the start does not.

use std::ops::Range;

use crate::model::{Annotation, Cursor, Part, Selection, Splice};

/// The identifier for an anchored comment, thread, or highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnchorId(pub u64);

/// A range in the document and its annotation state/wash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub id: AnchorId,
    pub range: Selection,
    pub state: Annotation,
}

impl Anchor {
    pub fn new(id: AnchorId, range: Selection) -> Self {
        Self {
            id,
            range,
            state: Annotation::default(),
        }
    }

    /// Whether the words this pointed at are completely gone.
    pub fn detached(&self) -> bool {
        self.range.is_collapsed()
    }

    /// Map the anchor's selection through an edit delta.
    pub fn map(&mut self, delta: &Delta) {
        let (start, end) = self.range.ordered();
        self.range = match (delta.cursor(start), delta.cursor(end)) {
            (Some(start), Some(end)) => Selection::new(start.min(end), start.max(end)),
            (surviving, other) => Selection::at(surviving.or(other).unwrap_or_default()),
        };
    }
}

/// What one mutation did to positions in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    /// Text was spliced.
    Spliced(Splice),
    /// A run of blocks moved or was removed.
    Moved { at: Range<usize>, to: Option<usize> },
    /// New blocks appeared at `at`.
    Opened { at: usize, count: usize },
    /// Table cells were inserted.
    Cells {
        block: usize,
        row: Option<usize>,
        column: Option<usize>,
    },
    /// Table cells were removed.
    CellsRemoved {
        block: usize,
        row: Option<usize>,
        column: Option<usize>,
    },
}

impl Delta {
    /// Calculate where `at` ends up after this delta, or `None` if its container was removed.
    pub fn cursor(&self, at: Cursor) -> Option<Cursor> {
        match self {
            Self::Spliced(splice) => {
                let (start, end) = splice.removed.ordered();
                if at < start {
                    return Some(at);
                }
                if at <= end {
                    return Some(splice.caret);
                }
                if at.block == end.block && at.part == end.part {
                    return Some(Cursor {
                        offset: splice.caret.offset + (at.offset - end.offset),
                        ..splice.caret
                    });
                }
                Some(Cursor {
                    block: at.block.checked_add_signed(splice.blocks)?,
                    ..at
                })
            }
            Self::Moved { at: span, to } => {
                if span.contains(&at.block) {
                    return Some(Cursor {
                        block: (*to)? + (at.block - span.start),
                        ..at
                    });
                }
                let pulled = if at.block >= span.end {
                    at.block - span.len()
                } else {
                    at.block
                };
                let block = match to {
                    Some(to) if pulled >= *to => pulled + span.len(),
                    _ => pulled,
                };
                Some(Cursor { block, ..at })
            }
            Self::Cells { block, row, column } => Some(match at.part {
                Part::Cell { row: r, column: c } if at.block == *block => {
                    let past = |line: usize, opened: Option<usize>| {
                        line + usize::from(opened.is_some_and(|opened| line >= opened))
                    };
                    Cursor {
                        part: Part::Cell {
                            row: past(r, *row),
                            column: past(c, *column),
                        },
                        ..at
                    }
                }
                _ => at,
            }),
            Self::CellsRemoved { block, row, column } => match at.part {
                Part::Cell { row: r, column: c } if at.block == *block => {
                    if Some(r) == *row || Some(c) == *column {
                        return None;
                    }
                    let past = |line: usize, removed: Option<usize>| {
                        line - usize::from(removed.is_some_and(|removed| line > removed))
                    };
                    Some(Cursor {
                        part: Part::Cell {
                            row: past(r, *row),
                            column: past(c, *column),
                        },
                        ..at
                    })
                }
                _ => Some(at),
            },
            Self::Opened { at: opened, count } => Some(Cursor {
                block: if at.block >= *opened {
                    at.block + count
                } else {
                    at.block
                },
                ..at
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_creation_and_detached() {
        let cursor = Cursor::new(0, Part::Body, 10);
        let mut anchor = Anchor::new(AnchorId(1), Selection::at(cursor));
        assert!(anchor.detached());

        let head = Cursor::new(0, Part::Body, 15);
        anchor.range = Selection::new(cursor, head);
        assert!(!anchor.detached());
    }

    #[test]
    fn test_anchor_spliced_delta_mapping() {
        let mut anchor = Anchor::new(
            AnchorId(1),
            Selection::new(
                Cursor::new(0, Part::Body, 5),
                Cursor::new(0, Part::Body, 10),
            ),
        );

        // Insertion before anchor (offset 0..2)
        let splice_before = Delta::Spliced(Splice {
            removed: Selection::at(Cursor::new(0, Part::Body, 0)),
            caret: Cursor::new(0, Part::Body, 3), // +3 chars
            blocks: 0,
        });
        anchor.map(&splice_before);
        assert_eq!(anchor.range.anchor.offset, 8);
        assert_eq!(anchor.range.head.offset, 13);

        // Insertion after anchor (offset 20)
        let splice_after = Delta::Spliced(Splice {
            removed: Selection::at(Cursor::new(0, Part::Body, 20)),
            caret: Cursor::new(0, Part::Body, 25),
            blocks: 0,
        });
        anchor.map(&splice_after);
        assert_eq!(anchor.range.anchor.offset, 8);
        assert_eq!(anchor.range.head.offset, 13);
    }

    #[test]
    fn test_anchor_moved_blocks() {
        let mut anchor = Anchor::new(
            AnchorId(2),
            Selection::new(Cursor::new(1, Part::Body, 2), Cursor::new(1, Part::Body, 8)),
        );

        // Move block 1..2 to index 3
        let move_delta = Delta::Moved {
            at: 1..2,
            to: Some(3),
        };
        anchor.map(&move_delta);
        assert_eq!(anchor.range.anchor.block, 3);
        assert_eq!(anchor.range.head.block, 3);
        assert_eq!(anchor.range.anchor.offset, 2);
        assert_eq!(anchor.range.head.offset, 8);
    }

    #[test]
    fn test_anchor_opened_blocks() {
        let mut anchor = Anchor::new(
            AnchorId(3),
            Selection::new(Cursor::new(2, Part::Body, 0), Cursor::new(2, Part::Body, 4)),
        );

        // Insert 2 blocks at index 1
        let open_delta = Delta::Opened { at: 1, count: 2 };
        anchor.map(&open_delta);
        assert_eq!(anchor.range.anchor.block, 4);
        assert_eq!(anchor.range.head.block, 4);
    }

    #[test]
    fn test_anchor_cells_manipulation() {
        let at = Cursor {
            block: 0,
            part: Part::Cell { row: 1, column: 2 },
            offset: 3,
        };

        // Insert row at 0
        let delta_row = Delta::Cells {
            block: 0,
            row: Some(0),
            column: None,
        };
        let shifted = delta_row.cursor(at).unwrap();
        assert_eq!(shifted.part, Part::Cell { row: 2, column: 2 });

        // Remove row 1
        let delta_rm = Delta::CellsRemoved {
            block: 0,
            row: Some(1),
            column: None,
        };
        assert!(delta_rm.cursor(at).is_none());
    }
}
