//! Browser tabs in the right panel.
//!
//! A tab holds a tab id, not the page. Pages live in [`Pages`], app-wide, so a
//! panel or window dropping does not drop them; only closing the tab does.

use std::collections::{HashMap, VecDeque};

use bezel::{
    gpui::{
        self, AnyElement, App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
        Global, KeyBinding, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, Subscription,
        Window, actions, div, prelude::*, px,
    },
    motion::{Fade, Painter},
    theme::Theme,
    ui::{
        context_menu::{ContextMenu, Place},
        icons,
        input::TextField,
        menu::Item,
        tooltip::Tooltip,
        widgets::{ButtonStyle, Buttons as _},
    },
};
use browser::{ConsoleMessage, DataStore, WebView, WebViewEvent};

/// Claimed on the address field, so `enter` loads what it holds.
pub const ADDRESS_CONTEXT: &str = "BrowserAddress";

/// Claimed on the whole tab, the address field included.
pub const CONTEXT: &str = "Browser";

/// Messages a tab's log keeps; older ones are dropped.
const LOG_LIMIT: usize = 500;

actions!(padu_browser, [Go, Reload, HardReload]);

/// Settings for browsing in the right panel.
pub const BUILTIN_SEARCH_ENGINES: [(&str, &str); 7] = [
    ("DuckDuckGo", "https://duckduckgo.com/?q=%s"),
    ("Google", "https://www.google.com/search?q=%s"),
    ("Bing", "https://www.bing.com/search?q=%s"),
    ("Kagi", "https://kagi.com/search?q=%s"),
    ("Brave", "https://search.brave.com/search?q=%s"),
    ("Ecosia", "https://www.ecosia.org/search?q=%s"),
    ("Perplexity", "https://www.perplexity.ai/search?q=%s"),
];

#[allow(dead_code)]
pub const SEARCH_ENGINES: [(&str, &str); 7] = BUILTIN_SEARCH_ENGINES;

/// Settings for browsing in the right panel.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Browsing {
    /// What a new tab opens on.
    pub home: String,
    /// Where a search from the address field goes: `%s` is the query.
    pub search: String,
    /// Whether agents are offered the browser tools that read pages.
    pub agents_read: bool,
    /// Whether agents are offered the browser tools that click and type.
    pub agents_act: bool,
    /// Hosts agents may not read or act on.
    pub agents_blocked: Vec<String>,
    /// Whether pages keep cookies and storage across restarts.
    pub keep_signed_in: bool,
    /// Additional search engine options configured by the user.
    #[serde(default)]
    pub custom_engines: Vec<(String, String)>,
}

impl Global for Browsing {}

impl Default for Browsing {
    fn default() -> Self {
        Self {
            home: "https://duckduckgo.com".to_owned(),
            search: "https://duckduckgo.com/?q=%s".to_owned(),
            agents_read: true,
            agents_act: true,
            agents_blocked: Vec::new(),
            keep_signed_in: true,
            custom_engines: Vec::new(),
        }
    }
}

