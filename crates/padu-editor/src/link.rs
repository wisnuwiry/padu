//! Link paste handling and conversion choices.

use crate::model::{Cursor, is_image};

/// What a pasted link can be converted into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Dismiss,
    Chip,
    Bookmark,
    Embed,
    Image,
}

impl Choice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dismiss => "Dismiss",
            Self::Chip => "Create chip",
            Self::Bookmark => "Create bookmark",
            Self::Embed => "Create embed",
            Self::Image => "Create image",
        }
    }
}

/// An open paste popover for transforming a pasted URL.
#[derive(Debug, Clone)]
pub struct Paste {
    pub at: Cursor,
    pub url: String,
    pub rows: Vec<Choice>,
    pub active: usize,
}

impl Paste {
    pub fn open(at: Cursor, url: String, alone: bool) -> Self {
        let mut rows = vec![Choice::Dismiss, Choice::Chip];
        if alone {
            rows.extend([Choice::Bookmark, Choice::Embed]);
            if is_image(&url) {
                rows.push(Choice::Image);
            }
        }
        Self {
            at,
            url,
            rows,
            active: 0,
        }
    }

    pub fn step(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() as isize - 1;
        self.active = (self.active as isize + delta).clamp(0, last) as usize;
    }

    pub fn choice(&self) -> Choice {
        self.rows
            .get(self.active)
            .copied()
            .unwrap_or(Choice::Dismiss)
    }
}
