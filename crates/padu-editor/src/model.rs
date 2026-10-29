//! The document and selection model for the markdown editor.

use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use std::ops::Range;

/// Text alignment for table columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// A GitHub-flavored Markdown alert quote kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuoteKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl QuoteKind {
    pub fn marker(self) -> &'static str {
        match self {
            Self::Note => "[!NOTE]",
            Self::Tip => "[!TIP]",
            Self::Important => "[!IMPORTANT]",
            Self::Warning => "[!WARNING]",
            Self::Caution => "[!CAUTION]",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }
}

/// Form of a link/mention display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Form {
    #[default]
    Auto,
    Chip,
    Embed,
}

/// Inline text styling marks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mark {
    Bold,
    Italic,
    Strike,
    Code,
    Link(String),
    Mention { url: String, form: Form },
}

/// A mark applied over a byte range in a [`Text`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarkSpan {
    pub range: Range<usize>,
    pub mark: Mark,
}

/// Inline text content with styling marks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Text {
    pub text: String,
    pub marks: Vec<MarkSpan>,
}

impl Text {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            marks: Vec::new(),
        }
    }

    pub fn link(url: &str) -> Self {
        Self {
            text: url.to_string(),
            marks: vec![MarkSpan {
                range: 0..url.len(),
                mark: Mark::Link(url.to_string()),
            }],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Self::plain(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Self::plain(s)
    }
}

/// Which sub-part of a block an offset or caret refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Part {
    #[default]
    Body,
    Code,
    Caption,
    Cell {
        row: usize,
        column: usize,
    },
}

/// The block kinds supported by the document model.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockKind {
    Paragraph(Text),
    Heading {
        level: u8,
        text: Text,
    },
    Bullet(Text),
    Ordered {
        number: u64,
        text: Text,
    },
    Task {
        checked: bool,
        text: Text,
    },
    Quote {
        kind: Option<QuoteKind>,
        text: Text,
    },
    Code {
        language: Option<String>,
        code: Text,
    },
    Image {
        url: String,
        alt: Text,
        width: Option<u32>,
    },
    Bookmark {
        url: String,
        form: Form,
    },
    Table {
        align: Vec<Align>,
        header: Vec<Text>,
        rows: Vec<Vec<Text>>,
    },
    Rule,
}

/// A block in the document with an indentation level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Block {
    pub kind: BlockKind,
    pub indent: u8,
}

impl Block {
    pub fn new(kind: BlockKind) -> Self {
        Self { kind, indent: 0 }
    }

    pub fn at(kind: BlockKind, indent: u8) -> Self {
        Self { kind, indent }
    }

    pub fn text_at(&self, part: Part) -> Option<&Text> {
        match (&self.kind, part) {
            (
                BlockKind::Paragraph(text)
                | BlockKind::Heading { text, .. }
                | BlockKind::Bullet(text)
                | BlockKind::Ordered { text, .. }
                | BlockKind::Task { text, .. }
                | BlockKind::Quote { text, .. },
                Part::Body,
            ) => Some(text),
            (BlockKind::Code { code, .. }, Part::Code) => Some(code),
            (BlockKind::Image { alt, .. }, Part::Caption) => Some(alt),
            (BlockKind::Table { header, .. }, Part::Cell { row: 0, column }) => header.get(column),
            (BlockKind::Table { rows, .. }, Part::Cell { row, column }) => {
                rows.get(row.checked_sub(1)?)?.get(column)
            }
            _ => None,
        }
    }

    pub fn text_at_mut(&mut self, part: Part) -> Option<&mut Text> {
        match (&mut self.kind, part) {
            (
                BlockKind::Paragraph(text)
                | BlockKind::Heading { text, .. }
                | BlockKind::Bullet(text)
                | BlockKind::Ordered { text, .. }
                | BlockKind::Task { text, .. }
                | BlockKind::Quote { text, .. },
                Part::Body,
            ) => Some(text),
            (BlockKind::Code { code, .. }, Part::Code) => Some(code),
            (BlockKind::Image { alt, .. }, Part::Caption) => Some(alt),
            (BlockKind::Table { header, .. }, Part::Cell { row: 0, column }) => {
                header.get_mut(column)
            }
            (BlockKind::Table { rows, .. }, Part::Cell { row, column }) => {
                let r = row.checked_sub(1)?;
                rows.get_mut(r)?.get_mut(column)
            }
            _ => None,
        }
    }

