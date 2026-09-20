/**
 * Encoding and decoding for the one-shot connect link that carries a daemon
 * endpoint from Padu Desktop to a phone.
 *
 * Desktop renders this as a QR code; the phone either scans it with the system
 * camera (which hands the URL to the app through the `padu://` scheme) or
 * pastes it. The shape is:
 *
 *   padu://connect?v=1&kind=<kind>&url=<wss url>&token=<base64url>&name=<name>
 *
 * The token is base64url-encoded because it is opaque bytes to this decoder and
 * because it keeps the QR dense. Everything else is percent-encoded, matching
 * what `url::Url`'s form serializer emits on the Rust side — which encodes a
 * space as `+`, so the decoder must treat `+` as a space.
 */

export const CONNECT_URL_PREFIX = "padu://connect";
export const CONNECT_LINK_VERSION = 1;

export interface ConnectLink {
  /** Transport discriminator, mirroring the Rust `HostKind`. */
  kind: "direct" | "tailscale" | "cloudflare" | "ssh_relay";
  /** Resolved `ws://` or `wss://` address. */
  address: string;
  token: string;
  name: string;
}

const BASE64URL_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

const KINDS: ConnectLink["kind"][] = [
  "direct",
  "tailscale",
  "cloudflare",
  "ssh_relay",
];

/** Decode base64url (no padding) to a UTF-8 string, or `null` when malformed. */
function decodeBase64Url(input: string): string | null {
  const cleaned = input.replace(/=+$/, "");
  if (!/^[A-Za-z0-9_-]*$/.test(cleaned)) return null;
  const bytes: number[] = [];
  let bits = 0;
  let accumulator = 0;
  for (const char of cleaned) {
    const index = BASE64URL_ALPHABET.indexOf(char);
    if (index < 0) return null;
    accumulator = (accumulator << 6) | index;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((accumulator >> bits) & 0xff);
    }
  }
  try {
    return new TextDecoder().decode(new Uint8Array(bytes));
  } catch {
    return null;
  }
}

/** Encode a UTF-8 string as base64url without padding. */
export function encodeBase64Url(input: string): string {
  const bytes = new TextEncoder().encode(input);
  let output = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const a = bytes[index]!;
    const b = bytes[index + 1];
    const c = bytes[index + 2];
    output += BASE64URL_ALPHABET[a >> 2];
    output += BASE64URL_ALPHABET[((a & 0x03) << 4) | ((b ?? 0) >> 4)];
    if (b === undefined) break;
    output += BASE64URL_ALPHABET[((b & 0x0f) << 2) | ((c ?? 0) >> 6)];
    if (c === undefined) break;
    output += BASE64URL_ALPHABET[c & 0x3f];
  }
  return output;
}

/**
 * Parse a `padu://connect?...` link. Returns `null` for anything that is not a
 * well-formed link of a supported version, so callers can surface a single
 * "invalid code" message rather than branching on partial failures.
 */
export function parseConnectUrl(raw: string): ConnectLink | null {
  const trimmed = raw.trim();
  if (!trimmed.startsWith(CONNECT_URL_PREFIX)) return null;
  const remainder = trimmed.slice(CONNECT_URL_PREFIX.length);
  const query = remainder.startsWith("?") ? remainder.slice(1) : remainder;

  const params = new Map<string, string>();
  for (const pair of query.split("&")) {
    if (!pair) continue;
    const separator = pair.indexOf("=");
    if (separator < 0) continue;
    const key = decodeURIComponent(pair.slice(0, separator));
    const value = decodeURIComponent(
      pair.slice(separator + 1).replace(/\+/g, " "),
    );
    params.set(key, value);
  }

  const version = params.get("v");
  if (version !== String(CONNECT_LINK_VERSION)) return null;

  const kind = params.get("kind");
  if (!kind || !(KINDS as string[]).includes(kind)) return null;

  const address = params.get("url");
  if (!address) return null;
  if (!address.startsWith("ws://") && !address.startsWith("wss://")) {
    return null;
  }

  const encodedToken = params.get("token");
  if (encodedToken === undefined) return null;
  const token = decodeBase64Url(encodedToken);
  if (token === null) return null;

  return {
    kind: kind as ConnectLink["kind"],
    address,
    token,
    name: params.get("name") ?? "",
  };
}

/** Build a connect link. Mirrors the desktop encoder for round-trip tests. */
export function buildConnectUrl(link: ConnectLink): string {
  const query = [
    ["v", String(CONNECT_LINK_VERSION)],
    ["kind", link.kind],
    ["url", link.address],
    ["token", encodeBase64Url(link.token)],
    ["name", link.name],
  ]
    .map(([key, value]) => `${key}=${encodeURIComponent(value!)}`)
    .join("&");
  return `${CONNECT_URL_PREFIX}?${query}`;
}

/** A short, user-facing name for a parsed link, used when none is supplied. */
export function connectLinkFallbackName(link: ConnectLink): string {
  if (link.name.trim()) return link.name.trim();
  try {
    const url = new URL(link.address);
    return url.port ? `${url.hostname}:${url.port}` : url.hostname;
  } catch {
    return link.address;
  }
}
