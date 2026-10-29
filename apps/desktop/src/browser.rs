//! Decoupled browser surface for the right panel: placeholder interface
//! preparing for the external / next-gen browser migration.

use std::rc::Rc;

use gpui::{
    App, Context, Div, ElementId, Entity, FocusHandle, Focusable, IntoElement,
    Render, Stateful, Subscription, Window, div, prelude::*, px,
};

use crate::input::{InputEvent, TextInput};
use crate::theme::{Theme, sp};
use crate::ui::icon;
use crate::ui::text_field::TextField;
use crate::ui::tooltip::Tooltip;
use crate::{
    BrowserBack, BrowserForward, BrowserHardReload, BrowserReload, BrowserStop,
    FocusBrowserAddress, WebviewCopy, WebviewCut, WebviewPaste, WebviewSelectAll,
};

const TOOLBAR_HEIGHT: f32 = 38.0;

mod host {
    use gpui::{Bounds, Pixels};

    pub(super) struct WebviewHost;

    #[allow(dead_code)]
    impl WebviewHost {
        pub fn sync_bounds(&self, _bounds: Bounds<Pixels>, _scale: f32) {}
        pub fn set_visible(&self, _visible: bool) {}
        pub fn native_focus_within(&self) -> bool {
            false
        }
        pub fn set_backgrounded(&self, _backgrounded: bool) {}
    }
}

use host::WebviewHost;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserLifecycle {
    Active,
    Backgrounded,
    #[allow(dead_code)]
    Closing,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AddressTarget {
    Url(String),
    Search(String),
}

fn search_url(query: &str) -> String {
    let mut encoded = String::with_capacity(query.len() * 3);
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                use std::fmt::Write as _;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    format!("https://duckduckgo.com/?q={encoded}")
}

fn resolve_address(input: &str) -> Option<AddressTarget> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(url) = url::Url::parse(trimmed) {
        if matches!(url.scheme(), "http" | "https" | "file") {
            return Some(AddressTarget::Url(url.to_string()));
        }
    }
    if trimmed.starts_with("localhost:")
        || trimmed.starts_with("127.0.0.1:")
        || trimmed.starts_with("0.0.0.0:")
    {
        return Some(AddressTarget::Url(format!("http://{trimmed}")));
    }
    if trimmed.contains('.') && !trimmed.contains(' ') {
        return Some(AddressTarget::Url(format!("https://{trimmed}")));
    }
    Some(AddressTarget::Search(trimmed.to_owned()))
}

fn display_url(url: &str) -> &str {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
}

fn is_secure_url(url: &str) -> bool {
    url.starts_with("https://")
}

pub struct BrowserView {
    focus_handle: FocusHandle,
    lifecycle: BrowserLifecycle,
    address: Entity<TextInput>,
    #[allow(dead_code)]
    host: Option<Rc<WebviewHost>>,
    navigation_requested: bool,
    current_url: Option<String>,
    page_title: Option<String>,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    #[allow(dead_code)]
    address_dirty: bool,
    #[allow(dead_code)]
    occluded: bool,
    _subscriptions: Vec<Subscription>,
}

