//! Strips `<foreignObject>` elements from the SVG.
//!
//! merman's raster-safe SVG pipeline converts `<foreignObject>` labels into
//! native `<text>` fallback groups (`data-merman-foreignobject="fallback"`) and
//! removes the original `<foreignObject>` elements before Padu-specific
//! post-processing runs. Fallback groups are preserved here; duplicate
//! resolution belongs to the switch-local resvg-safe pipeline, where it has
//! the diagram context needed to make that decision.
//!
//! ```xml
//! <!-- before -->
//! <text class="task">Make tea</text>
//! <g data-merman-foreignobject="fallback"><text>Make tea</text></g>
//!
//! <!-- after -->
//! <text class="task">Make tea</text>
//! ```

use std::collections::VecDeque;

use anyhow::Result;
use quick_xml::events::Event;

struct StripForeignObject<'a, I> {
    inner: I,
    /// Depth inside a `<foreignObject>` element being stripped.
    foreign_depth: usize,
    /// Buffered events of the fallback group currently being inspected, plus
    /// the nesting depth within it.
    buffer: Vec<Event<'a>>,
    fallback_depth: usize,
    /// Events ready to emit (a flushed fallback group).
    output: VecDeque<Event<'a>>,
}

impl<'a, I: Iterator<Item = Result<Event<'a>>>> Iterator for StripForeignObject<'a, I> {
    type Item = Result<Event<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(event) = self.output.pop_front() {
                return Some(Ok(event));
            }

            let event = match self.inner.next()? {
                Ok(event) => event,
                Err(e) => return Some(Err(e)),
            };

            // Strip foreignObject elements and their contents (defensive: merman
            // already removes them, but a stray one cannot be rasterized).
            match &event {
                Event::Start(e) if e.name().as_ref() == b"foreignObject" => {
                    self.foreign_depth += 1;
                    continue;
                }
                Event::Start(_) if self.foreign_depth > 0 => {
                    self.foreign_depth += 1;
                    continue;
                }
                Event::End(_) if self.foreign_depth > 0 => {
                    self.foreign_depth -= 1;
                    continue;
                }
                Event::Empty(e) if e.name().as_ref() == b"foreignObject" => {
                    continue;
                }
                _ if self.foreign_depth > 0 => {
                    continue;
                }
                _ => {}
            }

            if self.fallback_depth > 0 {
                self.buffer_fallback_event(event);
                continue;
            }

            // Start buffering a fallback group so we can decide whether it is a
            // duplicate of a native label once we have seen its text.
            if let Event::Start(e) = &event {
                if e.name().as_ref() == b"g" && is_fallback_group(e) {
                    self.fallback_depth = 1;
                    self.buffer.push(event);
                    continue;
                }
            }

            return Some(Ok(event));
        }
    }
}

impl<'a, I> StripForeignObject<'a, I> {
    fn buffer_fallback_event(&mut self, event: Event<'a>) {
        match &event {
            Event::Start(_) => self.fallback_depth += 1,
            Event::End(_) => self.fallback_depth = self.fallback_depth.saturating_sub(1),
            _ => {}
        }
        self.buffer.push(event);

        if self.fallback_depth == 0 {
            let group = std::mem::take(&mut self.buffer);
            self.output.extend(group);
        }
    }
}

fn is_fallback_group(e: &quick_xml::events::BytesStart<'_>) -> bool {
    e.try_get_attribute("data-merman-foreignobject")
        .ok()
        .flatten()
        .is_some_and(|attr| attr.value.as_ref() == b"fallback")
}

pub(super) fn process<'a>(
    inner: impl Iterator<Item = Result<Event<'a>>>,
) -> impl Iterator<Item = Result<Event<'a>>> {
    StripForeignObject {
        inner,
        foreign_depth: 0,
        buffer: Vec::new(),
        fallback_depth: 0,
        output: VecDeque::new(),
    }
}
