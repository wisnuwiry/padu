//! Block-level markdown parsing over `pulldown-cmark`.
//!
//! A full parse produces a [`BlockTree`]: top-level blocks paired with their
//! byte ranges in the source. The range start of the second-to-last
//! source-level block is a *stable boundary* — appending to the source cannot
//! change anything before it (see [`IncrementalParser::settled_prefix`] for
//! why the final block alone is not enough) — which is what
//! [`IncrementalParser`] exploits so a streamed delta costs roughly
//! O(delta + last two blocks) instead of O(document).
//!
//! Soundness guard: link reference definitions (`[label]: url`) resolve
//! non-locally, so a source containing one drops back to full reparses.

use std::ops::Range;
use std::sync::LazyLock;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use regex::Regex;

/// CommonMark only recognizes angle-bracket autolinks. Transcript content is
/// conversational, so bare web URLs should be useful without requiring the
/// author to write `<https://...>` or `[label](https://...)`.
static BARE_WEB_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\bhttps?://[^\s<>"`\\]+"#).expect("bare web URL regex should compile")
});

static MENTION_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\A|\s)@([a-zA-Z0-9_.\-\\/]+)").expect("mention regex should compile")
});

// ── Tree model ─────────────────────────────────────────────────────────────

/// Inline styling threaded through nested emphasis and links.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub mention: bool,
    pub strikethrough: bool,
    /// Destination URL when inside a link.
    pub link: Option<String>,
}

/// One run of identically styled inline text.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineRun {
    pub text: String,
    pub style: InlineStyle,
}

impl InlineRun {
    pub(crate) fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: InlineStyle::default(),
        }
    }
}

/// An open inline-HTML tag, tracked across `Event::InlineHtml` fragments so a
/// `<b>…</b>` pair styles the `Event::Text` runs between its two halves.
#[derive(Clone, Debug)]
enum HtmlOp {
    Bold,
    Italic,
    Code,
    Strikethrough,
    Sup,
    Sub,
    Link { url: Option<String> },
}

/// The effective inline style: the markdown style threaded through emphasis
/// and links, plus every open HTML tag.
fn effective_style(markdown: &InlineStyle, html: &[HtmlOp]) -> InlineStyle {
    let mut style = markdown.clone();
    for op in html {
        match op {
            HtmlOp::Bold => style.bold = true,
            HtmlOp::Italic => style.italic = true,
            HtmlOp::Code => style.code = true,
            HtmlOp::Strikethrough => style.strikethrough = true,
            HtmlOp::Link { url } => style.link = url.clone(),
            HtmlOp::Sup | HtmlOp::Sub => {}
        }
    }
    style
}

fn pop_html_op(html: &mut Vec<HtmlOp>, kind: HtmlOpMatch) {
    let Some(index) = html.iter().rposition(|op| match kind {
        HtmlOpMatch::Bold => matches!(op, HtmlOp::Bold),
        HtmlOpMatch::Italic => matches!(op, HtmlOp::Italic),
        HtmlOpMatch::Code => matches!(op, HtmlOp::Code),
        HtmlOpMatch::Strikethrough => matches!(op, HtmlOp::Strikethrough),
        HtmlOpMatch::Sup => matches!(op, HtmlOp::Sup),
        HtmlOpMatch::Sub => matches!(op, HtmlOp::Sub),
        HtmlOpMatch::Link => matches!(op, HtmlOp::Link { .. }),
    }) else {
        return;
    };
    html.remove(index);
}

#[derive(Clone, Copy)]
enum HtmlOpMatch {
    Bold,
    Italic,
    Code,
    Strikethrough,
    Sup,
    Sub,
    Link,
}

/// GFM column alignment. Unspecified renders as `Left`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TableAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// A list item: its blocks plus an optional task-list checkbox state.
#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// One piece of inline content. Images interrupt a run of text rather than
/// styling it, so they cannot be an [`InlineStyle`] flag. The same goes for
/// inline math (`$…$`) and HTML sup/sub, which render as their own elements.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InlinePiece {
    Run(InlineRun),
    Image { url: String, alt: String },
    Math { source: String, display: bool },
    SubSup { text: String, sup: bool },
}

/// A markdown block. Containers nest.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Paragraph {
        runs: Vec<InlineRun>,
    },
    /// A paragraph that mixes text with inline math or sup/sub, which cannot
    /// be shaped into a single run. Rendered as a wrapping row of text
    /// elements and math images.
    InlineRich {
        pieces: Vec<InlinePiece>,
    },
    /// A standalone image. Inline images split their paragraph so the text
    /// before and after keeps its order around them.
    Image {
        url: String,
        alt: String,
    },
    /// Display math (`$$…$$`), rendered as an SVG image by the rich renderer.
    Math {
        source: String,
        display: bool,
    },
    /// A ` ```mermaid ` fenced block, rendered as an SVG diagram.
    Mermaid {
        source: String,
    },
    Heading {
        level: u8,
        runs: Vec<InlineRun>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    BlockQuote {
        children: Vec<Block>,
    },
    List {
        /// `Some(n)` for an ordered list starting at `n`.
        ordered_start: Option<u64>,
        items: Vec<ListItem>,
    },
    Table {
        header: Vec<Vec<InlineRun>>,
        rows: Vec<Vec<Vec<InlineRun>>>,
        align: Vec<TableAlign>,
    },
    Rule,
}

/// A top-level block plus its byte range in the source.
#[derive(Clone, Debug, PartialEq)]
pub struct TopBlock {
    pub range: Range<usize>,
    pub block: Block,
}

/// Top-level blocks in document order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockTree {
    pub blocks: Vec<TopBlock>,
}

#[cfg(test)]
impl BlockTree {
    pub fn len(&self) -> usize {
        self.blocks.len()
    }
}

// ── Full parse ─────────────────────────────────────────────────────────────

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
}

/// Parse a whole source into a [`BlockTree`].
pub fn parse(source: &str) -> BlockTree {
    let events = Parser::new_ext(source, options())
        .into_offset_iter()
        .collect::<Vec<_>>();
    let mut cursor = Cursor {
        events: &events,
        index: 0,
    };
    let mut blocks = Vec::new();
    while let Some((event, range)) = cursor.peek() {
        let range = range.clone();
        match event {
            Event::Rule => {
                cursor.bump();
                blocks.push(TopBlock {
                    range,
                    block: Block::Rule,
                });
            }
            Event::Start(_) => {
                for block in parse_started_block(&mut cursor) {
                    blocks.push(TopBlock {
                        range: range.clone(),
                        block,
                    });
                }
            }
            // Stray inline events at the top level should not occur; skip them
            // rather than looping forever.
            _ => cursor.bump(),
        }
    }
    BlockTree { blocks }
}

