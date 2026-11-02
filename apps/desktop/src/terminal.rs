use std::borrow::Cow;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use crate::ui::icon;
use crate::ui::icon_button;
use crate::ui::menu::{ContextMenuHandle, MenuItem, context_menu};
use crate::ui::tooltip::Tooltip;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{BidirectionalIterator, Dimensions, Scroll};
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point as TerminalPoint, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty::{self, Shell};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use anyhow::{Context as _, Result};
use crossbeam_channel::{Receiver, Sender, unbounded};
use gpui::prelude::*;
use gpui::{
    App, AppContext, Bounds, ClipboardItem, Context, DragMoveEvent, ElementId, FocusHandle,
    Focusable, FontFallbacks, FontStyle, FontWeight, Hsla, InteractiveElement, IntoElement,
    KeyBinding, KeyDownEvent, Keystroke, Modifiers, ModifiersChangedEvent, MouseButton,
    MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    Render, ScrollDelta, ScrollWheelEvent, SharedString, StatefulInteractiveElement,
    StrikethroughStyle, Styled, StyledText, Subscription, Task, TextRun, UnderlineStyle, Window,
    actions, canvas, div, font, px, relative, rgb,
};
use parking_lot::Mutex;

use bezel::ui::docking::{Zone, preview_bounds, zone};
use bezel::ui::widgets::{Layout as BezelLayout, SplitStyle, axis_fraction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraggedTerminalPane(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSplitDrag {
    pub split_id: u64,
}

#[derive(Clone, Copy, Debug)]
struct ActiveSplitDrag {
    split_id: u64,
    axis: gpui::Axis,
    bounds: Bounds<Pixels>,
}

use crate::persistence::DEFAULT_RIGHT_PANEL_WIDTH;
use crate::theme::{Theme, sp};
use crate::ui::scrollbar::{self, ScrollbarState};

actions!(
    padu_terminal,
    [
        SplitTerminalHorizontal,
        SplitTerminalVertical,
        ToggleTerminalExpand,
        CloseTerminalPane,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            "secondary-shift-h",
            SplitTerminalHorizontal,
            Some("Terminal"),
        ),
        KeyBinding::new("secondary-shift-v", SplitTerminalVertical, Some("Terminal")),
        KeyBinding::new(
            "secondary-shift-enter",
            ToggleTerminalExpand,
            Some("Terminal"),
        ),
        KeyBinding::new("secondary-shift-w", CloseTerminalPane, Some("Terminal")),
    ]);
}

/// Fallback advance width, used only until the font has been measured.
const TERMINAL_CELL_WIDTH: f32 = 7.8;
const TERMINAL_CELL_HEIGHT: f32 = 18.0;
const TERMINAL_FONT_SIZE: f32 = 12.5;

#[inline]
fn primary_modifier_pressed(modifiers: &Modifiers) -> bool {
    modifiers.secondary()
}

#[inline]
fn terminal_clipboard_modifier_pressed(modifiers: &Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.secondary() && !modifiers.control && !modifiers.alt
    } else {
        // Preserve Ctrl+C for SIGINT and follow Linux terminal convention.
        modifiers.control && modifiers.shift && !modifiers.alt && !modifiers.platform
    }
}
const TERMINAL_PADDING_X: f32 = 10.0;
const TERMINAL_PADDING_Y: f32 = 8.0;
const TERMINAL_MIN_COLUMNS: usize = 20;
const TERMINAL_MIN_ROWS: usize = 8;
const TERMINAL_SCROLLBACK_LINES: usize = 10_000;
const TERMINAL_CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);
const TERMINAL_CURSOR_BLINK_PAUSE: Duration = Duration::from_millis(300);

/// Alacritty's default URL hint plus local file paths containing a slash.
/// Requiring a slash keeps ordinary dotted words from becoming links.
#[rustfmt::skip]
const TERMINAL_LINK_REGEX: &str = "((ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file:|git://|ssh:|ftp://)|\
                                    (/|~/|\\./|\\.\\./|[A-Za-z0-9._@%+~-]+/))\
                                   [^\u{0000}-\u{001F}\u{007F}-\u{009F}<>\"\\s{-}\\^⟨⟩`\\\\]+";
const MAX_TERMINAL_LINK_SEARCH_LINES: i32 = 100;

/// Icon glyphs (nerd-font private-use codepoints) resolve through CoreText's
/// cascade rather than run splitting: JetBrains Mono itself keeps the
/// Powerline range it covers, everything else falls through to the bundled
/// symbols face registered in [`crate::assets::register_fonts`].
static TERMINAL_FONT_FALLBACKS: LazyLock<FontFallbacks> = LazyLock::new(|| {
    FontFallbacks::from_fonts(vec![crate::assets::SYMBOLS_FONT_FAMILY.to_owned()])
});

fn terminal_font() -> gpui::Font {
    let mut terminal_font = font("JetBrains Mono");
    terminal_font.fallbacks = Some(TERMINAL_FONT_FALLBACKS.clone());
    terminal_font
}

enum TerminalUiEvent {
    Title(String),
    ResetTitle,
    ClipboardStore(String),
    ClipboardLoad(Arc<dyn Fn(&str) -> String + Send + Sync>),
    Exited,
}

#[derive(Clone)]
struct TerminalEventProxy {
    dirty: Arc<AtomicBool>,
    sender: Arc<OnceLock<EventLoopSender>>,
    ui_events: Sender<TerminalUiEvent>,
    window_size: Arc<Mutex<WindowSize>>,
    /// Palette OSC replies run on the PTY thread; `snapshot` refreshes this
    /// so they track the active theme.
    dark_theme: Arc<AtomicBool>,
}

impl TerminalEventProxy {
    fn write_pty(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        if let Some(sender) = self.sender.get() {
            let _ = sender.send(Msg::Input(bytes.into()));
        }
    }
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup | Event::MouseCursorDirty | Event::CursorBlinkingChange => {
                self.dirty.store(true, Ordering::Release);
            }
            Event::Title(title) => {
                let _ = self.ui_events.send(TerminalUiEvent::Title(title));
            }
            Event::ResetTitle => {
                let _ = self.ui_events.send(TerminalUiEvent::ResetTitle);
            }
            Event::ClipboardStore(_, text) => {
                let _ = self.ui_events.send(TerminalUiEvent::ClipboardStore(text));
            }
            Event::ClipboardLoad(_, formatter) => {
                let _ = self
                    .ui_events
                    .send(TerminalUiEvent::ClipboardLoad(formatter));
            }
            Event::PtyWrite(text) => self.write_pty(text.into_bytes()),
            Event::ColorRequest(index, formatter) => {
                let is_dark = self.dark_theme.load(Ordering::Acquire);
                self.write_pty(formatter(terminal_rgb(index, is_dark)).into_bytes());
            }
            Event::TextAreaSizeRequest(formatter) => {
                self.write_pty(formatter(*self.window_size.lock()).into_bytes());
            }
            Event::Bell => {}
            Event::Exit | Event::ChildExit(_) => {
                let _ = self.ui_events.send(TerminalUiEvent::Exited);
                self.dirty.store(true, Ordering::Release);
            }
        }
    }
}

struct TerminalDimensions {
    columns: usize,
    rows: usize,
}

impl Dimensions for TerminalDimensions {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

struct TerminalSession {
    term: Arc<FairMutex<Term<TerminalEventProxy>>>,
    sender: EventLoopSender,
    dirty: Arc<AtomicBool>,
    ui_events: Receiver<TerminalUiEvent>,
    window_size: Arc<Mutex<WindowSize>>,
    grid_size: (usize, usize),
    dark_theme: Arc<AtomicBool>,
    url_regex: RegexSearch,
}

impl TerminalSession {
    fn new(working_directory: &Path, columns: usize, rows: usize) -> Result<Self> {
        let columns = columns.max(TERMINAL_MIN_COLUMNS);
        let rows = rows.max(TERMINAL_MIN_ROWS);
        let window_size = WindowSize {
            num_lines: rows.min(u16::MAX as usize) as u16,
            num_cols: columns.min(u16::MAX as usize) as u16,
            cell_width: TERMINAL_CELL_WIDTH.round() as u16,
            cell_height: TERMINAL_CELL_HEIGHT.round() as u16,
        };
        let shared_window_size = Arc::new(Mutex::new(window_size));
        let dirty = Arc::new(AtomicBool::new(true));
        let sender_slot = Arc::new(OnceLock::new());
        let dark_theme = Arc::new(AtomicBool::new(true));
        let (ui_event_tx, ui_events) = unbounded();
        let url_regex = RegexSearch::new(TERMINAL_LINK_REGEX)
            .map_err(|error| anyhow::anyhow!("compile terminal link regex: {error}"))?;
        let proxy = TerminalEventProxy {
            dirty: dirty.clone(),
            sender: sender_slot.clone(),
            ui_events: ui_event_tx,
            window_size: shared_window_size.clone(),
            dark_theme: dark_theme.clone(),
        };

        let config = Config {
            scrolling_history: TERMINAL_SCROLLBACK_LINES,
            ..Default::default()
        };
        let dimensions = TerminalDimensions { columns, rows };
        let term = Arc::new(FairMutex::new(Term::new(
            config,
            &dimensions,
            proxy.clone(),
        )));

        let shell = crate::command_env::default_terminal_shell();
        let shell_args = crate::command_env::default_terminal_shell_args(&shell);
        let mut options = tty::Options {
            shell: Some(Shell::new(shell.to_string_lossy().into_owned(), shell_args)),
            working_directory: Some(working_directory.to_path_buf()),
            drain_on_exit: false,
            ..Default::default()
        };
        options.env.insert("TERM".into(), "xterm-256color".into());
        options.env.insert("COLORTERM".into(), "truecolor".into());
        if let Some(path) = crate::command_env::executable_search_path() {
            options
                .env
                .insert("PATH".into(), path.to_string_lossy().into_owned());
        }

        let pty = tty::new(&options, window_size, 0)
            .with_context(|| format!("spawn terminal in {}", working_directory.display()))?;
        let event_loop = EventLoop::new(term.clone(), proxy, pty, false, false)
            .context("create Alacritty PTY event loop")?;
        let sender = event_loop.channel();
        sender_slot
            .set(sender.clone())
            .map_err(|_| anyhow::anyhow!("initialize Alacritty PTY sender"))?;
        event_loop.spawn();

        Ok(Self {
            term,
            sender,
            dirty,
            ui_events,
            window_size: shared_window_size,
            grid_size: (columns, rows),
            dark_theme,
            url_regex,
        })
    }

    fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let bytes = bytes.into();
        if !bytes.is_empty() {
            let _ = self.sender.send(Msg::Input(bytes));
        }
    }

