---
title: Self-hosting the web UI
description: Access the Padu web client from any browser or host it with a reverse proxy.
nav: Web UI
order: 3
category: Getting started
---

# Self-hosting the web UI

Padu includes a full-featured browser web client (`apps/web`). You can access the official hosted client at [app.padu.dev](https://app.padu.dev) or connect directly to a locally running daemon instance.

## Connecting from the Browser

Expose the daemon (see [Connectivity](/docs/connectivity)), then open your browser and connect to its WebSocket URL. The web client provides feature parity with the desktop interface, including split diff inspection, multi-agent turns, and session switching.

A page served over HTTPS can only open a `wss://` socket, so the daemon needs TLS in front of it — either a reverse proxy (below) or a tunnel.

## Reverse Proxy Configuration

If you host the daemon on a remote development machine and wish to access it over HTTPS, you can place a reverse proxy (such as Caddy or Nginx) in front of port `34123`.

### Caddy (Recommended)

Caddy manages TLS certificates, headers, and WebSocket upgrades automatically:

```caddy
padu.example.com {
  reverse_proxy 127.0.0.1:34123
}
```

### Nginx

```nginx
map $http_upgrade $connection_upgrade {
  default upgrade;
  ''      close;
}

server {
  listen 443 ssl http2;
  server_name padu.example.com;

  ssl_certificate     /etc/letsencrypt/live/padu.example.com/fullchain.pem;
  ssl_certificate_key /etc/letsencrypt/live/padu.example.com/privkey.pem;

  client_max_body_size 100m;

  location / {
    proxy_pass http://127.0.0.1:34123;
    proxy_http_version 1.1;

    # WebSocket upgrade
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $connection_upgrade;

    # Headers for origin detection
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;

    # Unbuffered streaming for real-time tokens
    proxy_buffering off;
    proxy_read_timeout 3600s;
    proxy_send_timeout 3600s;
  }
}
```

## Security Best Practices

When exposing the daemon beyond `localhost`:
1. Use private networks (such as Tailscale or WireGuard) whenever possible.
2. Terminate TLS with valid certificates on public networks.
3. Pass `--allow-origin` flags to the daemon to restrict cross-origin requests.

See [Security & Privacy](/docs/security) and [Configuration](/docs/configuration) for more details.
