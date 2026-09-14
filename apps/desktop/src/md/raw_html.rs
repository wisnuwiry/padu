//! GitHub-style raw HTML: an allowlist over `tl`'s HTML parser mapped onto
//! the existing markdown model, mirroring the tag set the web client keeps
//! through its sanitize schema (`apps/web/src/lib/markdown-html.ts`).
//!
//! Block HTML is converted to existing [`Block`]s (tables reuse the table
//! renderer, quotes reuse the quote renderer, lists reuse the list renderer),
//! and inline HTML becomes styled runs, images, and sup/sub pieces at parse
//! time — so rendering and find-in-page stay structurally identical to plain
//! markdown. Anything outside the allowlist degrades to its text content;
//! `<script>`, `<style>`, `<iframe>` and friends are dropped outright, and URLs
//! that could execute code are refused.

use std::borrow::Cow;

use tl::{Node, ParserOptions};

use super::parser::{Block, InlinePiece, InlineRun, InlineStyle, ListItem, TableAlign};

fn tag_name(name: &tl::Bytes<'_>) -> String {
    name.as_utf8_str().to_ascii_lowercase()
}

fn is_safe_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    !lower.starts_with("javascript:") && !lower.starts_with("vbscript:")
}

/// `tl` hands back raw source slices with entities intact, so `&nbsp;` would
/// otherwise render literally. Decode them the way browsers (and the web
/// client's DOM parser) do — before the safety check, so an encoded
/// `javascript:` URL is still refused.
fn decode_entities(text: &str) -> Cow<'_, str> {
    html_escape::decode_html_entities(text)
}

/// Collapse runs of whitespace the way block HTML does, so a multi-line
/// `<div>` fragment becomes one readable paragraph.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn trim_end_newlines(mut text: String) -> String {
    while text.ends_with('\n') {
        text.pop();
    }
    text
}

fn paragraph_of(text: &str) -> Block {
    Block::Paragraph {
        runs: vec![InlineRun::plain(text.to_owned())],
    }
}

/// Parse a block-level HTML fragment into markdown blocks. `None` means nothing
/// usable survived (e.g. a fragment that stripped entirely), and the caller
/// falls back to rendering it literally.
pub fn blocks_from_html(fragment: &str) -> Option<Vec<Block>> {
    let dom = tl::parse(fragment, ParserOptions::default()).ok()?;
    let mut blocks = Vec::new();
    for handle in dom.children() {
        if let Some(node) = handle.get(dom.parser()) {
            blocks.extend(node_blocks(&dom, node));
        }
    }
    if blocks.is_empty() {
        let text = normalize_whitespace(&tag_inner_text(&dom, dom.children()));
        if text.is_empty() {
            return None;
        }
        blocks.push(paragraph_of(&text));
    }
    Some(blocks)
}

fn tag_inner_text(dom: &tl::VDom<'_>, handles: &[tl::NodeHandle]) -> String {
    decode_entities(
        &handles
            .iter()
            .filter_map(|handle| handle.get(dom.parser()))
            .map(|node| node.inner_text(dom.parser()).into_owned())
            .collect::<String>(),
    )
    .into_owned()
}

