//! The window-wide command palette opened with the platform's primary shortcut + K.
//!
//! The search field keeps real focus while the list cursor is drawn, matching
//! native pickers and Zed's command palette. Metadata results rebuild when the
//! query changes; persisted transcript matches join from a debounced background
//! SQLite scan. Caret blinks therefore repaint one in-memory snapshot instead
//! of re-fuzzy-matching history or touching storage every frame.

use gpui::{KeyBinding, StyledText, TextRun, actions};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use super::notes::Note;
use super::*;

actions!(
    padu_command_palette,
    [
        SelectNext,
        SelectPrevious,
        SelectFirst,
        SelectLast,
        SelectPageDown,
        SelectPageUp,
        Confirm,
        Dismiss
    ]
);

const SEARCH_CONTEXT: &str = "CommandPalette > TextInput";
const MAX_TASK_RESULTS: usize = 12;
const MAX_RESUME_RESULTS: usize = 30;
const PROVIDER_SESSION_CATALOG_LIMIT: usize = 250;
const MESSAGE_SEARCH_LIMIT: usize = 50;
const MESSAGE_SEARCH_CACHE_CAPACITY: usize = 24;
const PAGE_STEP: isize = 7;
const MESSAGE_SEARCH_DEBOUNCE: Duration = Duration::from_millis(90);
const SEARCH_ROW_HEIGHT: f32 = 52.0;
const SECTION_HEADER_HEIGHT: f32 = 26.0;
const PROVIDER_SECTION_TOP_MARGIN: f32 = 6.0;
const RESULT_ROW_HEIGHT: f32 = 40.0;
const CONTENT_RESULT_ROW_HEIGHT: f32 = 54.0;
const FILE_RESULT_ROW_HEIGHT: f32 = 48.0;
const EMPTY_RESULTS_HEIGHT: f32 = 160.0;
const RESULTS_BOTTOM_PADDING: f32 = 6.0;
const MAX_CARD_HEIGHT: f32 = 440.0;
const FOOTER_HEIGHT: f32 = 36.0;

/// Bind list navigation beneath the focused one-line input. This is registered
/// after the input's bindings, although the more-specific key context would
/// win either way.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, Some(SEARCH_CONTEXT)),
        KeyBinding::new("up", SelectPrevious, Some(SEARCH_CONTEXT)),
        KeyBinding::new("ctrl-n", SelectNext, Some(SEARCH_CONTEXT)),
        KeyBinding::new("ctrl-p", SelectPrevious, Some(SEARCH_CONTEXT)),
        KeyBinding::new("tab", SelectNext, Some(SEARCH_CONTEXT)),
        KeyBinding::new("shift-tab", SelectPrevious, Some(SEARCH_CONTEXT)),
        KeyBinding::new("home", SelectFirst, Some(SEARCH_CONTEXT)),
        KeyBinding::new("end", SelectLast, Some(SEARCH_CONTEXT)),
        KeyBinding::new("pagedown", SelectPageDown, Some(SEARCH_CONTEXT)),
        KeyBinding::new("pageup", SelectPageUp, Some(SEARCH_CONTEXT)),
        KeyBinding::new("enter", Confirm, Some(SEARCH_CONTEXT)),
        // Bound at the palette, not the field: the query field's own
        // clear-on-escape outranks this (deeper context) while it has text,
        // and an empty field propagates the keystroke down to it.
        KeyBinding::new("escape", Dismiss, Some("CommandPalette")),
    ]);
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PaletteSection {
    Suggested,
    Tasks,
    Sessions,
    Providers,
    Commands,
    Settings,
}

impl PaletteSection {
    fn label(self) -> String {
        crate::i18n::translate(match self {
            Self::Suggested => "command_palette.suggested",
            Self::Tasks => "command_palette.tasks",
            Self::Sessions => "command_palette.sessions",
            Self::Providers => "command_palette.providers",
            Self::Commands => "command_palette.commands",
            Self::Settings => "command_palette.settings",
        })
    }