struct Cursor<'a, 'e> {
    events: &'a [(Event<'e>, Range<usize>)],
    index: usize,
}

impl<'e> Cursor<'_, 'e> {
    fn peek(&self) -> Option<&(Event<'e>, Range<usize>)> {
        self.events.get(self.index)
    }

    fn peek_event(&self) -> Option<&Event<'e>> {
        self.peek().map(|(event, _)| event)
    }

    fn bump(&mut self) {
        self.index += 1;
    }

    fn next_event(&mut self) -> Option<Event<'e>> {
        let event = self.events.get(self.index).map(|(event, _)| event.clone());
        if event.is_some() {
            self.index += 1;
        }
        event
    }
}

fn is_block_tag(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::CodeBlock(_)
            | Tag::BlockQuote(_)
            | Tag::List(_)
            | Tag::Item
            | Tag::Table(_)
            | Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
    )
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Consume a `Start(tag)` and everything through its matching `End`. Unknown
/// containers are transparent: their children splice into the parent.
fn parse_started_block(cursor: &mut Cursor) -> Vec<Block> {
    let Some(Event::Start(tag)) = cursor.next_event() else {
        return Vec::new();
    };
    match tag {
        Tag::Paragraph => pieces_into_blocks(parse_inline_container(cursor)),
        Tag::Heading { level, .. } => vec![Block::Heading {
            level: heading_level(level),
            runs: pieces_into_runs(parse_inline_container(cursor)),
        }],
        Tag::CodeBlock(kind) => {
            let language = match kind {
                CodeBlockKind::Fenced(info) => info
                    .split_whitespace()
                    .next()
                    .filter(|language| !language.is_empty())
                    .map(str::to_owned),
                CodeBlockKind::Indented => None,
            };
            let mut code = String::new();
            loop {
                match cursor.next_event() {
                    Some(Event::Text(text)) => code.push_str(&text),
                    Some(Event::End(_)) | None => break,
                    Some(_) => {}
                }
            }
            // Fenced blocks carry a trailing newline; rendering is per line so
            // the empty final line would otherwise add a phantom row.
            if code.ends_with('\n') {
                code.pop();
            }
            // `mermaid` fences are diagrams; LaTeX fences are explicit
            // display-math escapes (mirroring the web client).
            match language.as_deref() {
                Some("mermaid") => vec![Block::Mermaid { source: code }],
                Some("latex") | Some("tex") | Some("math") => vec![Block::Math {
                    source: code,
                    display: true,
                }],
                _ => vec![Block::CodeBlock { language, code }],
            }
        }
        Tag::BlockQuote(_) => vec![Block::BlockQuote {
            children: parse_block_sequence(cursor),
        }],
        Tag::List(ordered_start) => {
            let mut items = Vec::new();
            loop {
                match cursor.peek_event() {
                    Some(Event::Start(Tag::Item)) => {
                        cursor.bump();
                        items.push(parse_list_item(cursor));
                    }
                    Some(Event::End(_)) | None => {
                        cursor.bump();
                        break;
                    }
                    Some(_) => cursor.bump(),
                }
            }
            vec![Block::List {
                ordered_start,
                items,
            }]
        }
        Tag::Table(alignments) => {
            let align = alignments
                .iter()
                .map(|alignment| match alignment {
                    Alignment::Center => TableAlign::Center,
                    Alignment::Right => TableAlign::Right,
                    Alignment::None | Alignment::Left => TableAlign::Left,
                })
                .collect();
            vec![parse_table(cursor, align)]
        }
        Tag::HtmlBlock => {
            // Raw HTML renders structurally where the allowlist can map it
            // (tables, paragraphs, inline styles); anything else keeps its
            // text. An agent transcript is far more likely to be *discussing*
            // markup than asking us to apply it, so unmappable fragments stay
            // literal rather than being reinterpreted.
            let mut text = String::new();
            loop {
                match cursor.next_event() {
                    Some(Event::Html(chunk) | Event::Text(chunk)) => text.push_str(&chunk),
                    Some(Event::End(_)) | None => break,
                    Some(_) => {}
                }
            }
            let text = text.trim_end_matches('\n').to_owned();
            if text.is_empty() {
                Vec::new()
            } else if let Some(blocks) = super::raw_html::blocks_from_html(&text) {
                blocks
            } else {
                vec![Block::Paragraph {
                    runs: vec![InlineRun::plain(text)],
                }]
            }
        }
        // Transparent containers (footnote definitions, and anything a future
        // pulldown-cmark adds).
        _ => parse_block_sequence(cursor),
    }
}

/// Parse a list item, lifting a leading task-list marker out of its content.
fn parse_list_item(cursor: &mut Cursor) -> ListItem {
    let task = match cursor.peek_event() {
        Some(Event::TaskListMarker(checked)) => {
            let checked = *checked;
            cursor.bump();
            Some(checked)
        }
        _ => None,
    };
    ListItem {
        task,
        blocks: parse_block_sequence(cursor),
    }
}

/// Parse blocks until the container's `End` (which is consumed). Bare inline
/// events — tight list items — accumulate into an implicit paragraph.
fn parse_block_sequence(cursor: &mut Cursor) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut inline: Vec<InlinePiece> = Vec::new();
    let mut html: Vec<HtmlOp> = Vec::new();
    while let Some(event) = cursor.peek_event() {
        match event {
            Event::End(_) => {
                cursor.bump();
                break;
            }
            Event::Start(tag) if is_block_tag(tag) => {
                flush_paragraph(&mut blocks, &mut inline);
                blocks.extend(parse_started_block(cursor));
            }
            Event::Rule => {
                flush_paragraph(&mut blocks, &mut inline);
                cursor.bump();
                blocks.push(Block::Rule);
            }
            _ => parse_inline_event(cursor, &mut inline, &InlineStyle::default(), &mut html),
        }
    }
    flush_paragraph(&mut blocks, &mut inline);
    blocks
}

fn flush_paragraph(blocks: &mut Vec<Block>, inline: &mut Vec<InlinePiece>) {
    if !inline.is_empty() {
        blocks.extend(pieces_into_blocks(merge_pieces(std::mem::take(inline))));
    }
}

