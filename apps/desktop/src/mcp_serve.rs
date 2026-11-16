//! The MCP server Padu answers as — the tools an agent works a project through.

use padu_mcp::{Server, http::Door, tools};
use std::path::Path;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

static DOOR: OnceLock<Mutex<Option<Door>>> = OnceLock::new();
static WRITE: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static DELETE: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static BROWSER: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static BROWSER_ACT: OnceLock<Arc<AtomicBool>> = OnceLock::new();

const BROWSER_RESOURCE: &str = "browser";

fn held() -> &'static Mutex<Option<Door>> {
    DOOR.get_or_init(Mutex::default)
}

fn write() -> &'static Arc<AtomicBool> {
    WRITE.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

pub fn set_write(on: bool) {
    write().store(on, Ordering::Relaxed);
}

fn delete() -> &'static Arc<AtomicBool> {
    DELETE.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

pub fn set_delete(on: bool) {
    delete().store(on, Ordering::Relaxed);
}

fn browser() -> &'static Arc<AtomicBool> {
    BROWSER.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

fn browser_act() -> &'static Arc<AtomicBool> {
    BROWSER_ACT.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

pub fn set_browser(read: bool, act: bool) {
    let read = read && cfg!(not(target_os = "linux"));
    browser().store(read, Ordering::Relaxed);
    browser_act().store(read && act, Ordering::Relaxed);
}

pub fn hidden_resources() -> Vec<&'static str> {
    match browser().load(Ordering::Relaxed) {
        true => Vec::new(),
        false => vec![BROWSER_RESOURCE],
    }
}

pub fn serve(open: bool) {
    serve_configured(open, true);
}

pub fn serve_configured(open: bool, auto_configure: bool) {
    let Ok(mut door) = held().lock() else {
        return;
    };
    match open {
        false => *door = None,
        true if door.is_some() => {}
        true => {
            std::thread::spawn(move || {
                let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                let (ready_tx, _) = std::sync::mpsc::channel();
                rt.block_on(async move {
                    let d = padu_mcp::http::open(Arc::new(server())).await.ok();
                    if let Some(door) = &d {
                        if auto_configure {
                            auto_configure_claude_mcp(door.url());
                            auto_configure_codex_mcp(door.url());
                            auto_configure_opencode_mcp(door.url());
                            auto_configure_cursor_mcp(door.url());
                        }
                    }
                    if let Ok(mut held) = held().lock() {
                        *held = d;
                    }
                    let _ = ready_tx.send(());
                    futures::future::pending::<()>().await;
                });
            });
        }
    }
}

pub fn auto_configure_claude_mcp(url: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let claude_json_path = home.join(".claude.json");
    if !claude_json_path.exists() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&claude_json_path) else {
        return;
    };
    let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    let Some(obj) = val.as_object_mut() else {
        return;
    };
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    let Some(servers_obj) = servers.as_object_mut() else {
        return;
    };
    let target = serde_json::json!({
        "type": "http",
        "url": url,
    });
    if servers_obj.get("padu") == Some(&target) {
        return;
    }
    servers_obj.insert("padu".to_string(), target);
    if let Ok(serialized) = serde_json::to_string_pretty(&val) {
        let _ = std::fs::write(&claude_json_path, serialized);
    }
}

pub fn auto_configure_codex_mcp(url: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let codex_config_path = home.join(".codex").join("config.toml");
    if !codex_config_path.exists() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&codex_config_path) else {
        return;
    };
    let target_entry = format!("[mcp_servers.padu]\nurl = \"{url}\"");
    if content.contains(&target_entry) {
        return;
    }
    if content.contains("[mcp_servers.padu]") {
        let mut lines = Vec::new();
        let mut in_padu = false;
        for line in content.lines() {
            if line.trim() == "[mcp_servers.padu]" {
                in_padu = true;
                lines.push(line.to_string());
                lines.push(format!("url = \"{url}\""));
            } else if in_padu {
                if line.trim().starts_with("url =") {
                    continue;
                } else if line.trim().starts_with('[') {
                    in_padu = false;
                    lines.push(line.to_string());
                } else {
                    lines.push(line.to_string());
                }
            } else {
                lines.push(line.to_string());
            }
        }
        let _ = std::fs::write(&codex_config_path, lines.join("\n") + "\n");
    } else {
        let entry = format!("\n[mcp_servers.padu]\nurl = \"{url}\"\n");
        let _ = std::fs::write(&codex_config_path, format!("{content}{entry}"));
    }
}

pub fn auto_configure_opencode_mcp(url: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let opencode_dir = home.join(".config").join("opencode");
    let opencode_json_path = opencode_dir.join("opencode.json");
    let _ = std::fs::create_dir_all(&opencode_dir);
    let content = std::fs::read_to_string(&opencode_json_path).unwrap_or_else(|_| "{}".to_string());
    let mut val = serde_json::from_str::<serde_json::Value>(&content)
        .unwrap_or_else(|_| serde_json::json!({}));
    let Some(obj) = val.as_object_mut() else {
        return;
    };
    let mcp = obj.entry("mcp").or_insert_with(|| serde_json::json!({}));
    let Some(mcp_obj) = mcp.as_object_mut() else {
        return;
    };
    let target = serde_json::json!({
        "type": "remote",
        "url": url,
    });
    if mcp_obj.get("padu") == Some(&target) {
        return;
    }
    mcp_obj.insert("padu".to_string(), target);
    if let Ok(serialized) = serde_json::to_string_pretty(&val) {
        let _ = std::fs::write(&opencode_json_path, serialized);
    }
}

