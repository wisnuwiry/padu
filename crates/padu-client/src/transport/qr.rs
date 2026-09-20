//! QR payload encoding for transferring host profiles from desktop to mobile.
//!
//! The encoded URL has the form:
//!
//! ```text
//! padu://connect?v=1&kind=<kind>&url=<wss url>&token=<b64url token>&name=<display name>
//! ```
//!
//! Encoding uses [`url::Url`] so reserved characters in the display name are
//! percent-encoded per RFC 3986. The token is base64url-encoded without padding
//! so the QR stays dense (a typical 32-char token becomes 43 chars).

use anyhow::{Context as _, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use url::Url;

use crate::transport::HostKind;

pub const SCHEME: &str = "padu";
pub const HOST: &str = "connect";
pub const VERSION: u8 = 1;
const VERSION_STR: &str = "1";

/// Encoded payload returned to the desktop for QR rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrPayload {
    pub kind: HostKind,
    pub url: String,
    pub token: String,
    pub name: String,
}

impl QrPayload {
    pub fn encode(&self) -> Result<String> {
        let mut url = Url::parse(&format!("{SCHEME}://{HOST}")).context("construct QR base URL")?;
        let token_b64 = URL_SAFE_NO_PAD.encode(self.token.as_bytes());
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("v", VERSION_STR)
                .append_pair("kind", kind_to_str(self.kind))
                .append_pair("url", &self.url)
                .append_pair("token", &token_b64)
                .append_pair("name", &self.name);
        }
        Ok(url.to_string())
    }

    pub fn decode(raw: &str) -> Result<Self> {
        let url = Url::parse(raw).context("parse QR payload as URL")?;
        if url.scheme() != SCHEME || url.host_str() != Some(HOST) {
            bail!("not a padu://connect URL");
        }
        let mut kind = None;
        let mut ws = None;
        let mut token_b64 = None;
        let mut name = None;
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "v" => {
                    if value != VERSION.to_string() {
                        bail!("unsupported QR payload version: {value}");
                    }
                }
                "kind" => kind = Some(str_to_kind(&value).context("decode kind")?),
                "url" => ws = Some(value.into_owned()),
                "token" => token_b64 = Some(value.into_owned()),
                "name" => name = Some(value.into_owned()),
                _ => {}
            }
        }
        let (Some(kind), Some(ws), Some(token_b64), Some(name)) = (kind, ws, token_b64, name)
        else {
            bail!("missing required QR payload fields");
        };
        if !ws.starts_with("ws://") && !ws.starts_with("wss://") {
            bail!("QR payload url must be ws:// or wss://");
        }
        let token_bytes = URL_SAFE_NO_PAD
            .decode(token_b64.as_bytes())
            .context("base64url-decode token")?;
        let token = String::from_utf8(token_bytes).context("decoded token is not UTF-8")?;
        Ok(Self {
            kind,
            url: ws,
            token,
            name,
        })
    }
}

/// Render the QR payload as a self-contained SVG string sized to fit a
/// minimum width (in CSS pixels) while keeping the QR square.
///
/// `min_px` is applied to BOTH axes. The QR is rendered at native resolution
/// (1 module = 1 px) and the SVG `viewBox` is set so the caller can scale
/// freely.
pub fn render_svg(payload: &QrPayload, min_px: u32) -> Result<String> {
    let encoded = payload.encode().context("encode QR payload")?;
    let code = qrencode::QrCode::new(encoded.as_bytes()).context("build QR matrix")?;
    let svg = code
        .render::<qrencode::render::svg::Color<'_>>()
        .min_dimensions(min_px, min_px)
        .build();
    if !svg.contains("xmlns") {
        bail!("qrencode produced an SVG without an xmlns attribute");
    }
    Ok(svg)
}

fn kind_to_str(kind: HostKind) -> &'static str {
    match kind {
        HostKind::Direct => "direct",
        HostKind::Tailscale => "tailscale",
        HostKind::Cloudflare => "cloudflare",
        HostKind::SshRelay => "ssh_relay",
    }
}