fn node_blocks(dom: &tl::VDom<'_>, node: &Node) -> Vec<Block> {
    match node {
        Node::Comment(_) => Vec::new(),
        Node::Raw(bytes) => {
            let text = normalize_whitespace(&decode_entities(&bytes.as_utf8_str()));
            if text.is_empty() {
                Vec::new()
            } else {
                vec![paragraph_of(&text)]
            }
        }
        Node::Tag(tag) => {
            let name = tag_name(tag.name());
            match name.as_str() {
                "table" => table_blocks(dom, tag),
                "blockquote" => {
                    let children = tag
                        .children()
                        .top()
                        .iter()
                        .filter_map(|handle| handle.get(dom.parser()))
                        .flat_map(|child| node_blocks(dom, child))
                        .collect::<Vec<_>>();
                    if children.is_empty() {
                        Vec::new()
                    } else {
                        vec![Block::BlockQuote { children }]
                    }
                }
                "pre" => {
                    let code = tag
                        .children()
                        .top()
                        .iter()
                        .filter_map(|handle| handle.get(dom.parser()))
                        .map(|child| decode_entities(&child.inner_text(dom.parser())).into_owned())
                        .collect::<String>();
                    vec![Block::CodeBlock {
                        language: None,
                        code: trim_end_newlines(code),
                    }]
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = name[1..].parse().unwrap_or(1);
                    let runs = inline_runs_of(dom, tag);
                    vec![Block::Heading { level, runs }]
                }
                "hr" => vec![Block::Rule],
                "ul" | "ol" => list_blocks(dom, tag),
                // `<dl>` is transparent: its `<dt>`/`<dd>` children become
                // their own paragraphs below.
                "dl" => tag
                    .children()
                    .top()
                    .iter()
                    .filter_map(|handle| handle.get(dom.parser()))
                    .flat_map(|child| node_blocks(dom, child))
                    .collect(),
                "dt" => {
                    let mut runs = inline_runs_of(dom, tag);
                    for run in &mut runs {
                        run.style.bold = true;
                    }
                    if runs.is_empty() {
                        Vec::new()
                    } else {
                        vec![Block::Paragraph { runs }]
                    }
                }
                "picture" | "img" => picture_image(dom, tag)
                    .map(|(url, alt)| vec![Block::Image { url, alt }])
                    .unwrap_or_default(),
                // Paragraph-ish containers, including `<details>`/`<summary>`
                // which render expanded (a collapsible needs interactive state
                // the markdown surface does not own). Everything inside is
                // one inline paragraph — an `<img>`/`<picture>` degrades to
                // its alt text rather than splitting the paragraph.
                "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "aside"
                | "nav" | "figure" | "figcaption" | "details" | "summary" | "dd" | "li" => {
                    let runs = inline_runs_of(dom, tag);
                    if runs.is_empty() {
                        Vec::new()
                    } else {
                        vec![Block::Paragraph { runs }]
                    }
                }
                _ => {
                    let text =
                        normalize_whitespace(&decode_entities(&tag.inner_text(dom.parser())));
                    if text.is_empty() {
                        Vec::new()
                    } else {
                        vec![paragraph_of(&text)]
                    }
                }
            }
        }
    }
}

fn table_blocks(dom: &tl::VDom<'_>, table: &tl::HTMLTag<'_>) -> Vec<Block> {
    let mut rows: Vec<(bool, Vec<(Vec<InlineRun>, TableAlign)>)> = Vec::new();
    for row_handle in table.children().top().iter() {
        let Some(Node::Tag(row)) = row_handle.get(dom.parser()) else {
            continue;
        };
        if tag_name(row.name()) != "tr" {
            continue;
        }
        let mut cells = Vec::new();
        let mut is_header = false;
        for cell_handle in row.children().top().iter() {
            let Some(Node::Tag(cell)) = cell_handle.get(dom.parser()) else {
                continue;
            };
            let cell_name = tag_name(cell.name());
            if cell_name == "th" {
                is_header = true;
            }
            if cell_name != "th" && cell_name != "td" {
                continue;
            }
            let runs = inline_runs_of(dom, cell);
            let align = cell
                .attributes()
                .get("align")
                .and_then(|value| value)
                .map(|value| value.as_utf8_str().to_ascii_lowercase());
            let align = match align.as_deref() {
                Some("center") => TableAlign::Center,
                Some("right") => TableAlign::Right,
                _ => TableAlign::Left,
            };
            cells.push((runs, align));
        }
        if !cells.is_empty() {
            rows.push((is_header, cells));
        }
    }
    if rows.is_empty() {
        return Vec::new();
    }

    let mut header = Vec::new();
    let mut align = Vec::new();
    let body: Vec<Vec<Vec<InlineRun>>> = rows
        .iter()
        .filter_map(|(is_header, cells)| {
            if *is_header {
                header = cells.iter().map(|(runs, _)| runs.clone()).collect();
                align = cells.iter().map(|(_, align)| *align).collect();
                None
            } else {
                Some(cells.iter().map(|(runs, _)| runs.clone()).collect())
            }
        })
        .collect();

    vec![Block::Table {
        header,
        rows: body,
        align,
    }]
}

/// Collapse ASCII whitespace the way rendered HTML does: any run of spaces,
/// tabs, or newlines becomes one space. `&nbsp;` (U+00A0) is preserved — it
/// is a real character, not collapsible whitespace.
fn collapse_ascii_whitespace(text: &str) -> Cow<'_, str> {
    if !text
        .bytes()
        .any(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0C))
    {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0C') {
            pending_space = true;
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    if pending_space {
        out.push(' ');
    }
    Cow::Owned(out)
}