    fn resize(&mut self, columns: usize, rows: usize, cell_width: f32) {
        let columns = columns.max(TERMINAL_MIN_COLUMNS);
        let rows = rows.max(TERMINAL_MIN_ROWS);
        if self.grid_size == (columns, rows) {
            return;
        }

        self.grid_size = (columns, rows);
        let dimensions = TerminalDimensions { columns, rows };
        self.term.lock().resize(dimensions);
        let size = WindowSize {
            num_lines: rows.min(u16::MAX as usize) as u16,
            num_cols: columns.min(u16::MAX as usize) as u16,
            cell_width: cell_width.round() as u16,
            cell_height: TERMINAL_CELL_HEIGHT.round() as u16,
        };
        *self.window_size.lock() = size;
        let _ = self.sender.send(Msg::Resize(size));
        self.dirty.store(true, Ordering::Release);
    }

    fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    fn scroll(&self, lines: i32) {
        if lines == 0 {
            return;
        }
        self.term.lock().scroll_display(Scroll::Delta(lines));
        self.dirty.store(true, Ordering::Release);
    }

    fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    fn link_at(&mut self, point: TerminalPoint) -> Option<TerminalLink> {
        let TerminalSession {
            term, url_regex, ..
        } = self;
        let term = term.lock();
        let (value, bounds) =
            hyperlink_at(&term, point).or_else(|| plain_link_at(&term, url_regex, point))?;
        Some(TerminalLink { value, bounds })
    }

    fn snapshot(
        &self,
        theme: Theme,
        selection_color: Hsla,
        cursor_style: TerminalCursorStyle,
        hovered_link: Option<&Match>,
    ) -> TerminalSnapshot {
        self.dark_theme.store(theme.is_dark, Ordering::Release);
        let term = self.term.lock();
        let content = term.renderable_content();
        let columns = self.grid_size.0;
        let rows = self.grid_size.1;
        let selection = content.selection;
        let cursor_row = content.cursor.point.line.0 + content.display_offset as i32;
        let cursor_column = content.cursor.point.column.0;
        let cursor_style = if matches!(
            content.cursor.shape,
            alacritty_terminal::vte::ansi::CursorShape::Hidden
        ) {
            TerminalCursorStyle::Hidden
        } else {
            cursor_style
        };
        let outline_cursor = (cursor_style == TerminalCursorStyle::Outline
            && (0..rows as i32).contains(&cursor_row)
            && cursor_column < columns)
            .then_some((cursor_row as usize, cursor_column));
        let mut cells = vec![TerminalCell::blank(theme); columns * rows];

        for indexed in content.display_iter {
            let row = indexed.point.line.0 + content.display_offset as i32;
            let column = indexed.point.column.0;
            if row < 0 || row as usize >= rows || column >= columns {
                continue;
            }
            let cell = indexed.cell;
            let mut text = if cell.flags.contains(Flags::WIDE_CHAR_SPACER)
                || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
                || cell.flags.contains(Flags::HIDDEN)
            {
                " ".to_owned()
            } else {
                cell.c.to_string()
            };
            if let Some(zerowidth) = cell.zerowidth() {
                text.extend(zerowidth);
            }

            let mut foreground = resolve_color(cell.fg, content.colors, theme, true);
            let mut background = resolve_color(cell.bg, content.colors, theme, false);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut foreground, &mut background);
            }
            // `Flags::DIM_BOLD` and `Flags::BOLD_ITALIC` are unions of the
            // individual bits, so testing them with `intersects` would also
            // match plain bold or italic cells.
            if cell.flags.contains(Flags::DIM) {
                if theme.is_dark {
                    foreground.l *= 0.7;
                } else {
                    foreground.l = 1.0 - (1.0 - foreground.l) * 0.7;
                }
            }
            if selection.is_some_and(|selection| selection.contains(indexed.point)) {
                background = selection_color;
                foreground = theme.text;
            }
            if cursor_style == TerminalCursorStyle::Solid
                && row == cursor_row
                && column == cursor_column
            {
                background = theme.text;
                foreground = theme.surface;
            }

            cells[row as usize * columns + column] = TerminalCell {
                text,
                foreground,
                background,
                bold: cell.flags.contains(Flags::BOLD),
                italic: cell.flags.contains(Flags::ITALIC),
                underline: cell.flags.intersects(Flags::ALL_UNDERLINES)
                    || hovered_link.is_some_and(|bounds| bounds.contains(&indexed.point)),
                strikeout: cell.flags.contains(Flags::STRIKEOUT),
            };
        }

        let mut rendered_rows = Vec::with_capacity(rows);
        for row in cells.chunks(columns) {
            let mut text = String::new();
            let mut runs: Vec<TerminalRun> = Vec::new();
            for cell in row {
                let len = cell.text.len();
                text.push_str(&cell.text);
                let style = TerminalRunStyle {
                    foreground: cell.foreground,
                    background: cell.background,
                    bold: cell.bold,
                    italic: cell.italic,
                    underline: cell.underline,
                    strikeout: cell.strikeout,
                };
                if let Some(run) = runs.last_mut().filter(|run| run.style == style) {
                    run.len += len;
                } else {
                    runs.push(TerminalRun { len, style });
                }
            }
            rendered_rows.push(TerminalRow { text, runs });
        }

        TerminalSnapshot {
            rows: rendered_rows,
            outline_cursor,
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}

/// Adapts the alacritty grid to the overlay scrollbar: the scrollback history
/// is the content above the viewport, and `display_offset` is how far back up
/// into it the view currently sits (0 = pinned to the live bottom).
#[derive(Clone)]
struct TerminalScrollbarTarget {
    term: Arc<FairMutex<Term<TerminalEventProxy>>>,
    dirty: Arc<AtomicBool>,
    viewport_rows: usize,
}

impl scrollbar::Scrollable for TerminalScrollbarTarget {
    fn viewport_height(&self) -> Pixels {
        px(self.viewport_rows as f32 * TERMINAL_CELL_HEIGHT)
    }

    fn max_offset(&self) -> Pixels {
        px(self.term.lock().grid().history_size() as f32 * TERMINAL_CELL_HEIGHT)
    }

    fn scrolled(&self) -> Pixels {
        let term = self.term.lock();
        let grid = term.grid();
        let lines_above = grid.history_size().saturating_sub(grid.display_offset());
        px(lines_above as f32 * TERMINAL_CELL_HEIGHT)
    }

    fn scroll_to(&self, offset: Pixels) {
        let mut term = self.term.lock();
        let target_offset = (term.grid().history_size() as f32
            - f32::from(offset) / TERMINAL_CELL_HEIGHT)
            .round()
            .max(0.0) as usize;
        let delta = target_offset as i32 - term.grid().display_offset() as i32;
        if delta != 0 {
            term.scroll_display(Scroll::Delta(delta));
            self.dirty.store(true, Ordering::Release);
        }
    }
}

#[derive(Clone)]
struct TerminalCell {
    text: String,
    foreground: Hsla,
    background: Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
}