    fn query_rank(self) -> usize {
        match self {
            Self::Commands | Self::Suggested | Self::Sessions | Self::Providers => 0,
            Self::Tasks => 1,
            Self::Settings => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PaletteIcon {
    Asset(&'static str),
    File(&'static str),
    Provider(ProviderKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PaletteIdentifier {
    TaskId,
    AgentCliThreadId,
}

impl PaletteIdentifier {
    const ALL: [Self; 2] = [Self::TaskId, Self::AgentCliThreadId];

    fn value(self, session: Option<&AgentSession>) -> Option<String> {
        let session = session?;
        match self {
            Self::TaskId => Some(session.id.to_string()),
            Self::AgentCliThreadId => session.provider_native_id().map(str::to_owned),
        }
    }

    fn label(self) -> String {
        tr!(match self {
            Self::TaskId => "command_palette.copy_task_id",
            Self::AgentCliThreadId => "command_palette.copy_agent_cli_thread_id",
        })
    }

    fn copied_message(self) -> String {
        tr!(match self {
            Self::TaskId => "command_palette.task_id_copied",
            Self::AgentCliThreadId => "command_palette.agent_cli_thread_id_copied",
        })
    }

    fn keywords(self) -> &'static str {
        match self {
            Self::TaskId => "copy task id uuid identifier session debug",
            Self::AgentCliThreadId => {
                "copy agent cli thread id native session uuid identifier codex claude debug"
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PaletteAction {
    NewTask,
    Resume,
    ChooseResumeProvider,
    SelectResumeProvider(ProviderKind),
    ResumeProviderSession(ProviderSessionSummary),
    EmbedNote(Note),
    OpenProject,
    FocusComposer,
    CopyIdentifier(PaletteIdentifier),
    ChooseModel,
    ToggleUsage,
    CollapseSidebarGroups,
    ToggleSidebar,
    ToggleRightPanel,
    ToggleRightPanelFullscreen,
    NextRightPanelTab,
    PrevRightPanelTab,
    OpenBrowser,
    OpenTerminal,
    OpenFiles,
    FindFile,
    OpenFile(String),
    OpenReview,
    OpenNotes,
    OpenSettings(SettingsPage),
    OpenOnboarding,
    SelectTask(Uuid),
    SelectPreviousTask,
    SelectNextTask,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum CommandPaletteView {
    #[default]
    Commands,
    Resume,
    ResumeProviders,
    Notes,
    FindFile,
}

#[derive(Clone, Debug)]
pub(super) struct CommandPaletteItem {
    section: PaletteSection,
    label: String,
    detail: Option<String>,
    icon: PaletteIcon,
    shortcut: Option<&'static str>,
    action: PaletteAction,
    content_match: Option<crate::persistence::SessionMessageMatch>,
    search_text: String,
    order: usize,
    recency: u64,
}

impl CommandPaletteItem {
    fn command(
        section: PaletteSection,
        label: String,
        icon: &'static str,
        shortcut: Option<&'static str>,
        action: PaletteAction,
        keywords: &'static str,
        order: usize,
    ) -> Self {
        let search_text = format!("{label} {keywords}");
        Self {
            section,
            label,
            detail: None,
            icon: PaletteIcon::Asset(icon),
            shortcut,
            action,
            content_match: None,
            search_text,
            order,
            recency: 0,
        }
    }
}

struct ScoredPaletteItem {
    score: u32,
    item: CommandPaletteItem,
}

fn next_selection_index(selected: usize, len: usize, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let selected = selected.min(len - 1);
    Some(if delta == isize::MIN {
        0
    } else if delta == isize::MAX {
        len - 1
    } else if delta.unsigned_abs() > 1 {
        (selected as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize
    } else {
        (selected as isize + delta).rem_euclid(len as isize) as usize
    })
}

fn palette_content_match_text(
    matched: &crate::persistence::SessionMessageMatch,
    query: &str,
    window: &Window,
    theme: Theme,
) -> StyledText {
    let (source_label, source_color) = match matched.source {
        MessageRole::User => (tr!("command_palette.you"), theme.gauge),
        MessageRole::Assistant | MessageRole::System => {
            (tr!("command_palette.agent"), theme.success)
        }
    };
    let label = format!("{source_label}: ");
    let mut text = String::with_capacity(label.len() + matched.snippet.len());
    text.push_str(&label);
    text.push_str(&matched.snippet);

    let mut normal_font = window.text_style().font();
    normal_font.weight = FontWeight::NORMAL;
    let mut emphasized_font = normal_font.clone();
    emphasized_font.weight = FontWeight::SEMIBOLD;
    let mut runs = vec![TextRun {
        len: label.len(),
        font: emphasized_font.clone(),
        color: source_color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }];

    let query = query.trim();
    let match_range = (!query.is_empty())
        .then(|| {
            matched
                .snippet
                .to_ascii_lowercase()
                .find(&query.to_ascii_lowercase())
                .map(|start| start..start + query.len())
        })
        .flatten();
    let mut push = |len: usize, font, color| {
        if len > 0 {
            runs.push(TextRun {
                len,
                font,
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            });
        }
    };
    if let Some(range) = match_range {
        push(range.start, normal_font.clone(), theme.text_tertiary);
        push(range.len(), emphasized_font, theme.text_secondary);
        push(
            matched.snippet.len().saturating_sub(range.end),
            normal_font,
            theme.text_tertiary,
        );
    } else {
        push(matched.snippet.len(), normal_font, theme.text_tertiary);
    }
    StyledText::new(text).with_runs(runs)
}

fn command_palette_row_height(item: &CommandPaletteItem) -> f32 {
    if matches!(item.action, PaletteAction::OpenFile(_)) {
        FILE_RESULT_ROW_HEIGHT
    } else if item.content_match.is_some() {
        CONTENT_RESULT_ROW_HEIGHT
    } else {
        RESULT_ROW_HEIGHT
    }
}

fn should_show_command_palette_empty_state(result_count: usize, search_pending: bool) -> bool {
    result_count == 0 && !search_pending
}

fn should_keep_previous_command_palette_results(
    next_result_count: usize,
    search_pending: bool,
    previous_result_count: usize,
) -> bool {
    next_result_count == 0 && search_pending && previous_result_count > 0
}

fn same_provider_session(left: &ProviderResumeCursor, right: &ProviderResumeCursor) -> bool {
    left.provider() == right.provider() && left.native_id() == right.native_id()
}

fn command_palette_results_height(results: &[CommandPaletteItem], show_empty_state: bool) -> f32 {
    let content_height = if show_empty_state {
        EMPTY_RESULTS_HEIGHT
    } else {
        let mut previous_section = None;
        results
            .iter()
            .map(|item| {
                let section_leading_height = if previous_section != Some(item.section) {
                    if item.section == PaletteSection::Providers {
                        PROVIDER_SECTION_TOP_MARGIN
                    } else {
                        SECTION_HEADER_HEIGHT
                    }
                } else {
                    0.0
                };
                if previous_section != Some(item.section) {
                    previous_section = Some(item.section);
                }
                section_leading_height + command_palette_row_height(item)
            })
            .sum()
    };
    content_height + RESULTS_BOTTOM_PADDING
}

pub(super) struct CommandPaletteUi {
    pub(super) search: Entity<TextInput>,
    pub(super) open: bool,
    focus_generation: u64,
    previous_focus: Option<FocusHandle>,
    view: CommandPaletteView,
    results: Vec<CommandPaletteItem>,
    message_searches: QueryCache<String, Vec<crate::persistence::SessionMessageMatch>>,
    active_message_query: Option<String>,
    message_matches_query: Option<String>,
    message_matches: HashMap<Uuid, crate::persistence::SessionMessageMatch>,
    message_search_pending: bool,
    provider_sessions: Vec<ProviderSessionSummary>,
    resume_provider: ProviderKind,
    provider_sessions_pending: bool,
    provider_session_import: Option<ProviderResumeCursor>,
    provider_session_error: Option<String>,
    provider_session_generation: u64,
    selected: usize,
    scroll: ScrollHandle,
    matcher: Matcher,
}

impl CommandPaletteUi {
    pub(super) fn new(search: Entity<TextInput>) -> Self {
        Self {
            search,
            open: false,
            focus_generation: 0,
            previous_focus: None,
            view: CommandPaletteView::Commands,
            results: Vec::new(),
            message_searches: QueryCache::new(MESSAGE_SEARCH_CACHE_CAPACITY),
            active_message_query: None,
            message_matches_query: None,
            message_matches: HashMap::new(),
            message_search_pending: false,
            provider_sessions: Vec::new(),
            resume_provider: ProviderKind::default(),
            provider_sessions_pending: false,
            provider_session_import: None,
            provider_session_error: None,
            provider_session_generation: 0,
            selected: 0,
            scroll: ScrollHandle::new(),
            matcher: crate::composer_complete::matcher(),
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }
}

impl Padu {
    pub(super) fn open_resume_picker_action(
        &mut self,
        _: &OpenResumePicker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.command_palette.open {
            self.open_command_palette(window, cx);
        }
        self.open_command_palette_resume_view(None, cx);
    }

    pub(super) fn open_note_picker_action(
        &mut self,
        _: &OpenNotePicker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.command_palette.open {
            self.open_command_palette(window, cx);
        }
        self.load_notes_from_daemon(Uuid::nil(), cx);
        self.open_command_palette_note_view(cx);
    }

    pub(super) fn open_file_picker_action(
        &mut self,
        _: &OpenFilePicker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.command_palette.open {
            self.open_command_palette(window, cx);
        }
        self.refresh_composer_sources(cx);
        self.open_command_palette_file_view(cx);
    }

    pub(super) fn toggle_command_palette_action(
        &mut self,
        _: &ToggleCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.open {
            self.close_command_palette(window, cx);
        } else {
            self.open_command_palette(window, cx);
        }
    }

    fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open_menus = self
            .menus
            .borrow()
            .values()
            .filter(|menu| menu.is_open())
            .cloned()
            .collect::<Vec<_>>();
        // A focused picker input disappears when its menu closes. Restore to a
        // durable surface instead of remembering that soon-detached handle.
        self.command_palette.previous_focus = if open_menus.is_empty() {
            window.focused(cx)
        } else if self.settings_page.is_some() {
            Some(self.settings_focus.clone())
        } else {
            Some(self.composer_focus(cx))
        };

        self.command_palette.open = true;
        self.command_palette.view = CommandPaletteView::Commands;
        self.command_palette.focus_generation =
            self.command_palette.focus_generation.wrapping_add(1);
        self.command_palette.message_searches.clear();
        self.command_palette.active_message_query = None;
        self.command_palette.message_matches_query = None;
        self.command_palette.message_matches.clear();
        self.command_palette.message_search_pending = false;
        self.command_palette.provider_sessions.clear();
        self.command_palette.provider_sessions_pending = false;
        self.command_palette.provider_session_import = None;
        self.command_palette.provider_session_error = None;
        self.command_palette.provider_session_generation = self
            .command_palette
            .provider_session_generation
            .wrapping_add(1);
        let focus_generation = self.command_palette.focus_generation;
        self.command_palette
            .search
            .update(cx, |input, cx| input.clear(cx));
        self.refresh_command_palette_results("", false, cx);

        // Closing an open GPUI menu can call its toggle observers back into
        // this entity, so release this action listener's mutable borrow first.
        if !open_menus.is_empty() {
            window.defer(cx, move |window, cx| {
                for menu in open_menus {
                    menu.close(window, cx);
                }
            });
        }

        // The palette is deferred onto GPUI's overlay plane. Wait for that
        // subtree to join the dispatch tree before handing focus to its input.
        let focus = self.command_palette.search.read(cx).focus();
        let weak = cx.entity().downgrade();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let mut should_focus = false;
                let _ = weak.update(cx, |this, _| {
                    should_focus = this.command_palette.open
                        && this.command_palette.focus_generation == focus_generation;
                });
                if should_focus {
                    window.focus(&focus, cx);
                }
            });
        });
        cx.notify();
    }

    fn close_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.command_palette.open {
            return;
        }
        self.command_palette.open = false;
        self.command_palette.focus_generation =
            self.command_palette.focus_generation.wrapping_add(1);
        self.command_palette.active_message_query = None;
        self.command_palette.message_search_pending = false;
        self.command_palette.provider_sessions_pending = false;
        self.command_palette.provider_session_import = None;
        self.command_palette.provider_session_generation = self
            .command_palette
            .provider_session_generation
            .wrapping_add(1);
        if let Some(previous_focus) = self.command_palette.previous_focus.take() {
            window.focus(&previous_focus, cx);
        }
        cx.notify();
    }

    fn leave_command_palette_resume_view(&mut self, cx: &mut Context<Self>) {
        self.command_palette.view = CommandPaletteView::Commands;
        self.command_palette.provider_sessions.clear();
        self.command_palette.provider_sessions_pending = false;
        self.command_palette.provider_session_import = None;
        self.command_palette.provider_session_error = None;
        self.command_palette.provider_session_generation = self
            .command_palette
            .provider_session_generation
            .wrapping_add(1);
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();
    }

    fn open_command_palette_note_view(&mut self, cx: &mut Context<Self>) {
        self.command_palette.view = CommandPaletteView::Notes;
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.note_placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();
    }

    fn leave_command_palette_note_view(&mut self, cx: &mut Context<Self>) {
        self.command_palette.view = CommandPaletteView::Commands;
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();
    }

    fn open_command_palette_file_view(&mut self, cx: &mut Context<Self>) {
        self.command_palette.view = CommandPaletteView::FindFile;
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.find_file_placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();
    }

    fn leave_command_palette_file_view(&mut self, cx: &mut Context<Self>) {
        self.command_palette.view = CommandPaletteView::Commands;
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();
    }

    fn leave_command_palette_resume_provider_view(&mut self, cx: &mut Context<Self>) {
        self.command_palette.view = CommandPaletteView::Resume;
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.resume_placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();
    }

    fn dismiss_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.command_palette.view {
            CommandPaletteView::Commands => self.close_command_palette(window, cx),
            CommandPaletteView::Resume => self.leave_command_palette_resume_view(cx),
            CommandPaletteView::ResumeProviders => {
                self.leave_command_palette_resume_provider_view(cx)
            }
            CommandPaletteView::Notes => self.leave_command_palette_note_view(cx),
            CommandPaletteView::FindFile => self.leave_command_palette_file_view(cx),
        }
    }

    pub(super) fn refresh_command_palette_localized_text(&mut self, cx: &mut Context<Self>) {
        let placeholder = match self.command_palette.view {
            CommandPaletteView::Commands => tr!("command_palette.placeholder"),
            CommandPaletteView::Resume => tr!("command_palette.resume_placeholder"),
            CommandPaletteView::ResumeProviders => {
                tr!("command_palette.resume_provider_placeholder")
            }
            CommandPaletteView::Notes => tr!("command_palette.note_placeholder"),
            CommandPaletteView::FindFile => tr!("command_palette.find_file_placeholder"),
        };
        self.command_palette
            .search
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        if self.command_palette.open {
            let query = self.command_palette.search.read(cx).content().to_owned();
            self.refresh_command_palette_results(&query, true, cx);
        }
    }

    pub(super) fn command_palette_query_edited(&mut self, query: &str, cx: &mut Context<Self>) {
        if !self.command_palette.open {
            return;
        }
        if matches!(
            self.command_palette.view,
            CommandPaletteView::Resume
                | CommandPaletteView::ResumeProviders
                | CommandPaletteView::Notes
                | CommandPaletteView::FindFile
        ) {
            self.refresh_command_palette_results(query, false, cx);
            cx.notify();
            return;
        }
        let query = query.trim().to_owned();
        self.command_palette.active_message_query = (!query.is_empty()).then(|| query.clone());
        let fetch = if query.is_empty() {
            self.command_palette.message_matches_query = None;
            self.command_palette.message_matches.clear();
            self.command_palette.message_search_pending = false;
            None
        } else {
            match self.command_palette.message_searches.read(&query) {
                Query::Ready(matches) => {
                    self.command_palette.message_matches_query = Some(query.clone());
                    self.command_palette.message_matches = matches
                        .iter()
                        .cloned()
                        .map(|matched| (matched.session_id, matched))
                        .collect();
                    self.command_palette.message_search_pending = false;
                    None
                }
                Query::Pending => {
                    self.command_palette.message_search_pending = true;
                    None
                }
                Query::Missing(token) => {
                    self.command_palette.message_search_pending = true;
                    Some(token)
                }
            }
        };
        self.refresh_command_palette_results(&query, false, cx);
        cx.notify();

        let Some(token) = fetch else {
            return;
        };
        let search = self
            .store
            .session_message_search(query.clone(), MESSAGE_SEARCH_LIMIT);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(MESSAGE_SEARCH_DEBOUNCE)
                .await;
            let current = this
                .update(cx, |this, _| {
                    this.command_palette.open
                        && this.command_palette.active_message_query.as_deref()
                            == Some(query.as_str())
                })
                .unwrap_or(false);
            if !current {
                let _ = this.update(cx, |this, _| {
                    this.command_palette.message_searches.abandon(token)
                });
                return;
            }

            let matches = cx
                .background_executor()
                .spawn(async move { search().unwrap_or_default() })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this
                    .command_palette
                    .message_searches
                    .fulfill(token, matches)
                {
                    return;
                }
                if !this.command_palette.open
                    || this.command_palette.active_message_query.as_deref() != Some(query.as_str())
                {
                    return;
                }
                let Query::Ready(matches) = this.command_palette.message_searches.read(&query)
                else {
                    return;
                };
                this.command_palette.message_matches_query = Some(query.clone());
                this.command_palette.message_matches = matches
                    .iter()
                    .cloned()
                    .map(|matched| (matched.session_id, matched))
                    .collect();
                this.command_palette.message_search_pending = false;
                this.refresh_command_palette_results(&query, false, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn command_palette_commands(&self, searching: bool) -> Vec<CommandPaletteItem> {
        let display_section = |suggested| {
            if searching {
                PaletteSection::Commands
            } else {
                suggested
            }
        };
        let mut order = 0usize;
        let mut next = || {
            let current = order;
            order += 1;
            current
        };
        let mut commands = vec![
            CommandPaletteItem::command(
                display_section(PaletteSection::Suggested),
                tr!("command_palette.new_task"),
                "icons/pencil.svg",
                Some(crate::platform::primary_shortcut("⌘N", "Ctrl+N")),
                PaletteAction::NewTask,
                "new task session chat conversation start",
                next(),
            ),
            CommandPaletteItem::command(
                display_section(PaletteSection::Suggested),
                tr!("command_palette.resume"),
                "icons/rotate-cw.svg",
                None,
                PaletteAction::Resume,
                "resume continue restore import external terminal cli session conversation",
                next(),
            ),
            CommandPaletteItem::command(
                display_section(PaletteSection::Suggested),
                tr!("command_palette.open_project"),
                "icons/folder.svg",
                Some(crate::platform::primary_shortcut("⌘O", "Ctrl+O")),
                PaletteAction::OpenProject,
                "open add folder project workspace repository repo",
                next(),
            ),
        ];

        let can_choose_model = self
            .selected_session()
            .is_some_and(|session| session.can_choose_model(session.provider));
        if can_choose_model {
            commands.push(CommandPaletteItem::command(
                display_section(PaletteSection::Suggested),
                tr!("command_palette.choose_model"),
                "icons/bot.svg",
                Some(crate::platform::primary_shortcut("⌘/", "Ctrl+/")),
                PaletteAction::ChooseModel,
                "choose change select model provider agent",
                next(),
            ));
        }

        commands.push(CommandPaletteItem::command(
            PaletteSection::Commands,
            tr!("menu.focus_composer"),
            "icons/pencil.svg",
            Some(crate::platform::primary_shortcut("⌘L", "Ctrl+L")),
            PaletteAction::FocusComposer,
            "focus composer prompt input message",
            next(),
        ));
        commands.push(CommandPaletteItem::command(
            PaletteSection::Commands,
            tr!("onboarding.command_title"),
            "icons/sparkle.svg",
            None,
            PaletteAction::OpenOnboarding,
            "onboarding tour welcome help guide intro tutorial features",
            next(),
        ));
        for identifier in PaletteIdentifier::ALL {
            if identifier.value(self.selected_session()).is_some() {
                commands.push(CommandPaletteItem::command(
                    PaletteSection::Commands,
                    identifier.label(),
                    "icons/copy.svg",
                    None,
                    PaletteAction::CopyIdentifier(identifier),
                    identifier.keywords(),
                    next(),
                ));
            }
        }
        if self.usage_meter_available() {
            commands.push(CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!("menu.toggle_usage_panel"),
                "icons/command.svg",
                Some(crate::platform::primary_shortcut("⌘U", "Ctrl+U")),
                PaletteAction::ToggleUsage,
                "toggle usage limits rate quota panel",
                next(),
            ));
        }
        commands.push(CommandPaletteItem::command(
            PaletteSection::Commands,
            tr!("command_palette.collapse_sidebar_groups"),
            "icons/command.svg",
            None,
            PaletteAction::CollapseSidebarGroups,
            "collapse close fold all sidebar groups projects dates history",
            next(),
        ));
        commands.extend([
            CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!(if self.sidebar_visible {
                    "command_palette.hide_sidebar"
                } else {
                    "command_palette.show_sidebar"
                }),
                "icons/panel-left.svg",
                Some(crate::platform::primary_shortcut("⌘B", "Ctrl+B")),
                PaletteAction::ToggleSidebar,
                "toggle show hide left sidebar history tasks",
                next(),
            ),
            CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!(if self.right_panel_visible {
                    "command_palette.hide_right_panel"
                } else {
                    "command_palette.show_right_panel"
                }),
                "icons/panel-right.svg",
                Some(crate::platform::primary_shortcut("⇧⌘B", "Ctrl+Shift+B")),
                PaletteAction::ToggleRightPanel,
                "toggle show hide right panel files diff terminal browser",
                next(),
            ),
        ]);

        if self.right_panel_can_expand() || self.right_panel_fullscreen_active() {
            commands.push(CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!(if self.right_panel_fullscreen_active() {
                    "command_palette.exit_right_panel_fullscreen"
                } else {
                    "command_palette.enter_right_panel_fullscreen"
                }),
                if self.right_panel_fullscreen_active() {
                    "icons/minimize.svg"
                } else {
                    "icons/maximize.svg"
                },
                Some(crate::platform::primary_shortcut("⌘J", "Ctrl+J")),
                PaletteAction::ToggleRightPanelFullscreen,
                "fullscreen expand maximize collapse right panel conversation",
                next(),
            ));
        }

        if self.right_panel_visible || self.right_panel_fullscreen_active() {
            commands.extend([
                CommandPaletteItem::command(
                    PaletteSection::Commands,
                    tr!("command_palette.next_right_panel_tab"),
                    "icons/arrow-right.svg",
                    Some(crate::platform::primary_shortcut("⌥⌘→", "Ctrl+Alt+Right")),
                    PaletteAction::NextRightPanelTab,
                    "next right panel tab conversation browser terminal files review",
                    next(),
                ),
                CommandPaletteItem::command(
                    PaletteSection::Commands,
                    tr!("command_palette.prev_right_panel_tab"),
                    "icons/arrow-left.svg",
                    Some(crate::platform::primary_shortcut("⌥⌘←", "Ctrl+Alt+Left")),
                    PaletteAction::PrevRightPanelTab,
                    "previous right panel tab conversation browser terminal files review",
                    next(),
                ),
            ]);
        }

        commands.extend([
            CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!("command_palette.open_browser"),
                "icons/globe.svg",
                Some(crate::platform::primary_shortcut("⌥⌘B", "Ctrl+Alt+B")),
                PaletteAction::OpenBrowser,
                "browser web webview open right panel",
                next(),
            ),
            CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!("command_palette.previous_session"),
                "icons/arrow-up.svg",
                Some(crate::platform::primary_shortcut("⌥⌘↑", "Ctrl+Alt+Up")),
                PaletteAction::SelectPreviousTask,
                "previous conversation topic session switch navigate prev prior chat",
                next(),
            ),
            CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!("command_palette.next_session"),
                "icons/arrow-down.svg",
                Some(crate::platform::primary_shortcut("⌥⌘↓", "Ctrl+Alt+Down")),
                PaletteAction::SelectNextTask,
                "next conversation topic session switch navigate forward chat",
                next(),
            ),
            CommandPaletteItem::command(
                PaletteSection::Commands,
                tr!("command_palette.open_terminal"),
                "icons/terminal.svg",
                Some(crate::platform::primary_shortcut("⌘T", "Ctrl+T")),
                PaletteAction::OpenTerminal,
                "terminal shell console bash zsh open right panel",
                next(),
            ),
        ]);

        if self.active_project().is_some() {
            commands.extend([
                CommandPaletteItem::command(
                    PaletteSection::Commands,
                    tr!("command_palette.open_files"),
                    "icons/folder.svg",
                    Some(crate::platform::primary_shortcut("⇧⌘E", "Ctrl+Shift+E")),
                    PaletteAction::OpenFiles,
                    "files explorer tree workspace open right panel",
                    next(),
                ),
                CommandPaletteItem::command(
                    PaletteSection::Commands,
                    tr!("command_palette.find_file"),
                    "icons/search.svg",
                    Some(crate::platform::primary_shortcut("⌘P", "Ctrl+P")),
                    PaletteAction::FindFile,
                    "find search file path workspace project source",
                    next(),
                ),
                CommandPaletteItem::command(
                    PaletteSection::Commands,
                    tr!("command_palette.open_review"),
                    "icons/file-diff.svg",
                    Some(crate::platform::primary_shortcut("⌘D", "Ctrl+D")),
                    PaletteAction::OpenReview,
                    "review diff changes git open right panel",
                    next(),
                ),
            ]);
        }

        commands.push(CommandPaletteItem::command(
            PaletteSection::Commands,
            tr!("settings.notes"),
            "icons/package.svg",
            Some(crate::platform::primary_shortcut("⌘⇧N", "Ctrl+Shift+N")),
            PaletteAction::OpenNotes,
            "notes markdown documents writing snippets memos",
            next(),
        ));

        for (page, label_key, icon, keywords) in [
            (
                SettingsPage::General,
                "settings.general",
                "icons/settings.svg",
                "settings preferences general local privacy updates",
            ),
            (
                SettingsPage::Appearance,
                "settings.appearance",
                "icons/appearance.svg",
                "settings preferences appearance theme language light dark",
            ),
            (
                SettingsPage::Keybindings,
                "settings.keybindings",
                "icons/command.svg",
                "settings preferences keybindings keyboard shortcuts hotkeys bindings keys",
            ),
            (
                SettingsPage::Providers,
                "settings.providers",
                "icons/bot.svg",
                "settings preferences providers agents models cli",
            ),
            (
                SettingsPage::Skills,
                "settings.skills",
                "icons/package.svg",
                "settings preferences skills library create disable agent skill",
            ),
            (
                SettingsPage::Usage,
                "settings.usage",
                "icons/chart-column.svg",
                "settings preferences usage tokens cost history",
            ),
            (
                SettingsPage::Archived,
                "settings.archived",
                "icons/package.svg",
                "settings preferences archived archive hidden conversations restore",
            ),
            (
                SettingsPage::Daemon,
                "settings.daemon",
                "icons/server.svg",
                "settings preferences daemon server remote web network origin token port",
            ),
            (
                SettingsPage::ComputerUse,
                "settings.computer_use",
                "icons/cursor-spark.svg",
                "settings preferences computer use accessibility screen recording",
            ),
            (
                SettingsPage::About,
                "settings.about",
                "icons/info.svg",
                "settings preferences about info version update host connected github sponsor",
            ),
        ] {
            if !page.is_visible_in_navigation() {
                continue;
            }
            commands.push(CommandPaletteItem::command(
                PaletteSection::Settings,
                crate::i18n::translate(label_key),
                icon,
                (page == SettingsPage::General)
                    .then_some(crate::platform::primary_shortcut("⌘,", "Ctrl+,")),
                PaletteAction::OpenSettings(page),
                keywords,
                next(),
            ));
        }
        commands
    }

    fn command_palette_task_candidates(&self) -> Vec<CommandPaletteItem> {
        let projects = self
            .state
            .projects
            .iter()
            .map(|project| {
                (
                    project.id,
                    (
                        project.display_name(),
                        project.path.to_string_lossy().into_owned(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        self.state
            .sessions
            .iter()
            .filter(|session| session.has_started() && session.archived_at.is_none())
            .enumerate()
            .map(|(order, session)| {
                let (project, project_path) = projects
                    .get(&session.project_id)
                    .cloned()
                    .unwrap_or_else(|| (tr!("project.no_project_name"), String::new()));
                let (workspace_path, branch) = match &session.workspace {
                    SessionWorkspace::Local => (String::new(), None),
                    SessionWorkspace::NewWorktree { base_branch } => {
                        (String::new(), base_branch.as_deref())
                    }
                    SessionWorkspace::Worktree { path, branch } => {
                        (path.to_string_lossy().into_owned(), Some(branch.as_str()))
                    }
                };
                let mut details = vec![project.clone()];
                if let Some(branch) = branch {
                    details.push(format!("#{branch}"));
                }
                if Some(session.id) == self.state.selected_session {
                    details.push(tr!("command_palette.current"));
                }
                let detail = details.join(" · ");
                let label = session.display_title().to_owned();
                let content_match = self
                    .command_palette
                    .message_matches
                    .get(&session.id)
                    .cloned();
                CommandPaletteItem {
                    section: PaletteSection::Tasks,
                    search_text: format!(
                        "{label} {project} {project_path} {workspace_path} {} {} {} {} task session chat conversation",
                        branch.unwrap_or_default(),
                        session.provider.short_name(),
                        session.provider.display_name(),
                        session.model.as_deref().unwrap_or_default(),
                    ),
                    label,
                    detail: Some(detail),
                    icon: PaletteIcon::Provider(session.provider),
                    shortcut: None,
                    action: PaletteAction::SelectTask(session.id),
                    content_match,
                    order,
                    recency: session.updated_at,
                }
            })
            .collect()
    }

    fn command_palette_resume_candidates(&self) -> Vec<CommandPaletteItem> {
        let now = unix_time();
        self.command_palette
            .provider_sessions
            .iter()
            .filter(|native| {
                !self.state.sessions.iter().any(|session| {
                    session.provider_cursor.as_ref().is_some_and(|cursor| {
                        cursor.provider() == native.provider()
                            && cursor.native_id() == native.cursor.native_id()
                    })
                })
            })
            .enumerate()
            .map(|(order, native)| {
                let provider = native.provider();
                let age = super::sidebar::format_time_ago(now.saturating_sub(native.updated_at));
                let path = native.cwd.to_string_lossy();
                CommandPaletteItem {
                    section: PaletteSection::Sessions,
                    label: native.title.clone(),
                    detail: Some(format!("{} · {} · {age}", provider.short_name(), path)),
                    icon: PaletteIcon::Provider(provider),
                    shortcut: None,
                    action: PaletteAction::ResumeProviderSession(native.clone()),
                    content_match: None,
                    search_text: format!(
                        "{} {} {} {} {} resume continue terminal cli session conversation",
                        native.title,
                        path,
                        provider.short_name(),
                        provider.display_name(),
                        native.cursor.native_id(),
                    ),
                    order,
                    recency: native.updated_at,
                }
            })
            .collect()
    }

    fn command_palette_resume_provider_selector(&self) -> CommandPaletteItem {
        let provider = self.command_palette.resume_provider;
        CommandPaletteItem {
            section: PaletteSection::Providers,
            label: provider.display_name().to_owned(),
            detail: Some(tr!("command_palette.change_provider")),
            icon: PaletteIcon::Provider(provider),
            shortcut: None,
            action: PaletteAction::ChooseResumeProvider,
            content_match: None,
            search_text: format!(
                "{} {} change select provider agent cli",
                provider.short_name(),
                provider.display_name()
            ),
            order: 0,
            recency: u64::MAX,
        }
    }

    fn command_palette_resume_provider_candidates(&self) -> Vec<CommandPaletteItem> {
        ProviderKind::ALL
            .into_iter()
            .filter(|provider| {
                *provider == self.command_palette.resume_provider
                    || !self.state.disabled_providers.contains(provider)
            })
            .enumerate()
            .map(|(order, provider)| CommandPaletteItem {
                section: PaletteSection::Providers,
                label: provider.display_name().to_owned(),
                detail: (provider == self.command_palette.resume_provider)
                    .then(|| tr!("command_palette.current_provider")),
                icon: PaletteIcon::Provider(provider),
                shortcut: None,
                action: PaletteAction::SelectResumeProvider(provider),
                content_match: None,
                search_text: format!(
                    "{} {} provider agent cli terminal session",
                    provider.short_name(),
                    provider.display_name()
                ),
                order,
                recency: 0,
            })
            .collect()
    }

    fn refresh_command_palette_resume_results(&mut self, query: &str, preserve_selection: bool) {
        let query = query.trim();
        let selected_action = preserve_selection.then(|| {
            self.command_palette
                .results
                .get(self.command_palette.selected)
                .map(|item| item.action.clone())
        });
        let mut candidates = self.command_palette_resume_candidates();
        if query.is_empty() {
            candidates.sort_by(|a, b| b.recency.cmp(&a.recency).then(a.order.cmp(&b.order)));
            candidates.truncate(MAX_RESUME_RESULTS);
            self.command_palette.results = candidates;
        } else {
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let mut utf32 = Vec::new();
            let mut scored = candidates
                .into_iter()
                .filter_map(|item| {
                    pattern
                        .score(
                            Utf32Str::new(&item.search_text, &mut utf32),
                            &mut self.command_palette.matcher,
                        )
                        .map(|score| ScoredPaletteItem { score, item })
                })
                .collect::<Vec<_>>();
            scored.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then(b.item.recency.cmp(&a.item.recency))
                    .then(a.item.order.cmp(&b.item.order))
            });
            scored.truncate(MAX_RESUME_RESULTS);
            self.command_palette.results = scored.into_iter().map(|scored| scored.item).collect();
        }
        let provider_selector = self.command_palette_resume_provider_selector();
        self.command_palette.results.insert(0, provider_selector);
        self.command_palette.selected = selected_action
            .flatten()
            .and_then(|action| {
                self.command_palette
                    .results
                    .iter()
                    .position(|item| item.action == action)
            })
            .unwrap_or_else(|| usize::from(self.command_palette.results.len() > 1));
        let scroll_index = self.command_palette_scroll_index(self.command_palette.selected);
        self.command_palette.scroll.scroll_to_item(scroll_index);
    }

    fn refresh_command_palette_resume_provider_results(
        &mut self,
        query: &str,
        preserve_selection: bool,
    ) {
        let selected_action = preserve_selection.then(|| {
            self.command_palette
                .results
                .get(self.command_palette.selected)
                .map(|item| item.action.clone())
        });
        let query = query.trim();
        let mut candidates = self.command_palette_resume_provider_candidates();
        if !query.is_empty() {
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let mut utf32 = Vec::new();
            let mut scored = candidates
                .into_iter()
                .filter_map(|item| {
                    pattern
                        .score(
                            Utf32Str::new(&item.search_text, &mut utf32),
                            &mut self.command_palette.matcher,
                        )
                        .map(|score| ScoredPaletteItem { score, item })
                })
                .collect::<Vec<_>>();
            scored.sort_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then(left.item.order.cmp(&right.item.order))
            });
            candidates = scored.into_iter().map(|scored| scored.item).collect();
        }
        self.command_palette.results = candidates;
        self.command_palette.selected = selected_action
            .flatten()
            .and_then(|action| {
                self.command_palette
                    .results
                    .iter()
                    .position(|item| item.action == action)
            })
            .or_else(|| {
                self.command_palette.results.iter().position(|item| {
                    item.action
                        == PaletteAction::SelectResumeProvider(self.command_palette.resume_provider)
                })
            })
            .unwrap_or(0);
        self.command_palette
            .scroll
            .scroll_to_item(self.command_palette_scroll_index(self.command_palette.selected));
    }

    fn command_palette_note_candidates(&self) -> Vec<CommandPaletteItem> {
        self.notes
            .iter()
            .enumerate()
            .map(|(order, note)| CommandPaletteItem {
                section: PaletteSection::Commands,
                label: if note.title.is_empty() {
                    tr!("notes.untitled")
                } else {
                    note.title.clone()
                },
                detail: Some(note.body.lines().next().unwrap_or_default().to_owned()),
                icon: PaletteIcon::Asset("icons/file.svg"),
                shortcut: None,
                action: PaletteAction::EmbedNote(note.clone()),
                content_match: None,
                search_text: format!("{} {} {}", note.title, note.body, note.id),
                order,
                recency: note.updated_at,
            })
            .collect()
    }

    fn refresh_command_palette_note_results(&mut self, query: &str, preserve_selection: bool) {
        let selected_action = preserve_selection.then(|| {
            self.command_palette
                .results
                .get(self.command_palette.selected)
                .map(|item| item.action.clone())
        });
        let candidates = self.command_palette_note_candidates();
        let query = query.trim();
        let mut results = if query.is_empty() {
            candidates
        } else {
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let mut utf32 = Vec::new();
            let mut scored = candidates
                .into_iter()
                .filter_map(|item| {
                    pattern
                        .score(
                            Utf32Str::new(&item.search_text, &mut utf32),
                            &mut self.command_palette.matcher,
                        )
                        .map(|score| ScoredPaletteItem { score, item })
                })
                .collect::<Vec<_>>();
            scored.sort_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then(left.item.order.cmp(&right.item.order))
            });
            scored.into_iter().map(|scored| scored.item).collect()
        };
        results.sort_by(|left, right| {
            right
                .recency
                .cmp(&left.recency)
                .then(left.order.cmp(&right.order))
        });
        self.command_palette.results = results;
        self.command_palette.selected = selected_action
            .flatten()
            .and_then(|action| {
                self.command_palette
                    .results
                    .iter()
                    .position(|item| item.action == action)
            })
            .unwrap_or(0);
        self.command_palette
            .scroll
            .scroll_to_item(self.command_palette.selected);
    }