/// Trim collapsible edge whitespace from a run list, the way a browser trims
/// a block container's content.
fn trim_runs(runs: &mut Vec<InlineRun>) {
    if let Some(first) = runs.first_mut() {
        first.text = first
            .text
            .trim_start_matches([' ', '\t', '\n', '\r', '\x0C'])
            .to_owned();
    }
    if let Some(last) = runs.last_mut() {
        last.text = last
            .text
            .trim_end_matches([' ', '\t', '\n', '\r', '\x0C'])
            .to_owned();
    }
    runs.retain(|run| !run.text.is_empty());
}

/// Inline runs for a tag's content: images degrade to their alt text, inline
/// math to its source, so the caller always gets plain runs.
fn inline_runs_of(dom: &tl::VDom<'_>, tag: &tl::HTMLTag<'_>) -> Vec<InlineRun> {
    let mut pieces = Vec::new();
    walk_tag_children(dom, tag, &InlineStyle::default(), &mut pieces);
    let mut runs = super::parser::pieces_into_runs(pieces);
    trim_runs(&mut runs);
    runs
}

/// Parse an HTML list into a [`Block::List`]. A leading
/// `<input type="checkbox">` in an `<li>` becomes the item's task state, the
/// way GitHub renders HTML task lists.
fn list_blocks(dom: &tl::VDom<'_>, list: &tl::HTMLTag<'_>) -> Vec<Block> {
    let ordered = tag_name(list.name()) == "ol";
    let ordered_start = if ordered {
        Some(
            list.attributes()
                .get("start")
                .and_then(|value| value)
                .and_then(|value| value.as_utf8_str().parse::<u64>().ok())
                .unwrap_or(1),
        )
    } else {
        None
    };
    let mut items = Vec::new();
    for handle in list.children().top().iter() {
        let Some(Node::Tag(child)) = handle.get(dom.parser()) else {
            continue;
        };
        if tag_name(child.name()) != "li" {
            continue;
        }
        let mut task = None;
        let mut blocks = Vec::new();
        let mut skip_first_input = false;
        if let Some(first) = child.children().top().iter().find_map(|handle| {
            handle.get(dom.parser()).and_then(|node| match node {
                Node::Tag(tag) if tag_name(tag.name()) == "input" => Some(tag),
                _ => None,
            })
        }) {
            let is_checkbox = first
                .attributes()
                .get("type")
                .and_then(|value| value)
                .is_some_and(|value| value.as_utf8_str().eq_ignore_ascii_case("checkbox"));
            if is_checkbox {
                task = Some(first.attributes().get("checked").is_some());
                skip_first_input = true;
            }
        }
        for item_handle in child.children().top().iter() {
            let Some(node) = item_handle.get(dom.parser()) else {
                continue;
            };
            if skip_first_input
                && let Node::Tag(tag) = node
                && tag_name(tag.name()) == "input"
            {
                skip_first_input = false;
                continue;
            }
            blocks.extend(node_blocks(dom, node));
        }
        if blocks.is_empty() {
            continue;
        }
        items.push(ListItem { task, blocks });
    }
    if items.is_empty() {
        Vec::new()
    } else {
        vec![Block::List {
            ordered_start,
            items,
        }]
    }
}

fn attr_value(tag: &tl::HTMLTag<'_>, name: &str) -> Option<String> {
    tag.attributes()
        .get(name)
        .and_then(|value| value)
        .map(|value| decode_entities(&value.as_utf8_str()).into_owned())
}

/// First URL of a `srcset` list (`"a.png 1x, b.png 2x"` → `"a.png"`).
fn srcset_first_url(srcset: &str) -> Option<String> {
    srcset
        .split(',')
        .filter_map(|candidate| candidate.split_whitespace().next())
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_owned)
        .next()
}