    pub fn parts(&self) -> Vec<Part> {
        match &self.kind {
            BlockKind::Paragraph(_)
            | BlockKind::Heading { .. }
            | BlockKind::Bullet(_)
            | BlockKind::Ordered { .. }
            | BlockKind::Task { .. }
            | BlockKind::Quote { .. } => vec![Part::Body],
            BlockKind::Code { .. } => vec![Part::Code],
            BlockKind::Image { .. } => vec![Part::Caption],
            BlockKind::Table { header, rows, .. } => {
                let mut parts = Vec::new();
                for col in 0..header.len() {
                    parts.push(Part::Cell {
                        row: 0,
                        column: col,
                    });
                }
                for (r, row) in rows.iter().enumerate() {
                    for col in 0..row.len() {
                        parts.push(Part::Cell {
                            row: r + 1,
                            column: col,
                        });
                    }
                }
                parts
            }
            BlockKind::Bookmark { .. } | BlockKind::Rule => Vec::new(),
        }
    }
}

/// A flat list of blocks representing the document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

impl Doc {
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn renumber(&mut self) {
        let mut expected: Vec<Option<u64>> = Vec::new();
        for block in &mut self.blocks {
            let indent = block.indent as usize;
            if indent >= expected.len() {
                expected.resize(indent + 1, None);
            } else {
                expected.truncate(indent + 1);
            }

            if let BlockKind::Ordered { number, .. } = &mut block.kind {
                if let Some(next) = expected[indent] {
                    *number = next;
                }
                expected[indent] = Some(number.saturating_add(1));
            } else {
                expected[indent] = None;
            }
        }
    }

    pub fn spans(&self, selection: Selection) -> Vec<(Cursor, Range<usize>)> {
        let (start, end) = selection.ordered();
        let mut spans = Vec::new();
        for block_ix in start.block..=end.block.min(self.blocks.len().saturating_sub(1)) {
            let block = &self.blocks[block_ix];
            for part in block.parts() {
                let at = Cursor::new(block_ix, part, 0);
                if at.block == start.block && at.part < start.part {
                    continue;
                }
                if at.block == end.block && at.part > end.part {
                    continue;
                }
                let len = block.text_at(part).map(|t| t.len()).unwrap_or(0);
                let range_start = if block_ix == start.block && part == start.part {
                    start.offset.min(len)
                } else {
                    0
                };
                let range_end = if block_ix == end.block && part == end.part {
                    end.offset.min(len)
                } else {
                    len
                };
                if range_start <= range_end {
                    spans.push((at, range_start..range_end));
                }
            }
        }
        spans
    }
}

/// A position in the document: block index, part, and byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Cursor {
    pub block: usize,
    pub part: Part,
    pub offset: usize,
}

impl Cursor {
    pub fn new(block: usize, part: Part, offset: usize) -> Self {
        Self {
            block,
            part,
            offset,
        }
    }

    pub fn clamp(self, doc: &Doc) -> Self {
        if doc.blocks.is_empty() {
            return Self::default();
        }
        let block = self.block.min(doc.blocks.len() - 1);
        let part = match doc.blocks[block].text_at(self.part) {
            Some(_) => self.part,
            None => doc.blocks[block]
                .parts()
                .first()
                .copied()
                .unwrap_or_default(),
        };
        let here = Self::new(block, part, self.offset);
        let Some(text) = doc.blocks[block].text_at(part) else {
            return here;
        };
        let mut offset = self.offset.min(text.text.len());
        while offset > 0 && !text.text.is_char_boundary(offset) {
            offset -= 1;
        }
        Self { offset, ..here }
    }