impl TerminalCell {
    fn blank(theme: Theme) -> Self {
        Self {
            text: " ".into(),
            foreground: theme.text,
            background: theme.surface,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct TerminalRunStyle {
    foreground: Hsla,
    background: Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
}

struct TerminalRun {
    len: usize,
    style: TerminalRunStyle,
}

struct TerminalRow {
    text: String,
    runs: Vec<TerminalRun>,
}

struct TerminalSnapshot {
    rows: Vec<TerminalRow>,
    outline_cursor: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TerminalLink {
    value: String,
    bounds: Match,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TerminalLinkTarget {
    Url(String),
    File(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TerminalCursorStyle {
    Solid,
    Outline,
    Hidden,
}

struct TerminalCursorBlink {
    visible: bool,
    enabled: bool,
    epoch: usize,
    _task: Task<()>,
}

impl TerminalCursorBlink {
    fn new() -> Self {
        Self {
            visible: true,
            enabled: false,
            epoch: 0,
            _task: Task::ready(()),
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.enabled {
            return;
        }

        self.enabled = true;
        self.visible = true;
        let epoch = self.next_epoch();
        self.schedule_blink(epoch, TERMINAL_CURSOR_BLINK_INTERVAL, cx);
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if !self.enabled && self.visible {
            return;
        }

        self.enabled = false;
        self.visible = true;
        self.next_epoch();
        cx.notify();
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }

        self.visible = true;
        let epoch = self.next_epoch();
        self.schedule_blink(epoch, TERMINAL_CURSOR_BLINK_PAUSE, cx);
        cx.notify();
    }

    fn blink(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if !self.enabled || epoch != self.epoch {
            return;
        }

        self.visible = !self.visible;
        let epoch = self.next_epoch();
        self.schedule_blink(epoch, TERMINAL_CURSOR_BLINK_INTERVAL, cx);
        cx.notify();
    }

    fn schedule_blink(&mut self, epoch: usize, delay: Duration, cx: &mut Context<Self>) {
        self._task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| this.blink(epoch, cx));
            }
        });
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch
    }

    fn visible(&self) -> bool {
        self.visible
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug)]
pub enum PaneTree {
    Leaf(u64),
    Split {
        id: u64,
        direction: SplitDirection,
        fraction: f32,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}

impl PaneTree {
    pub fn pane_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.collect_ids(&mut ids);
        ids
    }

    fn collect_ids(&self, ids: &mut Vec<u64>) {
        match self {
            PaneTree::Leaf(id) => ids.push(*id),
            PaneTree::Split { first, second, .. } => {
                first.collect_ids(ids);
                second.collect_ids(ids);
            }
        }
    }

    pub fn split_pane(
        &mut self,
        target: u64,
        new_id: u64,
        split_id: u64,
        direction: SplitDirection,
    ) -> bool {
        match self {
            PaneTree::Leaf(id) if *id == target => {
                *self = PaneTree::Split {
                    id: split_id,
                    direction,
                    fraction: 0.5,
                    first: Box::new(PaneTree::Leaf(target)),
                    second: Box::new(PaneTree::Leaf(new_id)),
                };
                true
            }
            PaneTree::Leaf(_) => false,
            PaneTree::Split { first, second, .. } => {
                if first.split_pane(target, new_id, split_id, direction) {
                    true
                } else {
                    second.split_pane(target, new_id, split_id, direction)
                }
            }
        }
    }

    pub fn update_fraction(&mut self, target_split_id: u64, fraction: f32) -> bool {
        match self {
            PaneTree::Leaf(_) => false,
            PaneTree::Split {
                id,
                fraction: current_fraction,
                first,
                second,
                ..
            } => {
                if *id == target_split_id {
                    *current_fraction = fraction.clamp(0.05, 0.95);
                    true
                } else if first.update_fraction(target_split_id, fraction) {
                    true
                } else {
                    second.update_fraction(target_split_id, fraction)
                }
            }
        }
    }

    pub fn dock_pane(&mut self, target: u64, incoming: u64, split_id: u64, zone: Zone) -> bool {
        if target == incoming {
            return false;
        }

        // Remove incoming from the tree if already placed
        self.remove_pane(incoming);

        // Insert incoming adjacent to target according to zone
        self.insert_dock(target, incoming, split_id, zone)
    }

    fn insert_dock(&mut self, target: u64, incoming: u64, split_id: u64, zone: Zone) -> bool {
        match self {
            PaneTree::Leaf(id) if *id == target => {
                let (first, second, direction) = match zone {
                    Zone::Left => (
                        Box::new(PaneTree::Leaf(incoming)),
                        Box::new(PaneTree::Leaf(target)),
                        SplitDirection::Horizontal,
                    ),
                    Zone::Right | Zone::Join => (
                        Box::new(PaneTree::Leaf(target)),
                        Box::new(PaneTree::Leaf(incoming)),
                        SplitDirection::Horizontal,
                    ),
                    Zone::Top => (
                        Box::new(PaneTree::Leaf(incoming)),
                        Box::new(PaneTree::Leaf(target)),
                        SplitDirection::Vertical,
                    ),
                    Zone::Bottom => (
                        Box::new(PaneTree::Leaf(target)),
                        Box::new(PaneTree::Leaf(incoming)),
                        SplitDirection::Vertical,
                    ),
                };

                *self = PaneTree::Split {
                    id: split_id,
                    direction,
                    fraction: 0.5,
                    first,
                    second,
                };
                true
            }
            PaneTree::Leaf(_) => false,
            PaneTree::Split { first, second, .. } => {
                if first.insert_dock(target, incoming, split_id, zone) {
                    true
                } else {
                    second.insert_dock(target, incoming, split_id, zone)
                }
            }
        }
    }

    pub fn remove_pane(&mut self, target: u64) -> bool {
        match self {
            PaneTree::Leaf(_) => false,
            PaneTree::Split { first, second, .. } => {
                if let PaneTree::Leaf(id) = **first {
                    if id == target {
                        *self = (**second).clone();
                        return true;
                    }
                }
                if let PaneTree::Leaf(id) = **second {
                    if id == target {
                        *self = (**first).clone();
                        return true;
                    }
                }
                if first.remove_pane(target) {
                    true
                } else {
                    second.remove_pane(target)
                }
            }
        }
    }
}

pub struct TerminalPane {
    #[allow(dead_code)]
    id: u64,
    session: Option<TerminalSession>,
    error: Option<String>,
    focus_handle: FocusHandle,
    working_directory: PathBuf,
    title: String,
    exited: bool,
    scroll_accumulator: f32,
    measured_cell_width: Option<f32>,
    scrollbar_state: Rc<ScrollbarState>,
    grid_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    selecting: bool,
    hovered_link: Option<TerminalLink>,
    cursor_blink: gpui::Entity<TerminalCursorBlink>,
    cursor_focus_tracking_started: bool,
    context_menu: ContextMenuHandle,
    _subscriptions: Vec<Subscription>,
}

impl TerminalPane {
    pub fn new(id: u64, working_directory: PathBuf, cx: &mut Context<TerminalView>) -> Self {
        let terminal_cwd = working_directory.clone();
        cx.spawn(async move |this, cx| {
            let started = cx
                .background_executor()
                .spawn(async move { TerminalSession::new(&terminal_cwd, 52, 36) })
                .await;
            if this
                .update(cx, |this, cx| {
                    if let Some(pane) = this.panes.get_mut(&id) {
                        match started {
                            Ok(session) => pane.session = Some(session),
                            Err(error) => pane.error = Some(error.to_string()),
                        }
                        cx.notify();
                    }
                })
                .is_err()
            {
                return;
            }
        })
        .detach();

        let context_menu = ContextMenuHandle::new(cx);
        let cursor_blink = cx.new(|_| TerminalCursorBlink::new());
        let subscriptions = vec![cx.observe(&cursor_blink, |_, _, cx| cx.notify())];

        Self {
            id,
            session: None,
            error: None,
            focus_handle: cx.focus_handle(),
            working_directory,
            title: tr!("right_panel.terminal"),
            exited: false,
            scroll_accumulator: 0.0,
            measured_cell_width: None,
            scrollbar_state: ScrollbarState::new(),
            grid_bounds: Rc::new(Cell::new(None)),
            selecting: false,
            hovered_link: None,
            cursor_blink,
            cursor_focus_tracking_started: false,
            context_menu,
            _subscriptions: subscriptions,
        }
    }

    fn poll(&mut self, cx: &mut App) -> bool {
        let Some(session) = &self.session else {
            return false;
        };
        let mut changed = session.take_dirty();
        while let Ok(event) = session.ui_events.try_recv() {
            changed = true;
            match event {
                TerminalUiEvent::Title(title) => self.title = title,
                TerminalUiEvent::ResetTitle => self.title = tr!("right_panel.terminal"),
                TerminalUiEvent::ClipboardStore(text) => {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                }
                TerminalUiEvent::ClipboardLoad(formatter) => {
                    let text = cx
                        .read_from_clipboard()
                        .and_then(|item| item.text())
                        .unwrap_or_default();
                    session.write(formatter(&text).into_bytes());
                }
                TerminalUiEvent::Exited => self.exited = true,
            }
        }
        changed
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut App) {
        self.pause_cursor_blink(cx);
        let keystroke = &event.keystroke;
        if terminal_clipboard_modifier_pressed(&keystroke.modifiers)
            && keystroke.key.eq_ignore_ascii_case("c")
        {
            self.copy_selection(cx);
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if terminal_clipboard_modifier_pressed(&keystroke.modifiers)
            && keystroke.key.eq_ignore_ascii_case("a")
        {
            self.select_all(cx);
            window.prevent_default();
            cx.stop_propagation();
            return;
        }

        let Some(session) = &self.session else {
            return;
        };
        if terminal_clipboard_modifier_pressed(&keystroke.modifiers)
            && keystroke.key.eq_ignore_ascii_case("v")
        {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                session.term.lock().selection = None;
                let bytes = bracketed_paste(text, session.mode());
                session.write(bytes);
                session.dirty.store(true, Ordering::Release);
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }

        if let Some(bytes) = terminal_key_bytes(keystroke, session.mode()) {
            session.term.lock().selection = None;
            session.write(bytes);
            session.dirty.store(true, Ordering::Release);
            window.prevent_default();
            cx.stop_propagation();
        }
    }

    fn cell_width(&self) -> f32 {
        self.measured_cell_width.unwrap_or(TERMINAL_CELL_WIDTH)
    }

    fn grid_point_for_position(
        &self,
        position: Point<Pixels>,
        clamp_to_grid: bool,
    ) -> Option<(TerminalPoint, Side)> {
        let bounds = self.grid_bounds.get()?;
        let session = self.session.as_ref()?;
        let display_offset = session.term.lock().grid().display_offset();
        terminal_grid_point(
            bounds,
            position,
            self.cell_width(),
            session.grid_size.0,
            session.grid_size.1,
            display_offset,
            clamp_to_grid,
        )
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut App) {
        if self.scrollbar_state.engaged() {
            return;
        }
        window.focus(&self.focus_handle, cx);
        let Some((point, side)) = self.grid_point_for_position(event.position, false) else {
            return;
        };

        if primary_modifier_pressed(&event.modifiers)
            && let Some(link) = self
                .session
                .as_mut()
                .and_then(|session| session.link_at(point))
            && let Some(target) =
                terminal_link_target(&link.value, self.working_directory.as_path())
        {
            self.selecting = false;
            self.hovered_link = Some(link);
            match target {
                TerminalLinkTarget::Url(url) => cx.open_url(&url),
                TerminalLinkTarget::File(path) => {
                    crate::platform::reveal_in_file_manager(&path, cx)
                }
            }
            window.prevent_default();
            cx.stop_propagation();
            return;
        }

        let Some(session) = &self.session else {
            return;
        };

        let mut term = session.term.lock();
        if event.modifiers.shift
            && let Some(selection) = term.selection.as_mut()
        {
            selection.update(point, side);
        } else {
            let selection_type = match event.click_count {
                2 => SelectionType::Semantic,
                count if count >= 3 => SelectionType::Lines,
                _ => SelectionType::Simple,
            };
            term.selection = Some(Selection::new(selection_type, point, side));
        }
        drop(term);

        self.selecting = true;
        session.dirty.store(true, Ordering::Release);
        cx.stop_propagation();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut App) {
        let _hover_changed = if self.selecting {
            self.set_hovered_link(None)
        } else {
            self.refresh_hovered_link(primary_modifier_pressed(&event.modifiers), event.position)
        };
        if !self.selecting || event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let Some((point, side)) = self.grid_point_for_position(event.position, true) else {
            return;
        };
        let Some(session) = &self.session else {
            return;
        };

        if let Some(selection) = session.term.lock().selection.as_mut() {
            selection.update(point, side);
            session.dirty.store(true, Ordering::Release);
            cx.stop_propagation();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut App) {
        self.selecting = false;
    }

    fn on_mouse_exit(&mut self, _: &MouseExitEvent, _: &mut Window, _: &mut App) {
        self.set_hovered_link(None);
    }

    fn on_modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        _: &mut App,
    ) {
        self.refresh_hovered_link(
            primary_modifier_pressed(&event.modifiers),
            window.mouse_position(),
        );
    }

    fn refresh_hovered_link(&mut self, command_pressed: bool, position: Point<Pixels>) -> bool {
        let link = if command_pressed {
            self.grid_point_for_position(position, false)
                .and_then(|(point, _)| self.session.as_mut()?.link_at(point))
        } else {
            None
        };
        self.set_hovered_link(link)
    }

    fn set_hovered_link(&mut self, link: Option<TerminalLink>) -> bool {
        if self.hovered_link == link {
            return false;
        }
        self.hovered_link = link;
        true
    }

    fn selected_text(&self) -> Option<String> {
        self.session
            .as_ref()?
            .term
            .lock()
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }

    fn copy_selection(&self, cx: &mut App) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, text: String, cx: &mut App) {
        self.pause_cursor_blink(cx);
        let Some(session) = &self.session else {
            return;
        };
        session.term.lock().selection = None;
        session.write(bracketed_paste(text, session.mode()));
        session.dirty.store(true, Ordering::Release);
    }

    fn select_all(&mut self, _: &mut App) {
        let Some(session) = &self.session else {
            return;
        };
        let mut term = session.term.lock();
        let start = TerminalPoint::new(term.topmost_line(), Column(0));
        let end = TerminalPoint::new(term.bottommost_line(), term.last_column());
        let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
        selection.update(end, Side::Right);
        term.selection = Some(selection);
        drop(term);
        session.dirty.store(true, Ordering::Release);
    }

    fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut App) {
        let Some(session) = &self.session else {
            return;
        };
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => f32::from(delta.y) / TERMINAL_CELL_HEIGHT,
            ScrollDelta::Lines(delta) => delta.y,
        };
        self.scroll_accumulator += delta;
        let lines = self.scroll_accumulator.trunc() as i32;
        if lines != 0 {
            self.scroll_accumulator -= lines as f32;
            session.scroll(lines);
            cx.stop_propagation();
        }
    }

    fn ensure_cursor_focus_tracking(
        &mut self,
        pane_id: u64,
        window: &mut Window,
        cx: &mut Context<TerminalView>,
    ) {
        if self.cursor_focus_tracking_started {
            return;
        }
        self.cursor_focus_tracking_started = true;

        let focus_handle = self.focus_handle.clone();
        self._subscriptions.extend([
            cx.observe_window_activation(window, move |terminal, window, cx| {
                if let Some(pane) = terminal.panes.get_mut(&pane_id) {
                    pane.update_cursor_blinking(window, cx);
                }
            }),
            cx.on_focus(&focus_handle, window, move |terminal, window, cx| {
                terminal.active_pane = pane_id;
                if let Some(pane) = terminal.panes.get_mut(&pane_id) {
                    pane.update_cursor_blinking(window, cx);
                }
                cx.notify();
            }),
            cx.on_blur(&focus_handle, window, move |terminal, window, cx| {
                if let Some(pane) = terminal.panes.get_mut(&pane_id) {
                    pane.update_cursor_blinking(window, cx);
                }
                cx.notify();
            }),
        ]);
        self.update_cursor_blinking(window, cx);
    }

    fn update_cursor_blinking(&mut self, window: &Window, cx: &mut App) {
        let focused = window.is_window_active() && self.focus_handle.is_focused(window);
        self.cursor_blink.update(cx, |cursor, cx| {
            if focused {
                cursor.start(cx);
            } else {
                cursor.stop(cx);
            }
        });
    }

    fn pause_cursor_blink(&mut self, cx: &mut App) {
        self.cursor_blink.update(cx, |cursor, cx| cursor.pause(cx));
    }
}

pub struct TerminalView {
    panes: BTreeMap<u64, TerminalPane>,
    layout: PaneTree,
    active_pane: u64,
    expanded_pane: Option<u64>,
    next_pane_id: u64,
    next_split_id: u64,
    active_split: Option<ActiveSplitDrag>,
    dock_target: Option<(u64, Zone, Bounds<Pixels>)>,
    working_directory: PathBuf,
    panel_width: f32,
    focus_handle: FocusHandle,
}

impl TerminalView {
    pub fn new(working_directory: PathBuf, cx: &mut Context<Self>) -> Self {
        let initial_pane = TerminalPane::new(1, working_directory.clone(), cx);
        let focus_handle = initial_pane.focus_handle.clone();
        let mut panes = BTreeMap::new();
        panes.insert(1, initial_pane);

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(24))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.poll(cx) {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        Self {
            panes,
            layout: PaneTree::Leaf(1),
            active_pane: 1,
            expanded_pane: None,
            next_pane_id: 2,
            next_split_id: 1,
            active_split: None,
            dock_target: None,
            working_directory,
            panel_width: DEFAULT_RIGHT_PANEL_WIDTH,
            focus_handle,
        }
    }

