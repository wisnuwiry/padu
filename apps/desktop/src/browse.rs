//! What an agent does in a project's browser tabs: open a page, read it, act
//! on it — the window's half of [`padu_mcp::tools::browser`].

use std::{fmt::Write as _, path::Path, time::Duration};

use bezel::gpui::{AnyWindowHandle, AsyncApp, Context, Entity, WeakEntity, Window};
use browser::{ConsoleMessage, WebView};
use futures::{StreamExt as _, channel::mpsc};
use padu_mcp::rail::{self, Act, Browse};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{
    app::{Padu, RightPanelSurface},
    browser::{Browser, Browsing},
};

const MAX_TEXT: usize = 8000;
const MAX_ELEMENTS: usize = 200;
const EVAL: Duration = Duration::from_secs(10);
const BUILT: Duration = Duration::from_secs(3);
const SETTLE: Duration = Duration::from_secs(10);
const STEP: Duration = Duration::from_millis(100);

const FIND: &str = r#"const found = (() => {
  const pick = 'a[href], button, input, select, textarea, summary, [role=button], [role=link], [role=tab], [role=checkbox], [role=menuitem], [role=textbox], [contenteditable=""], [contenteditable=true], [onclick]';
  const out = [];
  for (const el of document.querySelectorAll(pick)) {
    if (out.length >= $MAX_ELEMENTS) break;
    if (el.disabled) continue;
    const box = el.getBoundingClientRect();
    if (box.width < 1 || box.height < 1) continue;
    const style = getComputedStyle(el);
    if (style.visibility === 'hidden' || style.opacity === '0') continue;
    out.push(el);
  }
  return out;
})();"#;

const SNAPSHOT: &str = r#"(() => { try {
  $FIND
  const listed = found.map((el, i) => {
    const tail = el.tagName === 'A' ? (el.getAttribute('href') || '').split(/[?#]/)[0].split('/').filter(Boolean).pop() : '';
    const name = (el.getAttribute('aria-label') || el.placeholder || el.innerText || el.value || el.alt || el.title || tail || '').replace(/\s+/g, ' ').trim().slice(0, 100);
    return { i, tag: el.tagName.toLowerCase(), kind: el.type || el.getAttribute('role') || null, name };
  });
  const text = (document.body ? document.body.innerText : '').replace(/\n\s*\n\s*\n+/g, '\n\n').trim();
  return { url: location.href, title: document.title, text: text.slice(0, $MAX_TEXT), truncated: text.length > $MAX_TEXT, elements: listed };
} catch (e) { return { url: location.href, title: document.title, text: String(e), elements: [] }; } })()"#;

const CLICK: &str = r#"(() => { try {
  $FIND
  const el = found[$N];
  if (!el) return 'gone';
  el.scrollIntoView({ block: 'center' });
  el.click();
  return 'ok';
} catch (e) { return String(e); } })()"#;

const FILL: &str = r#"(() => { try {
  $FIND
  const el = found[$N];
  if (!el) return 'gone';
  el.scrollIntoView({ block: 'center' });
  el.focus();
  const text = $TEXT;
  if (el.isContentEditable) el.textContent = text;
  else {
    const own = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value');
    if (own && own.set) own.set.call(el, text); else el.value = text;
  }
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
  if ($ENTER) {
    const key = { key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true, cancelable: true };
    const handled = !el.dispatchEvent(new KeyboardEvent('keydown', key));
    el.dispatchEvent(new KeyboardEvent('keypress', key));
    el.dispatchEvent(new KeyboardEvent('keyup', key));
    if (!handled && el.form && el.form.requestSubmit) el.form.requestSubmit();
  }
  return 'ok';
} catch (e) { return String(e); } })()"#;

const SCROLL: &str = r#"(() => { try {
  const page = document.scrollingElement || document.documentElement;
  const was = page.scrollTop;
  page.scrollTop += window.innerHeight * $PAGES;
  if (page.scrollTop === was) {
    const inner = [...document.querySelectorAll('*')]
      .filter((el) => el.scrollHeight > el.clientHeight + 20 && /auto|scroll/.test(getComputedStyle(el).overflowY))
      .sort((a, b) => b.clientHeight - a.clientHeight)[0];
    if (inner) inner.scrollTop += inner.clientHeight * $PAGES;
  }
  return 'ok';
} catch (e) { return String(e); } })()"#;

#[derive(Deserialize)]
struct Page {
    url: String,
    title: String,
    text: String,
    #[serde(default)]
    truncated: bool,
    elements: Vec<Element>,
}

#[derive(Deserialize)]
struct Element {
    i: usize,
    tag: String,
    kind: Option<String>,
    name: String,
}

fn script(js: &str) -> String {
    js.replace("$FIND", FIND)
        .replace("$MAX_ELEMENTS", &MAX_ELEMENTS.to_string())
        .replace("$MAX_TEXT", &MAX_TEXT.to_string())
}

