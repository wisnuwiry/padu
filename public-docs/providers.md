---
title: Providers
description: How Padu orchestrates external coding agent CLIs via native direct drivers and the Agent Client Protocol (ACP).
nav: Providers
order: 20
category: Providers
---

# Providers overview

Padu is a native control plane and workspace for AI coding agents. It launches and supervises **locally installed coding agent CLIs** (Claude Code, OpenAI Codex, OpenCode, Pi Agent, Cursor CLI, and more).

Your API keys, subscriptions, configuration files, and MCP servers remain directly in your control on your machine. Padu provides the native GPUI workspace, split-diff inspection, multi-agent orchestration, and worktree isolation on top.

## Mental Model

A **provider** defines the communication contract between Padu and an external agent CLI:
- How to spawn and supervise the CLI subprocess.
- How to parse streaming output, reasoning tokens, and tool execution events.
- How to deliver user steering messages and approval responses.
- Which models, thinking levels, and operational modes are supported.

## Integration Tiers

1. **Native Direct Drivers:** Built-in optimized drivers in `crates/padu-core` for top agents (Claude Code, Codex, OpenCode, Antigravity, Pi Agent, Amp, DeepSeek, Cursor CLI, Fx, Grok Build, Kimi Code).
2. **Agent Client Protocol (ACP):** Universal support for any agent implementing the open [Agent Client Protocol (ACP)](https://agentclientprotocol.com) over standard I/O streams (`stdio`).
3. **Daemon Overrides:** Configure binary paths or disable providers via `~/.padu/settings.json` or the Settings UI.

## Next Steps

- [Supported providers](/docs/supported-providers) — Explore all natively supported agent CLIs.
- [Claude Code guide](/docs/claude-code) — Setup and usage for Anthropic's Claude Code CLI.
- [OpenAI Codex guide](/docs/codex) — Setup and usage for OpenAI's Codex CLI.
- [Antigravity guide](/docs/antigravity) — Setup and usage for Google Antigravity.
- [Configuration & Settings](/docs/configuration) — Configure daemon settings and binary overrides.