/// Resolve a `<picture>` (or lone `<img>`) to one image. The first `<source
/// srcset>` wins — mirroring that browsers prefer sources over the fallback —
/// and the `<img>` child supplies both fallback URL and alt text.
fn picture_image(dom: &tl::VDom<'_>, tag: &tl::HTMLTag<'_>) -> Option<(String, String)> {
    if tag_name(tag.name()) == "img" {
        let src = attr_value(tag, "src").filter(|url| is_safe_url(url))?;
        let alt = attr_value(tag, "alt").unwrap_or_default();
        return Some((src, alt));
    }
    let mut source_url = None;
    let mut fallback = None;
    let mut alt = String::new();
    for handle in tag.children().top().iter() {
        let Some(Node::Tag(child)) = handle.get(dom.parser()) else {
            continue;
        };
        match tag_name(child.name()).as_str() {
            "source" => {
                if source_url.is_none()
                    && let Some(url) = child
                        .attributes()
                        .get("srcset")
                        .and_then(|value| value)
                        .and_then(|value| srcset_first_url(&decode_entities(&value.as_utf8_str())))
                        .filter(|url| is_safe_url(url))
                {
                    source_url = Some(url);
                }
            }
            "img" => {
                if alt.is_empty()
                    && let Some(value) = attr_value(child, "alt")
                {
                    alt = value;
                }
                if fallback.is_none() {
                    fallback = attr_value(child, "src").filter(|url| is_safe_url(url));
                }
            }
            _ => {}
        }
    }
    source_url.or(fallback).map(|url| (url, alt))
}

/// Append inline pieces for an inline HTML fragment with `style` applied.
pub fn push_inline_pieces(fragment: &str, style: &InlineStyle, pieces: &mut Vec<InlinePiece>) {
    let Ok(dom) = tl::parse(fragment, ParserOptions::default()) else {
        pieces.push(InlinePiece::Run(InlineRun {
            text: fragment.to_string(),
            style: style.clone(),
        }));
        return;
    };
    for handle in dom.children() {
        if let Some(node) = handle.get(dom.parser()) {
            push_inline_node(&dom, node, style, pieces);
        }
    }
}

fn walk_tag_children(
    dom: &tl::VDom<'_>,
    tag: &tl::HTMLTag<'_>,
    style: &InlineStyle,
    pieces: &mut Vec<InlinePiece>,
) {
    for handle in tag.children().top().iter() {
        if let Some(node) = handle.get(dom.parser()) {
            push_inline_node(dom, node, style, pieces);
        }
    }
}