    pub fn working_directory(&self) -> &Path {
        self.panes
            .get(&self.active_pane)
            .map(|p| p.working_directory.as_path())
            .unwrap_or(&self.working_directory)
    }

    pub fn is_ready(&self) -> bool {
        self.panes
            .get(&self.active_pane)
            .is_some_and(|p| p.session.is_some())
    }

    pub fn send_command(&mut self, command: &str) {
        if let Some(pane) = self.panes.get_mut(&self.active_pane) {
            if let Some(session) = &pane.session {
                let mut bytes = command.as_bytes().to_vec();
                if !bytes.ends_with(b"\n") {
                    bytes.push(b'\n');
                }
                session.write(bytes);
                session
                    .dirty
                    .store(true, std::sync::atomic::Ordering::Release);
            }
        }
    }

    pub fn set_panel_width(&mut self, width: f32) {
        self.panel_width = width;
    }

    pub fn title(&self) -> &str {
        self.panes
            .get(&self.active_pane)
            .map(|p| p.title.as_str())
            .unwrap_or("Terminal")
    }

    pub fn refresh_localized_text(&mut self, cx: &mut Context<Self>) {
        for pane in self.panes.values_mut() {
            if matches!(pane.title.as_str(), "Terminal" | "终端") {
                pane.title = tr!("right_panel.terminal");
            }
        }
        cx.notify();
    }

    fn poll(&mut self, cx: &mut App) -> bool {
        let mut changed = false;
        for pane in self.panes.values_mut() {
            if pane.poll(cx) {
                changed = true;
            }
        }
        changed
    }

    fn activate_pane(&mut self, pane_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_pane != pane_id {
            self.active_pane = pane_id;
            if let Some(pane) = self.panes.get(&pane_id) {
                window.focus(&pane.focus_handle, cx);
                pane.cursor_blink.update(cx, |cursor, cx| cursor.start(cx));
            }
            cx.notify();
        }
    }

    fn split_horizontal(&mut self, target_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.split_pane(target_id, SplitDirection::Horizontal, window, cx);
    }

    fn split_vertical(&mut self, target_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.split_pane(target_id, SplitDirection::Vertical, window, cx);
    }

    fn update_split_fraction(&mut self, split_id: u64, fraction: f32) {
        self.layout.update_fraction(split_id, fraction);
    }

    fn dock_pane(
        &mut self,
        incoming: u64,
        target: u64,
        zone: Zone,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if incoming == target {
            return;
        }

        let split_id = self.next_split_id;
        self.next_split_id += 1;

        if self.layout.dock_pane(target, incoming, split_id, zone) {
            self.expanded_pane = None;
            self.dock_target = None;
            self.activate_pane(incoming, window, cx);
        }
    }

    fn split_pane(
        &mut self,
        target_id: u64,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let new_id = self.next_pane_id;
        self.next_pane_id += 1;
        let split_id = self.next_split_id;
        self.next_split_id += 1;

        let cwd = self
            .panes
            .get(&target_id)
            .map(|p| p.working_directory.clone())
            .unwrap_or_else(|| self.working_directory.clone());

        let new_pane = TerminalPane::new(new_id, cwd, cx);
        let focus_handle = new_pane.focus_handle.clone();
        self.panes.insert(new_id, new_pane);
        self.layout
            .split_pane(target_id, new_id, split_id, direction);

        self.expanded_pane = None;
        self.active_pane = new_id;

        window.focus(&focus_handle, cx);
        cx.notify();
    }

    fn close_pane(&mut self, target_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.panes.len() <= 1 {
            return;
        }

        if self.expanded_pane == Some(target_id) {
            self.expanded_pane = None;
        }

        self.panes.remove(&target_id);
        self.layout.remove_pane(target_id);

        if self.active_pane == target_id {
            let remaining = self.layout.pane_ids();
            if let Some(&first) = remaining.first() {
                self.active_pane = first;
                if let Some(pane) = self.panes.get(&first) {
                    window.focus(&pane.focus_handle, cx);
                }
            }
        }

        cx.notify();
    }

    fn expand_pane(&mut self, target_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.expanded_pane = Some(target_id);
        self.active_pane = target_id;
        if let Some(pane) = self.panes.get(&target_id) {
            window.focus(&pane.focus_handle, cx);
        }
        cx.notify();
    }

    fn restore_all_panes(&mut self, cx: &mut Context<Self>) {
        self.expanded_pane = None;
        cx.notify();
    }

    fn toggle_expand(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.panes.len() <= 1 {
            return;
        }
        if self.expanded_pane.is_some() {
            self.restore_all_panes(cx);
        } else {
            self.expand_pane(self.active_pane, window, cx);
        }
    }

    #[allow(dead_code)]
    fn switch_expanded_pane(
        &mut self,
        target_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.panes.contains_key(&target_id) {
            self.expanded_pane = Some(target_id);
            self.active_pane = target_id;
            if let Some(pane) = self.panes.get(&target_id) {
                window.focus(&pane.focus_handle, cx);
            }
            cx.notify();
        }
    }

    fn on_pane_key_down(
        &mut self,
        pane_id: u64,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let secondary = keystroke.modifiers.secondary();
        let shift = keystroke.modifiers.shift;

        if secondary && shift {
            if keystroke.key.eq_ignore_ascii_case("h") {
                self.split_horizontal(pane_id, window, cx);
                window.prevent_default();
                cx.stop_propagation();
                return;
            }
            if keystroke.key.eq_ignore_ascii_case("v") {
                self.split_vertical(pane_id, window, cx);
                window.prevent_default();
                cx.stop_propagation();
                return;
            }
            if matches!(keystroke.key.as_str(), "enter" | "return") {
                self.toggle_expand(window, cx);
                window.prevent_default();
                cx.stop_propagation();
                return;
            }
            if keystroke.key.eq_ignore_ascii_case("w") && self.panes.len() > 1 {
                self.close_pane(pane_id, window, cx);
                window.prevent_default();
                cx.stop_propagation();
                return;
            }
        }

        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.on_key_down(event, window, cx);
        }
    }