pub(crate) fn take_browser(window: &Window, cx: &mut Context<Padu>) {
    let window_handle = window.window_handle();
    let (asked, mut asks) = mpsc::unbounded::<Browse>();
    rail::install_browser(move |browse| {
        let _ = asked.unbounded_send(browse);
    });
    cx.spawn(async move |this, cx| {
        while let Some(Browse {
            project,
            tab,
            act,
            reply,
        }) = asks.next().await
        {
            let answer = serve(&this, window_handle, &project, tab, act, cx).await;
            let _ = reply.send(answer);
            if this.upgrade().is_none() {
                return;
            }
        }
    })
    .detach();
}

async fn serve(
    this: &WeakEntity<Padu>,
    window: AnyWindowHandle,
    _project: &Path,
    tab: Option<u64>,
    act: Act,
    cx: &mut AsyncApp,
) -> Result<String, String> {
    let browsing = cx.update(|cx| cx.try_global::<Browsing>().cloned().unwrap_or_default());
    if !browsing.agents_read {
        return Err("browser reading tools are switched off in Padu's settings".to_owned());
    }
    if let Act::Click(_) | Act::Type { .. } = &act {
        if !browsing.agents_act {
            return Err(
                "browser click and typing tools are switched off in Padu's settings".to_owned(),
            );
        }
    }

    if let Act::Open(url) = &act {
        refuse_blocked(&browsing, url)?;
    }

    match act {
        Act::Tabs => this
            .update(cx, |padu, cx| tabs(padu, cx))
            .map_err(|e| e.to_string()),
        Act::Open(url) => {
            refuse_blocked(&browsing, &url)?;
            let browser = match tab {
                Some(id) => {
                    let browser = pick(this, Some(id), cx)?;
                    reveal_browser_tab(this, &browser, &window, cx)?;
                    browser.update(cx, |browser, cx| browser.navigate_to_url(url, cx));
                    browser
                }
                None => match showing(this, &url, cx) {
                    Some(browser) => {
                        reveal_browser_tab(this, &browser, &window, cx)?;
                        browser
                    }
                    None => {
                        let uuid = uuid::Uuid::new_v4();
                        let browser = window
                            .update(cx, |_, window, cx| {
                                this.update(cx, |padu, cx| {
                                    padu.open_right_panel_surface(
                                        RightPanelSurface::Browser(uuid),
                                        cx,
                                    );
                                    let browser = padu.ensure_right_panel_browser(uuid, window, cx);
                                    let _ = browser.update(cx, |b, cx| b.page(window, cx));
                                    browser.update(cx, |b, cx| b.navigate_to_url(url, cx));
                                    browser
                                })
                                .map_err(|e| e.to_string())
                            })
                            .map_err(|e| e.to_string())??;
                        browser
                    }
                },
            };
            let page = built(&browser, cx).await?;
            settle(&page, cx).await;
            read(&browser, &page, cx).await
        }
        Act::Read => {
            let browser = match pick(this, tab, cx) {
                Ok(browser) => browser,
                Err(_) if tab.is_none() => {
                    let home = cx.update(|cx| crate::browser::home(cx));
                    let uuid = uuid::Uuid::new_v4();
                    window
                        .update(cx, |_, window, cx| {
                            this.update(cx, |padu, cx| {
                                padu.open_right_panel_surface(RightPanelSurface::Browser(uuid), cx);
                                let browser = padu.ensure_right_panel_browser(uuid, window, cx);
                                let _ = browser.update(cx, |b, cx| b.page(window, cx));
                                browser.update(cx, |b, cx| b.navigate_to_url(home, cx));
                                browser
                            })
                            .map_err(|e| e.to_string())
                        })
                        .map_err(|e| e.to_string())??
                }
                Err(e) => return Err(e),
            };
            reveal_browser_tab(this, &browser, &window, cx)?;
            let page = built(&browser, cx).await?;
            settle(&page, cx).await;
            read(&browser, &page, cx).await
        }
        Act::Click(element) => {
            let js = script(CLICK).replace("$N", &element.to_string());
            act_on(this, tab, &js, element, &window, cx).await
        }
        Act::Type {
            element,
            text,
            enter,
        } => {
            let js = script(FILL)
                .replace("$N", &element.to_string())
                .replace(
                    "$TEXT",
                    &serde_json::to_string(&text).unwrap_or_else(|_| "''".into()),
                )
                .replace("$ENTER", if enter { "true" } else { "false" });
            act_on(this, tab, &js, element, &window, cx).await
        }
        Act::Console => {
            let browser = pick(this, tab, cx)?;
            reveal_browser_tab(this, &browser, &window, cx)?;
            Ok(browser.read_with(cx, |browser, cx| console(&browser.console(cx))))
        }
        Act::Scroll(pages) => {
            let js = SCROLL.replace("$PAGES", &format!("{:.2}", pages.clamp(-20., 20.)));
            act_on(this, tab, &js, 0, &window, cx).await
        }
    }
}