fn parse_table(cursor: &mut Cursor, align: Vec<TableAlign>) -> Block {
    let mut header = Vec::new();
    let mut rows = Vec::new();
    loop {
        match cursor.peek_event() {
            Some(Event::Start(Tag::TableHead)) => {
                cursor.bump();
                header = parse_table_row(cursor);
            }
            Some(Event::Start(Tag::TableRow)) => {
                cursor.bump();
                rows.push(parse_table_row(cursor));
            }
            Some(Event::End(_)) | None => {
                cursor.bump();
                break;
            }
            Some(_) => cursor.bump(),
        }
    }
    Block::Table {
        header,
        rows,
        align,
    }
}

fn parse_table_row(cursor: &mut Cursor) -> Vec<Vec<InlineRun>> {
    let mut cells = Vec::new();
    loop {
        match cursor.peek_event() {
            Some(Event::Start(Tag::TableCell)) => {
                cursor.bump();
                // A table cell is one line of text; an image there degrades to
                // its alt rather than breaking the row's geometry.
                cells.push(pieces_into_runs(parse_inline_container(cursor)));
            }
            Some(Event::End(_)) | None => {
                cursor.bump();
                break;
            }
            Some(_) => cursor.bump(),
        }
    }
    cells
}

/// Collect inline pieces until the container's `End` (which is consumed).
fn parse_inline_container(cursor: &mut Cursor) -> Vec<InlinePiece> {
    let mut pieces = Vec::new();
    let mut html: Vec<HtmlOp> = Vec::new();
    while let Some(event) = cursor.peek_event() {
        if matches!(event, Event::End(_)) {
            cursor.bump();
            break;
        }
        parse_inline_event(cursor, &mut pieces, &InlineStyle::default(), &mut html);
    }
    merge_pieces(pieces)
}

/// Split inline pieces into blocks, so images and display math become their
/// own blocks and the text around them keeps its order. A paragraph that mixes
/// plain text with inline math or sup/sub becomes an [`Block::InlineRich`]
/// rather than a single-shaped paragraph.
fn pieces_into_blocks(pieces: Vec<InlinePiece>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut inline: Vec<InlinePiece> = Vec::new();
    let flush = |blocks: &mut Vec<Block>, inline: &mut Vec<InlinePiece>| {
        if inline.is_empty() {
            return;
        }
        let has_rich = inline
            .iter()
            .any(|piece| matches!(piece, InlinePiece::Math { .. } | InlinePiece::SubSup { .. }));
        if has_rich {
            blocks.push(Block::InlineRich {
                pieces: std::mem::take(inline),
            });
        } else {
            let runs = std::mem::take(inline)
                .into_iter()
                .map(|piece| match piece {
                    InlinePiece::Run(run) => run,
                    _ => unreachable!("a non-rich paragraph holds only runs"),
                })
                .collect();
            blocks.push(Block::Paragraph { runs });
        }
    };
    for piece in pieces {
        match piece {
            InlinePiece::Image { url, alt } => {
                flush(&mut blocks, &mut inline);
                blocks.push(Block::Image { url, alt });
            }
            InlinePiece::Math {
                source,
                display: true,
            } => {
                flush(&mut blocks, &mut inline);
                blocks.push(Block::Math {
                    source,
                    display: true,
                });
            }
            piece => inline.push(piece),
        }
    }
    flush(&mut blocks, &mut inline);
    blocks
}

/// Flatten pieces to runs for contexts that cannot host a block-level image.
/// Images degrade to their alt text, inline math to its source, so the caller
/// always gets plain runs.
pub(crate) fn pieces_into_runs(pieces: Vec<InlinePiece>) -> Vec<InlineRun> {
    merge_runs(
        pieces
            .into_iter()
            .map(|piece| match piece {
                InlinePiece::Run(run) => run,
                InlinePiece::Image { alt, .. } => InlineRun::plain(alt),
                InlinePiece::Math { source, .. } => InlineRun::plain(source),
                InlinePiece::SubSup { text, .. } => InlineRun::plain(text),
            })
            .collect(),
    )
}

/// Consume one inline event, appending its runs with `style` applied. Nested
/// emphasis and links recurse with an extended style; `html` tracks open
/// inline-HTML tags so their text runs pick up the tag's styling.
fn parse_inline_event(
    cursor: &mut Cursor,
    pieces: &mut Vec<InlinePiece>,
    style: &InlineStyle,
    html: &mut Vec<HtmlOp>,
) {
    let Some(event) = cursor.next_event() else {
        return;
    };
    let mut push_run = |run: InlineRun| pieces.push(InlinePiece::Run(run));
    match event {
        Event::Text(text) => {
            // HTML sup/sub are their own elements; everything else applies the
            // combined markdown + HTML style.
            if let Some(op) = html.last() {
                match op {
                    HtmlOp::Sup => {
                        pieces.push(InlinePiece::SubSup {
                            text: text.to_string(),
                            sup: true,
                        });
                        return;
                    }
                    HtmlOp::Sub => {
                        pieces.push(InlinePiece::SubSup {
                            text: text.to_string(),
                            sup: false,
                        });
                        return;
                    }
                    _ => {}
                }
            }
            push_run(InlineRun {
                text: text.to_string(),
                style: effective_style(style, html),
            });
        }
        Event::Code(text) => {
            let mut style = effective_style(style, html);
            style.code = true;
            push_run(InlineRun {
                text: text.to_string(),
                style,
            });
        }
        Event::InlineMath(text) => pieces.push(InlinePiece::Math {
            source: text.to_string(),
            display: false,
        }),
        Event::DisplayMath(text) => pieces.push(InlinePiece::Math {
            source: text.to_string(),
            display: true,
        }),
        // A hard or soft break inside a paragraph is a line break in the
        // rendered run: shaped text splits on '\n' on its own.
        Event::SoftBreak | Event::HardBreak => push_run(InlineRun {
            text: "\n".to_owned(),
            style: effective_style(style, html),
        }),
        Event::Start(Tag::Image {
            dest_url, title, ..
        }) => {
            // The image's children are its alt text.
            let mut alt_pieces = Vec::new();
            while let Some(event) = cursor.peek_event() {
                if matches!(event, Event::End(_)) {
                    cursor.bump();
                    break;
                }
                parse_inline_event(
                    cursor,
                    &mut alt_pieces,
                    &InlineStyle::default(),
                    &mut Vec::new(),
                );
            }
            let alt = pieces_into_runs(alt_pieces)
                .into_iter()
                .map(|run| run.text)
                .collect::<String>();
            let alt = if alt.trim().is_empty() {
                title.to_string()
            } else {
                alt
            };
            pieces.push(InlinePiece::Image {
                url: dest_url.to_string(),
                alt,
            });
        }
        Event::Start(tag) => {
            let mut nested = style.clone();
            match &tag {
                Tag::Emphasis => nested.italic = true,
                Tag::Strong => nested.bold = true,
                Tag::Strikethrough => nested.strikethrough = true,
                Tag::Link { dest_url, .. } => nested.link = Some(dest_url.to_string()),
                _ => {}
            }
            while let Some(event) = cursor.peek_event() {
                if matches!(event, Event::End(_)) {
                    cursor.bump();
                    break;
                }
                parse_inline_event(cursor, pieces, &nested, html);
            }
        }
        // Inline HTML is mapped onto the allowlist (bold, links, images,
        // sup/sub); unknown tags degrade to their text.
        Event::Html(text) | Event::InlineHtml(text) => {
            handle_inline_html(&text, style, html, pieces)
        }
        Event::FootnoteReference(label) => push_run(InlineRun {
            text: format!("[{label}]"),
            style: effective_style(style, html),
        }),
        Event::TaskListMarker(checked) => push_run(InlineRun {
            text: if checked { "[x] " } else { "[ ] " }.to_owned(),
            style: style.clone(),
        }),
        Event::End(_) | Event::Rule => {}
    }
}

