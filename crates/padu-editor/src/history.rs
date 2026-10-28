//! Undo and redo history as coalesced document snapshots.

use std::collections::VecDeque;

use crate::anchor::Anchor;
use crate::model::{Cursor, Doc, Selection};

pub const DEFAULT_UNDO_LIMIT: usize = 100;

/// Bounded snapshot history storage.
pub struct SnapshotHistory<S> {
    undo: VecDeque<S>,
    redo: Vec<S>,
    limit: usize,
}

impl<S> SnapshotHistory<S> {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn record(&mut self, before: Option<S>) {
        self.redo.clear();
        if let Some(before) = before {
            self.undo.push_back(before);
            while self.undo.len() > self.limit {
                self.undo.pop_front();
            }
        }
    }

    pub fn undo(&mut self, current: impl FnOnce() -> S) -> Option<S> {
        let previous = self.undo.pop_back()?;
        self.redo.push(current());
        Some(previous)
    }

    pub fn redo(&mut self, current: impl FnOnce() -> S) -> Option<S> {
        let next = self.redo.pop()?;
        self.undo.push_back(current());
        while self.undo.len() > self.limit {
            self.undo.pop_front();
        }
        Some(next)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// The nature of an edit, used for coalescing consecutive typing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
    Structure,
}

/// A restored point-in-time state from undo or redo.
#[derive(Clone)]
pub struct Step {
    pub doc: Doc,
    pub selection: Selection,
    pub anchors: Vec<Anchor>,
}

#[derive(Clone)]
struct Snapshot {
    doc: Doc,
    selection: Selection,
    anchors: Vec<Anchor>,
}

impl From<Snapshot> for Step {
    fn from(s: Snapshot) -> Self {
        Self {
            doc: s.doc,
            selection: s.selection,
            anchors: s.anchors,
        }
    }
}

pub struct History {
    snapshots: SnapshotHistory<Snapshot>,
    last: Option<(EditKind, Cursor)>,
}

impl Default for History {
    fn default() -> Self {
        Self::with_limit(DEFAULT_UNDO_LIMIT)
    }
}

impl History {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            snapshots: SnapshotHistory::new(limit),
            last: None,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.snapshots.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.snapshots.can_redo()
    }

    pub fn record(&mut self, kind: EditKind, doc: &Doc, selection: Selection, anchors: &[Anchor]) {
        let joins = self.joins(kind, selection);
        self.snapshots.record((!joins).then(|| Snapshot {
            doc: doc.clone(),
            selection,
            anchors: anchors.to_vec(),
        }));
        if !joins {
            self.last = None;
        }
    }

    pub fn landed(&mut self, kind: EditKind, selection: Selection) {
        self.last = (kind != EditKind::Structure).then_some((kind, selection.head));
    }

    fn joins(&self, kind: EditKind, selection: Selection) -> bool {
        if kind == EditKind::Structure || !self.snapshots.can_undo() {
            return false;
        }
        self.last == Some((kind, selection.head)) && selection.is_collapsed()
    }

    pub fn interrupt(&mut self) {
        self.last = None;
    }

    pub fn undo(&mut self, doc: &Doc, selection: Selection, anchors: &[Anchor]) -> Option<Step> {
        let previous = self.snapshots.undo(|| Snapshot {
            doc: doc.clone(),
            selection,
            anchors: anchors.to_vec(),
        })?;
        self.last = None;
        Some(previous.into())
    }

    pub fn redo(&mut self, doc: &Doc, selection: Selection, anchors: &[Anchor]) -> Option<Step> {
        let next = self.snapshots.redo(|| Snapshot {
            doc: doc.clone(),
            selection,
            anchors: anchors.to_vec(),
        })?;
        self.last = None;
        Some(next.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Part;

    #[test]
    fn test_snapshot_history_stack() {
        let mut hist: SnapshotHistory<i32> = SnapshotHistory::new(3);
        assert!(!hist.can_undo());
        assert!(!hist.can_redo());

        hist.record(Some(1));
        hist.record(Some(2));
        hist.record(Some(3));
        assert!(hist.can_undo());

        // Limit overflow: pushing 4 drops 1
        hist.record(Some(4));

        let u1 = hist.undo(|| 5);
        assert_eq!(u1, Some(4));
        assert!(hist.can_redo());

        let u2 = hist.undo(|| 4);
        assert_eq!(u2, Some(3));

        let u3 = hist.undo(|| 3);
        assert_eq!(u3, Some(2));

        // 1 was dropped due to limit of 3
        let u4 = hist.undo(|| 2);
        assert_eq!(u4, None);

        // Redo
        let r1 = hist.redo(|| 2);
        assert_eq!(r1, Some(3));
    }

    #[test]
    fn test_history_coalescing_and_undo() {
        let mut history = History::default();
        let doc0 = Doc { blocks: vec![] };
        let sel0 = Selection::at(Cursor::new(0, Part::Body, 0));

        // First insert
        history.record(EditKind::Insert, &doc0, sel0, &[]);
        let sel1 = Selection::at(Cursor::new(0, Part::Body, 1));
        history.landed(EditKind::Insert, sel1);

        // Consecutive insert at sel1 should coalesce
        let doc1 = Doc { blocks: vec![] };
        history.record(EditKind::Insert, &doc1, sel1, &[]);
        let sel2 = Selection::at(Cursor::new(0, Part::Body, 2));
        history.landed(EditKind::Insert, sel2);

        // Only 1 undo step recorded because the second coalesced!
        let step = history.undo(&doc1, sel2, &[]);
        assert!(step.is_some());
        assert_eq!(step.unwrap().selection, sel0);

        // No more undo steps
        assert!(!history.can_undo());
    }

    #[test]
    fn test_history_structure_breaks_coalescing() {
        let mut history = History::default();
        let doc0 = Doc { blocks: vec![] };
        let sel0 = Selection::at(Cursor::new(0, Part::Body, 0));

        history.record(EditKind::Insert, &doc0, sel0, &[]);
        let sel1 = Selection::at(Cursor::new(0, Part::Body, 1));
        history.landed(EditKind::Insert, sel1);

        // Structure edit breaks coalescing
        history.record(EditKind::Structure, &doc0, sel1, &[]);
        let sel2 = Selection::at(Cursor::new(1, Part::Body, 0));
        history.landed(EditKind::Structure, sel2);

        assert!(history.can_undo());
        let s1 = history.undo(&doc0, sel2, &[]);
        assert!(s1.is_some());
        assert!(history.can_undo());
        let s2 = history.undo(&doc0, sel1, &[]);
        assert!(s2.is_some());
    }
}