impl Browsing {
    pub fn all_engines(&self) -> Vec<(String, String)> {
        let mut engines = BUILTIN_SEARCH_ENGINES
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>();
        for (name, template) in &self.custom_engines {
            if !engines.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
                engines.push((name.clone(), template.clone()));
            }
        }
        engines
    }

    pub fn engine_name_for_search(&self) -> String {
        for (name, template) in self.all_engines() {
            if template == self.search {
                return name;
            }
        }
        self.search.clone()
    }

    /// Whether `url`'s host is one [`Self::agents_blocked`] names.
    pub fn blocks(&self, url: &str) -> bool {
        let Some(host) = url::Url::parse(url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        else {
            return false;
        };
        self.agents_blocked.iter().any(|blocked| {
            let blocked = blocked.trim().trim_start_matches("*.").to_ascii_lowercase();
            !blocked.is_empty() && (host == blocked || host.ends_with(&format!(".{blocked}")))
        })
    }

    pub fn load_or_default() -> Self {
        let path = Self::settings_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(browsing) = serde_json::from_str::<Self>(&content) {
                return browsing;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn settings_path() -> std::path::PathBuf {
        if cfg!(debug_assertions) {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(std::path::Path::parent)
                .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
                .join("temp")
                .join("browser.json")
        } else {
            dirs::home_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(".padu")
                .join("browser.json")
        }
    }

    pub fn search_url(&self, query: &str) -> String {
        let query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        match self.search.contains("%s") {
            true => self.search.replace("%s", &query),
            false => format!("{}{query}", self.search),
        }
    }
}

/// Load browsing preferences into the app global. Called once at startup.
pub fn init(cx: &mut App) {
    cx.set_global(Browsing::load_or_default());
    bezel::ui::input::init(cx);
}

/// What a new tab opens on: the home page in Settings.
pub fn home(cx: &App) -> String {
    cx.try_global::<Browsing>().map_or_else(
        || Browsing::default().home,
        |browsing| browsing.home.clone(),
    )
}

pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("enter", Go, Some(ADDRESS_CONTEXT)),
        KeyBinding::new("shift-enter", Go, Some(ADDRESS_CONTEXT)),
        KeyBinding::new("enter", Go, Some("CydoniaAddress")),
        KeyBinding::new("secondary-r", Reload, Some(CONTEXT)),
        KeyBinding::new("f5", Reload, Some(CONTEXT)),
        KeyBinding::new("secondary-shift-r", HardReload, Some(CONTEXT)),
    ]
}

/// Every live page, by tab id.
#[derive(Default)]
struct Pages(HashMap<u64, Entity<WebView>>);

impl Global for Pages {}

/// What each live page has logged, by tab id, oldest first.
#[derive(Default)]
struct Logs(HashMap<u64, VecDeque<ConsoleMessage>>);

impl Global for Logs {}

/// A tab id no other tab holds: tab ids are saved across restarts.
#[allow(dead_code)]
pub fn new_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64)
}

/// Drop the tab's page. Called on closing the tab.
pub fn forget(id: u64, cx: &mut App) {
    if let Some(pages) = cx.try_global::<Pages>()
        && pages.0.contains_key(&id)
    {
        cx.global_mut::<Pages>().0.remove(&id);
    }
    if let Some(logs) = cx.try_global::<Logs>()
        && logs.0.contains_key(&id)
    {
        cx.global_mut::<Logs>().0.remove(&id);
    }
}

#[derive(Default)]
struct Clearing(bool);
impl Global for Clearing {}

pub(crate) fn clearing(cx: &App) -> bool {
    cx.try_global::<Clearing>().is_some_and(|state| state.0)
}

#[allow(dead_code)]
pub(crate) fn set_clearing(value: bool, cx: &mut App) {
    cx.default_global::<Clearing>().0 = value;
    cx.refresh_windows();
}

#[allow(dead_code)]
pub(crate) fn forget_all(cx: &mut App) {
    cx.default_global::<Pages>().0.clear();
    cx.default_global::<Logs>().0.clear();
}

/// The title or location changed.
pub struct Changed;

/// The page asked for a new window — a `target="_blank"` link or
/// `window.open` — on this URL.
#[allow(dead_code)]
pub struct OpenTab(pub String);

pub struct Browser {
    pub id: u64,
    /// Built on first render: a page needs a window.
    page: Option<Entity<WebView>>,
    /// The page's last location, or the saved one until it is built.
    url: String,
    pub(super) title: String,
    address: Entity<TextField>,
    focus: FocusHandle,
    /// The reload button's right-press menu.
    menu: ContextMenu<Self>,
    _page: Option<Subscription>,
    occluded: bool,
    address_was_focused: bool,
    address_mouse_down_pos: Option<Point<Pixels>>,
}

pub type BrowserView = Browser;

impl EventEmitter<Changed> for Browser {}
impl EventEmitter<OpenTab> for Browser {}

