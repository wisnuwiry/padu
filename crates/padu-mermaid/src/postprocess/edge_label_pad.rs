//! Horizontal padding for edge-label pills.
//!
//! merman sizes an edge label's background `<rect>` to the exact measured text
//! width, so short labels (`One`, `Two`) touch the pill edges. This pass widens
//! those rects symmetrically around the middle-anchored label text.
//!
//! Only fallback label groups carrying the `edgeLabel` class are touched: node
//! bodies, clusters, and every other rect keep the geometry merman measured.

use anyhow::Result;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

/// Padding added to each side of an edge-label pill, in SVG units. Labels
/// render at 16px, so this is just under half an em per side.
const PAD_X: f64 = 7.0;

struct EdgeLabelPad<I> {
    inner: I,
    fallback_depth: usize,
    pad_group: bool,
}

fn is_edge_label_fallback(e: &BytesStart<'_>) -> bool {
    let is_fallback = e
        .try_get_attribute("data-merman-foreignobject")
        .ok()
        .flatten()
        .is_some_and(|attr| attr.value.as_ref() == b"fallback");
    if !is_fallback {
        return false;
    }
    e.try_get_attribute("class")
        .ok()
        .flatten()
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.split_whitespace().any(|token| token == "edgeLabel"))
        })
        .unwrap_or(false)
}

fn attr_value(e: &BytesStart<'_>, name: &[u8]) -> Option<f64> {
    e.try_get_attribute(name)
        .ok()??
        .normalized_value(XmlVersion::Implicit1_0)
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Shortest stable decimal for SVG geometry: two decimals is far below a
/// device pixel after the 2x raster scale, and keeps the output readable.
fn fmt_num(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Widens `e`'s rect by [`PAD_X`] on each side. Returns `None` when the rect
/// carries no numeric `x`/`width` to pad.
fn pad_rect<'a>(e: &BytesStart<'_>) -> Result<Option<BytesStart<'a>>> {
    let (Some(x), Some(width)) = (attr_value(e, b"x"), attr_value(e, b"width")) else {
        return Ok(None);
    };
    let name = e.name();
    let tag = std::str::from_utf8(name.as_ref())?;
    let mut new_elem = BytesStart::new(tag.to_owned());
    for attr in e.attributes() {
        let attr = attr?;
        if attr.key.local_name().as_ref() == b"x" {
            new_elem.push_attribute(("x", fmt_num(x - PAD_X).as_str()));
        } else if attr.key.local_name().as_ref() == b"width" {
            new_elem.push_attribute(("width", fmt_num(width + 2.0 * PAD_X).as_str()));
        } else {
            new_elem.push_attribute(attr);
        }
    }
    Ok(Some(new_elem))
}

fn rewrap<'a>(event: &Event<'_>, elem: BytesStart<'a>) -> Event<'a> {
    match event {
        Event::Start(_) => Event::Start(elem),
        _ => Event::Empty(elem),
    }
}

impl<'a, I: Iterator<Item = Result<Event<'a>>>> Iterator for EdgeLabelPad<I> {
    type Item = Result<Event<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let event = match self.inner.next()? {
            Ok(event) => event,
            Err(e) => return Some(Err(e)),
        };
        match &event {
            Event::Start(e) if e.name().as_ref() == b"g" => {
                if self.fallback_depth > 0 {
                    self.fallback_depth += 1;
                } else if is_edge_label_fallback(e) {
                    self.fallback_depth = 1;
                    self.pad_group = true;
                }
                Some(Ok(event))
            }
            Event::End(e) if e.name().as_ref() == b"g" && self.fallback_depth > 0 => {
                self.fallback_depth -= 1;
                if self.fallback_depth == 0 {
                    self.pad_group = false;
                }
                Some(Ok(event))
            }
            Event::Start(e) | Event::Empty(e) if self.pad_group && e.name().as_ref() == b"rect" => {
                match pad_rect(e) {
                    Ok(Some(padded)) => Some(Ok(rewrap(&event, padded))),
                    Ok(None) => Some(Ok(event)),
                    Err(e) => Some(Err(e)),
                }
            }
            _ => Some(Ok(event)),
        }
    }
}