pub fn auto_configure_cursor_mcp(url: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let cursor_dir = home.join(".cursor");
    let cursor_json_path = cursor_dir.join("mcp.json");
    if !cursor_dir.exists() && !cursor_json_path.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&cursor_dir);
    let content = std::fs::read_to_string(&cursor_json_path).unwrap_or_else(|_| "{}".to_string());
    let mut val = serde_json::from_str::<serde_json::Value>(&content)
        .unwrap_or_else(|_| serde_json::json!({}));
    let Some(obj) = val.as_object_mut() else {
        return;
    };
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    let Some(servers_obj) = servers.as_object_mut() else {
        return;
    };
    let target = serde_json::json!({
        "url": url,
    });
    if servers_obj.get("padu") == Some(&target) {
        return;
    }
    servers_obj.insert("padu".to_string(), target);
    if let Ok(serialized) = serde_json::to_string_pretty(&val) {
        let _ = std::fs::write(&cursor_json_path, serialized);
    }
}

pub fn url() -> Option<String> {
    let door = held().lock().ok()?;
    door.as_ref().map(|door| door.url().to_owned())
}

pub fn project(cwd: &Path) -> (&'static str, String) {
    (padu_mcp::http::PROJECT, padu_mcp::http::encoded(cwd))
}

pub fn session(record: &str) -> (&'static str, String) {
    (padu_mcp::http::SESSION, record.to_owned())
}

fn server() -> Server {
    Server::new()
        .mount(&tools::workspace::TOOLS)
        .mount_switched(
            tools::browser::looking(),
            BROWSER_RESOURCE,
            browser().clone(),
        )
        .mount_switched(
            tools::browser::acting(),
            BROWSER_RESOURCE,
            browser_act().clone(),
        )
        .writable(write().clone())
        .deletes(delete().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_set_browser_and_switches() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_browser(false, false);
        assert!(!browser().load(Ordering::Relaxed));
        assert!(!browser_act().load(Ordering::Relaxed));
        assert_eq!(hidden_resources(), vec![BROWSER_RESOURCE]);

        set_browser(true, false);
        #[cfg(not(target_os = "linux"))]
        {
            assert!(browser().load(Ordering::Relaxed));
            assert!(!browser_act().load(Ordering::Relaxed));
            assert!(hidden_resources().is_empty());
        }

        set_browser(true, true);
        #[cfg(not(target_os = "linux"))]
        {
            assert!(browser().load(Ordering::Relaxed));
            assert!(browser_act().load(Ordering::Relaxed));
            assert!(hidden_resources().is_empty());
        }
    }

    #[test]
    fn test_server_tool_listings() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_browser(false, false);
        let s = server();
        let names: Vec<&str> = s.offered().map(|t| t.name).collect();
        assert!(names.contains(&"workspace_focus"));
        assert!(!names.contains(&"browser_tabs"));
        assert!(!names.contains(&"browser_click"));

        #[cfg(not(target_os = "linux"))]
        {
            set_browser(true, false);
            let s = server();
            let names: Vec<&str> = s.offered().map(|t| t.name).collect();
            assert!(names.contains(&"browser_tabs"));
            assert!(names.contains(&"browser_open"));
            assert!(names.contains(&"browser_read"));
            assert!(names.contains(&"browser_console"));
            assert!(!names.contains(&"browser_click"));

            set_browser(true, true);
            let s = server();
            let names: Vec<&str> = s.offered().map(|t| t.name).collect();
            assert!(names.contains(&"browser_click"));
            assert!(names.contains(&"browser_type"));
            assert!(names.contains(&"browser_scroll"));
        }
    }

    #[test]
    fn test_mcp_serve_lifecycle() {
        serve_configured(true, false);
        for _ in 0..50 {
            if url().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let u = url().expect("server should have bound and returned a URL");
        assert!(u.starts_with("http://127.0.0.1:"));
        assert!(u.ends_with("/mcp"));
    }

    #[test]
    fn test_auto_configure_claude_mcp() {
        let u = "http://127.0.0.1:7460/mcp";
        auto_configure_claude_mcp(u);
        if let Some(home) = dirs::home_dir() {
            let claude_json_path = home.join(".claude.json");
            if claude_json_path.exists() {
                let content = std::fs::read_to_string(&claude_json_path).unwrap();
                let val: serde_json::Value = serde_json::from_str(&content).unwrap();
                assert_eq!(
                    val.pointer("/mcpServers/padu/url"),
                    Some(&serde_json::Value::String(u.into()))
                );
            }
        }
    }

    #[test]
    fn test_auto_configure_codex_mcp() {
        let u = "http://127.0.0.1:7460/mcp";
        auto_configure_codex_mcp(u);
        if let Some(home) = dirs::home_dir() {
            let codex_config_path = home.join(".codex").join("config.toml");
            if codex_config_path.exists() {
                let content = std::fs::read_to_string(&codex_config_path).unwrap();
                assert!(content.contains("[mcp_servers.padu]"));
                assert!(content.contains(&format!("url = \"{u}\"")));
            }
        }
    }
}