    pub fn next_editable(doc: &Doc, from: usize) -> Option<Self> {
        (from..doc.blocks.len()).find_map(|ix| {
            doc.blocks[ix]
                .parts()
                .first()
                .map(|part| Self::new(ix, *part, 0))
        })
    }

    pub fn previous_editable(doc: &Doc, from: usize) -> Option<Self> {
        (0..=from.min(doc.blocks.len().saturating_sub(1)))
            .rev()
            .find_map(|ix| {
                let part = *doc.blocks[ix].parts().last()?;
                let offset = doc.blocks[ix].text_at(part).map(|t| t.len()).unwrap_or(0);
                Some(Self::new(ix, part, offset))
            })
    }

    pub fn left(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        if here.offset > 0
            && let Some(text) = doc.blocks[here.block].text_at(here.part)
        {
            let mut offset = here.offset - 1;
            while offset > 0 && !text.text.is_char_boundary(offset) {
                offset -= 1;
            }
            return Self { offset, ..here };
        }
        if here.block > 0
            && let Some(prev) = Self::previous_editable(doc, here.block - 1)
        {
            return prev;
        }
        here
    }

    pub fn right(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        if let Some(text) = doc.blocks[here.block].text_at(here.part)
            && here.offset < text.len()
        {
            let mut offset = here.offset + 1;
            while offset < text.len() && !text.text.is_char_boundary(offset) {
                offset += 1;
            }
            return Self { offset, ..here };
        }
        if here.block + 1 < doc.blocks.len()
            && let Some(next) = Self::next_editable(doc, here.block + 1)
        {
            return next;
        }
        here
    }

    pub fn home(self) -> Self {
        Self { offset: 0, ..self }
    }

    pub fn end(self, doc: &Doc) -> Self {
        let here = self.clamp(doc);
        let offset = doc.blocks[here.block]
            .text_at(here.part)
            .map(|t| t.len())
            .unwrap_or(0);
        Self { offset, ..here }
    }
}

/// A selection between two cursors in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Selection {
    pub anchor: Cursor,
    pub head: Cursor,
}

impl Selection {
    pub fn new(anchor: Cursor, head: Cursor) -> Self {
        Self { anchor, head }
    }

    pub fn at(cursor: Cursor) -> Self {
        Self {
            anchor: cursor,
            head: cursor,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }

    pub fn ordered(&self) -> (Cursor, Cursor) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }

    pub fn clamp(self, doc: &Doc) -> Self {
        Self {
            anchor: self.anchor.clamp(doc),
            head: self.head.clamp(doc),
        }
    }

    pub fn all(doc: &Doc) -> Self {
        let start = Cursor::next_editable(doc, 0).unwrap_or_default();
        let end =
            Cursor::previous_editable(doc, doc.blocks.len().saturating_sub(1)).unwrap_or(start);
        Self::new(start, end)
    }
}

/// What an edit mutation did to document positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Splice {
    pub removed: Selection,
    pub caret: Cursor,
    pub blocks: isize,
}

/// Highlight wash color.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum HighlightColor {
    Yellow,
    Red,
    Green,
    Blue,
    Purple,
}

/// State/wash for anchored annotations.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Annotation {
    #[default]
    Open,
    Resolved,
    Active,
    Highlight(HighlightColor),
    Match,
    Current,
}

/// Returns whether the URL targets a known image format.
pub fn is_image(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".svg")
}

/// Returns whether the text is a valid URL.
pub fn is_url(text: &str) -> bool {
    text.starts_with("http://") || text.starts_with("https://")
}