pub(super) fn process<'a>(
    events: impl Iterator<Item = Result<Event<'a>>>,
) -> impl Iterator<Item = Result<Event<'a>>> {
    EdgeLabelPad {
        inner: events,
        fallback_depth: 0,
        pad_group: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::Reader;

    #[test]
    fn pad_expands_symmetrically_with_short_decimals() {
        let mut elem = BytesStart::new("rect");
        elem.push_attribute(("x", "183.52"));
        elem.push_attribute(("width", "70.56000000000002"));
        elem.push_attribute(("height", "24"));
        let padded = pad_rect(&elem).expect("pad").expect("numeric");
        let attr = |name: &[u8]| {
            padded
                .try_get_attribute(name)
                .expect("attr")
                .expect("present")
                .normalized_value(XmlVersion::Implicit1_0)
                .expect("value")
                .into_owned()
        };
        assert_eq!(attr(b"x"), "176.52");
        assert_eq!(attr(b"width"), "84.56");
        assert_eq!(attr(b"height"), "24");
    }

    #[test]
    fn pad_skips_rects_without_geometry() {
        let elem = BytesStart::new("rect");
        assert!(pad_rect(&elem).expect("pad").is_none());
    }

    /// End to end over a chart with short edge labels: every labeled edge keeps
    /// a pill, the pill stays centered on its middle-anchored text, and short
    /// labels gain room (`One` measured ~23 wide unpadded, `Get money` ~70).
    #[test]
    fn labeled_edges_gain_padded_centered_pills() {
        let source = "flowchart TD\n    A[Christmas] -->|Get money| B(Go shopping)\n    B --> C{Let me think}\n    C -->|One| D[Laptop]\n    C -->|Two| E[iPhone]\n    C -->|Three| F[Car]";
        let svg = crate::render_to_svg(source, &crate::MermaidTheme::default()).expect("render");

        // (rect_x, rect_width, text_x, label) per edge-label pill.
        let mut pills: Vec<(f64, f64, f64, String)> = Vec::new();
        let mut reader = Reader::from_str(&svg);
        reader.config_mut().trim_text(true);
        let mut depth = 0usize;
        let mut rect: Option<(f64, f64)> = None;
        let mut text_x: Option<f64> = None;
        let mut text = String::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) if e.name().as_ref() == b"g" => {
                    if depth > 0 {
                        depth += 1;
                    } else if is_edge_label_fallback(&e) {
                        depth = 1;
                        rect = None;
                        text_x = None;
                        text.clear();
                    }
                }
                Ok(Event::Empty(e)) if depth > 0 && e.name().as_ref() == b"rect" => {
                    if let (Some(x), Some(w)) = (attr_value(&e, b"x"), attr_value(&e, b"width")) {
                        rect = Some((x, w));
                    }
                }
                Ok(Event::Start(e)) if depth > 0 && e.name().as_ref() == b"text" => {
                    text_x = attr_value(&e, b"x");
                }
                Ok(Event::Text(e)) if depth > 0 => {
                    if let Ok(decoded) = e.decode() {
                        text.push_str(&decoded);
                    }
                }
                Ok(Event::End(e)) if e.name().as_ref() == b"g" && depth > 0 => {
                    depth -= 1;
                    if depth == 0
                        && let (Some((x, w)), Some(tx)) = (rect, text_x)
                    {
                        pills.push((x, w, tx, text.clone()));
                    }
                }
                Ok(Event::Eof) => break,
                Err(error) => panic!("svg must parse: {error}"),
                _ => {}
            }
        }
        assert_eq!(pills.len(), 4, "one pill per labeled edge, got {pills:?}");
        for (x, w, tx, label) in &pills {
            assert!(
                (x + w / 2.0 - tx).abs() < 0.05,
                "{label} pill must stay centered on its text"
            );
        }
        let one = pills
            .iter()
            .find(|(_, _, _, label)| label == "One")
            .expect("One pill");
        assert!(
            one.1 > 30.0,
            "One pill must carry horizontal padding, got width {}",
            one.1
        );
        let money = pills
            .iter()
            .find(|(_, _, _, label)| label == "Get money")
            .expect("Get money pill");
        assert!(
            money.1 > 78.0,
            "Get money pill must carry horizontal padding, got width {}",
            money.1
        );
    }
}