fn push_inline_node(
    dom: &tl::VDom<'_>,
    node: &Node,
    style: &InlineStyle,
    pieces: &mut Vec<InlinePiece>,
) {
    match node {
        Node::Comment(_) => {}
        Node::Raw(bytes) => {
            let raw = bytes.as_utf8_str();
            let decoded = decode_entities(&raw);
            let text = collapse_ascii_whitespace(&decoded);
            if !text.is_empty() {
                pieces.push(InlinePiece::Run(InlineRun {
                    text: text.into_owned(),
                    style: style.clone(),
                }));
            }
        }
        Node::Tag(tag) => {
            let name = tag_name(tag.name());
            match name.as_str() {
                "br" => pieces.push(InlinePiece::Run(InlineRun {
                    text: "\n".into(),
                    style: style.clone(),
                })),
                "b" | "strong" => {
                    let mut nested = style.clone();
                    nested.bold = true;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "i" | "em" => {
                    let mut nested = style.clone();
                    nested.italic = true;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "code" | "kbd" | "samp" | "var" | "tt" => {
                    let mut nested = style.clone();
                    nested.code = true;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "s" | "del" | "strike" => {
                    let mut nested = style.clone();
                    nested.strikethrough = true;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "ins" | "u" => {
                    let mut nested = style.clone();
                    nested.underline = true;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "q" => {
                    pieces.push(InlinePiece::Run(InlineRun {
                        text: "\u{201C}".into(),
                        style: style.clone(),
                    }));
                    walk_tag_children(dom, tag, style, pieces);
                    pieces.push(InlinePiece::Run(InlineRun {
                        text: "\u{201D}".into(),
                        style: style.clone(),
                    }));
                }
                "a" => {
                    let href = attr_value(tag, "href").filter(|url| is_safe_url(url));
                    let mut nested = style.clone();
                    nested.link = href;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "sup" | "sub" => {
                    let raw = tag.inner_text(dom.parser());
                    let text = decode_entities(&raw);
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !text.is_empty() {
                        pieces.push(InlinePiece::SubSup {
                            text,
                            sup: name == "sup",
                        });
                    }
                }
                "img" | "picture" => {
                    if let Some((url, alt)) = picture_image(dom, tag) {
                        if style.link.is_some() {
                            // A linked image degrades to its alt text but
                            // keeps the link, so badges stay clickable.
                            pieces.push(InlinePiece::Run(InlineRun {
                                text: alt,
                                style: style.clone(),
                            }));
                        } else {
                            pieces.push(InlinePiece::Image { url, alt });
                        }
                    }
                }
                // Only meaningful inside `<picture>`; standalone it carries
                // no renderable content.
                "source" => {}
                "input" => {
                    let is_checkbox = tag
                        .attributes()
                        .get("type")
                        .and_then(|value| value)
                        .is_some_and(|value| value.as_utf8_str().eq_ignore_ascii_case("checkbox"));
                    if is_checkbox {
                        let checked = tag.attributes().get("checked").is_some();
                        pieces.push(InlinePiece::Run(InlineRun {
                            text: if checked { "[x] " } else { "[ ] " }.into(),
                            style: style.clone(),
                        }));
                    }
                }
                "script" | "style" | "iframe" | "object" | "embed" | "noscript" => {}
                _ => walk_tag_children(dom, tag, style, pieces),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs_of(fragment: &str) -> Vec<InlineRun> {
        let mut pieces = Vec::new();
        push_inline_pieces(fragment, &InlineStyle::default(), &mut pieces);
        super::super::parser::pieces_into_runs(pieces)
    }

    #[test]
    fn inline_tags_map_to_styles() {
        let runs = runs_of("a <b>bold</b> <i>italic</i> <code>c</code> <s>struck</s>");
        let bold = runs.iter().find(|run| run.style.bold).unwrap();
        assert_eq!(bold.text, "bold");
        assert!(
            runs.iter()
                .any(|run| run.style.italic && run.text == "italic")
        );
        assert!(runs.iter().any(|run| run.style.code && run.text == "c"));
        assert!(
            runs.iter()
                .any(|run| run.style.strikethrough && run.text == "struck")
        );
        assert_eq!(runs.first().unwrap().text, "a ");
    }

    #[test]
    fn inline_anchor_becomes_a_link() {
        let runs = runs_of("see <a href=\"https://example.com\">docs</a>");
        let link = runs.iter().find(|run| run.style.link.is_some()).unwrap();
        assert_eq!(link.text, "docs");
        assert_eq!(link.style.link.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn javascript_urls_are_refused() {
        let runs = runs_of("<a href=\"javascript:alert(1)\">x</a>");
        assert!(runs.iter().all(|run| run.style.link.is_none()));
    }

    #[test]
    fn scripts_and_event_handlers_are_stripped() {
        let runs = runs_of("a<script>alert(1)</script>b<img src=\"x.png\" onerror=\"alert(1)\">");
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        assert_eq!(text, "ab");
        assert!(!text.contains("alert(1)"));
        assert!(runs.iter().all(|run| run.style.link.is_none()));
    }

    #[test]
    fn sup_sub_become_pieces() {
        let mut pieces = Vec::new();
        push_inline_pieces(
            "x<sup>2</sup>y<sub>3</sub>",
            &InlineStyle::default(),
            &mut pieces,
        );
        assert!(matches!(
            pieces.iter().find(|p| matches!(p, InlinePiece::SubSup { sup: true, .. })),
            Some(InlinePiece::SubSup { text, .. }) if text == "2"
        ));
        assert!(matches!(
            pieces.iter().find(|p| matches!(p, InlinePiece::SubSup { sup: false, .. })),
            Some(InlinePiece::SubSup { text, .. }) if text == "3"
        ));
    }

    #[test]
    fn block_table_maps_to_table_block() {
        let blocks = blocks_from_html(
            "<table><tr><th align=\"right\">H1</th><th>H2</th></tr>\
             <tr><td>a</td><td><b>b</b></td></tr></table>",
        )
        .unwrap();
        assert_eq!(blocks.len(), 1);
        let Block::Table {
            header,
            rows,
            align,
        } = &blocks[0]
        else {
            panic!("expected a table");
        };
        assert_eq!(header.len(), 2);
        assert_eq!(header[0][0].text, "H1");
        assert_eq!(align, &[TableAlign::Right, TableAlign::Left]);
        assert_eq!(rows.len(), 1);
        assert!(rows[0][1][0].style.bold, "cell runs keep inline styles");
    }

    #[test]
    fn inline_ins_kbd_q_and_input_map_to_runs() {
        let mut pieces = Vec::new();
        push_inline_pieces(
            "<ins>u</ins> <kbd>K</kbd> <samp>S</samp> <var>V</var> <tt>T</tt> <q>cited</q> <input type=\"checkbox\" checked>",
            &InlineStyle::default(),
            &mut pieces,
        );
        let runs = super::super::parser::pieces_into_runs(pieces);
        assert!(
            runs.iter()
                .any(|run| run.style.underline && run.text == "u")
        );
        for text in ["K", "S", "V", "T"] {
            assert!(
                runs.iter().any(|run| run.style.code && run.text == text),
                "{text} should be code-styled"
            );
        }
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        assert!(
            text.contains("\u{201C}cited\u{201D}"),
            "q adds quotes: {text}"
        );
        assert!(
            text.contains("[x]"),
            "checked input becomes a marker: {text}"
        );
    }

    #[test]
    fn inline_picture_prefers_source_srcset() {
        let mut pieces = Vec::new();
        push_inline_pieces(
            "<picture><source srcset=\"https://example.com/big.png 2x, https://example.com/small.png 1x\"><img src=\"https://example.com/fallback.png\" alt=\"pic\"></picture>",
            &InlineStyle::default(),
            &mut pieces,
        );
        assert_eq!(pieces.len(), 1);
        let InlinePiece::Image { url, alt } = &pieces[0] else {
            panic!("expected an image, got {:?}", pieces[0]);
        };
        assert_eq!(url, "https://example.com/big.png");
        assert_eq!(alt, "pic");
    }

    #[test]
    fn inline_picture_falls_back_to_img_src() {
        let mut pieces = Vec::new();
        push_inline_pieces(
            "<picture><img src=\"https://example.com/fallback.png\" alt=\"pic\"></picture>",
            &InlineStyle::default(),
            &mut pieces,
        );
        assert!(matches!(
            &pieces[..],
            [InlinePiece::Image { url, alt }]
                if url == "https://example.com/fallback.png" && alt == "pic"
        ));
    }

    #[test]
    fn block_picture_becomes_an_image_block() {
        let blocks = blocks_from_html(
            "<picture><source srcset=\"https://example.com/big.png\"><img src=\"https://example.com/fallback.png\" alt=\"pic\"></picture>",
        )
        .unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0],
            Block::Image {
                url: "https://example.com/big.png".into(),
                alt: "pic".into(),
            }
        );
    }

    #[test]
    fn block_paragraph_image_degrades_to_alt_text() {
        let blocks = blocks_from_html(
            "<p>before <img src=\"https://example.com/x.png\" alt=\"shot\"> after</p>",
        )
        .unwrap();
        assert_eq!(blocks.len(), 1);
        let Block::Paragraph { runs } = &blocks[0] else {
            panic!("expected a paragraph, got {:?}", blocks[0]);
        };
        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<String>(),
            "before shot after"
        );
    }

    #[test]
    fn multiline_paragraph_collapses_to_inline_text() {
        let blocks = blocks_from_html(
            "<p align=\"center\">\n  <a href=\"https://padu.dev/download\">Download</a>&nbsp;·\n  <a href=\"#overview\">Overview</a>&nbsp;·\n  <a href=\"#license\">License</a>\n</p>",
        )
        .unwrap();
        assert_eq!(blocks.len(), 1);
        let Block::Paragraph { runs } = &blocks[0] else {
            panic!("expected a paragraph, got {:?}", blocks[0]);
        };
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        assert_eq!(text, "Download\u{a0}· Overview\u{a0}· License");
        assert!(!text.contains('\n'), "no raw line breaks survive: {text:?}");
        let links = runs
            .iter()
            .filter_map(|run| run.style.link.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            links,
            vec!["https://padu.dev/download", "#overview", "#license"]
        );
    }

    #[test]
    fn badge_paragraph_renders_link_alt_text() {
        let blocks = blocks_from_html(
            "<p align=\"center\">\n  <a href=\"https://github.com/wisnuwiry/padu/stargazers\"><img src=\"https://img.shields.io/github/stars/wisnuwiry/padu\" alt=\"Stars\"></a>\n</p>",
        )
        .unwrap();
        assert_eq!(blocks.len(), 1);
        let Block::Paragraph { runs } = &blocks[0] else {
            panic!("expected a paragraph, got {:?}", blocks[0]);
        };
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "Stars");
        assert_eq!(
            runs[0].style.link.as_deref(),
            Some("https://github.com/wisnuwiry/padu/stargazers")
        );
    }

    #[test]
    fn block_hr_becomes_a_rule() {
        let blocks = blocks_from_html("<div>a</div><hr><div>b</div>").unwrap();
        assert!(matches!(blocks[1], Block::Rule));
    }

    #[test]
    fn block_lists_map_to_list_blocks() {
        let blocks = blocks_from_html(
            "<ul><li><input type=\"checkbox\" checked>done</li><li>plain</li></ul>\
             <ol start=\"3\"><li>third</li></ol>",
        )
        .unwrap();
        assert_eq!(blocks.len(), 2);
        let Block::List {
            ordered_start,
            items,
        } = &blocks[0]
        else {
            panic!("expected a list, got {:?}", blocks[0]);
        };
        assert_eq!(*ordered_start, None);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].task, Some(true));
        assert_eq!(items[1].task, None);
        let Block::List {
            ordered_start,
            items,
        } = &blocks[1]
        else {
            panic!("expected a list, got {:?}", blocks[1]);
        };
        assert_eq!(*ordered_start, Some(3));
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn block_definition_lists_become_paragraphs() {
        let blocks = blocks_from_html("<dl><dt>term</dt><dd>definition</dd></dl>").unwrap();
        assert_eq!(blocks.len(), 2);
        let Block::Paragraph { runs } = &blocks[0] else {
            panic!("expected a paragraph, got {:?}", blocks[0]);
        };
        assert_eq!(runs[0].text, "term");
        assert!(runs[0].style.bold);
        let Block::Paragraph { runs } = &blocks[1] else {
            panic!("expected a paragraph, got {:?}", blocks[1]);
        };
        assert_eq!(runs[0].text, "definition");
    }

    #[test]
    fn ruby_and_span_content_survives_transparently() {
        let runs = runs_of("<ruby>漢<rt>kan</rt></ruby> <span>x</span> <mark>m</mark>");
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        assert_eq!(text, "漢kan x m");
        assert!(runs.iter().all(|run| {
            !run.style.bold && !run.style.italic && !run.style.code && !run.style.underline
        }));
    }

    #[test]
    fn entities_decode_in_text() {
        let runs = runs_of("a&nbsp;b &amp; &lt;tag&gt; &#65;&#x42; &nosuchentity;");
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
        assert_eq!(text, "a\u{a0}b & <tag> AB &nosuchentity;");
    }

    #[test]
    fn entities_decode_in_attributes() {
        let mut pieces = Vec::new();
        push_inline_pieces(
            "<img src=\"https://example.com/a&amp;b.png\" alt=\"a &lt; b\">",
            &InlineStyle::default(),
            &mut pieces,
        );
        assert!(matches!(
            &pieces[..],
            [InlinePiece::Image { url, alt }]
                if url == "https://example.com/a&b.png" && alt == "a < b"
        ));
    }

    #[test]
    fn encoded_javascript_urls_are_still_refused() {
        let runs = runs_of("<a href=\"javascript&#58;alert(1)\">x</a>");
        assert!(runs.iter().all(|run| run.style.link.is_none()));
    }

    #[test]
    fn block_html_falls_back_to_text() {
        let blocks = blocks_from_html("<p>hello <b>world</b></p>").unwrap();
        let Block::Paragraph { runs } = &blocks[0] else {
            panic!("expected a paragraph");
        };
        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<String>(),
            "hello world"
        );
        assert!(runs.iter().any(|run| run.style.bold));
    }

    #[test]
    fn unknown_blocks_keep_inner_text() {
        let blocks = blocks_from_html("<section><div>just text</div></section>").unwrap();
        let Block::Paragraph { runs } = &blocks[0] else {
            panic!("expected a paragraph");
        };
        assert_eq!(runs[0].text, "just text");
    }
}