/// Map one `InlineHtml` fragment onto the allowlist. Pulldown splits a tag pair
/// across events, so an opening tag (e.g. `<b>`) pushes onto `html` and its
/// closer pops; a self-contained fragment (text, `<img>`, `<br>`, `<a …>x</a>`)
/// is walked immediately with the currently-open tags applied.
fn handle_inline_html(
    fragment: &str,
    style: &InlineStyle,
    html: &mut Vec<HtmlOp>,
    pieces: &mut Vec<InlinePiece>,
) {
    let trimmed = fragment.trim();
    if trimmed.starts_with("</") {
        let name = trimmed[2..]
            .trim_end_matches('>')
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match name.as_str() {
            "b" | "strong" => pop_html_op(html, HtmlOpMatch::Bold),
            "i" | "em" => pop_html_op(html, HtmlOpMatch::Italic),
            "code" => pop_html_op(html, HtmlOpMatch::Code),
            "s" | "del" | "strike" => pop_html_op(html, HtmlOpMatch::Strikethrough),
            "sup" => pop_html_op(html, HtmlOpMatch::Sup),
            "sub" => pop_html_op(html, HtmlOpMatch::Sub),
            "a" => pop_html_op(html, HtmlOpMatch::Link),
            _ => {}
        }
        return;
    }

    let Ok(dom) = tl::parse(trimmed, tl::ParserOptions::default()) else {
        pieces.push(InlinePiece::Run(InlineRun {
            text: trimmed.to_string(),
            style: effective_style(style, html),
        }));
        return;
    };
    let handles = dom.children();
    let bare_tag = handles.len() == 1
        && handles
            .first()
            .and_then(|handle| handle.get(dom.parser()))
            .is_some_and(
                |node| matches!(node, tl::Node::Tag(tag) if tag.children().top().len() == 0),
            );

    if bare_tag {
        if let Some(node) = handles.first().and_then(|handle| handle.get(dom.parser())) {
            if let tl::Node::Tag(tag) = node {
                let name = tag.name().as_utf8_str().to_ascii_lowercase();
                match name.as_str() {
                    "b" | "strong" => html.push(HtmlOp::Bold),
                    "i" | "em" => html.push(HtmlOp::Italic),
                    "code" => html.push(HtmlOp::Code),
                    "s" | "del" | "strike" => html.push(HtmlOp::Strikethrough),
                    "sup" => html.push(HtmlOp::Sup),
                    "sub" => html.push(HtmlOp::Sub),
                    "a" => {
                        let url = tag
                            .attributes()
                            .get("href")
                            .and_then(|value| value)
                            .map(|value| value.as_utf8_str().into_owned())
                            .filter(|url| safe_inline_url(url));
                        html.push(HtmlOp::Link { url });
                    }
                    "br" => pieces.push(InlinePiece::Run(InlineRun {
                        text: "\n".into(),
                        style: effective_style(style, html),
                    })),
                    "img" => push_inline_image(tag, pieces),
                    "script" | "style" | "iframe" | "object" | "embed" | "noscript" => {}
                    _ => {}
                }
            }
        }
        return;
    }

    super::raw_html::push_inline_pieces(trimmed, &effective_style(style, html), pieces);
}

fn safe_inline_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    !lower.starts_with("javascript:") && !lower.starts_with("vbscript:")
}

fn push_inline_image(tag: &tl::HTMLTag<'_>, pieces: &mut Vec<InlinePiece>) {
    let src = tag
        .attributes()
        .get("src")
        .and_then(|value| value)
        .map(|value| value.as_utf8_str().into_owned())
        .filter(|url| safe_inline_url(url));
    let alt = tag
        .attributes()
        .get("alt")
        .and_then(|value| value)
        .map(|value| value.as_utf8_str().into_owned())
        .unwrap_or_default();
    if let Some(src) = src {
        pieces.push(InlinePiece::Image { url: src, alt });
    }
}

/// Sentence punctuation and unmatched closing delimiters are prose around a
/// URL, not part of it. Balanced delimiters remain valid URL characters, as in
/// Wikipedia paths ending in `(disambiguation)`.
fn trimmed_bare_url_end(text: &str, start: usize, candidate_end: usize) -> usize {
    let mut end = candidate_end;
    let mut parens = delimiter_balance(&text[start..end], '(', ')');
    let mut brackets = delimiter_balance(&text[start..end], '[', ']');
    let mut braces = delimiter_balance(&text[start..end], '{', '}');
    loop {
        let Some((offset, last)) = text[start..end].char_indices().next_back() else {
            return start;
        };
        let last_index = start + offset;
        let should_trim = match last {
            '.' | ',' | ':' | ';' | '?' | '!' | '\'' => true,
            ')' if parens < 0 => {
                parens += 1;
                true
            }
            ']' if brackets < 0 => {
                brackets += 1;
                true
            }
            '}' if braces < 0 => {
                braces += 1;
                true
            }
            _ => false,
        };
        if !should_trim {
            return end;
        }
        end = last_index;
    }
}