    fn command_palette_file_candidates(&mut self, query: &str) -> Vec<CommandPaletteItem> {
        crate::composer_complete::filter_files(
            &self.mention_file_index,
            query.trim(),
            &mut self.command_palette.matcher,
        )
        .into_iter()
        .filter(|file| !file.item.is_dir)
        .enumerate()
        .map(|(order, file)| {
            let path = file.item.path;
            let label = path.rsplit('/').next().unwrap_or(&path).to_owned();
            let detail = path
                .strip_suffix(&label)
                .map(|value| value.trim_end_matches('/'))
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            CommandPaletteItem {
                section: PaletteSection::Commands,
                label,
                detail,
                icon: PaletteIcon::File(crate::app::right_panel::file_icon_for_path(&path)),
                shortcut: None,
                action: PaletteAction::OpenFile(path.clone()),
                content_match: None,
                search_text: path,
                order,
                recency: 0,
            }
        })
        .collect()
    }

    pub(super) fn refresh_command_palette_results(
        &mut self,
        query: &str,
        preserve_selection: bool,
        _cx: &App,
    ) {
        match self.command_palette.view {
            CommandPaletteView::Resume => {
                self.refresh_command_palette_resume_results(query, preserve_selection);
                return;
            }
            CommandPaletteView::ResumeProviders => {
                self.refresh_command_palette_resume_provider_results(query, preserve_selection);
                return;
            }
            CommandPaletteView::Notes => {
                self.refresh_command_palette_note_results(query, preserve_selection);
                return;
            }
            CommandPaletteView::FindFile => {
                let selected_action = preserve_selection.then(|| {
                    self.command_palette
                        .results
                        .get(self.command_palette.selected)
                        .map(|item| item.action.clone())
                });
                self.command_palette.results = self.command_palette_file_candidates(query);
                self.command_palette.selected = selected_action
                    .flatten()
                    .and_then(|action| {
                        self.command_palette
                            .results
                            .iter()
                            .position(|item| item.action == action)
                    })
                    .unwrap_or(0);
                self.command_palette
                    .scroll
                    .scroll_to_item(self.command_palette.selected);
                return;
            }
            CommandPaletteView::Commands => {}
        }
        let query = query.trim();
        if query.is_empty() {
            self.command_palette.results = self.command_palette_commands(false);
            self.command_palette.selected = 0;
            self.command_palette.scroll.scroll_to_item(0);
            return;
        }

        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let message_matches_are_current =
            self.command_palette.message_matches_query.as_deref() == Some(query);
        let mut utf32 = Vec::new();
        let mut tasks = self
            .command_palette_task_candidates()
            .into_iter()
            .filter_map(|mut item| {
                let metadata_score = pattern.score(
                    Utf32Str::new(&item.search_text, &mut utf32),
                    &mut self.command_palette.matcher,
                );
                let content_score = item.content_match.as_ref().and_then(|matched| {
                    pattern.score(
                        Utf32Str::new(&matched.snippet, &mut utf32),
                        &mut self.command_palette.matcher,
                    )
                });
                let current_content_match =
                    message_matches_are_current && item.content_match.is_some();
                if content_score.is_none() && !current_content_match {
                    // Pending queries retain the last resolved transcript
                    // snapshot, but an unrelated old excerpt should neither
                    // keep a row alive nor be displayed under the new query.
                    item.content_match = None;
                }
                metadata_score
                    .into_iter()
                    .chain(content_score)
                    .max()
                    .or_else(|| current_content_match.then_some(0))
                    .map(|score| ScoredPaletteItem { score, item })
            })
            .collect::<Vec<_>>();
        tasks.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(b.item.recency.cmp(&a.item.recency))
                .then(a.item.order.cmp(&b.item.order))
        });
        tasks.truncate(MAX_TASK_RESULTS);

        let mut commands = self
            .command_palette_commands(true)
            .into_iter()
            .filter_map(|item| {
                pattern
                    .score(
                        Utf32Str::new(&item.search_text, &mut utf32),
                        &mut self.command_palette.matcher,
                    )
                    .map(|score| ScoredPaletteItem { score, item })
            })
            .collect::<Vec<_>>();
        commands.sort_by(|a, b| {
            a.item
                .section
                .query_rank()
                .cmp(&b.item.section.query_rank())
                .then(b.score.cmp(&a.score))
                .then(a.item.order.cmp(&b.item.order))
        });

        let selected_action = preserve_selection.then(|| {
            self.command_palette
                .results
                .get(self.command_palette.selected)
                .map(|item| item.action.clone())
        });
        let mut scored_results = tasks;
        scored_results.extend(commands);
        // Stable sorting keeps each section's existing score/recency order.
        scored_results.sort_by_key(|scored| scored.item.section.query_rank());
        let next_results = scored_results
            .into_iter()
            .map(|scored| scored.item)
            .collect::<Vec<_>>();
        // This is the palette equivalent of TanStack Query's
        // `keepPreviousData`: never replace useful rows with a transient blank
        // frame while the transcript query is still in flight.
        if should_keep_previous_command_palette_results(
            next_results.len(),
            self.command_palette.message_search_pending,
            self.command_palette.results.len(),
        ) {
            self.command_palette.selected = 0;
            self.command_palette.scroll.scroll_to_item(0);
            return;
        }
        self.command_palette.results = next_results;
        self.command_palette.selected = selected_action
            .flatten()
            .and_then(|action| {
                self.command_palette
                    .results
                    .iter()
                    .position(|item| item.action == action)
            })
            .unwrap_or(0);
        let scroll_index = self.command_palette_scroll_index(self.command_palette.selected);
        self.command_palette.scroll.scroll_to_item(scroll_index);
    }

    fn command_palette_scroll_index(&self, selected: usize) -> usize {
        let mut headers = 0;
        let mut previous = None;
        for item in self.command_palette.results.iter().take(selected + 1) {
            if previous != Some(item.section) {
                if item.section != PaletteSection::Providers {
                    headers += 1;
                }
                previous = Some(item.section);
            }
        }
        selected + headers
    }

    fn move_command_palette_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.command_palette.results.len();
        let Some(next) = next_selection_index(self.command_palette.selected, len, delta) else {
            return;
        };
        self.command_palette.selected = next;
        self.command_palette
            .scroll
            .scroll_to_item(self.command_palette_scroll_index(next));
        cx.notify();
    }

    fn set_command_palette_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.command_palette.results.len() && self.command_palette.selected != index {
            self.command_palette.selected = index;
            cx.notify();
        }
    }

    fn default_resume_provider(&self) -> ProviderKind {
        self.selected_session()
            .map(|session| session.provider)
            .or_else(|| {
                ProviderKind::ALL
                    .into_iter()
                    .find(|provider| !self.state.disabled_providers.contains(provider))
            })
            .unwrap_or_default()
    }

    fn open_command_palette_resume_provider_view(&mut self, cx: &mut Context<Self>) {
        self.command_palette.view = CommandPaletteView::ResumeProviders;
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.resume_provider_placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();
    }

    fn open_command_palette_resume_view(
        &mut self,
        provider: Option<ProviderKind>,
        cx: &mut Context<Self>,
    ) {
        let provider = provider.unwrap_or_else(|| self.default_resume_provider());
        self.command_palette.view = CommandPaletteView::Resume;
        self.command_palette.resume_provider = provider;
        self.command_palette.provider_sessions.clear();
        self.command_palette.provider_sessions_pending = true;
        self.command_palette.provider_session_import = None;
        self.command_palette.provider_session_error = None;
        self.command_palette.provider_session_generation = self
            .command_palette
            .provider_session_generation
            .wrapping_add(1);
        let generation = self.command_palette.provider_session_generation;
        self.command_palette.search.update(cx, |input, cx| {
            input.set_placeholder(tr!("command_palette.resume_placeholder"), cx);
            input.clear(cx);
        });
        self.refresh_command_palette_results("", false, cx);
        cx.notify();

        let fetch = self
            .store
            .provider_sessions(provider, PROVIDER_SESSION_CATALOG_LIMIT);
        cx.spawn(async move |padu, cx| {
            let result = cx.background_executor().spawn(async move { fetch() }).await;
            let _ = padu.update(cx, |padu, cx| {
                if !padu.command_palette.open
                    || padu.command_palette.provider_session_generation != generation
                    || padu.command_palette.resume_provider != provider
                {
                    return;
                }
                padu.command_palette.provider_sessions_pending = false;
                match result {
                    Ok(sessions) => {
                        padu.command_palette.provider_sessions = sessions;
                        padu.command_palette.provider_session_error = None;
                    }
                    Err(error) => {
                        padu.command_palette.provider_sessions.clear();
                        padu.command_palette.provider_session_error = Some(error.to_string());
                    }
                }
                if padu.command_palette.view == CommandPaletteView::Resume {
                    let query = padu.command_palette.search.read(cx).content().to_owned();
                    padu.refresh_command_palette_results(&query, false, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn import_provider_session(
        &mut self,
        summary: ProviderSessionSummary,
        history: ProviderSessionHistory,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(session_id) = self
            .state
            .sessions
            .iter()
            .find(|session| {
                session
                    .provider_cursor
                    .as_ref()
                    .is_some_and(|cursor| same_provider_session(cursor, &summary.cursor))
            })
            .map(|session| session.id)
        {
            self.close_command_palette(window, cx);
            self.settings_page = None;
            self.select_session(session_id, cx);
            let focus = self.composer_focus(cx);
            window.focus(&focus, cx);
            return;
        }

        let project_id = if let Some(project) = self
            .state
            .projects
            .iter()
            .find(|project| project.path == summary.cwd)
        {
            project.id
        } else {
            let project = Project::from_path(summary.cwd.clone());
            let project_id = project.id;
            self.state.projects.push(project);
            self.analytics.track(crate::analytics::Event::ProjectAdded);
            project_id
        };
        let provider = summary.provider();
        let runtime_mode = self
            .selected_session()
            .map(|session| session.runtime_mode)
            .unwrap_or(self.state.last_runtime_mode);
        let now = unix_time();
        let created_at = if summary.created_at == 0 {
            now
        } else {
            summary.created_at
        };
        let updated_at = summary.updated_at.max(created_at);
        let has_history = !history.messages.is_empty() || !history.turns.is_empty();
        let mut session = AgentSession::new(project_id, provider);
        session.runtime_mode = runtime_mode;
        session.auto_title = Some(summary.title);
        session.provider_cursor = Some(summary.cursor);
        session.created_at = created_at;
        session.updated_at = updated_at;
        session.last_reply_at = has_history.then_some(updated_at);
        session.messages = history.messages;
        session.turns = history.turns;
        let session_id = session.id;
        self.state.push_session(session);

        self.close_command_palette(window, cx);
        self.settings_page = None;
        self.select_session(session_id, cx);
        let focus = self.composer_focus(cx);
        window.focus(&focus, cx);
    }

    fn load_command_palette_provider_session(
        &mut self,
        summary: ProviderSessionSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.provider_session_import.is_some() {
            return;
        }
        if let Some(session_id) = self
            .state
            .sessions
            .iter()
            .find(|session| {
                session
                    .provider_cursor
                    .as_ref()
                    .is_some_and(|cursor| same_provider_session(cursor, &summary.cursor))
            })
            .map(|session| session.id)
        {
            self.close_command_palette(window, cx);
            self.settings_page = None;
            self.select_session(session_id, cx);
            let focus = self.composer_focus(cx);
            window.focus(&focus, cx);
            return;
        }

        self.command_palette.provider_session_import = Some(summary.cursor.clone());
        self.command_palette.provider_session_error = None;
        self.command_palette.provider_session_generation = self
            .command_palette
            .provider_session_generation
            .wrapping_add(1);
        let generation = self.command_palette.provider_session_generation;
        let cursor = summary.cursor.clone();
        let fetch = self
            .store
            .provider_session_history(cursor.clone(), summary.cwd.clone());
        let window_handle = window.window_handle();
        cx.notify();

        cx.spawn(async move |padu, cx| {
            let result = cx.background_executor().spawn(async move { fetch() }).await;
            let update = padu.update(cx, |padu, cx| {
                if !padu.command_palette.open
                    || padu.command_palette.view != CommandPaletteView::Resume
                    || padu.command_palette.provider_session_generation != generation
                    || padu
                        .command_palette
                        .provider_session_import
                        .as_ref()
                        .is_none_or(|pending| !same_provider_session(pending, &cursor))
                {
                    return None;
                }
                match result {
                    Ok(history) => Some((summary, history)),
                    Err(error) => {
                        let error = error.to_string();
                        padu.command_palette.provider_session_import = None;
                        padu.command_palette.provider_session_error = Some(error.clone());
                        padu.show_toast(tr!("command_palette.resume_failed", error = error));
                        cx.notify();
                        None
                    }
                }
            });
            let Ok(Some((summary, history))) = update else {
                return;
            };
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = padu.update(cx, |padu, cx| {
                    if padu.command_palette.open
                        && padu.command_palette.view == CommandPaletteView::Resume
                        && padu.command_palette.provider_session_generation == generation
                    {
                        padu.import_provider_session(summary, history, window, cx);
                    }
                });
            });
        })
        .detach();
    }

    fn execute_command_palette_selection(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.unwrap_or(self.command_palette.selected);
        let Some(action) = self
            .command_palette
            .results
            .get(index)
            .map(|item| item.action.clone())
        else {
            return;
        };

        match action {
            PaletteAction::Resume => {
                self.open_command_palette_resume_view(None, cx);
                return;
            }
            PaletteAction::ChooseResumeProvider => {
                self.open_command_palette_resume_provider_view(cx);
                return;
            }
            PaletteAction::SelectResumeProvider(provider) => {
                self.open_command_palette_resume_view(Some(provider), cx);
                return;
            }
            PaletteAction::ResumeProviderSession(summary) => {
                self.load_command_palette_provider_session(summary, window, cx);
                return;
            }
            PaletteAction::FindFile => {
                self.open_command_palette_file_view(cx);
                return;
            }
            PaletteAction::EmbedNote(note) => {
                self.composer_embedded_notes
                    .push(padu_protocol::notes::EmbeddedNote {
                        id: note.id,
                        title: note.title,
                        content: note.body,
                        revision: note.revision,
                    });
                self.close_command_palette(window, cx);
                let focus = self.composer_focus(cx);
                window.focus(&focus, cx);
                self.schedule_composer_draft_save(cx);
                cx.notify();
                return;
            }
            _ => {}
        }

        self.close_command_palette(window, cx);
        match action {
            PaletteAction::NewTask => self.new_session_action(&NewSession, window, cx),
            PaletteAction::OpenProject => self.new_project_action(&NewProject, window, cx),
            PaletteAction::FocusComposer => self.focus_composer_action(&FocusComposer, window, cx),
            PaletteAction::CopyIdentifier(identifier) => {
                if let Some(value) = identifier.value(self.selected_session()) {
                    cx.write_to_clipboard(ClipboardItem::new_string(value));
                    self.show_success_toast(identifier.copied_message());
                    cx.notify();
                }
            }
            PaletteAction::CollapseSidebarGroups => self.collapse_all_sidebar_groups(cx),
            PaletteAction::ToggleSidebar => self.toggle_sidebar_action(&ToggleSidebar, window, cx),
            PaletteAction::ToggleRightPanel => {
                self.toggle_right_panel_action(&ToggleRightPanel, window, cx)
            }
            PaletteAction::ToggleRightPanelFullscreen => {
                self.toggle_right_panel_fullscreen_action(&ToggleRightPanelFullscreen, window, cx)
            }
            PaletteAction::NextRightPanelTab => {
                self.next_right_panel_tab_action(&NextRightPanelTab, window, cx)
            }
            PaletteAction::PrevRightPanelTab => {
                self.prev_right_panel_tab_action(&PrevRightPanelTab, window, cx)
            }
            PaletteAction::OpenBrowser => self.open_browser_action(&OpenBrowser, window, cx),
            PaletteAction::OpenTerminal => self.open_terminal_action(&OpenTerminal, window, cx),
            PaletteAction::OpenFiles => self.open_files_action(&OpenFiles, window, cx),
            PaletteAction::OpenFile(path) => self.open_right_panel_file(path, cx),
            PaletteAction::OpenReview => self.open_review_action(&OpenReview, window, cx),
            PaletteAction::OpenNotes => self.open_notes(cx),
            PaletteAction::OpenSettings(page) => {
                self.open_settings_action(&OpenSettings, window, cx);
                self.open_settings_page(page, cx);
            }
            PaletteAction::OpenOnboarding => {
                self.settings_page = None;
                self.open_onboarding(window, cx);
            }
            PaletteAction::SelectTask(session_id) => {
                self.settings_page = None;
                self.select_session(session_id, cx);
                let focus = self.composer_focus(cx);
                window.focus(&focus, cx);
            }
            PaletteAction::SelectPreviousTask => {
                self.settings_page = None;
                self.select_adjacent_sidebar_session(self.state.selected_session, -1, cx);
                let focus = self.composer_focus(cx);
                window.focus(&focus, cx);
            }
            PaletteAction::SelectNextTask => {
                self.settings_page = None;
                self.select_adjacent_sidebar_session(self.state.selected_session, 1, cx);
                let focus = self.composer_focus(cx);
                window.focus(&focus, cx);
            }
            PaletteAction::ChooseModel | PaletteAction::ToggleUsage => {
                // These popovers are rendered by the composer. If the command
                // came from Settings, reveal one normal app frame first so its
                // persistent menu handle and anchor bounds are current.
                self.settings_page = None;
                let focus = self.composer_focus(cx);
                window.focus(&focus, cx);
                let weak = cx.entity().downgrade();
                let choose_model = matches!(action, PaletteAction::ChooseModel);
                window.on_next_frame(move |window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if choose_model {
                            this.toggle_model_picker_action(&ToggleModelPicker, window, cx)
                        } else {
                            this.toggle_usage_panel_action(&ToggleUsagePanel, window, cx)
                        }
                    });
                });
            }
            PaletteAction::Resume
            | PaletteAction::ChooseResumeProvider
            | PaletteAction::SelectResumeProvider(_)
            | PaletteAction::ResumeProviderSession(_)
            | PaletteAction::EmbedNote(_)
            | PaletteAction::FindFile => {
                unreachable!("resume actions are handled before closing the palette")
            }
        }
    }

    pub(super) fn render_command_palette(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.command_palette.open {
            return None;
        }
        let theme = Theme::current(cx);
        let viewport_height = f32::from(window.viewport_size().height);
        let top = (viewport_height * 0.09).clamp(48.0, 72.0);
        let card_max_height = (viewport_height - top - 36.0)
            .max(SEARCH_ROW_HEIGHT)
            .min(MAX_CARD_HEIGHT);
        let selected = self
            .command_palette
            .selected
            .min(self.command_palette.results.len().saturating_sub(1));
        let search_query = self.command_palette.search.read(cx).content().to_owned();
        let resume_view = self.command_palette.view == CommandPaletteView::Resume;
        let file_view = self.command_palette.view == CommandPaletteView::FindFile;
        let resume_session_count = self
            .command_palette
            .results
            .iter()
            .filter(|item| matches!(&item.action, PaletteAction::ResumeProviderSession(_)))
            .count();
        let results_pending = match self.command_palette.view {
            CommandPaletteView::Resume => self.command_palette.provider_sessions_pending,
            CommandPaletteView::Commands => self.command_palette.message_search_pending,
            CommandPaletteView::ResumeProviders | CommandPaletteView::Notes => false,
            CommandPaletteView::FindFile => self.mention_file_index_loading,
        };
        let show_empty_state = should_show_command_palette_empty_state(
            if resume_view {
                resume_session_count
            } else {
                self.command_palette.results.len()
            },
            results_pending,
        );
        let show_loading_state = (resume_view
            && resume_session_count == 0
            && self.command_palette.provider_sessions_pending)
            || (file_view
                && self.command_palette.results.is_empty()
                && self.mention_file_index_loading);
        let show_placeholder_state = show_empty_state || show_loading_state;
        let results_height =
            command_palette_results_height(&self.command_palette.results, show_placeholder_state)
                .min((card_max_height - SEARCH_ROW_HEIGHT - FOOTER_HEIGHT).max(0.0));
        let card_height = SEARCH_ROW_HEIGHT + results_height + FOOTER_HEIGHT;

        let mut results = div()
            .id("command-palette-results")
            .h(px(results_height))
            .flex_none()
            .overflow_y_scroll()
            .track_scroll(&self.command_palette.scroll)
            .px(px(6.0))
            .pb(px(6.0));

        if show_placeholder_state {
            let error = resume_view
                .then(|| self.command_palette.provider_session_error.clone())
                .flatten();
            let (icon_path, title, hint, spinning) = if show_loading_state {
                (
                    "icons/loader-circle.svg",
                    if file_view {
                        tr!("command_palette.loading_files")
                    } else {
                        tr!("command_palette.loading_sessions")
                    },
                    None,
                    true,
                )
            } else if let Some(error) = error {
                (
                    "icons/alert.svg",
                    tr!("command_palette.could_not_load_sessions"),
                    Some(error),
                    false,
                )
            } else if resume_view {
                (
                    "icons/search.svg",
                    tr!("command_palette.no_resume_sessions"),
                    Some(tr!("command_palette.no_resume_sessions_hint")),
                    false,
                )
            } else if file_view {
                (
                    "icons/search.svg",
                    tr!("command_palette.no_files"),
                    Some(tr!("command_palette.no_files_hint")),
                    false,
                )
            } else {
                (
                    "icons/search.svg",
                    tr!("command_palette.no_results"),
                    Some(tr!("command_palette.no_results_hint")),
                    false,
                )
            };
            let empty_icon = icon(icon_path, 16.0, theme.text_ghost);
            results = results.child(
                div()
                    .h(px(EMPTY_RESULTS_HEIGHT))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(if spinning {
                        motion::spin(empty_icon)
                    } else {
                        empty_icon.into_any_element()
                    })
                    .child(
                        div()
                            .mt(px(10.0))
                            .text_size(sp(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_secondary)
                            .child(title),
                    )
                    .when_some(hint, |empty, hint| {
                        empty.child(
                            div()
                                .max_w(px(520.0))
                                .mt(px(4.0))
                                .text_size(sp(12.0))
                                .text_color(theme.text_tertiary)
                                .child(hint),
                        )
                    })
                    .when(show_loading_state, |empty| {
                        let provider = self.command_palette.resume_provider;
                        empty.child(
                            div()
                                .mt(px(12.0))
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .child(icon(
                                    provider_icon(provider),
                                    13.0,
                                    provider_color(&theme, provider),
                                ))
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_secondary)
                                        .child(provider.display_name().to_owned()),
                                ),
                        )
                    })
                    .when(resume_view && !show_loading_state, |empty| {
                        let provider = self.command_palette.resume_provider;
                        empty.child(
                            div()
                                .id("command-palette-resume-provider")
                                .mt(px(12.0))
                                .h(px(28.0))
                                .px(px(9.0))
                                .rounded(px(8.0))
                                .border_1()
                                .border_color(theme.border)
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .cursor_pointer()
                                .hover(|button| button.bg(theme.overlay))
                                .active(|button| button.opacity(0.82))
                                .child(icon(
                                    provider_icon(provider),
                                    13.0,
                                    provider_color(&theme, provider),
                                ))
                                .child(
                                    div()
                                        .text_size(sp(12.5))
                                        .text_color(theme.text_secondary)
                                        .child(provider.display_name().to_owned()),
                                )
                                .child(icon("icons/chevron-down.svg", 11.0, theme.text_tertiary))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open_command_palette_resume_provider_view(cx);
                                    cx.stop_propagation();
                                })),
                        )
                    }),
            );
        } else {
            let mut previous_section = None;
            for (index, item) in self.command_palette.results.iter().enumerate() {
                let starts_section = previous_section != Some(item.section);
                if starts_section {
                    if item.section != PaletteSection::Providers {
                        results = results.child(
                            div()
                                .h(px(SECTION_HEADER_HEIGHT))
                                .px(px(8.0))
                                .pt(px(8.0))
                                .flex()
                                .items_center()
                                .text_size(sp(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_tertiary)
                                .child(item.section.label()),
                        );
                    }
                    previous_section = Some(item.section);
                }

                let highlighted = index == selected;
                let icon_color = match item.icon {
                    PaletteIcon::Asset(_) | PaletteIcon::File(_) => theme.text_secondary,
                    PaletteIcon::Provider(provider) => provider_color(&theme, provider),
                };
                let file_icon_path = match item.icon {
                    PaletteIcon::File(path) => Some(path),
                    _ => None,
                };
                let icon_path = match item.icon {
                    PaletteIcon::Asset(path) | PaletteIcon::File(path) => path,
                    PaletteIcon::Provider(provider) => provider_icon(provider),
                };
                let importing = match &item.action {
                    PaletteAction::ResumeProviderSession(summary) => self
                        .command_palette
                        .provider_session_import
                        .as_ref()
                        .is_some_and(|cursor| same_provider_session(cursor, &summary.cursor)),
                    _ => false,
                };
                let detail = if importing {
                    Some(tr!("command_palette.loading_selected_session"))
                } else {
                    item.detail.clone()
                };
                let file_result = matches!(&item.action, PaletteAction::OpenFile(_));
                let content_match = item.content_match.clone();
                let shortcut = item.shortcut;
                results = results.child(
                    div()
                        .id(SharedString::from(format!("command-palette-row-{index}")))
                        .when(
                            starts_section && item.section == PaletteSection::Providers,
                            |row| row.mt(px(PROVIDER_SECTION_TOP_MARGIN)),
                        )
                        .h(px(command_palette_row_height(item)))
                        .px(px(10.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(if highlighted {
                            theme.border_strong
                        } else {
                            gpui::transparent_black()
                        })
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .cursor_pointer()
                        .when(highlighted, |row| row.bg(theme.overlay_strong))
                        .hover(|row| row.bg(theme.overlay))
                        .active(|row| row.opacity(0.82))
                        .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                            if *hovering {
                                this.set_command_palette_selection(index, cx);
                            }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.execute_command_palette_selection(Some(index), window, cx);
                            cx.stop_propagation();
                        }))
                        .child(
                            div()
                                .size(px(if file_result { 20.0 } else { 18.0 }))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if importing {
                                    motion::spin(icon("icons/loader-circle.svg", 14.0, icon_color))
                                } else if let Some(path) = file_icon_path {
                                    file_icon(path, 16.0).into_any_element()
                                } else {
                                    icon(icon_path, 14.0, icon_color).into_any_element()
                                }),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .justify_center()
                                .when(!file_result, |column| column.gap(px(2.0)))
                                .child(if file_result {
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .justify_center()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(sp(13.0))
                                                .line_height(sp(15.0))
                                                .font_weight(if highlighted {
                                                    FontWeight::MEDIUM
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .text_color(if highlighted {
                                                    theme.text
                                                } else {
                                                    theme.text_secondary
                                                })
                                                .child(item.label.clone()),
                                        )
                                        .when_some(detail, |row, detail| {
                                            row.child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(sp(11.0))
                                                    .line_height(sp(11.0))
                                                    .text_color(theme.text_tertiary)
                                                    .child(detail),
                                            )
                                        })
                                        .into_any_element()
                                } else {
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .flex()
                                        .items_baseline()
                                        .gap(px(7.0))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(sp(13.0))
                                                .font_weight(if highlighted {
                                                    FontWeight::MEDIUM
                                                } else {
                                                    FontWeight::NORMAL
                                                })
                                                .text_color(if highlighted {
                                                    theme.text
                                                } else {
                                                    theme.text_secondary
                                                })
                                                .child(item.label.clone()),
                                        )
                                        .when_some(detail, |row, detail| {
                                            row.child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(sp(12.0))
                                                    .text_color(theme.text_tertiary)
                                                    .child(detail),
                                            )
                                        })
                                        .into_any_element()
                                })
                                .when_some(content_match, |column, matched| {
                                    column.child(
                                        div()
                                            .min_w_0()
                                            .w_full()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_size(sp(12.0))
                                            .child(palette_content_match_text(
                                                &matched,
                                                &search_query,
                                                window,
                                                theme,
                                            )),
                                    )
                                }),
                        )
                        .when_some(shortcut, |row, shortcut| {
                            row.child(
                                div()
                                    .h(px(20.0))
                                    .min_w(px(24.0))
                                    .px(px(6.0))
                                    .rounded(px(6.0))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme.overlay_strong)
                                    .text_size(sp(12.0))
                                    .text_color(theme.text_tertiary)
                                    .child(shortcut),
                            )
                        }),
                );
            }
        }

        let card = div()
            .id("command-palette-card")
            .key_context("CommandPalette")
            .on_action(cx.listener(Self::toggle_command_palette_action))
            .on_action(
                cx.listener(|this, _: &SelectNext, _, cx| {
                    this.move_command_palette_selection(1, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| {
                this.move_command_palette_selection(-1, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectFirst, _, cx| {
                this.move_command_palette_selection(isize::MIN, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectLast, _, cx| {
                this.move_command_palette_selection(isize::MAX, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectPageDown, _, cx| {
                this.move_command_palette_selection(PAGE_STEP, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectPageUp, _, cx| {
                this.move_command_palette_selection(-PAGE_STEP, cx)
            }))
            .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                this.execute_command_palette_selection(None, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Dismiss, window, cx| {
                    this.dismiss_command_palette(window, cx)
                }),
            )
            .w_full()
            .max_w(px(640.0))
            .h(px(card_height))
            .overflow_hidden()
            .rounded(px(12.0))
            .bg(theme.raised)
            .shadow_xl()
            .relative()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .h(px(SEARCH_ROW_HEIGHT))
                    .px(px(16.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(sp(14.0))
                    .text_color(theme.text)
                    .child(icon("icons/search.svg", 14.0, theme.text_tertiary))
                    .when(file_view, |header| {
                        header.child(
                            div()
                                .text_size(sp(12.5))
                                .text_color(theme.text_secondary)
                                .child(tr!("command_palette.find_file")),
                        )
                    })
                    .when(file_view, |header| {
                        header.child(icon("icons/chevron-right.svg", 11.0, theme.text_ghost))
                    })
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(self.command_palette.search.clone()),
                    ),
            )
            .child(results)
            .child(
                div()
                    .h(px(FOOTER_HEIGHT))
                    .px(px(16.0))
                    .py(px(6.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(18.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.canvas.opacity(0.75))
                    .text_size(sp(11.0))
                    .text_color(theme.text_tertiary)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .h(px(20.0))
                                    .min_w(px(24.0))
                                    .px(px(6.0))
                                    .rounded(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme.overlay_strong)
                                    .child(icon("icons/arrow-up.svg", 10.0, theme.text_tertiary)),
                            )
                            .child(
                                div()
                                    .h(px(20.0))
                                    .min_w(px(24.0))
                                    .px(px(6.0))
                                    .rounded(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme.overlay_strong)
                                    .child(icon("icons/arrow-down.svg", 10.0, theme.text_tertiary)),
                            )
                            .child(SharedString::from("Navigate")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .h(px(20.0))
                                    .min_w(px(24.0))
                                    .px(px(6.0))
                                    .rounded(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme.overlay_strong)
                                    .child(icon(
                                        "icons/corner-down-left.svg",
                                        10.0,
                                        theme.text_tertiary,
                                    )),
                            )
                            .child(SharedString::from("Select")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .h(px(20.0))
                                    .min_w(px(24.0))
                                    .px(px(6.0))
                                    .rounded(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme.overlay_strong)
                                    .text_size(sp(12.0))
                                    .text_color(theme.text_tertiary)
                                    .child(SharedString::from("Esc")),
                            )
                            .child(SharedString::from("Close")),
                    ),
            );

        let scrim = if theme.is_dark {
            gpui::hsla(0.0, 0.0, 0.0, 0.26)
        } else {
            gpui::hsla(0.0, 0.0, 0.0, 0.14)
        };
        let layer = div()
            .id("command-palette-layer")
            .absolute()
            .inset_0()
            .occlude()
            .bg(scrim)
            .px(px(24.0))
            .pt(px(top))
            .flex()
            .items_start()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_command_palette(window, cx)),
            )
            .child(card);
        Some(gpui::deferred(layer).with_priority(3).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(query: &str, candidate: &str) -> Option<u32> {
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = crate::composer_complete::matcher();
        let mut buf = Vec::new();
        pattern.score(Utf32Str::new(candidate, &mut buf), &mut matcher)
    }

    #[test]
    fn fuzzy_search_matches_command_words_and_initials() {
        assert!(score("open proj", "Open project folder repository").is_some());
        assert!(score("cmptr use", "Computer Use settings accessibility").is_some());
        assert!(score("totally absent", "Open project folder repository").is_none());
    }

    #[test]
    fn copy_identifier_commands_use_the_selected_tasks_live_ids() {
        let mut session = AgentSession::new(Uuid::nil(), ProviderKind::Codex);
        session.id = Uuid::parse_str("ed28ee51-43cf-4a83-a52f-04c509ca2c09").unwrap();

        assert_eq!(
            PaletteIdentifier::TaskId.value(Some(&session)).as_deref(),
            Some("ed28ee51-43cf-4a83-a52f-04c509ca2c09")
        );
        assert_eq!(
            PaletteIdentifier::AgentCliThreadId.value(Some(&session)),
            None
        );

        session.provider_cursor = Some(ProviderResumeCursor::Codex {
            thread_id: "019cfd7a-6942-78b1-9d47-30576c562321".into(),
        });
        assert_eq!(
            PaletteIdentifier::AgentCliThreadId
                .value(Some(&session))
                .as_deref(),
            Some("019cfd7a-6942-78b1-9d47-30576c562321")
        );
    }

    #[test]
    fn provider_session_identity_uses_provider_and_native_id() {
        let listed = ProviderResumeCursor::Claude {
            session_id: "11111111-1111-4111-8111-111111111111".into(),
            resume_at: None,
        };
        let imported = ProviderResumeCursor::Claude {
            session_id: "11111111-1111-4111-8111-111111111111".into(),
            resume_at: Some("native-message".into()),
        };
        let other = ProviderResumeCursor::Codex {
            thread_id: "11111111-1111-4111-8111-111111111111".into(),
        };

        assert!(same_provider_session(&listed, &imported));
        assert!(!same_provider_session(&listed, &other));
    }

    #[test]
    fn scroll_indexes_include_section_headers() {
        let sections = [
            PaletteSection::Tasks,
            PaletteSection::Tasks,
            PaletteSection::Commands,
            PaletteSection::Settings,
        ];
        let scroll_index = |selected: usize| {
            let mut headers = 0;
            let mut previous = None;
            for section in sections.iter().take(selected + 1) {
                if previous != Some(*section) {
                    headers += 1;
                    previous = Some(*section);
                }
            }
            selected + headers
        };
        assert_eq!(scroll_index(0), 1);
        assert_eq!(scroll_index(1), 2);
        assert_eq!(scroll_index(2), 4);
        assert_eq!(scroll_index(3), 6);
    }

    #[test]
    fn arrows_wrap_while_page_and_boundary_keys_clamp() {
        assert_eq!(next_selection_index(0, 5, -1), Some(4));
        assert_eq!(next_selection_index(4, 5, 1), Some(0));
        assert_eq!(next_selection_index(3, 5, PAGE_STEP), Some(4));
        assert_eq!(next_selection_index(1, 5, -PAGE_STEP), Some(0));
        assert_eq!(next_selection_index(2, 5, isize::MIN), Some(0));
        assert_eq!(next_selection_index(2, 5, isize::MAX), Some(4));
        assert_eq!(next_selection_index(0, 0, 1), None);
    }

    #[test]
    fn searched_commands_rank_before_tasks() {
        assert!(PaletteSection::Commands.query_rank() < PaletteSection::Tasks.query_rank());
        assert!(PaletteSection::Tasks.query_rank() < PaletteSection::Settings.query_rank());
    }

    #[test]
    fn no_results_appears_only_after_background_search_settles() {
        assert!(!should_show_command_palette_empty_state(0, true));
        assert!(should_show_command_palette_empty_state(0, false));
        assert!(!should_show_command_palette_empty_state(1, false));
        assert!(should_keep_previous_command_palette_results(0, true, 1));
        assert!(!should_keep_previous_command_palette_results(0, false, 1));
        assert!(!should_keep_previous_command_palette_results(1, true, 1));
    }

    #[test]
    fn result_height_hugs_rows_and_section_headers() {
        let item = |section, order| {
            CommandPaletteItem::command(
                section,
                format!("Item {order}"),
                "icons/search.svg",
                None,
                PaletteAction::NewTask,
                "",
                order,
            )
        };
        let items = vec![
            item(PaletteSection::Tasks, 0),
            item(PaletteSection::Tasks, 1),
            item(PaletteSection::Commands, 2),
        ];
        assert_eq!(
            command_palette_results_height(&items, false),
            SECTION_HEADER_HEIGHT * 2.0 + RESULT_ROW_HEIGHT * 3.0 + RESULTS_BOTTOM_PADDING
        );
        assert_eq!(
            command_palette_results_height(&[], true),
            EMPTY_RESULTS_HEIGHT + RESULTS_BOTTOM_PADDING
        );
        let providers = vec![
            item(PaletteSection::Providers, 0),
            item(PaletteSection::Providers, 1),
        ];
        assert_eq!(
            command_palette_results_height(&providers, false),
            PROVIDER_SECTION_TOP_MARGIN + RESULT_ROW_HEIGHT * 2.0 + RESULTS_BOTTOM_PADDING
        );
    }

    #[test]
    fn render_reads_only_the_cached_result_snapshot() {
        let source = include_str!("./command_palette.rs");
        let start = source
            .find("\n    pub(super) fn render_command_palette(")
            .expect("render function must exist");
        let end = source[start..]
            // Match from the final newline only so this accepts both LF and
            // CRLF checkouts.
            .find("\n#[cfg(test)]")
            .map(|offset| start + offset)
            .expect("test module marker must exist");
        let render = &source[start..end];
        for forbidden in [
            "refresh_command_palette_results(",
            "command_palette_task_candidates(",
            "session_message_search(",
            "background_executor(",
            "std::fs",
            "Command::new",
            "read_dir",
        ] {
            assert!(
                !render.contains(forbidden),
                "palette render must not call `{forbidden}`"
            );
        }
    }
}