fn refuse_blocked(browsing: &Browsing, url: &str) -> Result<(), String> {
    match browsing.blocks(url) {
        true => Err(format!(
            "{url} is on a site the user has kept agents off in Padu's settings"
        )),
        false => Ok(()),
    }
}

fn reveal_browser_tab(
    this: &WeakEntity<Padu>,
    browser: &Entity<Browser>,
    window: &AnyWindowHandle,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let browser_id = browser.read_with(cx, |b, _| b.id);
    window
        .update(cx, |_, window, cx| {
            this.update(cx, |padu, cx| {
                let matching = padu
                    .right_panel_browsers
                    .iter()
                    .find(|(_, b)| b.read(cx).id == browser_id)
                    .map(|(uuid, b)| (*uuid, b.clone()));
                if let Some((uuid, b)) = matching {
                    padu.open_right_panel_surface(RightPanelSurface::Browser(uuid), cx);
                    let _ = b.update(cx, |b, cx| b.page(window, cx));
                }
            })
            .map_err(|e| e.to_string())
        })
        .map_err(|e| e.to_string())?
}

async fn act_on(
    this: &WeakEntity<Padu>,
    tab: Option<u64>,
    js: &str,
    element: usize,
    window: &AnyWindowHandle,
    cx: &mut AsyncApp,
) -> Result<String, String> {
    let browser = pick(this, tab, cx)?;
    reveal_browser_tab(this, &browser, window, cx)?;
    let page = built(&browser, cx).await?;
    match eval::<String>(&page, js, cx).await?.as_str() {
        "ok" => {}
        "gone" => {
            return Err(format!(
                "element {element} is not on the page any more; read the tab for its current elements"
            ));
        }
        other => return Err(other.to_owned()),
    }
    cx.background_executor()
        .timer(Duration::from_millis(300))
        .await;
    settle(&page, cx).await;
    read(&browser, &page, cx).await
}

fn pick(
    this: &WeakEntity<Padu>,
    tab: Option<u64>,
    cx: &mut AsyncApp,
) -> Result<Entity<Browser>, String> {
    this.update(cx, |padu, cx| {
        let active_uuid = match padu.active_right_panel_surface() {
            Some(RightPanelSurface::Browser(uuid)) => Some(*uuid),
            _ => None,
        };
        if let Some(id) = tab {
            for (_, browser) in &padu.right_panel_browsers {
                if browser.read(cx).id == id {
                    return Ok(browser.clone());
                }
            }
            return Err(format!(
                "no browser tab {id} in this project; browser_tabs lists them"
            ));
        }
        if let Some(uuid) = active_uuid {
            if let Some(browser) = padu.right_panel_browsers.get(&uuid) {
                return Ok(browser.clone());
            }
        }
        if let Some(browser) = padu.right_panel_browsers.values().next() {
            return Ok(browser.clone());
        }
        Err("no browser tab open in this project; browser_open opens one".to_owned())
    })
    .map_err(|e| e.to_string())?
}

fn showing(this: &WeakEntity<Padu>, url: &str, cx: &mut AsyncApp) -> Option<Entity<Browser>> {
    let wanted = same_page(url);
    this.update(cx, |padu, cx| {
        for browser in padu.right_panel_browsers.values() {
            if same_page(browser.read(cx).url()) == wanted {
                return Some(browser.clone());
            }
        }
        None
    })
    .ok()
    .flatten()
}

fn same_page(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut url) => {
            url.set_fragment(None);
            url.into()
        }
        Err(_) => url.to_owned(),
    }
}

async fn built(browser: &Entity<Browser>, cx: &mut AsyncApp) -> Result<Entity<WebView>, String> {
    let mut waited = Duration::ZERO;
    loop {
        if let Some(page) = browser.read_with(cx, |browser, _| browser.webview()) {
            return Ok(page);
        }
        if waited >= BUILT {
            return Err(
                "the tab is open in the right panel, but its page loads only once that panel is on screen"
                    .to_owned(),
            );
        }
        cx.background_executor().timer(STEP).await;
        waited += STEP;
    }
}

async fn settle(page: &Entity<WebView>, cx: &mut AsyncApp) {
    let mut waited = Duration::ZERO;
    while waited < SETTLE && page.read_with(cx, |page, _| page.is_loading()) {
        cx.background_executor().timer(STEP).await;
        waited += STEP;
    }
}

