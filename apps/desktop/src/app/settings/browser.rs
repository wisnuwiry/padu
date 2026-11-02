//! The browser section: in-app browser preferences under Integrations.

use super::*;
use crate::browser::Browsing;
use crate::theme::sp;
use crate::ui::button::{Button, ButtonRole, ButtonStyle};
use ::browser::{DataStore, Usage};
use bezel::{
    gpui::{AnyElement, Context, Div, Task, div, px},
    theme::Theme as BezelTheme,
    ui::combobox::{Combobox, ComboboxEvent},
    ui::widgets::Controls as _,
};

#[derive(Default)]
pub(crate) struct BrowserData {
    pub(crate) usage: Option<Option<Usage>>,
    pub(crate) result: Option<bool>,
    pub(crate) request: Option<Task<()>>,
}

impl Padu {
    pub(super) fn render_browser_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let bezel_theme = BezelTheme::of(cx).clone();
        let browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        div()
            .pt(px(20.0))
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                bezel_theme
                    .group_box()
                    .child(self.render_browser_engine_row(&browsing, cx))
                    .child(self.render_browser_home_row(&browsing, cx)),
            )
            .child(
                bezel_theme
                    .group_box()
                    .child(self.render_browser_agents_read_row(&browsing, cx))
                    .child(self.render_browser_agents_act_row(&browsing, cx))
                    .child(self.render_browser_agents_blocked_row(&browsing, cx)),
            )
            .child(
                bezel_theme
                    .group_box()
                    .child(self.render_browser_keep_signed_in_row(&browsing, cx))
                    .when(browsing.keep_signed_in, |group| {
                        group.child(self.render_browser_clear_row(cx))
                    }),
            )
            .into_any_element()
    }

    /// Select search engine with custom options.
    fn render_browser_engine_row(&self, browsing: &Browsing, cx: &mut Context<Self>) -> AnyElement {
        let bezel_theme = BezelTheme::of(cx).clone();
        if self.adding_custom_engine {
            return bezel_theme
                .card_row(false)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(bezel_theme.row_title(tr!("settings.browser_add_search_engine")))
                        .child(
                            div()
                                .text_size(sp(12.5))
                                .text_color(bezel_theme.text_muted)
                                .child(tr!("settings.browser_add_search_engine_hint")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .w(px(160.0))
                                        .child(self.custom_engine_name_input.clone()),
                                )
                                .child(div().flex_1().child(self.custom_engine_url_input.clone()))
                                .child(
                                    Button::new("browser-custom-engine-save", tr!("common.save"))
                                        .button_style(ButtonStyle::Prominent)
                                        .on_press(cx.listener(|this, _, _, cx| {
                                            let name = this
                                                .custom_engine_name_input
                                                .read(cx)
                                                .content()
                                                .trim()
                                                .to_owned();
                                            let url = this
                                                .custom_engine_url_input
                                                .read(cx)
                                                .content()
                                                .trim()
                                                .to_owned();
                                            if !name.is_empty() && !url.is_empty() {
                                                this.add_custom_search_engine(name, url, cx);
                                            }
                                            this.adding_custom_engine = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(
                                        "browser-custom-engine-cancel",
                                        tr!("common.cancel"),
                                    )
                                    .button_style(ButtonStyle::Ghost)
                                    .on_press(cx.listener(
                                        |this, _, _, cx| {
                                            this.adding_custom_engine = false;
                                            this.browser_search_combobox =
                                                Self::create_browser_search_combobox(cx);
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                )
                .into_any_element();
        }

        let engine_name = browsing.engine_name_for_search();
        bezel_theme
            .card_row(false)
            .child(labelled(
                &bezel_theme,
                tr!("settings.browser_search_engine"),
                tr!(
                    "settings.browser_search_engine_description",
                    engine = &engine_name
                ),
                false,
            ))
            .child(
                div()
                    .w(px(200.0))
                    .flex_none()
                    .child(self.browser_search_combobox.clone()),
            )
            .into_any_element()
    }

    pub(crate) fn create_browser_search_combobox(cx: &mut Context<Self>) -> Entity<Combobox> {
        let browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        let all_engines = browsing.all_engines();
        let mut items = all_engines
            .iter()
            .map(|(name, _)| SharedString::from(name.clone()))
            .collect::<Vec<_>>();
        items.push(SharedString::from(tr!(
            "settings.browser_add_search_engine"
        )));

        let selected_index = all_engines
            .iter()
            .position(|(_, template)| *template == browsing.search);

        let combobox = cx.new(|cx| {
            let mut combo = Combobox::new(items, tr!("settings.browser_search_engine"), cx);
            if let Some(index) = selected_index {
                combo = combo.with_selection(index);
            }
            combo
        });

        cx.subscribe(
            &combobox,
            move |this: &mut Self, _, event: &ComboboxEvent, cx| match event {
                ComboboxEvent::Selected(index) => {
                    let browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
                    let all_engines = browsing.all_engines();
                    if *index < all_engines.len() {
                        let template = all_engines[*index].1.clone();
                        this.set_browser_search(template, cx);
                        this.adding_custom_engine = false;
                    } else if *index == all_engines.len() {
                        this.adding_custom_engine = true;
                        this.custom_engine_name_input
                            .update(cx, |input, cx| input.clear(cx));
                        this.custom_engine_url_input
                            .update(cx, |input, cx| input.clear(cx));
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        combobox
    }

    pub(crate) fn add_custom_search_engine(
        &mut self,
        name: String,
        mut url: String,
        cx: &mut Context<Self>,
    ) {
        if !url.contains("://") && !url.starts_with("about:") {
            url = format!("https://{url}");
        }
        if !url.contains("%s") {
            if url.ends_with('/') || url.ends_with('=') {
                url.push_str("%s");
            } else if url.contains('?') {
                url.push_str("&q=%s");
            } else {
                url.push_str("/?q=%s");
            }
        }
        let mut browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        browsing.custom_engines.push((name, url.clone()));
        browsing.search = url;
        browsing.save();
        cx.set_global(browsing);
        self.browser_search_combobox = Self::create_browser_search_combobox(cx);
        cx.notify();
    }

    fn render_browser_home_row(&self, browsing: &Browsing, cx: &mut Context<Self>) -> AnyElement {
        let bezel_theme = BezelTheme::of(cx).clone();
        if self.editing_browser_home {
            return bezel_theme
                .card_row(false)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(bezel_theme.row_title(tr!("settings.browser_home")))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(div().flex_1().child(self.browser_home_input.clone()))
                                .child(
                                    Button::new("browser-home-save", tr!("common.save"))
                                        .button_style(ButtonStyle::Prominent)
                                        .on_press(cx.listener(|this, _, _, cx| {
                                            let typed = this
                                                .browser_home_input
                                                .read(cx)
                                                .content()
                                                .to_owned();
                                            this.save_browser_home(&typed, cx);
                                            this.editing_browser_home = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("browser-home-cancel", tr!("common.cancel"))
                                        .button_style(ButtonStyle::Ghost)
                                        .on_press(cx.listener(|this, _, _, cx| {
                                            this.editing_browser_home = false;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .into_any_element();
        }

        let home = browsing.home.clone();
        bezel_theme
            .card_row(false)
            .child(labelled(
                &bezel_theme,
                tr!("settings.browser_home"),
                div().child(tr!("settings.browser_home_description")).child(
                    div()
                        .mt(px(2.0))
                        .text_color(bezel_theme.text)
                        .child(home.clone()),
                ),
                false,
            ))
            .child(
                Button::new("browser-home-change", tr!("common.change"))
                    .button_style(ButtonStyle::Ghost)
                    .on_press(cx.listener(move |this, _, window, cx| {
                        this.editing_browser_home = true;
                        this.browser_home_input.update(cx, |input, cx| {
                            input.set_content(&home, cx);
                            input.select_all_text(cx);
                        });
                        let focus = this.browser_home_input.read(cx).focus();
                        window.focus(&focus, cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_browser_agents_read_row(
        &self,
        browsing: &Browsing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on = browsing.agents_read;
        self.browser_switch_row(
            "browser-agents-read",
            tr!("settings.browser_agents_read"),
            tr!("settings.browser_agents_read_description"),
            on,
            true,
            false,
            cx,
            move |this, cx| {
                this.set_browser_agents_read(!on, cx);
            },
        )
    }

    /// Read only while [`Self::render_browser_agents_read_row`] is on.
    fn render_browser_agents_act_row(
        &self,
        browsing: &Browsing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on = browsing.agents_act;
        let disabled = !browsing.agents_read;
        self.browser_switch_row(
            "browser-agents-act",
            tr!("settings.browser_agents_act"),
            tr!("settings.browser_agents_act_description"),
            on && !disabled,
            false,
            disabled,
            cx,
            move |this, cx| {
                if !disabled {
                    this.set_browser_agents_act(!on, cx);
                }
            },
        )
    }

    fn render_browser_agents_blocked_row(
        &self,
        browsing: &Browsing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let bezel_theme = BezelTheme::of(cx).clone();
        let blocked_str = browsing.agents_blocked.join(", ");
        if self.editing_browser_blocked {
            return bezel_theme
                .card_row(false)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(bezel_theme.row_title(tr!("settings.browser_blocked")))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(div().flex_1().child(self.browser_blocked_input.clone()))
                                .child(
                                    Button::new("browser-blocked-save", tr!("common.save"))
                                        .button_style(ButtonStyle::Prominent)
                                        .on_press(cx.listener(|this, _, _, cx| {
                                            let typed = this
                                                .browser_blocked_input
                                                .read(cx)
                                                .content()
                                                .to_owned();
                                            this.save_browser_blocked(&typed, cx);
                                            this.editing_browser_blocked = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("browser-blocked-cancel", tr!("common.cancel"))
                                        .button_style(ButtonStyle::Ghost)
                                        .on_press(cx.listener(|this, _, _, cx| {
                                            this.editing_browser_blocked = false;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .into_any_element();
        }

        let current = blocked_str.clone();
        bezel_theme
            .card_row(false)
            .child(labelled(
                &bezel_theme,
                tr!("settings.browser_blocked"),
                div()
                    .child(tr!("settings.browser_blocked_description"))
                    .child(div().mt(px(2.0)).text_color(bezel_theme.text).child(
                        if blocked_str.is_empty() {
                            tr!("settings.browser_blocked_none").to_string()
                        } else {
                            blocked_str
                        },
                    )),
                false,
            ))
            .child(
                Button::new("browser-blocked-change", tr!("common.change"))
                    .button_style(ButtonStyle::Ghost)
                    .on_press(cx.listener(move |this, _, window, cx| {
                        this.editing_browser_blocked = true;
                        this.browser_blocked_input.update(cx, |input, cx| {
                            input.set_content(&current, cx);
                            input.select_all_text(cx);
                        });
                        let focus = this.browser_blocked_input.read(cx).focus();
                        window.focus(&focus, cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_browser_keep_signed_in_row(
        &self,
        browsing: &Browsing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on = browsing.keep_signed_in;
        self.browser_switch_row(
            "browser-keep",
            tr!("settings.browser_keep_signed_in"),
            tr!("settings.browser_keep_signed_in_description"),
            on,
            true,
            false,
            cx,
            move |this, cx| {
                this.set_browser_keep_signed_in(!on, cx);
            },
        )
    }

    fn render_browser_clear_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let bezel_theme = BezelTheme::of(cx).clone();
        let clearing = crate::browser::clearing(cx);
        let note = if clearing {
            tr!("settings.browser_clearing").to_string()
        } else if self.browser_data.result == Some(false) {
            tr!("settings.browser_clear_failed").to_string()
        } else if self.browser_data.result == Some(true) {
            tr!("settings.browser_cleared").to_string()
        } else if !cfg!(target_os = "macos") {
            tr!("settings.browser_clear_unavailable").to_string()
        } else {
            let amount = match &self.browser_data.usage {
                None => tr!("settings.browser_calculating").to_string(),
                Some(None) => tr!("settings.browser_data_unavailable").to_string(),
                Some(Some(usage)) => match usage.sites.len() {
                    0 => tr!("settings.browser_no_data").to_string(),
                    1 => tr!("settings.browser_one_site").to_string(),
                    count => tr!("settings.browser_sites_data", count = count),
                },
            };
            format!("{amount} {}", tr!("settings.browser_clear_footer"))
        };

        bezel_theme
            .card_row(false)
            .child(labelled(
                &bezel_theme,
                tr!("settings.browser_data"),
                note,
                false,
            ))
            .child(
                Button::new(
                    "browser-clear",
                    if clearing {
                        tr!("settings.browser_clearing")
                    } else {
                        tr!("settings.browser_clear_data")
                    },
                )
                .button_style(ButtonStyle::Ghost)
                .role(ButtonRole::Destructive)
                .enabled(!clearing && cfg!(target_os = "macos"))
                .on_press(cx.listener(|this, _, _, cx| {
                    this.clear_browsing_data(cx);
                })),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn browser_switch_row(
        &self,
        id: &'static str,
        title: impl Into<SharedString>,
        blurb: impl Into<SharedString>,
        on: bool,
        first: bool,
        disabled: bool,
        cx: &Context<Self>,
        flip: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let bezel_theme = BezelTheme::of(cx).clone();
        bezel_theme
            .card_row(first)
            .child(labelled(&bezel_theme, title, blurb.into(), false))
            .child(
                div()
                    .id(id)
                    .when(!disabled, |d| {
                        d.cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                flip(this, cx);
                            }))
                    })
                    .when(disabled, |d| d.opacity(0.5))
                    .child(bezel_theme.toggle(on)),
            )
            .into_any_element()
    }

    pub(crate) fn load_browser_usage(&mut self, cx: &mut Context<Self>) {
        if crate::browser::clearing(cx) {
            return;
        }
        self.browser_data.usage = None;
        let usage = DataStore::new().usage(cx);
        self.browser_data.request = Some(cx.spawn(async move |this, cx| {
            let usage = usage.await;
            let _ = this.update(cx, |this, cx| {
                this.browser_data.usage = Some(usage);
                cx.notify();
            });
        }));
    }

    pub(crate) fn clear_browsing_data(&mut self, cx: &mut Context<Self>) {
        if crate::browser::clearing(cx) {
            return;
        }
        self.browser_data.request = None;
        self.browser_data.result = None;
        crate::browser::set_clearing(true, cx);
        let this = cx.entity().downgrade();
        cx.defer(move |cx| {
            let close_res = this.update(cx, |this, cx| {
                this.close_all_right_panel_browsers(cx);
            });
            if close_res.is_err() {
                crate::browser::set_clearing(false, cx);
                let _ = this.update(cx, |this, cx| {
                    this.browser_data.result = Some(false);
                    this.load_browser_usage(cx);
                    cx.notify();
                });
                return;
            }
            // Release closed entities before touching their store; finish even
            // if Settings is closed while the operation is running.
            cx.defer(move |cx| {
                let store = DataStore::new().clear(cx);
                cx.spawn(async move |cx| {
                    let cleared = store.await;
                    cx.update(|cx| crate::browser::set_clearing(false, cx));
                    let _ = this.update(cx, |this, cx| {
                        this.browser_data.result = Some(cleared);
                        this.load_browser_usage(cx);
                        cx.notify();
                    });
                })
                .detach();
            });
        });
        cx.notify();
    }

    pub(crate) fn close_all_right_panel_browsers(&mut self, cx: &mut Context<Self>) {
        let browser_indices: Vec<usize> = self
            .right_panel_surfaces
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                if s.browser_id().is_some() {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        for index in browser_indices.into_iter().rev() {
            self.close_right_panel_surface(index, cx);
        }
        let remaining: Vec<Uuid> = self.right_panel_browsers.keys().copied().collect();
        for id in remaining {
            self.destroy_right_panel_browser(id, cx);
        }
        crate::browser::forget_all(cx);
    }

    /// Take the typed address. A bare host is taken over https; an empty
    /// field leaves the home page as it was.
    pub(super) fn save_browser_home(&mut self, typed: &str, cx: &mut Context<Self>) {
        let typed = typed.trim();
        if typed.is_empty() {
            return;
        }
        let home = match typed.contains("://") || typed.starts_with("about:") {
            true => typed.to_owned(),
            false => format!("https://{typed}"),
        };
        self.set_browser_home(home, cx);
    }

    /// Take the typed hosts: split on commas and whitespace, a URL cut to its
    /// host.
    pub(super) fn save_browser_blocked(&mut self, typed: &str, cx: &mut Context<Self>) {
        let mut hosts: Vec<String> = typed
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                url::Url::parse(entry)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
                    .unwrap_or_else(|| entry.to_owned())
                    .to_ascii_lowercase()
            })
            .collect();
        hosts.sort();
        hosts.dedup();
        self.set_browser_agents_blocked(hosts, cx);
    }

    pub(crate) fn set_browser_search(&mut self, search: String, cx: &mut Context<Self>) {
        let mut browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        browsing.search = search;
        browsing.save();
        cx.set_global(browsing);
        self.browser_search_combobox = Self::create_browser_search_combobox(cx);
        cx.notify();
    }

    pub(crate) fn set_browser_home(&mut self, home: String, cx: &mut Context<Self>) {
        let mut browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        browsing.home = home;
        browsing.save();
        cx.set_global(browsing);
        cx.notify();
    }

    pub(crate) fn set_browser_agents_read(&mut self, agents_read: bool, cx: &mut Context<Self>) {
        let mut browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        browsing.agents_read = agents_read;
        if !agents_read {
            browsing.agents_act = false;
        }
        browsing.save();
        cx.set_global(browsing);
        cx.notify();
    }

    pub(crate) fn set_browser_agents_act(&mut self, agents_act: bool, cx: &mut Context<Self>) {
        let mut browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        browsing.agents_act = agents_act;
        browsing.save();
        cx.set_global(browsing);
        cx.notify();
    }

    pub(crate) fn set_browser_agents_blocked(
        &mut self,
        blocked: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let mut browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        browsing.agents_blocked = blocked;
        browsing.save();
        cx.set_global(browsing);
        cx.notify();
    }

    pub(crate) fn set_browser_keep_signed_in(
        &mut self,
        keep_signed_in: bool,
        cx: &mut Context<Self>,
    ) {
        let mut browsing = cx.try_global::<Browsing>().cloned().unwrap_or_default();
        browsing.keep_signed_in = keep_signed_in;
        browsing.save();
        cx.set_global(browsing);
        cx.notify();
    }
}

/// A row's title over a line of explanation, filling the row's free width.
fn labelled(
    theme: &BezelTheme,
    title: impl Into<SharedString>,
    blurb: impl IntoElement,
    truncate: bool,
) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(theme.row_title(title))
        .child(
            div()
                .mt(px(4.0))
                .when(truncate, |el| el.truncate())
                .text_size(sp(12.5))
                .line_height(sp(17.0))
                .text_color(theme.text_muted)
                .child(blurb),
        )
}