    fn on_pane_mouse_down(
        &mut self,
        pane_id: u64,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_pane(pane_id, window, cx);
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.on_mouse_down(event, window, cx);
            cx.notify();
        }
    }

    fn on_pane_mouse_move(
        &mut self,
        pane_id: u64,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.on_mouse_move(event, window, cx);
            cx.notify();
        }
    }

    fn on_pane_mouse_up(
        &mut self,
        pane_id: u64,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.on_mouse_up(event, window, cx);
        }
    }

    fn on_pane_mouse_exit(
        &mut self,
        pane_id: u64,
        event: &MouseExitEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.on_mouse_exit(event, window, cx);
            cx.notify();
        }
    }

    fn on_pane_modifiers_changed(
        &mut self,
        pane_id: u64,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.on_modifiers_changed(event, window, cx);
            cx.notify();
        }
    }

    fn on_pane_scroll_wheel(
        &mut self,
        pane_id: u64,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.on_scroll_wheel(event, window, cx);
            cx.notify();
        }
    }

    fn render_pane(
        &mut self,
        pane_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = Theme::current(cx);
        let selection_color = theme.selection;
        let viewport = window.viewport_size();
        let is_active = self.active_pane == pane_id;
        let is_expanded = self.expanded_pane == Some(pane_id);
        let opacity = if is_active || is_expanded { 1.0 } else { 0.70 };
        let pane_count = self.panes.len();
        let is_single = pane_count <= 1;

        let pane = self.panes.get_mut(&pane_id).expect("pane exists");
        pane.ensure_cursor_focus_tracking(pane_id, window, cx);

        let cell_width = *pane.measured_cell_width.get_or_insert_with(|| {
            let text_system = cx.text_system();
            let font_id = text_system.resolve_font(&terminal_font());
            text_system
                .advance(font_id, px(TERMINAL_FONT_SIZE), 'm')
                .map_or(TERMINAL_CELL_WIDTH, |advance| f32::from(advance.width))
        });

        let (columns, rows) = terminal_grid_dimensions(
            pane.grid_bounds.get(),
            self.panel_width,
            viewport.height,
            cell_width,
        );

        let terminal_focused = window.is_window_active() && pane.focus_handle.is_focused(window);
        let cursor_style =
            terminal_cursor_style(terminal_focused, pane.cursor_blink.read(cx).visible());
        if let Some(session) = pane.session.as_mut() {
            session.resize(columns, rows, cell_width);
        }
        if pane.selecting {
            pane.set_hovered_link(None);
        } else {
            pane.refresh_hovered_link(
                primary_modifier_pressed(&window.modifiers()),
                window.mouse_position(),
            );
        }
        let hovered_link = pane.hovered_link.as_ref().map(|link| &link.bounds);
        let snapshot = pane
            .session
            .as_ref()
            .map(|session| session.snapshot(theme, selection_color, cursor_style, hovered_link));

        let mut screen = div()
            .size_full()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .relative()
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    this.on_pane_mouse_down(pane_id, event, window, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    this.on_pane_mouse_up(pane_id, event, window, cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    this.on_pane_mouse_up(pane_id, event, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, event, window, cx| {
                this.on_pane_mouse_move(pane_id, event, window, cx);
            }))
            .on_mouse_exit(cx.listener(move |this, event, window, cx| {
                this.on_pane_mouse_exit(pane_id, event, window, cx);
            }))
            .on_modifiers_changed(cx.listener(move |this, event, window, cx| {
                this.on_pane_modifiers_changed(pane_id, event, window, cx);
            }));

        if pane.hovered_link.is_some() {
            screen = screen.cursor_pointer();
        }

        if pane.exited {
            screen = screen.child(
                div()
                    .absolute()
                    .bottom(px(8.0))
                    .right(px(14.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(theme.raised)
                    .border_1()
                    .border_color(theme.border)
                    .text_size(sp(11.0))
                    .text_color(theme.danger)
                    .child(tr!("terminal.process_exited")),
            );
        }

        if let Some(snapshot) = snapshot {
            let TerminalSnapshot {
                rows: snapshot_rows,
                outline_cursor,
            } = snapshot;
            for row in snapshot_rows {
                let runs = row
                    .runs
                    .into_iter()
                    .map(|run| {
                        let mut run_font = terminal_font();
                        if run.style.bold {
                            run_font.weight = FontWeight::BOLD;
                        }
                        if run.style.italic {
                            run_font.style = FontStyle::Italic;
                        }
                        TextRun {
                            len: run.len,
                            font: run_font,
                            color: run.style.foreground,
                            background_color: Some(run.style.background),
                            underline: run.style.underline.then_some(UnderlineStyle {
                                thickness: px(1.0),
                                color: Some(run.style.foreground),
                                wavy: false,
                            }),
                            strikethrough: run.style.strikeout.then_some(StrikethroughStyle {
                                thickness: px(1.0),
                                color: Some(run.style.foreground),
                            }),
                        }
                    })
                    .collect::<Vec<_>>();
                screen = screen.child(
                    div()
                        .h(px(TERMINAL_CELL_HEIGHT))
                        .flex_none()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_size(px(TERMINAL_FONT_SIZE))
                        .line_height(px(TERMINAL_CELL_HEIGHT))
                        .child(StyledText::new(row.text).with_runs(runs)),
                );
            }
            if let Some((row, column)) = outline_cursor {
                screen = screen.child(
                    div()
                        .absolute()
                        .left(px(column as f32 * cell_width))
                        .top(px(row as f32 * TERMINAL_CELL_HEIGHT))
                        .w(px(cell_width))
                        .h(px(TERMINAL_CELL_HEIGHT))
                        .border_1()
                        .border_color(theme.text),
                );
            }
        } else {
            screen = screen.child(
                div()
                    .p(px(12.0))
                    .text_size(sp(12.5))
                    .line_height(sp(17.0))
                    .text_color(if pane.error.is_some() {
                        theme.danger
                    } else {
                        theme.text_tertiary
                    })
                    .child(
                        pane.error
                            .clone()
                            .unwrap_or_else(|| tr!("terminal.starting")),
                    ),
            );
        }

        let grid_bounds = pane.grid_bounds.clone();
        let terminal_view_id = cx.entity().entity_id();
        screen = screen.child(
            canvas(
                move |bounds, _, cx| {
                    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                        return;
                    }
                    let old_bounds = grid_bounds.get();
                    grid_bounds.set(Some(bounds));
                    let needs_notify = match old_bounds {
                        Some(old) => {
                            (old.size.width - bounds.size.width).abs() > px(1.0)
                                || (old.size.height - bounds.size.height).abs() > px(1.0)
                        }
                        None => true,
                    };
                    if needs_notify {
                        cx.notify(terminal_view_id);
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );

        let context_terminal = cx.entity();
        let context_menu_handle = &pane.context_menu;
        let screen = context_menu(
            screen,
            SharedString::from(format!("terminal-context-menu-{pane_id}")),
            context_menu_handle,
            move |cx| {
                let (has_selection, has_session) = context_terminal
                    .read(cx)
                    .panes
                    .get(&pane_id)
                    .map(|p| (p.selected_text().is_some(), p.session.is_some()))
                    .unwrap_or((false, false));
                let can_paste = cx
                    .read_from_clipboard()
                    .and_then(|item| item.text())
                    .is_some_and(|text| !text.is_empty());

                let copy_terminal = context_terminal.clone();
                let paste_terminal = context_terminal.clone();
                let select_all_terminal = context_terminal.clone();
                vec![
                    MenuItem::new(tr!("menu.copy"), move |_, cx| {
                        let selected_text = copy_terminal
                            .read(cx)
                            .panes
                            .get(&pane_id)
                            .and_then(|p| p.selected_text());
                        if let Some(text) = selected_text {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                        }
                    })
                    .disabled(!has_selection),
                    MenuItem::new(tr!("menu.paste"), move |_, cx| {
                        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
                        else {
                            return;
                        };
                        paste_terminal.update(cx, |terminal, cx| {
                            if let Some(pane) = terminal.panes.get_mut(&pane_id) {
                                pane.paste(text, cx);
                            }
                        });
                    })
                    .disabled(!can_paste),
                    MenuItem::Separator,
                    MenuItem::new(tr!("menu.select_all"), move |_, cx| {
                        select_all_terminal.update(cx, |terminal, cx| {
                            if let Some(pane) = terminal.panes.get_mut(&pane_id) {
                                pane.select_all(cx);
                            }
                        });
                    })
                    .disabled(!has_session),
                ]
            },
        );

        let scrollbar = pane.session.as_ref().map(|session| {
            scrollbar::vertical(
                &TerminalScrollbarTarget {
                    term: session.term.clone(),
                    dirty: session.dirty.clone(),
                    viewport_rows: session.grid_size.1,
                },
                &pane.scrollbar_state,
            )
        });

        let h_shortcut = crate::platform::primary_shortcut("⌘⇧H", "Ctrl+Shift+H");
        let v_shortcut = crate::platform::primary_shortcut("⌘⇧V", "Ctrl+Shift+V");
        let expand_shortcut = crate::platform::primary_shortcut("⌘⇧Enter", "Ctrl+Shift+Enter");
        let close_shortcut = crate::platform::primary_shortcut("⌘⇧W", "Ctrl+Shift+W");

        let mut actions_bar = div().flex().items_center().gap(px(2.0));

        let split_h_shortcut = h_shortcut;
        actions_bar = actions_bar.child(
            icon_button(
                SharedString::from(format!("terminal-split-h-{pane_id}")),
                "icons/split.svg",
                theme,
            )
            .tooltip(move |window, cx| {
                Tooltip::new(tr!(
                    "terminal.split_horizontal",
                    shortcut = split_h_shortcut
                ))
                .build(window, cx)
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.split_horizontal(pane_id, window, cx);
            })),
        );

        let split_v_shortcut = v_shortcut;
        actions_bar = actions_bar.child(
            icon_button(
                SharedString::from(format!("terminal-split-v-{pane_id}")),
                "icons/split-vertical.svg",
                theme,
            )
            .tooltip(move |window, cx| {
                Tooltip::new(tr!("terminal.split_vertical", shortcut = split_v_shortcut))
                    .build(window, cx)
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.split_vertical(pane_id, window, cx);
            })),
        );

        if !is_single && !is_expanded {
            actions_bar = actions_bar.child(
                icon_button(
                    SharedString::from(format!("terminal-drag-{pane_id}")),
                    "icons/grip-vertical.svg",
                    theme,
                )
                .cursor_grab()
                .tooltip(move |window, cx| {
                    Tooltip::new(tr!("terminal.drag_to_dock")).build(window, cx)
                })
                .on_drag(DraggedTerminalPane(pane_id), |_, _, _, cx| {
                    cx.new(|_| gpui::Empty)
                }),
            );
        }

        if !is_single {
            if is_expanded {
                let show_shortcut = expand_shortcut;
                actions_bar = actions_bar.child(
                    icon_button(
                        SharedString::from(format!("terminal-restore-{pane_id}")),
                        "icons/minimize.svg",
                        theme,
                    )
                    .tooltip(move |window, cx| {
                        Tooltip::new(tr!("terminal.show_all_panes", shortcut = show_shortcut))
                            .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.restore_all_panes(cx);
                    })),
                );
            } else {
                let exp_shortcut = expand_shortcut;
                actions_bar = actions_bar.child(
                    icon_button(
                        SharedString::from(format!("terminal-expand-{pane_id}")),
                        "icons/maximize.svg",
                        theme,
                    )
                    .tooltip(move |window, cx| {
                        Tooltip::new(tr!("terminal.expand_full", shortcut = exp_shortcut))
                            .build(window, cx)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.expand_pane(pane_id, window, cx);
                    })),
                );
            }

            let cls_shortcut = close_shortcut;
            actions_bar = actions_bar.child(
                icon_button(
                    SharedString::from(format!("terminal-close-{pane_id}")),
                    "icons/x.svg",
                    theme,
                )
                .tooltip(move |window, cx| {
                    Tooltip::new(tr!("terminal.close_pane", shortcut = cls_shortcut))
                        .build(window, cx)
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.close_pane(pane_id, window, cx);
                })),
            );
        }

        let actions_overlay = div()
            .id(SharedString::from(format!("terminal-actions-{pane_id}")))
            .absolute()
            .top(px(6.0))
            .right(px(12.0))
            .flex()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded(px(bezel::theme::Theme::control_radius()))
            .bg(theme.frosted.opacity(0.85))
            .border_1()
            .border_color(theme.border.opacity(0.5))
            .shadow_sm()
            .when(pane.exited, |el| {
                el.child(
                    div()
                        .px(px(4.0))
                        .py(px(1.0))
                        .mr(px(2.0))
                        .rounded(px(3.0))
                        .bg(theme.danger.opacity(0.15))
                        .text_size(sp(10.0))
                        .text_color(theme.danger)
                        .child(tr!("terminal.process_exited")),
                )
            })
            .child(actions_bar);

        let dock_preview = if let Some((target_id, target_zone, bounds)) = self.dock_target {
            if target_id == pane_id && cx.has_active_drag() {
                let preview = preview_bounds(bounds, target_zone);
                let offset = preview.origin - bounds.origin;
                Some(
                    div()
                        .absolute()
                        .left(offset.x)
                        .top(offset.y)
                        .w(preview.size.width)
                        .h(preview.size.height)
                        .rounded(px(bezel::theme::Theme::panel_radius()))
                        .bg(theme.accent.opacity(0.18))
                        .border_2()
                        .border_color(theme.accent),
                )
            } else {
                None
            }
        } else {
            None
        };

        let grid = div()
            .id(SharedString::from(format!("terminal-grid-{pane_id}")))
            .key_context("Terminal")
            .track_focus(&pane.focus_handle)
            .size_full()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .px(px(TERMINAL_PADDING_X))
            .py(px(TERMINAL_PADDING_Y))
            .bg(theme.surface)
            .overflow_hidden()
            .flex()
            .flex_col()
            .relative()
            .child(screen)
            .children(scrollbar)
            .children(dock_preview)
            .when(is_active, |el| el.child(actions_overlay))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                this.on_pane_key_down(pane_id, event, window, cx);
            }))
            .on_scroll_wheel(cx.listener(move |this, event, window, cx| {
                this.on_pane_scroll_wheel(pane_id, event, window, cx);
            }));

        div()
            .id(SharedString::from(format!(
                "terminal-pane-container-{pane_id}"
            )))
            .size_full()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded(px(bezel::theme::Theme::panel_radius()))
            .border_1()
            .border_color(if is_active {
                theme.border
            } else {
                theme.border.opacity(0.6)
            })
            .bg(theme.surface)
            .overflow_hidden()
            .opacity(opacity)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.activate_pane(pane_id, window, cx);
                }),
            )
            .on_drag_move(cx.listener(
                move |view, event: &DragMoveEvent<DraggedTerminalPane>, _, cx| {
                    let incoming = event.drag(cx).0;
                    if incoming != pane_id {
                        if let Some(target_zone) = zone(event.bounds, px(0.0), event.event.position)
                        {
                            if view.dock_target != Some((pane_id, target_zone, event.bounds)) {
                                view.dock_target = Some((pane_id, target_zone, event.bounds));
                                cx.notify();
                            }
                        }
                    }
                },
            ))
            .on_drop(
                cx.listener(move |view, drag: &DraggedTerminalPane, window, cx| {
                    let incoming = drag.0;
                    if incoming != pane_id {
                        if let Some((target, target_zone, _)) = view.dock_target.take() {
                            if target == pane_id {
                                view.dock_pane(incoming, target, target_zone, window, cx);
                            }
                        }
                    }
                }),
            )
            .child(grid)
            .into_any_element()
    }

    fn render_pane_tree(
        &mut self,
        tree: &PaneTree,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match tree {
            PaneTree::Leaf(pane_id) => self.render_pane(*pane_id, window, cx),
            PaneTree::Split {
                id: split_id,
                direction,
                fraction,
                first,
                second,
            } => {
                let first_el = self.render_pane_tree(first, window, cx);
                let second_el = self.render_pane_tree(second, window, cx);
                let bezel_theme = bezel::theme::Theme::of(cx);

                let split_id = *split_id;
                let fraction = *fraction;
                let direction = *direction;

                let axis = match direction {
                    SplitDirection::Horizontal => gpui::Axis::Horizontal,
                    SplitDirection::Vertical => gpui::Axis::Vertical,
                };

                let handle = bezel_theme
                    .split_handle(axis, SplitStyle::Ghost)
                    .id(ElementId::NamedInteger(
                        "terminal-split-handle".into(),
                        split_id,
                    ))
                    .when(axis == gpui::Axis::Horizontal, |el| el.w(px(8.0)).h_full())
                    .when(axis == gpui::Axis::Vertical, |el| el.h(px(8.0)).w_full())
                    .on_drag(TerminalSplitDrag { split_id }, |_, _, _, cx| {
                        cx.new(|_| gpui::Empty)
                    });

                let container = div()
                    .id(ElementId::NamedInteger(
                        "terminal-split-container".into(),
                        split_id,
                    ))
                    .size_full()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .on_drag_move(cx.listener(
                        move |view, event: &DragMoveEvent<TerminalSplitDrag>, _, cx| {
                            if event.drag(cx).split_id == split_id {
                                view.active_split = Some(ActiveSplitDrag {
                                    split_id,
                                    axis,
                                    bounds: event.bounds,
                                });
                                let new_frac =
                                    axis_fraction(event.event.position, event.bounds, axis, 0.10);
                                view.update_split_fraction(split_id, new_frac);
                                cx.notify();
                            }
                        },
                    ));

                match direction {
                    SplitDirection::Horizontal => container
                        .flex_row()
                        .child(
                            div()
                                .flex_none()
                                .w(relative(fraction))
                                .h_full()
                                .min_w_0()
                                .min_h_0()
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .child(first_el),
                        )
                        .child(handle)
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .min_h_0()
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .child(second_el),
                        )
                        .into_any_element(),
                    SplitDirection::Vertical => container
                        .flex_col()
                        .child(
                            div()
                                .flex_none()
                                .h(relative(fraction))
                                .w_full()
                                .min_h_0()
                                .min_w_0()
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .child(first_el),
                        )
                        .child(handle)
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .w_full()
                                .min_h_0()
                                .min_w_0()
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .child(second_el),
                        )
                        .into_any_element(),
                }
            }
        }
    }

    fn render_bottom_bar(
        total_count: usize,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let count_label = if total_count == 1 {
            tr!("terminal.terminals_count_one", count = total_count)
        } else {
            tr!("terminal.terminals_count_many", count = total_count)
        };
        let restore_shortcut = crate::platform::primary_shortcut("⌘⇧Enter", "Ctrl+Shift+Enter");

        div()
            .id("terminal-fullsize-bottom-bar")
            .h(px(32.0))
            .flex_none()
            .px(px(10.0))
            .rounded(px(bezel::theme::Theme::control_radius()))
            .bg(theme.raised)
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon(
                        "icons/terminal-square.svg",
                        13.0,
                        theme.text_secondary,
                    ))
                    .child(
                        div()
                            .text_size(sp(11.5))
                            .text_color(theme.text_secondary)
                            .child(count_label),
                    ),
            )
            .child(
                div()
                    .h(px(24.0))
                    .px(px(8.0))
                    .rounded(px(4.0))
                    .flex_none()
                    .bg(theme.overlay)
                    .hover(|el| el.bg(theme.overlay_strong))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .id("terminal-restore-panes-button")
                    .child(icon("icons/minimize.svg", 12.0, theme.text_secondary))
                    .child(
                        div()
                            .text_size(sp(11.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("terminal.show_all_panes", shortcut = restore_shortcut)),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.restore_all_panes(cx);
                    })),
            )
            .into_any_element()
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.panes
            .get(&self.active_pane)
            .map(|p| p.focus_handle.clone())
            .unwrap_or_else(|| self.focus_handle.clone())
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.active_split.is_some() && !cx.has_active_drag() {
            self.active_split = None;
        }
        if self.dock_target.is_some() && !cx.has_active_drag() {
            self.dock_target = None;
        }

        let theme = Theme::current(cx);

        let body = if let Some(expanded_id) = self.expanded_pane {
            if self.panes.contains_key(&expanded_id) {
                let bottom_bar = Self::render_bottom_bar(self.panes.len(), theme, cx);
                let pane_el = self.render_pane(expanded_id, window, cx);
                div()
                    .size_full()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(pane_el)
                    .child(bottom_bar)
                    .into_any_element()
            } else {
                self.expanded_pane = None;
                let layout = self.layout.clone();
                self.render_pane_tree(&layout, window, cx)
            }
        } else {
            let layout = self.layout.clone();
            self.render_pane_tree(&layout, window, cx)
        };

        div()
            .id("padu-terminal-root")
            .size_full()
            .h_full()
            .w_full()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .on_drag_move(cx.listener(
                move |view, event: &DragMoveEvent<TerminalSplitDrag>, _, cx| {
                    let drag_id = event.drag(cx).split_id;
                    if let Some(active) = view.active_split {
                        if active.split_id == drag_id {
                            let new_frac = axis_fraction(
                                event.event.position,
                                active.bounds,
                                active.axis,
                                0.10,
                            );
                            view.update_split_fraction(drag_id, new_frac);
                            cx.notify();
                        }
                    }
                },
            ))
            .child(body)
            .on_action(
                cx.listener(|this, _: &SplitTerminalHorizontal, window, cx| {
                    this.split_horizontal(this.active_pane, window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &SplitTerminalVertical, window, cx| {
                this.split_vertical(this.active_pane, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminalExpand, window, cx| {
                this.toggle_expand(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTerminalPane, window, cx| {
                if this.panes.len() > 1 {
                    this.close_pane(this.active_pane, window, cx);
                }
            }))
    }
}

fn terminal_cursor_style(focused: bool, blink_visible: bool) -> TerminalCursorStyle {
    if !focused {
        TerminalCursorStyle::Outline
    } else if blink_visible {
        TerminalCursorStyle::Solid
    } else {
        TerminalCursorStyle::Hidden
    }
}

fn terminal_grid_dimensions(
    bounds: Option<Bounds<Pixels>>,
    fallback_panel_width: f32,
    viewport_height: Pixels,
    cell_width: f32,
) -> (usize, usize) {
    if let Some(bounds) = bounds {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let cols = (width / cell_width)
            .floor()
            .max(TERMINAL_MIN_COLUMNS as f32) as usize;
        let rows = (height / TERMINAL_CELL_HEIGHT)
            .floor()
            .max(TERMINAL_MIN_ROWS as f32) as usize;
        (cols, rows)
    } else {
        let pane_width = fallback_panel_width.max(300.0);
        let pane_height = (f32::from(viewport_height) - 56.0).max(120.0);
        let cols = ((pane_width - TERMINAL_PADDING_X * 2.0) / cell_width)
            .floor()
            .max(TERMINAL_MIN_COLUMNS as f32) as usize;
        let rows = ((pane_height - TERMINAL_PADDING_Y * 2.0) / TERMINAL_CELL_HEIGHT)
            .floor()
            .max(TERMINAL_MIN_ROWS as f32) as usize;
        (cols, rows)
    }
}

fn terminal_grid_point(
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
    cell_width: f32,
    columns: usize,
    rows: usize,
    display_offset: usize,
    clamp_to_grid: bool,
) -> Option<(TerminalPoint, Side)> {
    if columns == 0 || rows == 0 || (!clamp_to_grid && !bounds.contains(&position)) {
        return None;
    }

    let x = f32::from(position.x - bounds.origin.x);
    let y = f32::from(position.y - bounds.origin.y);
    let max_x = columns as f32 * cell_width;
    let max_y = rows as f32 * TERMINAL_CELL_HEIGHT;
    let x = x.clamp(0.0, max_x);
    let y = y.clamp(0.0, max_y);
    let column = ((x / cell_width).floor() as usize).min(columns - 1);
    let viewport_row = ((y / TERMINAL_CELL_HEIGHT).floor() as usize).min(rows - 1) as i32;
    let side = if x >= max_x || x % cell_width >= cell_width / 2.0 {
        Side::Right
    } else {
        Side::Left
    };
    let line = Line(viewport_row - display_offset.min(i32::MAX as usize) as i32);

    Some((TerminalPoint::new(line, Column(column)), side))
}

/// Apply the same delimiter heuristics as Alacritty's hint system.
fn post_process_terminal_link<T: EventListener>(
    term: &Term<T>,
    regex_match: &Match,
) -> Option<Match> {
    let mut iter = term.grid().iter_from(*regex_match.start());
    let mut character = iter.cell().c;
    let end = *regex_match.end();
    let mut open_parens = 0;
    let mut open_brackets = 0;

    loop {
        match character {
            '(' => open_parens += 1,
            '[' => open_brackets += 1,
            ')' if open_parens == 0 => {
                iter.prev();
                break;
            }
            ')' => open_parens -= 1,
            ']' if open_brackets == 0 => {
                iter.prev();
                break;
            }
            ']' => open_brackets -= 1,
            _ => {}
        }

        if iter.point() == end {
            break;
        }
        let Some(indexed) = iter.next() else {
            break;
        };
        character = indexed.cell.c;
    }

    let start = *regex_match.start();
    while iter.point() != start {
        if !matches!(
            character,
            '.' | ',' | ':' | ';' | '?' | '!' | '(' | '[' | '\''
        ) {
            break;
        }
        let Some(indexed) = iter.prev() else {
            break;
        };
        character = indexed.cell.c;
    }

    (start <= iter.point()).then(|| start..=iter.point())
}

fn plain_link_at<T: EventListener>(
    term: &Term<T>,
    regex: &mut RegexSearch,
    point: TerminalPoint,
) -> Option<(String, Match)> {
    let mut start = term.line_search_left(point);
    let mut end = term.line_search_right(point);
    start.line = start.line.max(point.line - MAX_TERMINAL_LINK_SEARCH_LINES);
    end.line = end.line.min(point.line + MAX_TERMINAL_LINK_SEARCH_LINES);

    let raw_match = RegexIter::new(start, end, Direction::Right, term, regex)
        .find(|bounds| bounds.contains(&point))?;
    let raw_end = *raw_match.end();
    let mut next_match = Some(raw_match);

    // Post-processing can split a greedy match at an unmatched closing
    // bracket. Continue inside the original range in case the clicked URL is
    // a later segment of that match.
    while let Some(regex_match) = next_match {
        let processed = post_process_terminal_link(term, &regex_match);
        if processed
            .as_ref()
            .is_some_and(|bounds| bounds.contains(&point))
        {
            let bounds = processed.unwrap();
            let value = term.bounds_to_string(*bounds.start(), *bounds.end());
            return Some((value, bounds));
        }

        let next_start = processed
            .as_ref()
            .map_or_else(|| *regex_match.start(), |bounds| *bounds.end())
            .add(term, Boundary::Grid, 1);
        if next_start > raw_end {
            return None;
        }
        next_match = term.regex_search_right(regex, next_start, raw_end);
    }

    None
}

fn hyperlink_at<T: EventListener>(term: &Term<T>, point: TerminalPoint) -> Option<(String, Match)> {
    let hyperlink = term.grid()[point].hyperlink()?;
    let grid = term.grid();

    let mut end = point;
    for cell in grid.iter_from(point) {
        if cell.hyperlink().as_ref() == Some(&hyperlink) {
            end = cell.point;
        } else {
            break;
        }
    }

    let mut start = point;
    let mut iter = grid.iter_from(point);
    while let Some(cell) = iter.prev() {
        if cell.hyperlink().as_ref() == Some(&hyperlink) {
            start = cell.point;
        } else {
            break;
        }
    }

    Some((hyperlink.uri().to_owned(), start..=end))
}

fn terminal_link_target(value: &str, working_directory: &Path) -> Option<TerminalLinkTarget> {
    if let Some(path) = existing_terminal_file_path(value, working_directory) {
        return Some(TerminalLinkTarget::File(path));
    }

    let url = url::Url::parse(value).ok()?;
    if url.scheme() == "file" {
        return None;
    }
    Some(TerminalLinkTarget::Url(url.to_string()))
}

fn existing_terminal_file_path(value: &str, working_directory: &Path) -> Option<PathBuf> {
    let mut path = match url::Url::parse(value) {
        Ok(url) if url.scheme() == "file" => url.to_file_path().ok()?,
        Ok(_) => return None,
        Err(_) => {
            if let Some(relative) = value.strip_prefix("~/") {
                dirs::home_dir()?.join(relative)
            } else {
                let path = Path::new(value);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    working_directory.join(path)
                }
            }
        }
    };

    loop {
        if path.exists() {
            return Some(path);
        }
        let value = path.to_string_lossy();
        let (prefix, suffix) = value.rsplit_once(':')?;
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        path = PathBuf::from(prefix);
    }
}

fn bracketed_paste(text: String, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::BRACKETED_PASTE) {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.into_bytes()
    }
}

fn terminal_key_bytes(keystroke: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let modifiers = keystroke.modifiers;
    let key = keystroke.key.as_str();

    if modifiers.platform {
        #[cfg(target_os = "macos")]
        return match key {
            "left" if !modifiers.alt && !modifiers.control => Some(vec![0x01]),
            "right" if !modifiers.alt && !modifiers.control => Some(vec![0x05]),
            "backspace" if !modifiers.alt && !modifiers.control => Some(vec![0x15]),
            _ => None,
        };

        #[cfg(not(target_os = "macos"))]
        return None;
    }

    let modifier = 1
        + u8::from(modifiers.shift)
        + u8::from(modifiers.alt) * 2
        + u8::from(modifiers.control) * 4;
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let special = match key {
        "enter" | "return" => Some("\r".to_owned()),
        "tab" if modifiers.shift => Some("\x1b[Z".to_owned()),
        "tab" => Some("\t".to_owned()),
        "backspace" => Some("\x7f".to_owned()),
        "escape" => Some("\x1b".to_owned()),
        "up" => Some(cursor_sequence('A', modifier, app_cursor)),
        "down" => Some(cursor_sequence('B', modifier, app_cursor)),
        "right" => Some(cursor_sequence('C', modifier, app_cursor)),
        "left" => Some(cursor_sequence('D', modifier, app_cursor)),
        "home" => Some(csi_sequence('H', modifier)),
        "end" => Some(csi_sequence('F', modifier)),
        "insert" => Some(tilde_sequence(2, modifier)),
        "delete" | "forwarddelete" => Some(tilde_sequence(3, modifier)),
        "pageup" => Some(tilde_sequence(5, modifier)),
        "pagedown" => Some(tilde_sequence(6, modifier)),
        "f1" => Some(function_sequence('P', modifier)),
        "f2" => Some(function_sequence('Q', modifier)),
        "f3" => Some(function_sequence('R', modifier)),
        "f4" => Some(function_sequence('S', modifier)),
        "f5" => Some(tilde_sequence(15, modifier)),
        "f6" => Some(tilde_sequence(17, modifier)),
        "f7" => Some(tilde_sequence(18, modifier)),
        "f8" => Some(tilde_sequence(19, modifier)),
        "f9" => Some(tilde_sequence(20, modifier)),
        "f10" => Some(tilde_sequence(21, modifier)),
        "f11" => Some(tilde_sequence(23, modifier)),
        "f12" => Some(tilde_sequence(24, modifier)),
        _ => None,
    };
    if let Some(special) = special {
        return Some(special.into_bytes());
    }

    let text = keystroke
        .key_char
        .as_deref()
        .or_else(|| (key.chars().count() == 1).then_some(key))?;
    let mut bytes = if modifiers.control {
        control_bytes(text)?
    } else {
        text.as_bytes().to_vec()
    };
    if modifiers.alt {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

fn control_bytes(text: &str) -> Option<Vec<u8>> {
    let character = text.chars().next()?.to_ascii_lowercase();
    let byte = match character {
        ' ' | '@' => 0,
        'a'..='z' => character as u8 - b'a' + 1,
        '[' => 27,
        '\\' => 28,
        ']' => 29,
        '^' => 30,
        '_' => 31,
        '?' => 127,
        _ => return None,
    };
    Some(vec![byte])
}

fn cursor_sequence(final_byte: char, modifier: u8, app_cursor: bool) -> String {
    if modifier == 1 {
        format!("\x1b{}{}", if app_cursor { 'O' } else { '[' }, final_byte)
    } else {
        format!("\x1b[1;{modifier}{final_byte}")
    }
}

fn csi_sequence(final_byte: char, modifier: u8) -> String {
    if modifier == 1 {
        format!("\x1b[{final_byte}")
    } else {
        format!("\x1b[1;{modifier}{final_byte}")
    }
}

fn function_sequence(final_byte: char, modifier: u8) -> String {
    if modifier == 1 {
        format!("\x1bO{final_byte}")
    } else {
        format!("\x1b[1;{modifier}{final_byte}")
    }
}

fn tilde_sequence(number: u8, modifier: u8) -> String {
    if modifier == 1 {
        format!("\x1b[{number}~")
    } else {
        format!("\x1b[{number};{modifier}~")
    }
}

fn resolve_color(
    color: Color,
    colors: &alacritty_terminal::term::color::Colors,
    theme: Theme,
    foreground: bool,
) -> Hsla {
    let rgb = match color {
        Color::Spec(color) => Some(color),
        Color::Indexed(index) => {
            colors[index as usize].or_else(|| Some(terminal_rgb(index as usize, theme.is_dark)))
        }
        Color::Named(NamedColor::Foreground | NamedColor::BrightForeground) => None,
        Color::Named(NamedColor::Background) => return theme.surface,
        Color::Named(NamedColor::Cursor) => return theme.text,
        Color::Named(named) => {
            colors[named as usize].or_else(|| Some(terminal_rgb(named as usize, theme.is_dark)))
        }
    };
    rgb.map(rgb_to_hsla).unwrap_or(if foreground {
        theme.text
    } else {
        theme.surface
    })
}

fn rgb_to_hsla(color: Rgb) -> Hsla {
    rgb((u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)).into()
}

fn terminal_rgb(index: usize, is_dark: bool) -> Rgb {
    // Tomorrow Night on dark surfaces, Tomorrow on light — same hues tuned
    // for the opposite background so ANSI text stays legible on white.
    const ANSI_DARK: [u32; 16] = [
        0x1d1f21, 0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x666666,
        0xd54e53, 0xb9ca4a, 0xe7c547, 0x7aa6da, 0xc397d8, 0x70c0b1, 0xeaeaea,
    ];
    const ANSI_LIGHT: [u32; 16] = [
        0x000000, 0xc82829, 0x718c00, 0xeab700, 0x4271ae, 0x8959a8, 0x3e999f, 0xc7c7c7, 0x8e908c,
        0xc82829, 0x718c00, 0xeab700, 0x4271ae, 0x8959a8, 0x3e999f, 0xffffff,
    ];
    let ansi = if is_dark { &ANSI_DARK } else { &ANSI_LIGHT };
    let value = match index {
        0..=15 => ansi[index],
        16..=231 => {
            let index = index - 16;
            let channel = |value: usize| {
                if value == 0 {
                    0
                } else {
                    55 + value as u32 * 40
                }
            };
            let red = channel(index / 36);
            let green = channel((index / 6) % 6);
            let blue = channel(index % 6);
            (red << 16) | (green << 8) | blue
        }
        232..=255 => {
            let value = 8 + (index as u32 - 232) * 10;
            (value << 16) | (value << 8) | value
        }
        value
            if value >= NamedColor::DimBlack as usize && value <= NamedColor::DimWhite as usize =>
        {
            let base = ansi[value - NamedColor::DimBlack as usize];
            // Faint fades toward the surface: black on dark, white on light.
            let dim = |channel: u32| {
                if is_dark {
                    channel * 2 / 3
                } else {
                    channel + (255 - channel) / 3
                }
            };
            (dim((base >> 16) & 0xff) << 16) | (dim((base >> 8) & 0xff) << 8) | dim(base & 0xff)
        }
        _ => {
            if is_dark {
                0xe5e5e5
            } else {
                0x242424
            }
        }
    };
    Rgb {
        r: (value >> 16) as u8,
        g: (value >> 8) as u8,
        b: value as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::vte::ansi::Processor;
    use gpui::{Modifiers, point, size};

    fn key(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            key: key.into(),
            key_char: key_char.map(str::to_owned),
            modifiers,
        }
    }

    fn parse_terminal(input: &[u8]) -> Term<VoidListener> {
        let dimensions = TerminalDimensions {
            columns: 40,
            rows: 3,
        };
        let mut term = Term::new(Config::default(), &dimensions, VoidListener);
        let mut processor: Processor = Processor::new();
        processor.advance(&mut term, input);
        term
    }

    fn terminal_point_for(content: &str, needle: &str) -> TerminalPoint {
        let offset = content.find(needle).unwrap();
        TerminalPoint::new(Line((offset / 40) as i32), Column(offset % 40))
    }

    fn plain_link_value(term: &Term<VoidListener>, point: TerminalPoint) -> Option<String> {
        let mut regex = RegexSearch::new(TERMINAL_LINK_REGEX).unwrap();
        plain_link_at(term, &mut regex, point).map(|(value, _)| value)
    }

    #[test]
    fn detects_plain_and_osc8_terminal_links() {
        let content = "visit (https://example.com/docs). next";
        let term = parse_terminal(content.as_bytes());
        assert_eq!(
            plain_link_value(&term, terminal_point_for(content, "example")),
            Some("https://example.com/docs".to_owned())
        );
        assert_eq!(
            plain_link_value(&term, terminal_point_for(content, ").")),
            None
        );

        let term = parse_terminal(b"x\x1b]8;;https://padu.gg\x1b\\Padu\x1b]8;;\x1b\\ y");
        let (value, bounds) = hyperlink_at(&term, TerminalPoint::new(Line(0), Column(2))).unwrap();
        assert_eq!(value, "https://padu.gg");
        assert_eq!(
            bounds,
            TerminalPoint::new(Line(0), Column(1))..=TerminalPoint::new(Line(0), Column(4))
        );
    }

    #[test]
    fn detects_links_across_soft_wrapped_lines() {
        let content = "prefix https://example.com/a/very/long/path/that/wraps suffix";
        let term = parse_terminal(content.as_bytes());
        let mut regex = RegexSearch::new(TERMINAL_LINK_REGEX).unwrap();
        let (value, bounds) =
            plain_link_at(&term, &mut regex, terminal_point_for(content, "that")).unwrap();

        assert_eq!(value, "https://example.com/a/very/long/path/that/wraps");
        assert_eq!(*bounds.start(), terminal_point_for(content, "https"));
        assert_eq!(bounds.end().line, Line(1));
    }

    #[test]
    fn resolves_terminal_urls_and_existing_project_paths() {
        let working_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            terminal_link_target("https://example.com/docs", working_directory),
            Some(TerminalLinkTarget::Url(
                "https://example.com/docs".to_owned()
            ))
        );
        assert_eq!(
            terminal_link_target("src/terminal.rs:42:8", working_directory),
            Some(TerminalLinkTarget::File(
                working_directory.join("src/terminal.rs")
            ))
        );
        assert_eq!(
            terminal_link_target("src/does-not-exist.rs", working_directory),
            None
        );
    }

    #[test]
    fn encodes_terminal_control_and_cursor_keys() {
        assert_eq!(
            terminal_key_bytes(
                &key(
                    "c",
                    Some("c"),
                    Modifiers {
                        control: true,
                        ..Default::default()
                    }
                ),
                TermMode::empty()
            ),
            Some(vec![3])
        );
        assert_eq!(
            terminal_key_bytes(&key("up", None, Modifiers::default()), TermMode::APP_CURSOR),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            terminal_key_bytes(
                &key(
                    "left",
                    None,
                    Modifiers {
                        control: true,
                        ..Default::default()
                    }
                ),
                TermMode::empty()
            ),
            Some(b"\x1b[1;5D".to_vec())
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            terminal_key_bytes(
                &key(
                    "right",
                    None,
                    Modifiers {
                        platform: true,
                        alt: true,
                        ..Default::default()
                    }
                ),
                TermMode::empty()
            ),
            None
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_terminal_clipboard_shortcuts_preserve_ctrl_c_for_sigint() {
        let control = Modifiers {
            control: true,
            ..Default::default()
        };
        let control_shift = Modifiers {
            control: true,
            shift: true,
            ..Default::default()
        };

        assert!(primary_modifier_pressed(&control));
        assert!(!terminal_clipboard_modifier_pressed(&control));
        assert!(terminal_clipboard_modifier_pressed(&control_shift));
    }

    #[test]
    fn wraps_bracketed_paste_only_when_requested() {
        assert_eq!(
            bracketed_paste("hello".into(), TermMode::BRACKETED_PASTE),
            b"\x1b[200~hello\x1b[201~"
        );
        assert_eq!(bracketed_paste("hello".into(), TermMode::empty()), b"hello");
    }

    #[test]
    fn cursor_blinks_while_focused_and_outlines_when_unfocused() {
        assert_eq!(
            terminal_cursor_style(false, false),
            TerminalCursorStyle::Outline
        );
        assert_eq!(
            terminal_cursor_style(false, true),
            TerminalCursorStyle::Outline
        );
        assert_eq!(
            terminal_cursor_style(true, false),
            TerminalCursorStyle::Hidden
        );
        assert_eq!(
            terminal_cursor_style(true, true),
            TerminalCursorStyle::Solid
        );
    }

    #[test]
    fn maps_pointer_positions_into_scrollback_grid_coordinates() {
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(72.0), px(64.0)));
        let position = point(
            px(10.0 + TERMINAL_CELL_WIDTH * 2.0 + 5.0),
            px(20.0 + TERMINAL_CELL_HEIGHT + 8.0),
        );

        assert_eq!(
            terminal_grid_point(bounds, position, TERMINAL_CELL_WIDTH, 10, 4, 3, false),
            Some((TerminalPoint::new(Line(-2), Column(2)), Side::Right))
        );
        assert_eq!(
            terminal_grid_point(
                bounds,
                point(px(0.0), px(0.0)),
                TERMINAL_CELL_WIDTH,
                10,
                4,
                3,
                false
            ),
            None
        );
    }

    #[test]
    fn clamps_selection_drags_to_the_terminal_grid_edges() {
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(72.0), px(64.0)));

        assert_eq!(
            terminal_grid_point(
                bounds,
                point(px(500.0), px(500.0)),
                TERMINAL_CELL_WIDTH,
                10,
                4,
                0,
                true
            ),
            Some((TerminalPoint::new(Line(3), Column(9)), Side::Right))
        );
        assert_eq!(
            terminal_grid_point(
                bounds,
                point(px(-50.0), px(-50.0)),
                TERMINAL_CELL_WIDTH,
                10,
                4,
                3,
                true
            ),
            Some((TerminalPoint::new(Line(-3), Column(0)), Side::Left))
        );
    }

    #[test]
    fn test_pane_tree_split_horizontal_vertical_and_remove() {
        let mut tree = PaneTree::Leaf(1);
        assert_eq!(tree.pane_ids(), vec![1]);

        // Split 1 horizontally into 1 and 2
        assert!(tree.split_pane(1, 2, 1, SplitDirection::Horizontal));
        assert_eq!(tree.pane_ids(), vec![1, 2]);

        // Split 2 vertically into 2 and 3
        assert!(tree.split_pane(2, 3, 2, SplitDirection::Vertical));
        assert_eq!(tree.pane_ids(), vec![1, 2, 3]);

        // Update split fraction for split 1
        assert!(tree.update_fraction(1, 0.35));
        if let PaneTree::Split { fraction, .. } = tree {
            assert!((fraction - 0.35).abs() < 1e-4);
        } else {
            panic!("expected Split");
        }

        // Remove 2 -> 3 takes its place
        assert!(tree.remove_pane(2));
        assert_eq!(tree.pane_ids(), vec![1, 3]);

        // Remove 3 -> 1 takes the whole space
        assert!(tree.remove_pane(3));
        assert_eq!(tree.pane_ids(), vec![1]);

        // Cannot remove the only remaining pane
        assert!(!tree.remove_pane(1));
        assert_eq!(tree.pane_ids(), vec![1]);
    }

    #[test]
    fn test_pane_tree_dock_pane() {
        let mut tree = PaneTree::Leaf(1);

        // Dock 2 to the right of 1
        assert!(tree.dock_pane(1, 2, 10, Zone::Right));
        assert_eq!(tree.pane_ids(), vec![1, 2]);

        // Dock 3 to the top of 2
        assert!(tree.dock_pane(2, 3, 11, Zone::Top));
        assert_eq!(tree.pane_ids(), vec![1, 3, 2]);

        // Redock 1 to the bottom of 3
        assert!(tree.dock_pane(3, 1, 12, Zone::Bottom));
        assert_eq!(tree.pane_ids(), vec![3, 1, 2]);

        // Docking onto itself returns false
        assert!(!tree.dock_pane(1, 1, 13, Zone::Left));
    }

    #[test]
    fn test_terminal_grid_dimensions_expand_to_container() {
        let cell_width = 8.0;
        // Measured container bounds (e.g. 400px wide, 720px tall)
        let bounds = Bounds::new(point(px(10.0), px(8.0)), size(px(400.0), px(720.0)));
        let (cols, rows) = terminal_grid_dimensions(Some(bounds), 300.0, px(800.0), cell_width);

        // 400.0 / 8.0 = 50 cols
        assert_eq!(cols, 50);
        // 720.0 / 18.0 = 40 rows: must expand to all 40 available rows
        assert_eq!(rows, 40);

        // Fallback when bounds are not measured yet
        let (fallback_cols, fallback_rows) =
            terminal_grid_dimensions(None, 400.0, px(800.0), cell_width);
        // (400 - 20) / 8 = 47 cols
        assert_eq!(fallback_cols, 47);
        // (800 - 56 - 16) / 18 = 40 rows
        assert_eq!(fallback_rows, 40);
    }
}