impl Browser {
    pub fn new(id: u64, url: String, title: String, cx: &mut Context<Self>) -> Self {
        let address = cx.new(|cx| {
            TextField::new(cx)
                .with_key_context(ADDRESS_CONTEXT)
                .with_placeholder("Search or enter address")
        });
        address.update(cx, |field, cx| field.set_content(url.clone(), cx));
        Self {
            id,
            page: None,
            url,
            title,
            address,
            focus: cx.focus_handle(),
            menu: ContextMenu::default(),
            _page: None,
            occluded: false,
            address_was_focused: false,
            address_mouse_down_pos: None,
        }
    }

    #[allow(dead_code)]
    pub fn new_view(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(new_id(), home(cx), String::new(), cx)
    }

    #[allow(dead_code)]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The page's title, or its location while it has none.
    pub fn title(&self) -> &str {
        if self.title.is_empty() {
            &self.url
        } else {
            &self.title
        }
    }

    pub fn tab_label(&self) -> Option<String> {
        let title = self.title();
        if title.is_empty() {
            None
        } else {
            Some(title.to_owned())
        }
    }

    /// What the tab's page has logged, oldest first.
    #[allow(dead_code)]
    pub(crate) fn console(&self, cx: &App) -> Vec<ConsoleMessage> {
        cx.try_global::<Logs>()
            .and_then(|logs| logs.0.get(&self.id))
            .map(|log| log.iter().cloned().collect())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub(crate) fn webview(&self) -> Option<Entity<WebView>> {
        self.page.clone()
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self._page = None;
        self.page = None;
        forget(self.id, cx);
    }

    pub fn shutdown(&mut self, cx: &mut Context<Self>) {
        self.close(cx);
    }

    /// Load `url`: in the page where it is built, else the page is built on it.
    pub fn navigate(&mut self, url: String, cx: &mut Context<Self>) {
        self.url = url.clone();
        self.address
            .update(cx, |field, cx| field.set_content(url.clone(), cx));
        if let Some(page) = &self.page {
            page.update(cx, |page, _| page.load(url));
        }
        cx.emit(Changed);
        cx.notify();
    }

    pub fn navigate_to_url(&mut self, url: String, cx: &mut Context<Self>) {
        self.navigate(url, cx);
    }

    pub fn address_focus(&self, cx: &App) -> FocusHandle {
        self.address.focus_handle(cx)
    }

    pub fn focus_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.address_focus(cx);
        window.focus(&focus, cx);
        self.address.update(cx, |field, cx| {
            let len = field.content().len();
            field.select(0..len, cx);
        });
    }