/// Parse a markdown string into a [`Doc`].
pub fn parse(source: &str) -> Doc {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;

    let parser = Parser::new_ext(source, options);
    let mut blocks = Vec::new();
    let mut current_text = String::new();
    let mut current_marks: Vec<MarkSpan> = Vec::new();
    let mut mark_stack: Vec<(Mark, usize)> = Vec::new();
    let mut in_quote = false;
    let mut current_quote_kind: Option<QuoteKind> = None;
    let mut list_indent: u8 = 0;
    let mut current_task_checked: Option<bool> = None;
    let mut list_stack: Vec<Option<u64>> = Vec::new();

    let mut table_align: Vec<Align> = Vec::new();
    let mut table_header: Vec<Text> = Vec::new();
    let mut table_rows: Vec<Vec<Text>> = Vec::new();
    let mut current_row: Vec<Text> = Vec::new();
    let mut in_table_head = false;

    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => {
                current_text.clear();
                current_marks.clear();
                mark_stack.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                if !current_text.is_empty() || in_quote {
                    let text = Text {
                        text: std::mem::take(&mut current_text),
                        marks: std::mem::take(&mut current_marks),
                    };
                    let kind = if in_quote {
                        BlockKind::Quote {
                            kind: current_quote_kind,
                            text,
                        }
                    } else {
                        BlockKind::Paragraph(text)
                    };
                    blocks.push(Block::at(kind, list_indent));
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                current_text.clear();
                current_marks.clear();
                mark_stack.clear();
                let lvl = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                let _ = lvl;
            }
            Event::End(TagEnd::Heading(level)) => {
                let lvl = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                let text = Text {
                    text: std::mem::take(&mut current_text),
                    marks: std::mem::take(&mut current_marks),
                };
                blocks.push(Block::at(
                    BlockKind::Heading { level: lvl, text },
                    list_indent,
                ));
            }
            Event::Start(Tag::BlockQuote(kind)) => {
                in_quote = true;
                current_quote_kind = kind.map(|k| match k {
                    BlockQuoteKind::Note => QuoteKind::Note,
                    BlockQuoteKind::Tip => QuoteKind::Tip,
                    BlockQuoteKind::Important => QuoteKind::Important,
                    BlockQuoteKind::Warning => QuoteKind::Warning,
                    BlockQuoteKind::Caution => QuoteKind::Caution,
                });
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_quote = false;
                current_quote_kind = None;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                current_text.clear();
                let language = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let l = lang.trim();
                        if l.is_empty() {
                            None
                        } else {
                            Some(l.to_string())
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                blocks.push(Block::at(
                    BlockKind::Code {
                        language,
                        code: Text::default(),
                    },
                    list_indent,
                ));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(Block {
                    kind: BlockKind::Code { code, .. },
                    ..
                }) = blocks.last_mut()
                {
                    *code = Text::plain(std::mem::take(&mut current_text));
                }
            }
            Event::Start(Tag::List(first_num)) => {
                list_stack.push(first_num);
                list_indent = list_indent.saturating_add(1);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                list_indent = list_indent.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                current_text.clear();
                current_marks.clear();
                current_task_checked = None;
            }
            Event::TaskListMarker(checked) => {
                current_task_checked = Some(checked);
            }
            Event::End(TagEnd::Item) => {
                if !current_text.is_empty() || current_task_checked.is_some() {
                    let text = Text {
                        text: std::mem::take(&mut current_text),
                        marks: std::mem::take(&mut current_marks),
                    };
                    let kind = if let Some(checked) = current_task_checked.take() {
                        BlockKind::Task { checked, text }
                    } else if let Some(Some(num)) = list_stack.last_mut() {
                        let cur = *num;
                        *num += 1;
                        BlockKind::Ordered { number: cur, text }
                    } else {
                        BlockKind::Bullet(text)
                    };
                    blocks.push(Block::at(kind, list_indent.saturating_sub(1)));
                }
            }
            Event::Start(Tag::Table(alignments)) => {
                table_align = alignments
                    .into_iter()
                    .map(|a| match a {
                        pulldown_cmark::Alignment::Right => Align::Right,
                        pulldown_cmark::Alignment::Center => Align::Center,
                        _ => Align::Left,
                    })
                    .collect();
                table_header.clear();
                table_rows.clear();
            }
            Event::End(TagEnd::Table) => {
                blocks.push(Block::at(
                    BlockKind::Table {
                        align: std::mem::take(&mut table_align),
                        header: std::mem::take(&mut table_header),
                        rows: std::mem::take(&mut table_rows),
                    },
                    list_indent,
                ));
            }
            Event::Start(Tag::TableHead) => {
                in_table_head = true;
            }
            Event::End(TagEnd::TableHead) => {
                in_table_head = false;
            }
            Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                table_rows.push(std::mem::take(&mut current_row));
            }
            Event::Start(Tag::TableCell) => {
                current_text.clear();
                current_marks.clear();
                mark_stack.clear();
            }
            Event::End(TagEnd::TableCell) => {
                let cell_text = Text {
                    text: std::mem::take(&mut current_text),
                    marks: std::mem::take(&mut current_marks),
                };
                if in_table_head {
                    table_header.push(cell_text);
                } else {
                    current_row.push(cell_text);
                }
            }
            Event::Rule => {
                blocks.push(Block::at(BlockKind::Rule, list_indent));
            }
            Event::Text(s) => {
                current_text.push_str(&s);
            }
            Event::Code(s) => {
                let start = current_text.len();
                current_text.push_str(&s);
                current_marks.push(MarkSpan {
                    range: start..current_text.len(),
                    mark: Mark::Code,
                });
            }
            Event::Start(Tag::Strong) => {
                mark_stack.push((Mark::Bold, current_text.len()));
            }
            Event::End(TagEnd::Strong) => {
                if let Some((Mark::Bold, start)) = mark_stack.pop() {
                    current_marks.push(MarkSpan {
                        range: start..current_text.len(),
                        mark: Mark::Bold,
                    });
                }
            }
            Event::Start(Tag::Emphasis) => {
                mark_stack.push((Mark::Italic, current_text.len()));
            }
            Event::End(TagEnd::Emphasis) => {
                if let Some((Mark::Italic, start)) = mark_stack.pop() {
                    current_marks.push(MarkSpan {
                        range: start..current_text.len(),
                        mark: Mark::Italic,
                    });
                }
            }
            Event::Start(Tag::Strikethrough) => {
                mark_stack.push((Mark::Strike, current_text.len()));
            }
            Event::End(TagEnd::Strikethrough) => {
                if let Some((Mark::Strike, start)) = mark_stack.pop() {
                    current_marks.push(MarkSpan {
                        range: start..current_text.len(),
                        mark: Mark::Strike,
                    });
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                mark_stack.push((Mark::Link(dest_url.to_string()), current_text.len()));
            }
            Event::End(TagEnd::Link) => {
                if let Some((Mark::Link(url), start)) = mark_stack.pop() {
                    current_marks.push(MarkSpan {
                        range: start..current_text.len(),
                        mark: Mark::Link(url),
                    });
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                current_text.push('\n');
            }
            _ => {}
        }
    }

    if !current_text.is_empty() {
        let text = Text {
            text: current_text,
            marks: current_marks,
        };
        blocks.push(Block::at(BlockKind::Paragraph(text), list_indent));
    }

    if blocks.is_empty() && !source.is_empty() {
        blocks.push(Block::new(BlockKind::Paragraph(Text::plain(source))));
    }

    Doc { blocks }
}

/// Serialize a [`Doc`] back to markdown wire representation.
pub fn serialize(doc: &Doc) -> String {
    let mut out = String::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        let indent_str = "  ".repeat(block.indent as usize);
        match &block.kind {
            BlockKind::Paragraph(text) => {
                out.push_str(&indent_str);
                out.push_str(&text.text);
            }
            BlockKind::Heading { level, text } => {
                out.push_str(&indent_str);
                for _ in 0..*level {
                    out.push('#');
                }
                out.push(' ');
                out.push_str(&text.text);
            }
            BlockKind::Bullet(text) => {
                out.push_str(&indent_str);
                out.push_str("- ");
                out.push_str(&text.text);
            }
            BlockKind::Ordered { number, text } => {
                out.push_str(&indent_str);
                out.push_str(&format!("{number}. "));
                out.push_str(&text.text);
            }
            BlockKind::Task { checked, text } => {
                out.push_str(&indent_str);
                if *checked {
                    out.push_str("- [x] ");
                } else {
                    out.push_str("- [ ] ");
                }
                out.push_str(&text.text);
            }
            BlockKind::Quote { kind, text } => {
                out.push_str(&indent_str);
                out.push_str("> ");
                if let Some(k) = kind {
                    out.push_str(k.marker());
                    out.push(' ');
                }
                out.push_str(&text.text);
            }
            BlockKind::Code { language, code } => {
                out.push_str(&indent_str);
                out.push_str("```");
                if let Some(lang) = language {
                    out.push_str(lang);
                }
                out.push('\n');
                out.push_str(&code.text);
                if !code.text.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&indent_str);
                out.push_str("```");
            }
            BlockKind::Image { url, alt, .. } => {
                out.push_str(&indent_str);
                out.push_str(&format!("![{}]({})", alt.text, url));
            }
            BlockKind::Bookmark { url, .. } => {
                out.push_str(&indent_str);
                out.push_str(url);
            }
            BlockKind::Table { header, rows, .. } => {
                out.push_str(&indent_str);
                out.push('|');
                for col in header {
                    out.push(' ');
                    out.push_str(&col.text);
                    out.push_str(" |");
                }
                out.push('\n');
                out.push_str(&indent_str);
                out.push('|');
                for _ in header {
                    out.push_str(" --- |");
                }
                for row in rows {
                    out.push('\n');
                    out.push_str(&indent_str);
                    out.push('|');
                    for cell in row {
                        out.push(' ');
                        out.push_str(&cell.text);
                        out.push_str(" |");
                    }
                }
            }
            BlockKind::Rule => {
                out.push_str(&indent_str);
                out.push_str("---");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_headings() {
        let md = "# Heading 1\n\n## Heading 2\n\n### Heading 3";
        let doc = parse(md);
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(
            doc.blocks[0].kind,
            BlockKind::Heading {
                level: 1,
                text: Text::plain("Heading 1")
            }
        );
        assert_eq!(
            doc.blocks[1].kind,
            BlockKind::Heading {
                level: 2,
                text: Text::plain("Heading 2")
            }
        );
        assert_eq!(
            doc.blocks[2].kind,
            BlockKind::Heading {
                level: 3,
                text: Text::plain("Heading 3")
            }
        );

        let output = serialize(&doc);
        assert!(output.contains("# Heading 1"));
        assert!(output.contains("## Heading 2"));
        assert!(output.contains("### Heading 3"));
    }

    #[test]
    fn test_parse_lists_and_tasks() {
        let md = "- [ ] Task 1\n- [x] Task 2\n- Bullet item\n1. Numbered item";
        let doc = parse(md);
        assert_eq!(doc.blocks.len(), 4);

        assert_eq!(
            doc.blocks[0].kind,
            BlockKind::Task {
                checked: false,
                text: Text::plain("Task 1")
            }
        );
        assert_eq!(
            doc.blocks[1].kind,
            BlockKind::Task {
                checked: true,
                text: Text::plain("Task 2")
            }
        );
        assert_eq!(
            doc.blocks[2].kind,
            BlockKind::Bullet(Text::plain("Bullet item"))
        );
        assert_eq!(
            doc.blocks[3].kind,
            BlockKind::Ordered {
                number: 1,
                text: Text::plain("Numbered item")
            }
        );

        let serialized = serialize(&doc);
        assert!(serialized.contains("- [ ] Task 1"));
        assert!(serialized.contains("- [x] Task 2"));
        assert!(serialized.contains("- Bullet item"));
        assert!(serialized.contains("1. Numbered item"));
    }

    #[test]
    fn test_parse_quotes_and_alerts() {
        let md = "> Simple blockquote\n\n> [!NOTE]\n> Note body\n\n> [!WARNING]\n> Warning body";
        let doc = parse(md);
        assert_eq!(doc.blocks.len(), 3);

        if let BlockKind::Quote { kind, text } = &doc.blocks[0].kind {
            assert_eq!(*kind, None);
            assert_eq!(text.text, "Simple blockquote");
        } else {
            panic!("expected quote");
        }

        if let BlockKind::Quote { kind, text } = &doc.blocks[1].kind {
            assert_eq!(*kind, Some(QuoteKind::Note));
            assert_eq!(text.text, "Note body");
        } else {
            panic!("expected Note quote");
        }

        if let BlockKind::Quote { kind, text } = &doc.blocks[2].kind {
            assert_eq!(*kind, Some(QuoteKind::Warning));
            assert_eq!(text.text, "Warning body");
        } else {
            panic!("expected Warning quote");
        }

        let serialized = serialize(&doc);
        assert!(serialized.contains("> Simple blockquote"));
        assert!(serialized.contains("> [!NOTE] Note body"));
        assert!(serialized.contains("> [!WARNING] Warning body"));
    }

    #[test]
    fn test_parse_code_block() {
        let md = "```rust\nfn main() {\n    println!(\"hello\");\n}\n```";
        let doc = parse(md);
        assert_eq!(doc.blocks.len(), 1);

        if let BlockKind::Code { language, code } = &doc.blocks[0].kind {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(code.text.contains("fn main()"));
        } else {
            panic!("expected code block");
        }

        let serialized = serialize(&doc);
        assert!(serialized.contains("```rust"));
        assert!(serialized.contains("fn main()"));
    }

    #[test]
    fn test_parse_tables() {
        let md = "| Header 1 | Header 2 |\n| --- | --- |\n| Cell 1 | Cell 2 |";
        let doc = parse(md);
        assert_eq!(doc.blocks.len(), 1);

        if let BlockKind::Table {
            align,
            header,
            rows,
        } = &doc.blocks[0].kind
        {
            assert_eq!(header.len(), 2);
            assert_eq!(header[0].text, "Header 1");
            assert_eq!(header[1].text, "Header 2");
            assert_eq!(align.len(), 2);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0][0].text, "Cell 1");
            assert_eq!(rows[0][1].text, "Cell 2");
        } else {
            panic!("expected table block");
        }

        let serialized = serialize(&doc);
        assert!(serialized.contains("| Header 1 | Header 2 |"));
        assert!(serialized.contains("| Cell 1 | Cell 2 |"));
    }

    #[test]
    fn test_thematic_rule() {
        let md = "Paragraph 1\n\n---\n\nParagraph 2";
        let doc = parse(md);
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(doc.blocks[1].kind, BlockKind::Rule);

        let serialized = serialize(&doc);
        assert!(serialized.contains("---"));
    }

    #[test]
    fn test_cursor_and_selection_clamping() {
        let doc = parse("Short\n\nSecond line");
        let cursor = Cursor::new(0, Part::Body, 100);
        let clamped = cursor.clamp(&doc);
        assert_eq!(clamped.block, 0);
        assert_eq!(clamped.offset, 5); // "Short" has len 5

        let out_of_bounds = Cursor::new(10, Part::Body, 0);
        let clamped_oob = out_of_bounds.clamp(&doc);
        assert_eq!(clamped_oob.block, 1); // doc has 2 blocks (0 and 1)

        let sel = Selection::new(
            Cursor::new(0, Part::Body, 0),
            Cursor::new(5, Part::Body, 50),
        );
        let clamped_sel = sel.clamp(&doc);
        assert_eq!(clamped_sel.anchor.block, 0);
        assert_eq!(clamped_sel.head.block, 1);
    }

    #[test]
    fn test_cursor_left_right() {
        let doc = parse("A\n\nB");
        let start = Cursor::new(0, Part::Body, 0);
        let right = start.right(&doc);
        assert_eq!(right.offset, 1);
        let right2 = right.right(&doc);
        // Moves to next block
        assert_eq!(right2.block, 1);
        assert_eq!(right2.offset, 0);

        let left = right2.left(&doc);
        assert_eq!(left.block, 0);
        assert_eq!(left.offset, 1);
    }
}