fn delimiter_balance(text: &str, open: char, close: char) -> i32 {
    text.chars().fold(0, |balance, character| {
        balance + i32::from(character == open) - i32::from(character == close)
    })
}

/// Coalesce neighbouring runs that share a style, leaving images in place.
fn merge_pieces(pieces: Vec<InlinePiece>) -> Vec<InlinePiece> {
    let mut merged: Vec<InlinePiece> = Vec::with_capacity(pieces.len());
    for piece in pieces {
        match piece {
            InlinePiece::Run(run) if run.text.is_empty() => {}
            InlinePiece::Run(run) => match merged.last_mut() {
                Some(InlinePiece::Run(last)) if last.style == run.style => {
                    last.text.push_str(&run.text)
                }
                _ => merged.push(InlinePiece::Run(run)),
            },
            image => merged.push(image),
        }
    }
    detect_inline_mentions(linkify_bare_urls(merged))
}

fn detect_inline_mentions(pieces: Vec<InlinePiece>) -> Vec<InlinePiece> {
    let mut out = Vec::with_capacity(pieces.len());
    for piece in pieces {
        match piece {
            InlinePiece::Run(run)
                if !run.style.code && run.style.link.is_none() && !run.style.mention =>
            {
                push_mentioned_run(run, &mut out);
            }
            piece => out.push(piece),
        }
    }
    out
}

fn push_mentioned_run(run: InlineRun, pieces: &mut Vec<InlinePiece>) {
    let mut cursor = 0;
    for candidate in MENTION_PATTERN.find_iter(&run.text) {
        let match_str = candidate.as_str();
        let at_offset = match_str.find('@').unwrap_or(0);
        let at_start = candidate.start() + at_offset;
        let candidate_end = candidate.end();
        let mention_text = &run.text[at_start..candidate_end];
        let trimmed_mention =
            mention_text.trim_end_matches([',', ';', '!', '?', ':', ')', ']', '}', '"', '\'']);
        if trimmed_mention.len() <= 1 {
            continue;
        }
        // File references such as `@src/app.rs` should remain ordinary text;
        // only conversational mentions receive the chip treatment.
        let file_like = trimmed_mention
            .trim_start_matches('@')
            .contains(['/', '\\'])
            || trimmed_mention
                .trim_start_matches('@')
                .rsplit_once('.')
                .is_some_and(|(_, extension)| !extension.is_empty());
        if file_like {
            continue;
        }
        let end = at_start + trimmed_mention.len();
        if cursor < at_start {
            pieces.push(InlinePiece::Run(InlineRun {
                text: run.text[cursor..at_start].to_owned(),
                style: run.style.clone(),
            }));
        }
        let mut mention_style = run.style.clone();
        mention_style.mention = true;
        pieces.push(InlinePiece::Run(InlineRun {
            text: run.text[at_start..end].to_owned(),
            style: mention_style,
        }));
        cursor = end;
    }

    if cursor < run.text.len() {
        pieces.push(InlinePiece::Run(InlineRun {
            text: run.text[cursor..].to_owned(),
            style: run.style,
        }));
    }
}

/// Linkify after Markdown has produced and merged its inline runs. Pulldown
/// can split ordinary text at potential emphasis punctuation inside a URL;
/// merging first lets the detector recover the whole displayed target.
fn linkify_bare_urls(pieces: Vec<InlinePiece>) -> Vec<InlinePiece> {
    let mut linked = Vec::with_capacity(pieces.len());
    for piece in pieces {
        match piece {
            InlinePiece::Run(run) if !run.style.code && run.style.link.is_none() => {
                push_linkified_run(run, &mut linked);
            }
            piece => linked.push(piece),
        }
    }
    linked
}

fn push_linkified_run(run: InlineRun, pieces: &mut Vec<InlinePiece>) {
    let mut cursor = 0;
    for candidate in BARE_WEB_URL.find_iter(&run.text) {
        let end = trimmed_bare_url_end(&run.text, candidate.start(), candidate.end());
        if end <= candidate.start() {
            continue;
        }
        if cursor < candidate.start() {
            pieces.push(InlinePiece::Run(InlineRun {
                text: run.text[cursor..candidate.start()].to_owned(),
                style: run.style.clone(),
            }));
        }

        let url = &run.text[candidate.start()..end];
        let mut link_style = run.style.clone();
        link_style.link = Some(url.to_owned());
        pieces.push(InlinePiece::Run(InlineRun {
            text: url.to_owned(),
            style: link_style,
        }));
        cursor = end;
    }

    if cursor < run.text.len() {
        pieces.push(InlinePiece::Run(InlineRun {
            text: run.text[cursor..].to_owned(),
            style: run.style,
        }));
    }
}

/// Coalesce neighbouring runs that share a style, so shaping sees the fewest
/// possible `TextRun`s.
fn merge_runs(runs: Vec<InlineRun>) -> Vec<InlineRun> {
    let mut merged: Vec<InlineRun> = Vec::with_capacity(runs.len());
    for run in runs {
        if run.text.is_empty() {
            continue;
        }
        match merged.last_mut() {
            Some(last) if last.style == run.style => last.text.push_str(&run.text),
            _ => merged.push(run),
        }
    }
    merged
}

// ── Incremental parse ──────────────────────────────────────────────────────

/// Streaming parser over an append-only source.
///
/// Keeps the settled prefix of top-level blocks and reparses only the tail from
/// the last stable boundary. `display_tree` additionally mends hanging inline
/// markers (see [`super::mend`]) so a closing `**` never reflows text that is
/// already on screen.
pub struct IncrementalParser {
    text: String,
    tree: BlockTree,
    /// Blocks before this index are settled: no append can change them.
    stable_prefix: usize,
    /// A link reference definition anywhere forces full reparses.
    full_reparse_only: bool,
}

impl Default for IncrementalParser {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalParser {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            tree: BlockTree::default(),
            stable_prefix: 0,
            full_reparse_only: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn tree(&self) -> &BlockTree {
        &self.tree
    }

    /// Point the parser at `text`. Appends reparse incrementally; any other
    /// change falls back to a full reparse.
    pub fn set_text(&mut self, text: &str) {
        if text == self.text {
            return;
        }
        match text.strip_prefix(self.text.as_str()) {
            Some(delta) if !self.text.is_empty() && !self.full_reparse_only => {
                let delta = delta.to_owned();
                self.append(&delta);
            }
            _ => self.reset(text),
        }
    }