impl BrowserView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let address = cx.new(|cx| {
            TextInput::new(window, cx)
                .select_all_on_focus_click()
                .placeholder(tr!("input.search_or_enter_address"))
        });

        let submit_subscription = cx.subscribe(
            &address,
            |this: &mut Self, address, event: &InputEvent, cx| match event {
                InputEvent::Submit(text) => this.navigate_to_input(text.clone(), cx),
                InputEvent::Edited => {
                    let shown = this.current_url.as_deref().map(display_url).unwrap_or("");
                    this.address_dirty = address.read(cx).content() != shown;
                }
                InputEvent::Focus => {}
                InputEvent::BackspaceOnEmpty => {}
            },
        );

        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            lifecycle: BrowserLifecycle::Active,
            address,
            host: Some(Rc::new(WebviewHost)),
            navigation_requested: false,
            current_url: None,
            page_title: None,
            loading: false,
            can_go_back: false,
            can_go_forward: false,
            address_dirty: false,
            occluded: false,
            _subscriptions: vec![submit_subscription],
        }
    }

    pub fn refresh_localized_text(&mut self, cx: &mut Context<Self>) {
        self.address.update(cx, |address, cx| {
            address.set_placeholder(tr!("input.search_or_enter_address"), cx);
        });
        cx.notify();
    }

    pub fn tab_label(&self) -> Option<String> {
        if let Some(title) = self.page_title.as_deref().filter(|t| !t.trim().is_empty()) {
            return Some(title.to_owned());
        }
        self.current_url
            .as_deref()
            .map(|url| display_url(url).to_owned())
    }

    pub fn navigate_to_url(&mut self, url: String, cx: &mut Context<Self>) {
        self.navigation_requested = true;
        self.current_url = Some(url);
        self.address_dirty = false;
        self.echo_page_url(cx);
        cx.notify();
    }

    fn navigate_to_input(&mut self, input: String, cx: &mut Context<Self>) {
        let Some(target) = resolve_address(&input) else {
            return;
        };
        let url = match target {
            AddressTarget::Url(url) => url,
            AddressTarget::Search(query) => search_url(&query),
        };
        self.navigate_to_url(url, cx);
    }

    fn echo_page_url(&mut self, cx: &mut Context<Self>) {
        let Some(url) = &self.current_url else {
            return;
        };
        let display = display_url(url).to_owned();
        self.address.update(cx, |address, cx| {
            address.set_content(display, cx);
        });
    }

    fn restore_address(&mut self, cx: &mut Context<Self>) {
        let shown = self
            .current_url
            .as_deref()
            .map(display_url)
            .unwrap_or("")
            .to_owned();
        self.address.update(cx, |address, cx| {
            address.set_content(shown, cx);
        });
        self.address_dirty = false;
        cx.notify();
    }

    pub fn focus_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.navigation_requested {
            window.focus(&self.focus_handle, cx);
        } else {
            self.focus_address(window, cx);
        }
    }

    pub fn focus_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.address.read(cx).focus();
        window.focus(&focus, cx);
    }

    pub fn sync_native_state(
        &mut self,
        logically_active: bool,
        _surface_visible: bool,
        occluded: bool,
        _cx: &mut Context<Self>,
    ) {
        if matches!(
            self.lifecycle,
            BrowserLifecycle::Closing | BrowserLifecycle::Closed
        ) {
            return;
        }
        self.occluded = occluded;
        if logically_active {
            self.lifecycle = BrowserLifecycle::Active;
        } else {
            self.lifecycle = BrowserLifecycle::Backgrounded;
        }
    }

    pub fn shutdown(&mut self, cx: &mut Context<Self>) {
        self.lifecycle = BrowserLifecycle::Closed;
        cx.notify();
    }

    pub fn overlay_open(&self, _cx: &App) -> bool {
        false
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn go_forward(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn hard_reload(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn stop_loading(&mut self, cx: &mut Context<Self>) {
        self.loading = false;
        cx.notify();
    }

    fn webview_copy(&self) {}
    fn webview_cut(&self) {}
    fn webview_paste(&self) {}
    fn webview_select_all(&self) {}

    fn open_external(&self, cx: &mut Context<Self>) {
        if let Some(url) = &self.current_url {
            cx.open_url(url);
        }
    }

    fn toolbar_button(
        &self,
        id: ElementId,
        icon_path: &'static str,
        enabled: bool,
        tooltip: String,
        theme: Theme,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let base = div()
            .id(id)
            .size(px(26.0))
            .rounded(px(6.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center();
        if !enabled {
            return base
                .cursor_default()
                .child(icon(icon_path, 14.0, theme.text_ghost));
        }
        base.cursor_pointer()
            .hover(|element| element.bg(theme.overlay))
            .active(|element| element.bg(theme.overlay_strong))
            .child(icon(icon_path, 14.0, theme.text_secondary))
            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            .on_click(cx.listener(move |this, _, window, cx| {
                on_click(this, window, cx);
            }))
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let has_page = self.navigation_requested;
        let secure = self.current_url.as_deref().is_some_and(is_secure_url);

        div()
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .px(px(10.0))
            .flex()
            .items_center()
            .gap(px(2.0))
            .border_b_1()
            .border_color(theme.border)
            .child(self.toolbar_button(
                "browser-back".into(),
                "icons/arrow-left.svg",
                self.can_go_back,
                tr!(
                    "browser.back",
                    shortcut = crate::platform::primary_shortcut("⌘[", "Ctrl+[")
                ),
                theme,
                |this, _, cx| this.go_back(cx),
                cx,
            ))
            .child(self.toolbar_button(
                "browser-forward".into(),
                "icons/arrow-right.svg",
                self.can_go_forward,
                tr!(
                    "browser.forward",
                    shortcut = crate::platform::primary_shortcut("⌘]", "Ctrl+]")
                ),
                theme,
                |this, _, cx| this.go_forward(cx),
                cx,
            ))
            .child(if self.loading {
                self.toolbar_button(
                    "browser-stop".into(),
                    "icons/x.svg",
                    true,
                    tr!("browser.stop_loading"),
                    theme,
                    |this, _, cx| this.stop_loading(cx),
                    cx,
                )
            } else {
                self.toolbar_button(
                    "browser-reload".into(),
                    "icons/rotate-cw.svg",
                    has_page,
                    tr!(
                        "browser.reload",
                        shortcut = crate::platform::primary_shortcut("⌘R", "Ctrl+R")
                    ),
                    theme,
                    |this, _, cx| this.reload(cx),
                    cx,
                )
            })
            .child(
                TextField::new("browser-address", self.address.clone())
                    .icon(
                        if secure {
                            "icons/lock.svg"
                        } else {
                            "icons/globe.svg"
                        },
                        11.0,
                    )
                    .key_context("BrowserAddress")
                    .on_action(cx.listener(|this, _: &crate::BrowserAddressCancel, _, cx| {
                        this.restore_address(cx);
                    }))
                    .min_w_0()
                    .flex_1()
                    .mx(px(4.0))
                    .relative(),
            )
            .child(self.toolbar_button(
                "browser-open-external".into(),
                "icons/external-link.svg",
                has_page,
                tr!("browser.open_external"),
                theme,
                |this, _, cx| this.open_external(cx),
                cx,
            ))
    }

    fn render_start_page(&self, theme: Theme) -> Div {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px(px(48.0))
            .pb(px(40.0))
            .child(icon("icons/globe.svg", 24.0, theme.text_ghost))
            .child(
                div()
                    .mt(px(14.0))
                    .text_size(sp(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(tr!("browser.browse_web")),
            )
            .child(
                div()
                    .mt(px(6.0))
                    .max_w(px(310.0))
                    .text_center()
                    .text_size(sp(12.0))
                    .text_color(theme.text_tertiary)
                    .child(tr!("browser.browse_web_hint")),
            )
    }

    fn render_page_area(&self, theme: Theme) -> Div {
        let url = self.current_url.clone().unwrap_or_default();
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .bg(theme.surface)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p(px(24.0))
            .child(
                div()
                    .max_w(px(480.0))
                    .p(px(20.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.raised)
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(sp(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("Browser Engine Decoupled"),
                    )
                    .child(
                        div()
                            .text_size(sp(13.0))
                            .text_color(theme.text_secondary)
                            .child("The embedded browser surface is being upgraded to a standalone, more capable engine. Target URL:"),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(6.0))
                            .rounded(px(6.0))
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.border)
                            .text_size(sp(12.0))
                            .text_color(theme.text)
                            .child(url),
                    ),
            )
    }
}

impl Focusable for BrowserView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for BrowserView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        let body = if self.navigation_requested {
            self.render_page_area(theme).into_any_element()
        } else {
            self.render_start_page(theme).into_any_element()
        };

        div()
            .id("browser-surface")
            .track_focus(&self.focus_handle)
            .key_context("Browser")
            .on_action(cx.listener(|this, _: &BrowserBack, _, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &BrowserForward, _, cx| this.go_forward(cx)))
            .on_action(cx.listener(|this, _: &BrowserReload, _, cx| this.reload(cx)))
            .on_action(cx.listener(|this, _: &BrowserHardReload, _, cx| this.hard_reload(cx)))
            .on_action(cx.listener(|this, _: &BrowserStop, _, cx| this.stop_loading(cx)))
            .on_action(cx.listener(|this, _: &FocusBrowserAddress, window, cx| {
                this.focus_address(window, cx);
            }))
            .on_action(cx.listener(|this, _: &WebviewCopy, _, _| this.webview_copy()))
            .on_action(cx.listener(|this, _: &WebviewCut, _, _| this.webview_cut()))
            .on_action(cx.listener(|this, _: &WebviewPaste, _, _| this.webview_paste()))
            .on_action(cx.listener(|this, _: &WebviewSelectAll, _, _| this.webview_select_all()))
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.render_toolbar(cx))
            .child(body)
    }
}