async fn eval<T: DeserializeOwned + 'static>(
    page: &Entity<WebView>,
    js: &str,
    cx: &mut AsyncApp,
) -> Result<T, String> {
    let task = page.read_with(cx, |page, cx| page.eval::<T>(js, EVAL, cx));
    task.await
        .map_err(|error| format!("the page did not answer: {error}"))
}

async fn read(
    browser: &Entity<Browser>,
    page: &Entity<WebView>,
    cx: &mut AsyncApp,
) -> Result<String, String> {
    let id = browser.read_with(cx, |browser, _| browser.id);
    let page: Page = eval(page, &script(SNAPSHOT), cx).await?;
    let browsing = cx.update(|cx| cx.try_global::<Browsing>().cloned().unwrap_or_default());
    refuse_blocked(&browsing, &page.url)?;
    Ok(described(id, &page))
}

fn described(id: u64, page: &Page) -> String {
    let mut out = format!("tab {id}: {}\n{}\n\n{}", page.title, page.url, page.text);
    if page.truncated {
        out.push_str("\n\n[text truncated; scroll to reach the rest]");
    }
    if !page.elements.is_empty() {
        out.push_str("\n\nelements:");
        for element in &page.elements {
            let _ = write!(out, "\n[{}] {}", element.i, element.tag);
            if let Some(kind) = &element.kind {
                let _ = write!(out, " ({kind})");
            }
            if !element.name.is_empty() {
                let _ = write!(out, " {}", element.name);
            }
        }
    }
    out
}

fn console(log: &[ConsoleMessage]) -> String {
    if log.is_empty() {
        return "the page has logged nothing".to_owned();
    }
    log.iter()
        .map(|message| {
            format!(
                "[{}] {} ({})",
                format!("{:?}", message.level).to_lowercase(),
                message.text,
                message.source
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tabs(padu: &Padu, cx: &Context<Padu>) -> String {
    if padu.right_panel_browsers.is_empty() {
        return "no browser tabs open in this project".to_owned();
    }
    padu.right_panel_browsers
        .iter()
        .map(|(uuid, browser)| {
            let browser = browser.read(cx);
            let is_front =
                padu.active_right_panel_surface() == Some(&RightPanelSurface::Browser(*uuid));
            format!(
                "- tab {}: {} — {}{}",
                browser.id,
                browser.title(),
                browser.url(),
                if is_front { " (front)" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refuse_blocked() {
        let mut browsing = Browsing::default();
        browsing.agents_blocked = vec!["bank.com".into(), "secret.internal.net".into()];

        assert!(refuse_blocked(&browsing, "https://github.com/crabtalk/cydonia").is_ok());
        assert!(refuse_blocked(&browsing, "https://bank.com/dashboard").is_err());
        assert!(refuse_blocked(&browsing, "https://app.bank.com/login").is_err());
        assert!(refuse_blocked(&browsing, "http://secret.internal.net:8080/admin").is_err());
        let err = refuse_blocked(&browsing, "https://bank.com").unwrap_err();
        assert!(err.contains("site the user has kept agents off"));
    }

    #[test]
    fn test_described_page() {
        let page = Page {
            url: "https://example.com".into(),
            title: "Example Domain".into(),
            text: "This domain is for use in illustrative examples.".into(),
            truncated: false,
            elements: vec![
                Element {
                    i: 0,
                    tag: "a".into(),
                    kind: Some("link".into()),
                    name: "More information...".into(),
                },
                Element {
                    i: 1,
                    tag: "button".into(),
                    kind: None,
                    name: "Submit".into(),
                },
            ],
        };
        let desc = described(42, &page);
        assert!(desc.contains("tab 42: Example Domain"));
        assert!(desc.contains("https://example.com"));
        assert!(desc.contains("This domain is for use in illustrative examples."));
        assert!(desc.contains("[0] a (link) More information..."));
        assert!(desc.contains("[1] button Submit"));
    }

    #[test]
    fn test_described_truncated_page() {
        let page = Page {
            url: "https://example.com/long".into(),
            title: "Long Page".into(),
            text: "Lots of content here".into(),
            truncated: true,
            elements: vec![],
        };
        let desc = described(1, &page);
        assert!(desc.contains("[text truncated; scroll to reach the rest]"));
    }

    #[test]
    fn test_console_formatting() {
        let empty: Vec<ConsoleMessage> = vec![];
        assert_eq!(console(&empty), "the page has logged nothing");

        let messages = vec![
            ConsoleMessage {
                level: browser::ConsoleLevel::Log,
                text: "Page loaded".into(),
                source: "app.js:10".into(),
            },
            ConsoleMessage {
                level: browser::ConsoleLevel::Error,
                text: "Uncaught TypeError".into(),
                source: "main.js:42".into(),
            },
        ];
        let out = console(&messages);
        assert!(out.contains("[log] Page loaded (app.js:10)"));
        assert!(out.contains("[error] Uncaught TypeError (main.js:42)"));
    }
}
