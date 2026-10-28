//! The slash `/` command menu and block vocabulary.

use gpui::SharedString;

use crate::model::{Align, BlockKind, Cursor, QuoteKind, Text};

/// Every block the slash menu offers and what each template creates.
pub fn items() -> Vec<(SharedString, BlockKind)> {
    let text = Text::default;
    vec![
        ("Text".into(), BlockKind::Paragraph(text())),
        (
            "Heading 1".into(),
            BlockKind::Heading {
                level: 1,
                text: text(),
            },
        ),
        (
            "Heading 2".into(),
            BlockKind::Heading {
                level: 2,
                text: text(),
            },
        ),
        (
            "Heading 3".into(),
            BlockKind::Heading {
                level: 3,
                text: text(),
            },
        ),
        ("Bullet List".into(), BlockKind::Bullet(text())),
        (
            "Numbered List".into(),
            BlockKind::Ordered {
                number: 1,
                text: text(),
            },
        ),
        (
            "Task List".into(),
            BlockKind::Task {
                checked: false,
                text: text(),
            },
        ),
        (
            "Quote".into(),
            BlockKind::Quote {
                kind: None,
                text: text(),
            },
        ),
        (
            "Quote (Note)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Note),
                text: text(),
            },
        ),
        (
            "Quote (Tip)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Tip),
                text: text(),
            },
        ),
        (
            "Quote (Important)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Important),
                text: text(),
            },
        ),
        (
            "Quote (Warning)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Warning),
                text: text(),
            },
        ),
        (
            "Quote (Caution)".into(),
            BlockKind::Quote {
                kind: Some(QuoteKind::Caution),
                text: text(),
            },
        ),
        (
            "Code Block".into(),
            BlockKind::Code {
                language: None,
                code: text(),
            },
        ),
        (
            "Table".into(),
            BlockKind::Table {
                align: vec![Align::Left; 2],
                header: vec![text(), text()],
                rows: vec![vec![text(), text()]],
            },
        ),
        (
            "Image".into(),
            BlockKind::Image {
                url: String::new(),
                alt: text(),
                width: None,
            },
        ),
        ("Divider".into(), BlockKind::Rule),
    ]
}

/// Returns the user-facing label for a block kind if recognized.
pub fn label(kind: &BlockKind) -> Option<SharedString> {
    items()
        .into_iter()
        .find(|(_, row)| same(row, kind))
        .map(|(label, _)| label)
}

fn same(row: &BlockKind, kind: &BlockKind) -> bool {
    match (row, kind) {
        (BlockKind::Heading { level: a, .. }, BlockKind::Heading { level: b, .. }) => a == b,
        (BlockKind::Quote { kind: a, .. }, BlockKind::Quote { kind: b, .. }) => a == b,
        (row, kind) => std::mem::discriminant(row) == std::mem::discriminant(kind),
    }
}

/// An open slash menu popover.
#[derive(Debug, Clone)]
pub struct Slash {
    /// Where the `/` was typed in the document.
    pub at: Cursor,
    /// Indices into `items()` matching the query.
    pub rows: Vec<usize>,
    pub selected: usize,
}

impl Slash {
    pub fn open(at: Cursor) -> Self {
        let mut slash = Self {
            at,
            rows: Vec::new(),
            selected: 0,
        };
        slash.refilter("");
        slash
    }

    /// Re-evaluates matching rows for the given search query.
    pub fn refilter(&mut self, query: &str) {
        let q = query.trim().to_lowercase();
        let all = items();
        if q.is_empty() {
            self.rows = (0..all.len()).collect();
        } else {
            self.rows = all
                .iter()
                .enumerate()
                .filter(|(_, (label, _))| label.to_lowercase().contains(&q))
                .map(|(ix, _)| ix)
                .collect();
        }
        self.selected = 0;
    }

    /// Step up or down through the filtered rows.
    pub fn step(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }

    /// The template block created by confirming the selected choice.
    pub fn choice(&self) -> Option<BlockKind> {
        let ix = *self.rows.get(self.selected)?;
        items().into_iter().nth(ix).map(|(_, kind)| kind)
    }

    /// Extract query text typed after `/` at the given cursor.
    pub fn query(&self, caret: Cursor, text: &str) -> Option<String> {
        if caret.block != self.at.block || caret.part != self.at.part {
            return None;
        }
        let start = self.at.offset + 1;
        if caret.offset < start {
            return None;
        }
        let query = text.get(start..caret.offset)?;
        if query.contains(char::is_whitespace) {
            return None;
        }
        Some(query.to_string())
    }
}
