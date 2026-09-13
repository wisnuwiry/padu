//! GFM-basic raw HTML: a small allowlist over `tl`'s HTML parser mapped onto
//! the existing markdown model.
//!
//! Block HTML is converted to existing [`Block`]s (tables reuse the table
//! renderer, quotes reuse the quote renderer), and inline HTML becomes styled
//! runs, images, and sup/sub pieces at parse time — so rendering and
//! find-in-page stay structurally identical to plain markdown. Anything
//! outside the allowlist degrades to its text content; `<script>`,
//! `<style>`, `<iframe>` and friends are dropped outright, and URLs that could
//! execute code are refused.

use tl::{Node, ParserOptions};

use super::parser::{Block, InlinePiece, InlineRun, InlineStyle, TableAlign};

fn tag_name(name: &tl::Bytes<'_>) -> String {
    name.as_utf8_str().to_ascii_lowercase()
}

fn is_safe_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    !lower.starts_with("javascript:") && !lower.starts_with("vbscript:")
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
    handles
        .iter()
        .filter_map(|handle| handle.get(dom.parser()))
        .map(|node| node.inner_text(dom.parser()).into_owned())
        .collect::<String>()
}

fn node_blocks(dom: &tl::VDom<'_>, node: &Node) -> Vec<Block> {
    match node {
        Node::Comment(_) => Vec::new(),
        Node::Raw(bytes) => {
            let text = normalize_whitespace(&bytes.as_utf8_str());
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
                        .map(|child| child.inner_text(dom.parser()).into_owned())
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
                // Paragraph-ish containers, including `<details>`/`<summary>`
                // which render expanded (a collapsible needs interactive state
                // the markdown surface does not own).
                "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "aside"
                | "nav" | "figure" | "figcaption" | "details" | "summary" => {
                    let runs = inline_runs_of(dom, tag);
                    if runs.is_empty() {
                        Vec::new()
                    } else {
                        vec![Block::Paragraph { runs }]
                    }
                }
                _ => {
                    let text = normalize_whitespace(&tag.inner_text(dom.parser()));
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

/// Inline runs for a tag's content: images degrade to their alt text, inline
/// math to its source, so the caller always gets plain runs.
fn inline_runs_of(dom: &tl::VDom<'_>, tag: &tl::HTMLTag<'_>) -> Vec<InlineRun> {
    let mut pieces = Vec::new();
    walk_tag_children(dom, tag, &InlineStyle::default(), &mut pieces);
    super::parser::pieces_into_runs(pieces)
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
            let text = bytes.as_utf8_str();
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
                "code" => {
                    let mut nested = style.clone();
                    nested.code = true;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "s" | "del" | "strike" => {
                    let mut nested = style.clone();
                    nested.strikethrough = true;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "a" => {
                    let href = tag
                        .attributes()
                        .get("href")
                        .and_then(|value| value)
                        .map(|value| value.as_utf8_str().into_owned())
                        .filter(|url| is_safe_url(url));
                    let mut nested = style.clone();
                    nested.link = href;
                    walk_tag_children(dom, tag, &nested, pieces);
                }
                "sup" | "sub" => {
                    let text = tag.inner_text(dom.parser());
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !text.is_empty() {
                        pieces.push(InlinePiece::SubSup {
                            text,
                            sup: name == "sup",
                        });
                    }
                }
                "img" => {
                    let src = tag
                        .attributes()
                        .get("src")
                        .and_then(|value| value)
                        .map(|value| value.as_utf8_str().into_owned())
                        .filter(|url| is_safe_url(url));
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
