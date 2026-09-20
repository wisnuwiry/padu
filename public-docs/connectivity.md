---
title: Connectivity
description: Connect Padu desktop, web, and mobile clients to your daemon over loopback, Tailscale, Cloudflare Tunnel, or a reverse SSH relay.
nav: Connectivity
order: 2
category: Getting started
---

# Connectivity

Padu follows a local-first client-server model. The desktop GUI, browser client, and mobile companion all talk to a lightweight daemon (`padu-daemon`) over a WebSocket, either on loopback or across the network.

Every remote connection comes down to two things: an address the client can reach, and the daemon's bearer token. The transport you pick decides how that address is made reachable.

## 1. Local loopback (default)

The desktop app starts its own daemon and talks to it over loopback. No network setup is required, and nothing is exposed.

Loopback binds to an ephemeral port chosen by the OS. Only once you expose the daemon does it take a fixed port (`34123` by default).

## 2. Exposing the daemon

To let another device connect, turn on **Settings → Daemon → Expose**. This binds the daemon to a network interface instead of loopback, and gives you:

- **WebSocket URL** — the address clients use, e.g. `ws://192.168.1.10:34123`.
- **Token** — the bearer credential every client must present. Regenerate it to invalidate existing clients.
- **Allowed origins** — the browser origins permitted to connect. Native clients are exempt.

> **Keep exposure on a network you trust.** Anyone who can reach the port *and* has the token can drive the daemon. Prefer Tailscale or a tunnel over opening a port to the internet, and never expose the daemon on plain `ws://` across an untrusted network.

## 3. Adding a host

Open **Settings → Add host** and pick a transport.

### Direct

Enter the `ws://` or `wss://` address and token. Use this when the daemon is already reachable — a LAN address, a public reverse proxy, or a VPN you manage yourself.

### Tailscale

Enter the machine's MagicDNS name or `100.x` address together with the daemon port, for example `my-mac.tail-abc.ts.net:34123`.

The daemon must be exposed on a non-loopback bind for the tailnet to reach it. Once both devices are on the same tailnet, nothing needs to be opened to the public internet.

### Cloudflare Tunnel

Cloudflare Tunnel reaches the daemon without an account or firewall change. In the host dialog choose **Cloudflare**, then **Start Tunnel**. Padu runs `cloudflared` locally and shows a QR code for the resulting `*.trycloudflare.com` address.

Scan that code with the Padu mobile app — or with your phone's camera, which opens Padu through the `padu://` link.

> A Quick Tunnel's address changes every time it restarts. When it does, other clients hold a stale address: re-scan the QR, or update the host's address in **Settings → Daemon**.

### SSH relay

An SSH relay forwards the daemon through a jump host you already have access to, using your existing SSH keys. Padu never sees or stores your SSH credentials.

Choose **SSH**, then fill in:

- **User** and **Jump host** — for example `alice` and `jump.example.com`.
- **Remote port** — the port the jump host binds, for example `19999`.
- **Identity file** — optional path to a private key. Leave blank to use your SSH agent.

The remote `sshd` must permit the bind: set `GatewayPorts yes` (or `clientspecified`) so the forwarded port is reachable from outside the jump host itself.

## 4. Mobile

The mobile app can import a host without you typing an address or token:

1. In the desktop's host dialog, choose **Cloudflare** and press **Start Tunnel**.
2. Scan the QR code it shows with your phone's camera. The `padu://connect` link opens Padu and saves the host, token included.
3. If the camera hands the link to another app, use **Import from Link** in the host list and paste it.

Hosts can also be added by hand for **Direct** and **Tailscale** addresses.

## 5. Web client

The browser client needs a `wss://` address reachable from the browser. A page served over HTTPS cannot open a `ws://` socket, so terminate TLS in front of the daemon (see [Web UI](/docs/web-ui)) or reach it over Tailscale.

Browser connections are checked against the daemon's allowed origins. Native clients send a marker header and skip that check.

## Choosing a transport

| Situation | Use |
| --- | --- |
| Same machine | Loopback (default) |
| Same LAN | Direct, over the machine's LAN address |
| Devices already on a tailnet | Tailscale |
| No shared network, no account | Cloudflare Tunnel |
| You have SSH access to a jump host | SSH relay |