    pub fn focus_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = &self.page {
            window.focus(&page.focus_handle(cx), cx);
        } else {
            self.focus_address(window, cx);
        }
    }

    pub fn refresh_localized_text(&mut self, cx: &mut Context<Self>) {
        self.address.update(cx, |field, cx| {
            field.set_placeholder("Search or enter address", cx);
        });
        cx.notify();
    }

    pub fn restore_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current_url = self.url.clone();
        let changed = self.address.read(cx).content().as_str() != current_url.as_str();
        if changed {
            self.address.update(cx, |field, cx| {
                field.set_content(current_url, cx);
                let len = field.content().len();
                field.select(0..len, cx);
            });
            cx.notify();
        } else if let Some(page) = &self.page {
            window.focus(&page.focus_handle(cx), cx);
        }
    }

    pub fn sync_native_state(
        &mut self,
        _logically_active: bool,
        _surface_visible: bool,
        occluded: bool,
        cx: &mut Context<Self>,
    ) {
        if self.occluded != occluded {
            self.occluded = occluded;
            cx.notify();
        }
    }

    pub fn overlay_open(&self, _cx: &App) -> bool {
        self.menu.is_open()
    }

    pub fn back_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.page(window, cx);
        page.update(cx, |page, _| page.back());
    }

    pub fn forward_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.page(window, cx);
        page.update(cx, |page, _| page.forward());
    }

    pub(crate) fn page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<WebView> {
        if let Some(page) = &self.page {
            return page.clone();
        }
        let existing = cx
            .try_global::<Pages>()
            .and_then(|pages| pages.0.get(&self.id).cloned());
        let page = existing.unwrap_or_else(|| {
            let url = self.url.clone();
            let keep = cx
                .try_global::<Browsing>()
                .is_none_or(|browsing| browsing.keep_signed_in);
            let store = match keep {
                true => DataStore::new(),
                false => DataStore::new().incognito(),
            };
            let page = cx.new(|cx| {
                WebView::new(url, window, cx)
                    .with_data_store(store)
                    .with_console()
            });
            cx.default_global::<Pages>().0.insert(self.id, page.clone());
            page
        });
        let current = page.read(cx);
        if let Some(location) = current.location() {
            self.url = location.to_owned();
        }
        if !current.title().is_empty() {
            self.title = current.title().to_owned();
        }
        self._page = Some(cx.subscribe_in(&page, window, Self::report));
        self.page = Some(page.clone());
        page
    }

    fn report(
        &mut self,
        _: &Entity<WebView>,
        event: &WebViewEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            WebViewEvent::Location(url) => {
                self.url = url.clone();
                if !self.address.focus_handle(cx).is_focused(window) {
                    self.address
                        .update(cx, |field, cx| field.set_content(url.clone(), cx));
                }
            }
            WebViewEvent::Title(title) => self.title = title.clone(),
            WebViewEvent::Load(_) | WebViewEvent::History { .. } => {}
            WebViewEvent::Console(message) => {
                let log = cx.default_global::<Logs>().0.entry(self.id).or_default();
                if log.len() == LOG_LIMIT {
                    log.pop_front();
                }
                log.push_back(message.clone());
                return;
            }
            WebViewEvent::NewWindow(url) => {
                cx.emit(OpenTab(url.clone()));
                return;
            }
        }
        cx.emit(Changed);
        cx.notify();
    }

    fn reload(&mut self, _: &Reload, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.page(window, cx);
        page.update(cx, |page, _| page.reload());
    }

    /// Reload with nothing taken from the cache.
    fn hard_reload(&mut self, _: &HardReload, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.page(window, cx);
        page.update(cx, |page, _| page.reload_bypassing_cache());
    }

    /// Open the reload button's menu at `at`.
    fn open_reload_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let items = vec![
            Item::action("Reload").with_shortcut_in(&Reload, CONTEXT, window),
            Item::action("Hard Reload").with_shortcut_in(&HardReload, CONTEXT, window),
        ];
        self.menu.open(
            (),
            Place::At(at),
            items,
            |this, path, window, cx| match path.first() {
                Some(0) => this.reload(&Reload, window, cx),
                Some(1) => this.hard_reload(&HardReload, window, cx),
                _ => {}
            },
            window,
            cx,
        );
    }

    /// The reload button's menu, while it is open.
    fn reload_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.menu
            .render("browser-reload-menu", |this| &mut this.menu, window, cx)
    }

    fn go(&mut self, _: &Go, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.address.read(cx).content().trim().to_owned();
        if typed.is_empty() {
            return;
        }
        let url = address(&typed, cx);
        self.url = url.clone();
        self.address
            .update(cx, |field, cx| field.set_content(url.clone(), cx));
        let page = self.page(window, cx);
        page.update(cx, |page, _| page.load(url));
        window.focus(&page.focus_handle(cx), cx);
        cx.emit(Changed);
        cx.notify();
    }
}