    /// Discard all state and parse `text` from scratch.
    pub fn reset(&mut self, text: &str) {
        self.text = text.to_owned();
        self.tree = parse(&self.text);
        self.full_reparse_only = has_link_definition(&self.text);
        self.stable_prefix = self.settled_prefix();
    }

    /// Append `delta`, reparsing only from the last stable block boundary.
    pub fn append(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if self.full_reparse_only {
            let mut text = std::mem::take(&mut self.text);
            text.push_str(delta);
            self.reset(&text);
            return;
        }

        let boundary = self
            .tree
            .blocks
            .get(self.stable_prefix)
            .map_or(self.text.len(), |block| block.range.start);
        self.text.push_str(delta);
        if has_link_definition(delta) {
            let text = std::mem::take(&mut self.text);
            self.reset(&text);
            return;
        }

        let tail = parse(&self.text[boundary..]);
        self.tree.blocks.truncate(self.stable_prefix);
        self.tree
            .blocks
            .extend(tail.blocks.into_iter().map(|mut block| {
                block.range.start += boundary;
                block.range.end += boundary;
                block
            }));
        self.stable_prefix = self.settled_prefix();
    }

    /// Replacement blocks for the final block while streaming, with its hanging
    /// inline markers closed so styling does not flip as the closer arrives.
    /// `None` means the canonical tree already renders correctly.
    ///
    /// This returns only the tail — never the whole tree — so a streaming
    /// response re-derives O(last block) per delta rather than cloning every
    /// settled block on every frame.
    pub fn display_tail(&self) -> Option<Vec<TopBlock>> {
        let last = self.tree.blocks.last()?;
        // A code block's content is literal: mending would corrupt it, and a
        // half-typed fence must not be reinterpreted.
        if matches!(last.block, Block::CodeBlock { .. }) {
            return None;
        }
        let mended = super::mend::close_hanging(&self.text[last.range.start..])?;
        let offset = last.range.start;
        Some(
            parse(&mended)
                .blocks
                .into_iter()
                .map(|mut block| {
                    block.range.start += offset;
                    block.range.end = (block.range.end + offset).min(self.text.len());
                    block
                })
                .collect(),
        )
    }

    /// The whole tree as it should be displayed. The renderer works from
    /// [`Self::display_tail`] to avoid this clone; this exists so the mending
    /// contract can be asserted directly.
    #[cfg(test)]
    pub fn display_tree(&self) -> BlockTree {
        let Some(tail) = self.display_tail() else {
            return self.tree.clone();
        };
        let mut blocks = self.tree.blocks[..self.tree.blocks.len() - 1].to_vec();
        blocks.extend(tail);
        BlockTree { blocks }
    }

