//! The MCP server Padu answers as: the tools an agent works a project through.

#[cfg(feature = "http")]
pub mod http;
pub mod proto;
pub mod rail;
mod resources;
mod server;
pub mod tool;
pub mod tools;

pub use server::*;
pub use tool::Tool;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_proto_response() {
        let ok = proto::Response::ok(json!(1), json!({"success": true}));
        assert_eq!(ok.jsonrpc, "2.0");
        assert!(ok.result.is_some());
        assert!(ok.error.is_none());

        let err = proto::Response::fail(json!(2), proto::Error::method_not_found("test"));
        assert_eq!(err.jsonrpc, "2.0");
        assert!(err.result.is_none());
        assert_eq!(err.error.unwrap().code, proto::METHOD_NOT_FOUND);
    }

    #[test]
    fn test_server_initialize() {
        let server = Server::new().mount(&tools::workspace::TOOLS);
        let req = proto::Request {
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let res = server.handle(&req, None).expect("handled");
        let result = res.result.expect("result");
        assert_eq!(result["protocolVersion"], proto::VERSION);
        assert_eq!(result["serverInfo"]["name"], "padu");
    }

    #[test]
    fn test_server_tools_list() {
        let server = Server::new()
            .mount(&tools::workspace::TOOLS)
            .mount(&tools::browser::TOOLS);
        let req = proto::Request {
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let res = server.handle(&req, None).expect("handled");
        let tools_list = res.result.expect("result")["tools"]
            .as_array()
            .cloned()
            .expect("array");
        assert_eq!(tools_list.len(), 8); // 1 workspace + 7 browser
        assert!(tools_list.iter().any(|t| t["name"] == "browser_open"));
        assert!(tools_list.iter().any(|t| t["name"] == "workspace_focus"));
    }

    #[test]
    fn test_resources_list_and_read() {
        let server = Server::new();
        let list_req = proto::Request {
            id: Some(json!(3)),
            method: "resources/list".to_string(),
            params: None,
        };
        let list_res = server.handle(&list_req, None).expect("handled");
        let resources = list_res.result.expect("result")["resources"]
            .as_array()
            .cloned()
            .expect("array");
        assert!(resources.iter().any(|r| r["name"] == "browser"));

        let read_req = proto::Request {
            id: Some(json!(4)),
            method: "resources/read".to_string(),
            params: Some(json!({ "uri": "padu://resources/browser" })),
        };
        let read_res = server.handle(&read_req, None).expect("handled");
        let contents = read_res.result.expect("result")["contents"]
            .as_array()
            .cloned()
            .expect("array");
        assert!(!contents.is_empty());
        assert!(
            contents[0]["text"]
                .as_str()
                .unwrap()
                .contains("# The in-app browser")
        );
    }

    #[test]
    fn test_rail_browser_dispatch() {
        rail::install_browser(|browse| {
            if browse.act == rail::Act::Tabs {
                let _ = browse
                    .reply
                    .send(Ok("front: tab 1 https://padu.dev (Padu)".to_string()));
            } else {
                let _ = browse.reply.send(Ok("ok".to_string()));
            }
        });

        let cwd = std::env::current_dir().unwrap();
        rail::set_open(vec![cwd.clone()]);

        let server = Server::new().mount(&tools::browser::TOOLS);
        let req = proto::Request {
            id: Some(json!(5)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "browser_tabs",
                "arguments": {
                    "project": cwd.to_str().unwrap()
                }
            })),
        };

        let res = server.handle(&req, None).expect("handled");
        let result = res.result.expect("result");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("front: tab 1"));
    }

    #[test]
    fn test_rail_change() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static CALLED: AtomicBool = AtomicBool::new(false);
        rail::install(|change| {
            if let rail::Change::Open(_) = change {
                CALLED.store(true, Ordering::Relaxed);
            }
        });
        rail::ask(rail::Change::Open(PathBuf::from("/test"))).expect("asked");
        assert!(CALLED.load(Ordering::Relaxed));
    }

    #[cfg(feature = "http")]
    #[test]
    fn test_http_encoded_decoded() {
        let p = std::path::Path::new("/tmp/test dir/project");
        let enc = http::encoded(p);
        assert!(enc.contains("%20"));
        let dec = http::decoded(&enc).expect("decoded");
        assert_eq!(dec, p);
    }
}
