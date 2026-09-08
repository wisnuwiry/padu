---
title: Getting started
description: Install Padu and orchestrate local AI coding agents from desktop or web.
nav: Getting started
order: 1
category: Getting started
---

# Getting started

Padu is a high-performance, native desktop and web workspace for orchestrating local AI coding agents. Built in Rust with GPUI (the GPU-accelerated engine behind Zed), Padu runs directly on your machine, keeping all code, transcripts, checkpoints, and credentials 100% local.

## 1. Desktop App (Recommended)

Download the native release for macOS, Linux, or Windows from [padu.dev/download](https://padu.dev/download) or the [GitHub releases page](https://github.com/wisnuwiry/padu/releases).

The desktop app bundles its own lightweight daemon and starts it automatically on loopback (`127.0.0.1:4789`). No separate server install or cloud account is required.

## 2. Headless Daemon (Remote Devboxes / Servers)

For remote devboxes, headless Linux servers, or continuous environments, you can run the standalone daemon binary:

```bash
# Start the standalone daemon on loopback port 4789
padu-daemon --bind 127.0.0.1:4789

# Or allow remote connections over private VPN / Tailscale
padu-daemon --bind 100.101.102.103:4789 --allow-non-loopback
```

The daemon can also serve the bundled web client directly. See [Self-hosting the web UI](/docs/web-ui).

Configuration and local state live under `~/.padu/` (or your OS user data directory).

## 3. Prerequisites

Padu manages external agent CLIs; it does not bundle AI models itself. Before launching an agent in Padu, make sure you have installed and authenticated at least one supported agent CLI:

- **[Claude Code](/docs/claude-code):** `claude` (Anthropic CLI)
- **[OpenAI Codex](/docs/codex):** `codex` (OpenAI CLI)
- **[OpenCode](https://opencode.ai/):** `opencode`
- **[Pi Agent](https://pi.dev):** `pi`
- **Other ACP Agents:** Antigravity (`agy`), Cursor CLI, Amp, Grok Build, Kimi Code, etc.

See [Supported providers](/docs/supported-providers) for the full list of supported agents.

## Next Steps

- [Workspaces](/docs/workspaces) — Understand Padu's workspace, session, and worktree model.
- [Git worktrees](/docs/worktrees) — Run concurrent agents in isolated branches.
- [Connectivity](/docs/connectivity) — Connect remote web or mobile clients via LAN, Tailscale, or SSH.
- [Performance Architecture](/docs/performance) — Learn how GPUI delivers 120 FPS rendering with zero UI-thread blocking.
- [Security & Privacy](/docs/security) — How Padu protects your source code and credentials.
- [Configuration & Settings](/docs/configuration) — Configure `~/.padu/settings.json` and provider binary overrides.