/// What the address field's text loads: a URL as typed, a bare host over
/// https, anything else as a search with the engine in Settings.
fn address(typed: &str, cx: &App) -> String {
    let trimmed = typed.trim();
    if trimmed.contains("://") || trimmed.starts_with("about:") || trimmed.starts_with("data:") {
        trimmed.to_owned()
    } else if trimmed.starts_with("localhost:")
        || trimmed.starts_with("localhost/")
        || trimmed == "localhost"
        || trimmed.starts_with("127.0.0.1:")
        || trimmed.starts_with("127.0.0.1/")
        || trimmed == "127.0.0.1"
        || trimmed.starts_with("0.0.0.0:")
        || trimmed.starts_with("0.0.0.0/")
        || trimmed == "0.0.0.0"
    {
        format!("http://{trimmed}")
    } else if !trimmed.contains(char::is_whitespace) && trimmed.contains('.') {
        format!("https://{trimmed}")
    } else {
        cx.try_global::<Browsing>()
            .cloned()
            .unwrap_or_default()
            .search_url(trimmed)
    }
}

impl Focusable for Browser {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.page {
            Some(page) => page.focus_handle(cx),
            None => self.focus.clone(),
        }
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.occluded || clearing(cx) {
            return div().into_any_element();
        }
        let page = self.page(window, cx);
        let theme = Theme::of(cx).clone();
        let loading = page.read(cx).is_loading();
        let nav = |icon: &'static [u8], id: &'static str, tip: &'static str| {
            theme
                .icon_button(
                    icon,
                    ButtonStyle::Ghost,
                    Some(Fade::new(Painter::of(cx), id)),
                )
                .id(id)
                .flex_none()
                .tooltip(move |window, cx| Tooltip::text(tip, window, cx))
        };
        let (can_back, can_forward) = {
            let page = page.read(cx);
            (page.can_go_back(), page.can_go_forward())
        };
        // Nowhere to go: faint, no hover, no press.
        let stuck = |icon: &'static [u8], id: &'static str| {
            theme
                .tinted_icon_button(icon, theme.text_faint)
                .id(id)
                .flex_none()
                .into_any_element()
        };
        let back = page.clone();
        let forward = page.clone();
        div()
            .id("browser-surface")
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::go))
            .on_action(cx.listener(Self::reload))
            .on_action(cx.listener(Self::hard_reload))
            .on_action(cx.listener(|this, _: &crate::BrowserReload, window, cx| {
                this.reload(&Reload, window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &crate::BrowserHardReload, window, cx| {
                    this.hard_reload(&HardReload, window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &crate::BrowserBack, window, cx| {
                this.back_action(window, cx);
            }))
            .on_action(cx.listener(|this, _: &crate::BrowserForward, window, cx| {
                this.forward_action(window, cx);
            }))
            .on_action(
                cx.listener(|this, _: &crate::FocusBrowserAddress, window, cx| {
                    this.focus_address(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::BrowserAddressCancel, window, cx| {
                    this.restore_address(window, cx);
                }),
            )
            .on_action(cx.listener(|_, _: &crate::BrowserStop, _, _| {}))
            .on_action(cx.listener(|_, _: &crate::WebviewCopy, _, _| {}))
            .on_action(cx.listener(|_, _: &crate::WebviewCut, _, _| {}))
            .on_action(cx.listener(|_, _: &crate::WebviewPaste, _, _| {}))
            .on_action(cx.listener(|_, _: &crate::WebviewSelectAll, _, _| {}))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(2.))
                    .px(px(6.))
                    .py(px(4.))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(match can_back {
                        true => nav(icons::arrows::ArrowLeft, "browser-back", "Back")
                            .on_click(move |_, _, cx| back.update(cx, |page, _| page.back()))
                            .into_any_element(),
                        false => stuck(icons::arrows::ArrowLeft, "browser-back"),
                    })
                    .child(match can_forward {
                        true => nav(icons::arrows::ArrowRight, "browser-forward", "Forward")
                            .on_click(move |_, _, cx| forward.update(cx, |page, _| page.forward()))
                            .into_any_element(),
                        false => stuck(icons::arrows::ArrowRight, "browser-forward"),
                    })
                    .child(
                        nav(
                            icons::arrows::RefreshCw,
                            "browser-reload",
                            if loading {
                                "Loading…"
                            } else {
                                "Reload — ⇧-click or right-click for Hard Reload"
                            },
                        )
                        .on_click(cx.listener(|this, click: &ClickEvent, window, cx| {
                            match click.modifiers().shift {
                                true => this.hard_reload(&HardReload, window, cx),
                                false => this.reload(&Reload, window, cx),
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, press: &MouseDownEvent, window, cx| {
                                this.open_reload_menu(press.position, window, cx);
                                cx.stop_propagation();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .on_action(cx.listener(Self::go))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                    this.address_was_focused =
                                        this.address.focus_handle(cx).is_focused(window);
                                    this.address_mouse_down_pos = Some(event.position);
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, _, cx| {
                                    let was_focused = this.address_was_focused;
                                    let click_pos = this.address_mouse_down_pos.take();
                                    let is_click = click_pos.is_some_and(|pos| {
                                        let dx = (event.position.x - pos.x).abs();
                                        let dy = (event.position.y - pos.y).abs();
                                        dx < px(4.0) && dy < px(4.0)
                                    });
                                    if !was_focused && event.click_count <= 1 && is_click {
                                        this.address.update(cx, |field, cx| {
                                            let len = field.content().len();
                                            field.select(0..len, cx);
                                        });
                                    }
                                }),
                            )
                            .child(self.address.clone()),
                    )
                    .child(
                        nav(
                            icons::arrows::ExternalLink,
                            "browser-system",
                            "Open in system browser",
                        )
                        .on_click(cx.listener(|this, _, _, cx| cx.open_url(&this.url))),
                    ),
            )
            .child(div().flex_1().min_h_0().child(page))
            .children(self.reload_menu(window, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bezel::gpui::TestAppContext;

    #[gpui::test]
    fn test_address_resolution(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.set_global(Browsing::default());

            // Full URL
            assert_eq!(
                address("https://example.com/path", cx),
                "https://example.com/path"
            );
            assert_eq!(address("http://example.com", cx), "http://example.com");
            assert_eq!(address("about:blank", cx), "about:blank");
            assert_eq!(
                address("data:text/html,<h1>hi</h1>", cx),
                "data:text/html,<h1>hi</h1>"
            );

            // Localhost and loopback
            assert_eq!(address("localhost", cx), "http://localhost");
            assert_eq!(address("localhost:3000", cx), "http://localhost:3000");
            assert_eq!(address("localhost/app", cx), "http://localhost/app");
            assert_eq!(address("127.0.0.1:8080", cx), "http://127.0.0.1:8080");
            assert_eq!(
                address("127.0.0.1/dashboard", cx),
                "http://127.0.0.1/dashboard"
            );
            assert_eq!(address("0.0.0.0:4000", cx), "http://0.0.0.0:4000");

            // Domain names
            assert_eq!(address("google.com", cx), "https://google.com");
            assert_eq!(
                address("github.com/pingdotgg/t3code", cx),
                "https://github.com/pingdotgg/t3code"
            );
            assert_eq!(address("sub.domain.co.uk", cx), "https://sub.domain.co.uk");

            // Search queries
            assert_eq!(
                address("rust programming", cx),
                "https://duckduckgo.com/?q=rust+programming"
            );
            assert_eq!(
                address("hello world", cx),
                "https://duckduckgo.com/?q=hello+world"
            );
            assert_eq!(
                address("singlewordsearch", cx),
                "https://duckduckgo.com/?q=singlewordsearch"
            );
        });
    }

    #[gpui::test]
    fn test_browser_bindings_contain_address_keys() {
        let all_bindings = bindings();
        assert!(all_bindings.iter().any(|b| b.action().as_any().is::<Go>()));
        assert!(
            all_bindings
                .iter()
                .any(|b| b.action().as_any().is::<Reload>())
        );
        assert!(
            all_bindings
                .iter()
                .any(|b| b.action().as_any().is::<HardReload>())
        );
    }
}
