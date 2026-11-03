//! The door: Streamable HTTP on loopback, one endpoint for the whole app.

use crate::{Server, proto::Request};
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{net::TcpListener, sync::oneshot};

pub const PATH: &str = "/mcp";
pub const PORT: u16 = 7460;
pub const PROJECT: &str = "x-padu-project";
pub const SESSION: &str = "x-padu-session";
const SPAN: u16 = 16;
const HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;

pub struct Door {
    url: String,
    close: Option<oneshot::Sender<()>>,
}

impl Door {
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        if let Some(close) = self.close.take() {
            let _ = close.send(());
        }
    }
}

pub async fn open(server: Arc<Server>) -> std::io::Result<Door> {
    let mut refused = None;
    for port in PORT..PORT.saturating_add(SPAN) {
        match open_at(port, server.clone()).await {
            Ok(door) => return Ok(door),
            Err(why) => refused = Some(why),
        }
    }
    Err(refused.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            "no port in the range is free",
        )
    }))
}

pub async fn open_at(port: u16, server: Arc<Server>) -> std::io::Result<Door> {
    let listener = TcpListener::bind(SocketAddr::from((HOST, port))).await?;
    let url = format!("http://{}{PATH}", listener.local_addr()?);
    let router = Router::new()
        .route(PATH, post(call).get(no_stream))
        .with_state(server);
    let (close, closed) = oneshot::channel();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = closed.await;
            })
            .await;
    });
    Ok(Door {
        url,
        close: Some(close),
    })
}

async fn call(State(server): State<Arc<Server>>, headers: HeaderMap, body: String) -> Response {
    let at = headers
        .get(PROJECT)
        .and_then(|value| value.to_str().ok())
        .and_then(decoded);
    let session = headers
        .get(SESSION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if headers.contains_key(header::ORIGIN) {
        return (StatusCode::FORBIDDEN, "this server is not for a browser").into_response();
    }
    let Ok(request) = serde_json::from_str::<Request>(&body) else {
        return (StatusCode::BAD_REQUEST, "not a JSON-RPC request").into_response();
    };
    let answer = tokio::task::block_in_place(|| {
        server.handle_from(&request, at.as_deref(), session.as_deref())
    });
    match answer {
        Some(response) => axum::Json(response).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

async fn no_stream() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, "this server pushes nothing").into_response()
}

pub fn encoded(path: &Path) -> String {
    let mut out = String::new();
    for byte in path.to_string_lossy().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(*byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn decoded(value: &str) -> Option<PathBuf> {
    let raw = value.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut at = 0;
    while at < raw.len() {
        match raw[at] {
            b'%' if at + 2 < raw.len() => {
                let hex = std::str::from_utf8(&raw[at + 1..at + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                at += 3;
            }
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}