    /// Index of the first block an append could still change.
    ///
    /// Appending mostly only extends the final block, but two cases reach
    /// further back, so the last *two* source-level groups stay unsettled:
    ///
    /// - A GFM table absorbs the line after it once that line becomes a valid
    ///   row, yet a partial row of just `|` transiently parses as its own
    ///   paragraph. Settling the table then would strand every later row in
    ///   that trailing paragraph.
    /// - A paragraph split around an inline image yields several blocks that
    ///   share one source range; they must settle and reparse as a unit or the
    ///   pieces before the image get re-emitted on the next append.
    fn settled_prefix(&self) -> usize {
        let blocks = &self.tree.blocks;
        let mut index = blocks.len();
        for _ in 0..2 {
            let Some(group_start) = index.checked_sub(1).map(|last| blocks[last].range.start)
            else {
                break;
            };
            while index > 0 && blocks[index - 1].range.start == group_start {
                index -= 1;
            }
        }
        index
    }
}

/// Cheap scan for a link reference definition (`[label]: destination`), which
/// resolves references anywhere in the document and so breaks locality.
fn has_link_definition(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix('[') else {
            return false;
        };
        rest.find("]:").is_some_and(|end| end > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph_text(block: &Block) -> String {
        match block {
            Block::Paragraph { runs } | Block::Heading { runs, .. } => {
                runs.iter().map(|run| run.text.as_str()).collect()
            }
            _ => panic!("expected a text block, got {block:?}"),
        }
    }

    #[test]
    fn inline_math_becomes_a_rich_paragraph() {
        let tree = parse("Solve $x^2 + 1 = 0$ for real $x$.");
        assert_eq!(tree.len(), 1);
        let Block::InlineRich { pieces } = &tree.blocks[0].block else {
            panic!("expected an inline-rich paragraph");
        };
        assert!(matches!(
            pieces.iter().find(|piece| matches!(piece, InlinePiece::Math { .. })),
            Some(InlinePiece::Math { source, display: false }) if source == "x^2 + 1 = 0"
        ));
        // The surrounding prose keeps its order as runs.
        assert!(pieces.iter().any(|piece| matches!(
            piece,
            InlinePiece::Run(run) if run.text == "Solve "
        )));
    }

    #[test]
    fn display_math_becomes_its_own_block() {
        let tree = parse("Before.\n\n$$\\int_0^\\infty e^{-x} dx$$\n\nAfter.");
        assert_eq!(tree.len(), 3);
        assert!(matches!(
            tree.blocks[1].block,
            Block::Math { display: true, ref source } if source == "\\int_0^\\infty e^{-x} dx"
        ));
        assert_eq!(paragraph_text(&tree.blocks[0].block), "Before.");
    }

    #[test]
    fn mermaid_fences_become_diagram_blocks() {
        let tree = parse("```mermaid\nflowchart LR\n  A --> B\n```\n");
        assert_eq!(tree.len(), 1);
        assert!(matches!(
            tree.blocks[0].block,
            Block::Mermaid { ref source } if source == "flowchart LR\n  A --> B"
        ));
    }

    #[test]
    fn latex_fences_become_display_math() {
        for fence in ["latex", "tex", "math"] {
            let tree = parse(&format!("```{fence}\nE = mc^2\n```\n"));
            assert!(
                matches!(
                    tree.blocks[0].block,
                    Block::Math { display: true, ref source } if source == "E = mc^2"
                ),
                "{fence} fence should become display math"
            );
        }
    }

    #[test]
    fn other_fences_stay_code_blocks() {
        let tree = parse("```mermaid\nx\n```\n\n```text\nmermaid\n```\n");
        assert!(matches!(tree.blocks[0].block, Block::Mermaid { .. }));
        assert!(matches!(tree.blocks[1].block, Block::CodeBlock { .. }));
    }

    #[test]
    fn inline_html_maps_to_styles_not_literal_text() {
        let tree = parse("Go <b>bold</b> and <i>italic</i>.");
        let Block::Paragraph { runs } = &tree.blocks[0].block else {
            panic!("expected a paragraph, got {:?}", tree.blocks[0]);
        };
        assert!(runs.iter().any(|run| run.style.bold && run.text == "bold"));
        assert!(
            runs.iter()
                .any(|run| run.style.italic && run.text == "italic")
        );
        assert!(
            runs.iter().all(|run| !run.text.contains('<')),
            "inline HTML should be interpreted, not literal"
        );
    }

    #[test]
    fn block_html_tables_become_tables() {
        let tree = parse("<table><tr><th>A</th></tr><tr><td>1</td></tr></table>");
        assert!(matches!(tree.blocks[0].block, Block::Table { .. }));
    }

    #[test]
    fn parses_top_level_blocks_with_source_ranges() {
        let source = "# Title\n\nBody text.\n\n```rust\nfn main() {}\n```\n";
        let tree = parse(source);

        assert_eq!(tree.len(), 3);
        assert!(matches!(
            tree.blocks[0].block,
            Block::Heading { level: 1, .. }
        ));
        assert_eq!(paragraph_text(&tree.blocks[0].block), "Title");
        assert_eq!(paragraph_text(&tree.blocks[1].block), "Body text.");
        assert_eq!(
            tree.blocks[2].block,
            Block::CodeBlock {
                language: Some("rust".into()),
                code: "fn main() {}".into(),
            }
        );
        // Ranges point back into the source and are ascending.
        assert!(tree.blocks[0].range.start < tree.blocks[1].range.start);
        assert_eq!(&source[tree.blocks[1].range.clone()].trim(), &"Body text.");
    }

    #[test]
    fn file_mentions_remain_plain_text() {
        let tree = parse("See @src/app.rs and @alice");
        let Block::Paragraph { runs } = &tree.blocks[0].block else {
            panic!("expected a paragraph");
        };
        let file = runs
            .iter()
            .find(|run| run.text.contains("@src/app.rs"))
            .unwrap();
        assert!(!file.style.mention);
        let person = runs.iter().find(|run| run.text == "@alice").unwrap();
        assert!(person.style.mention);
    }

    #[test]
    fn inline_styles_nest_and_merge() {
        let tree = parse("plain **bold `code`** and *italic* [link](https://example.com)");
        let Block::Paragraph { runs } = &tree.blocks[0].block else {
            panic!("expected a paragraph");
        };

        let bold_code = runs
            .iter()
            .find(|run| run.style.bold && run.style.code)
            .expect("bold inline code should keep both styles");
        assert_eq!(bold_code.text, "code");

        let italic = runs
            .iter()
            .find(|run| run.style.italic)
            .expect("italic run");
        assert_eq!(italic.text, "italic");

        let link = runs
            .iter()
            .find(|run| run.style.link.is_some())
            .expect("link run");
        assert_eq!(link.text, "link");
        assert_eq!(link.style.link.as_deref(), Some("https://example.com"));

        // Adjacent identically styled runs are coalesced.
        assert_eq!(runs[0].text, "plain ");
    }

    #[test]
    fn bare_web_urls_become_links_without_swallowing_prose_punctuation() {
        let tree = parse(
            "See https://example.com/docs?q=one, then \
             (https://en.wikipedia.org/wiki/Rust_(programming_language)).",
        );
        let Block::Paragraph { runs } = &tree.blocks[0].block else {
            panic!("expected a paragraph");
        };
        let links = runs
            .iter()
            .filter_map(|run| {
                run.style
                    .link
                    .as_deref()
                    .map(|target| (run.text.as_str(), target))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            links,
            vec![
                (
                    "https://example.com/docs?q=one",
                    "https://example.com/docs?q=one"
                ),
                (
                    "https://en.wikipedia.org/wiki/Rust_(programming_language)",
                    "https://en.wikipedia.org/wiki/Rust_(programming_language)"
                ),
            ]
        );
        assert_eq!(
            paragraph_text(&tree.blocks[0].block),
            "See https://example.com/docs?q=one, then \
             (https://en.wikipedia.org/wiki/Rust_(programming_language))."
        );
    }

    #[test]
    fn explicit_links_and_inline_code_are_not_relinkified() {
        let tree =
            parse("[docs at https://example.com](https://padu.gg) and `https://example.com/code`");
        let Block::Paragraph { runs } = &tree.blocks[0].block else {
            panic!("expected a paragraph");
        };

        assert!(runs.iter().any(|run| {
            run.text == "docs at https://example.com"
                && run.style.link.as_deref() == Some("https://padu.gg")
        }));
        assert!(runs.iter().any(|run| {
            run.text == "https://example.com/code" && run.style.code && run.style.link.is_none()
        }));
    }

    #[test]
    fn task_list_markers_lift_out_of_item_content() {
        let tree = parse("- [x] done\n- [ ] pending\n- plain\n");
        let Block::List { items, .. } = &tree.blocks[0].block else {
            panic!("expected a list");
        };

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].task, Some(true));
        assert_eq!(items[1].task, Some(false));
        assert_eq!(items[2].task, None);
        assert_eq!(paragraph_text(&items[0].blocks[0]), "done");
        assert_eq!(paragraph_text(&items[2].blocks[0]), "plain");
    }

    #[test]
    fn tables_keep_alignment_and_cells() {
        let tree = parse("| a | b |\n|:--|--:|\n| 1 | 2 |\n");
        let Block::Table {
            header,
            rows,
            align,
        } = &tree.blocks[0].block
        else {
            panic!("expected a table");
        };

        assert_eq!(align, &[TableAlign::Left, TableAlign::Right]);
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][1][0].text, "2");
    }

    /// An image is content, not styling, so it becomes its own block and the
    /// text around it keeps its order.
    #[test]
    fn images_split_their_paragraph_and_keep_order() {
        let tree = parse("before ![a shot](https://example.com/x.png) after");
        assert_eq!(tree.len(), 3);
        assert_eq!(paragraph_text(&tree.blocks[0].block), "before ");
        assert_eq!(
            tree.blocks[1].block,
            Block::Image {
                url: "https://example.com/x.png".into(),
                alt: "a shot".into(),
            }
        );
        assert_eq!(paragraph_text(&tree.blocks[2].block), " after");
    }

    #[test]
    fn a_standalone_image_is_one_block() {
        let tree = parse("![](data:image/png;base64,aGk=)");
        assert_eq!(tree.len(), 1);
        assert!(matches!(tree.blocks[0].block, Block::Image { .. }));
    }

    /// A table cell has no room for a block-level image, so it degrades to alt
    /// text rather than dropping the content entirely.
    #[test]
    fn images_in_table_cells_fall_back_to_alt_text() {
        let tree = parse("| a | b |\n|---|---|\n| ![alt](x.png) | plain |\n");
        let Block::Table { rows, .. } = &tree.blocks[0].block else {
            panic!("expected a table");
        };
        assert_eq!(rows[0][0][0].text, "alt");
    }

    #[test]
    fn soft_breaks_become_newlines_in_the_run() {
        let tree = parse("first\nsecond");
        assert_eq!(paragraph_text(&tree.blocks[0].block), "first\nsecond");
    }

    /// The incremental path must agree with a full parse at every prefix —
    /// that equality is the whole justification for the fast path.
    #[test]
    fn incremental_appends_match_full_parses() {
        let source = "# Heading\n\nA paragraph with **bold**.\n\n- one\n- two\n\n```js\nlet x = 1;\n```\n\nTail.";
        for chunk_size in [1, 3, 7, 64] {
            let mut incremental = IncrementalParser::new();
            let mut built = String::new();
            let mut chars = source.chars().peekable();
            while chars.peek().is_some() {
                let chunk = chars.by_ref().take(chunk_size).collect::<String>();
                built.push_str(&chunk);
                incremental.append(&chunk);
                assert_eq!(
                    incremental.tree(),
                    &parse(&built),
                    "divergence at {} bytes with chunk size {chunk_size}",
                    built.len()
                );
            }
        }
    }

    /// Tables must survive streaming: a chunk boundary that lands after the
    /// delimiter row must not settle a header-only table and strand the body
    /// rows in a trailing paragraph.
    #[test]
    fn streamed_tables_match_full_parses() {
        let source = "**After** \u{2014} it's a proper neutral chip:\n\n\
            | State | Fill | Icon |\n\
            |---|---|---|\n\
            | Rest | white @ 0.12 | `.labelColor` (~85%) |\n\
            | Hover | white @ 0.20 | pure white |\n\
            | Pressed | white @ 0.26 | pure white |\n\
            | Copied | green @ 0.12 | `.systemGreen` |\n\n\
            It now reads as a real button sitting beside Export.";
        for chunk_size in [1, 2, 3, 5, 7, 11, 17, 64] {
            let mut incremental = IncrementalParser::new();
            let mut built = String::new();
            let mut chars = source.chars().peekable();
            while chars.peek().is_some() {
                let chunk = chars.by_ref().take(chunk_size).collect::<String>();
                built.push_str(&chunk);
                incremental.append(&chunk);
                assert_eq!(
                    incremental.tree(),
                    &parse(&built),
                    "divergence at {} bytes with chunk size {chunk_size}",
                    built.len()
                );
            }
        }
    }

    /// The blocks a paragraph splits into around an inline image share one
    /// source range, so they must settle and reparse as a unit: settling only
    /// part of the group would re-emit the earlier pieces on the next append.
    #[test]
    fn streamed_inline_images_do_not_duplicate_blocks() {
        let source = "before ![a shot](https://example.com/x.png) after, and more prose.";
        for chunk_size in [1, 3, 7] {
            let mut incremental = IncrementalParser::new();
            let mut built = String::new();
            let mut chars = source.chars().peekable();
            while chars.peek().is_some() {
                let chunk = chars.by_ref().take(chunk_size).collect::<String>();
                built.push_str(&chunk);
                incremental.append(&chunk);
                assert_eq!(
                    incremental.tree(),
                    &parse(&built),
                    "divergence at {} bytes with chunk size {chunk_size}",
                    built.len()
                );
            }
        }
    }

    #[test]
    fn set_text_appends_incrementally_and_resets_on_rewrite() {
        let mut parser = IncrementalParser::new();
        parser.set_text("hello");
        parser.set_text("hello world");
        assert_eq!(parser.text(), "hello world");
        assert_eq!(parser.tree(), &parse("hello world"));

        // A non-append edit still lands on the right tree.
        parser.set_text("different");
        assert_eq!(parser.tree(), &parse("different"));
    }

    #[test]
    fn link_definitions_force_full_reparses() {
        let mut parser = IncrementalParser::new();
        parser.set_text("See [docs].\n\n[docs]: https://example.com\n");
        assert!(parser.full_reparse_only);

        parser.set_text("See [docs].\n\n[docs]: https://example.com\n\nMore.");
        assert_eq!(
            parser.tree(),
            &parse("See [docs].\n\n[docs]: https://example.com\n\nMore.")
        );
        let Block::Paragraph { runs } = &parser.tree().blocks[0].block else {
            panic!("expected a paragraph");
        };
        assert_eq!(
            runs.iter().find(|run| run.style.link.is_some()).is_some(),
            true,
            "the definition should resolve the earlier reference"
        );
    }

    #[test]
    fn display_tree_closes_hanging_emphasis_while_streaming() {
        let mut parser = IncrementalParser::new();
        parser.set_text("Settled.\n\nNow **bold");

        // The canonical tree keeps the markers literal.
        let Block::Paragraph { runs } = &parser.tree().blocks[1].block else {
            panic!("expected a paragraph");
        };
        assert!(runs.iter().all(|run| !run.style.bold));

        // The display tree styles it immediately.
        let display = parser.display_tree();
        assert_eq!(display.len(), 2);
        let Block::Paragraph { runs } = &display.blocks[1].block else {
            panic!("expected a paragraph");
        };
        assert!(
            runs.iter().any(|run| run.style.bold && run.text == "bold"),
            "streaming bold should be styled before its closer arrives: {runs:?}"
        );
        // Settled blocks are untouched.
        assert_eq!(display.blocks[0], parser.tree().blocks[0]);
    }

    #[test]
    fn display_tree_leaves_code_blocks_literal() {
        let mut parser = IncrementalParser::new();
        parser.set_text("```rust\nlet a = **b;\n");
        assert_eq!(parser.display_tree(), *parser.tree());
    }
}
