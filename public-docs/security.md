---
title: Security & Privacy
description: "Padu's security architecture: a 100% local-first data model, credential storage per client, and zero external telemetry."
nav: Security
order: 13
category: Architecture
---

# Security & Privacy

Padu is designed as a **100% local-first control plane** for AI coding agents. Your source code, conversation transcripts, prompts, and credentials never leave your hardware.

## Core Security Principles

### 1. Zero Cloud Intermediary & Zero Telemetry
- The Padu application and background daemon contain **no analytics trackers**, telemetry beacons, advertising SDKs, or crash uploaders.
- All session databases, Git worktrees, and checkpoint logs are stored strictly on your local disk in `.padu/` and OS-specific user data directories.

### 2. Credential Storage
- Provider credentials belong to the provider CLIs (`claude`, `codex`, `opencode`) and stay in their own configuration. Padu does not copy, proxy, or upload them.
- The **mobile** app keeps daemon tokens in the OS secure store (Keychain / Keystore).
- The **desktop** app stores a remote host's token in `~/.padu/app.json`, and the **browser** client in local storage. Treat both as secrets at rest.

### 3. Direct Subprocess & Provider Communication
- Agent CLIs (like `claude`, `codex`, and `opencode`) run as local child subprocesses supervised via standard streams (`stdio` and PTY).
- When agents call LLM endpoints, the HTTP/WebSocket requests originate directly from your computer to the AI provider's API (e.g. `api.anthropic.com` or `api.openai.com`) using your own credentials.

## Daemon Access Controls

### Loopback by default

By default, the daemon binds exclusively to loopback, accepting connections only from processes on the same machine. It only takes a fixed, network-reachable port (`34123` by default) once you explicitly turn exposure on.

### Bearer-token authentication

Every client must present the daemon's bearer token in its opening handshake. The comparison is constant-time, and **Settings → Daemon → Regenerate token** invalidates every existing client — the mobile app then has to be given the new token (or a fresh QR scan).

### Browser origin allowlist

Handshakes are only accepted on the `/v1` path. Connections from browsers must also present an `Origin` that matches the daemon's configured allowlist; native clients identify themselves with an `X-Padu-Client: native` header and skip that check, since they carry the token directly.

### What exposure actually means

Exposure is not a hardening step — it is a deliberate widening of access. Once the daemon listens on a network interface, anyone who can reach that port *and* has the token can drive it: run agents, read files, and execute commands in your workspaces. Keep exposure on a network you control, prefer a tunnel or tailnet over forwarding a port, and never serve it as plain `ws://` over an untrusted network.

## Vulnerability Reporting

If you discover a potential security vulnerability in Padu, please review [SECURITY.md](https://github.com/wisnuwiry/padu/blob/main/SECURITY.md) on GitHub or contact the maintainers confidentially at [support@padu.dev](mailto:support@padu.dev).