fn str_to_kind(value: &str) -> Result<HostKind> {
    Ok(match value {
        "direct" => HostKind::Direct,
        "tailscale" => HostKind::Tailscale,
        "cloudflare" => HostKind::Cloudflare,
        "ssh_relay" => HostKind::SshRelay,
        other => bail!("unknown transport kind in QR payload: {other}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: HostKind) -> QrPayload {
        QrPayload {
            kind,
            url: "wss://xyz.trycloudflare.com".into(),
            token: "secret-token-12345678".into(),
            name: "My MacBook".into(),
        }
    }

    #[test]
    fn qr_payload_roundtrips_for_each_kind() {
        for kind in [
            HostKind::Direct,
            HostKind::Tailscale,
            HostKind::Cloudflare,
            HostKind::SshRelay,
        ] {
            let payload = sample(kind);
            let encoded = payload.encode().unwrap();
            let parsed = QrPayload::decode(&encoded).unwrap();
            assert_eq!(payload, parsed, "round-trip failed for {kind:?}");
            assert!(encoded.starts_with("padu://connect?"));
        }
    }

    #[test]
    fn qr_payload_handles_non_ascii_name() {
        let payload = QrPayload {
            kind: HostKind::Cloudflare,
            url: "wss://xyz.trycloudflare.com".into(),
            token: "secret".into(),
            name: "My MacBook (dev) 🌱".into(),
        };
        let encoded = payload.encode().unwrap();
        let parsed = QrPayload::decode(&encoded).unwrap();
        assert_eq!(parsed.name, "My MacBook (dev) 🌱");
        // Parenthesis and emoji must be percent-encoded.
        assert!(encoded.contains("%28"));
        assert!(encoded.contains("%29"));
    }

    #[test]
    fn qr_payload_rejects_non_padu_scheme() {
        let err = QrPayload::decode("https://example.com/?v=1").unwrap_err();
        assert!(err.to_string().contains("not a padu"));
    }

    #[test]
    fn qr_payload_rejects_missing_fields() {
        let err = QrPayload::decode("padu://connect?v=1").unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn qr_payload_rejects_wrong_version() {
        let err = QrPayload::decode(
            "padu://connect?v=99&kind=direct&url=wss%3A%2F%2Fx&token=YWJj&name=x",
        )
        .unwrap_err();
        assert!(err.to_string().contains("unsupported QR payload version"));
    }

    #[test]
    fn qr_payload_rejects_non_ws_url() {
        let err = QrPayload::decode(
            "padu://connect?v=1&kind=direct&url=http%3A%2F%2Fx&token=YWJj&name=x",
        )
        .unwrap_err();
        assert!(err.to_string().contains("ws:// or wss://"));
    }

    #[test]
    fn render_svg_returns_well_formed_svg() {
        let payload = sample(HostKind::Cloudflare);
        let svg = render_svg(&payload, 180).unwrap();
        assert!(svg.starts_with("<?xml"));
        assert!(svg.contains("<svg"));
        assert!(svg.contains("viewBox"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        // resvg (which GPUI uses for ImageFormat::Svg) needs a concrete,
        // square width/height or the code rasterizes blank or distorted.
        // `min_dimensions` rounds up to whole modules, so the result is at
        // least the requested size, never exactly it.
        let attr = |name: &str| -> u32 {
            svg.split(&format!("{name}=\""))
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .and_then(|value| value.parse().ok())
                .unwrap_or(0)
        };
        let width = attr("width");
        let height = attr("height");
        assert_eq!(width, height, "QR SVG must be square");
        assert!(
            width >= 180,
            "QR SVG must honor the minimum size, got {width}"
        );
    }

    #[test]
    fn typical_payload_stays_within_scannable_qr_version() {
        // Realistic worst case: a base64url-encoded 32-char token (43 chars),
        // a 40-char trycloudflare URL, and a moderate display name. At the
        // default error-correction level (M) this lands around version 9
        // (~53x53 modules), which scans easily from a phone held ~30cm away.
        // Version 12 (~65x65) is still comfortable; above that the modules
        // get small enough to need a closer scan.
        let payload = QrPayload {
            kind: HostKind::Cloudflare,
            url: "wss://random-words-12345.trycloudflare.com".into(),
            token: "a".repeat(32),
            name: "My Mac Studio".into(),
        };
        let encoded = payload.encode().unwrap();
        let code = qrencode::QrCode::new(encoded.as_bytes()).unwrap();
        let numeric_version: i16 = match code.version() {
            qrencode::Version::Normal(v) => v.into(),
            _ => 99,
        };
        assert!(
            numeric_version <= 12,
            "QR version too large to scan comfortably: {numeric_version}"
        );
    }

    #[test]
    fn base64_token_round_trip() {
        let original = "secret-token-with-padding==";
        let encoded = URL_SAFE_NO_PAD.encode(original.as_bytes());
        let decoded = URL_SAFE_NO_PAD.decode(&encoded).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), original);
    }
}
