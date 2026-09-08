---
title: Supported providers
description: Every coding agent CLI Padu can launch and orchestrate, natively supported drivers and the ACP catalog.
nav: Supported providers
order: 21
category: Providers
---

# Supported providers

Padu communicates directly with locally installed agent CLIs via native process adapters in `crates/padu-core` and the open Agent Client Protocol (ACP).

For an architectural overview, see [Providers overview](/docs/providers). To configure binary path overrides or disable providers, see [Configuration](/docs/configuration).

## Native Drivers

These providers include first-class native driver implementations with structured token streaming, checkpoint capture, and reasoning token demuxing:

- **[Claude Code](/docs/claude-code)** — Anthropic's official coding agent CLI with tool streaming and deep reasoning.
- **[OpenAI Codex](/docs/codex)** — OpenAI's workspace agent with sandbox execution and model switching.
- **[OpenCode](https://opencode.ai/)** — Open-source terminal assistant with multi-provider model routing.
- **[Pi Agent](https://pi.dev)** — Minimal, fast terminal coding agent with multi-provider support.
- **Oh My Pi** — Pi coding agent with interactive approvals, multi-turn reasoning, and local checkpoints.
- **[Amp](https://github.com/tao12345666333/amp-acp)** — Frontier coding agent with worktree capabilities.
- **[DeepSeek TUI](https://deepseek.com)** — High-reasoning open model assistant and CodeWhale CLI.

## ACP (Agent Client Protocol) Integrations

These providers are integrated via the open [Agent Client Protocol (ACP)](https://agentclientprotocol.com):

- **[Cursor CLI](https://cursor.com)** — Cursor's autonomous terminal coding companion.
- **Fx** — Fast terminal coding assistant with live streaming and diffs.
- **[Grok Build](https://docs.x.ai/build/overview)** — xAI's agentic coding CLI.
- **[Kimi Code](https://github.com/MoonshotAI/kimi-code)** — Moonshot AI's long-context assistant.
- **[Antigravity](/docs/antigravity)** — Google DeepMind's autonomous coding agent with native ACP integration, one-click installer, and dedicated reasoning effort controls.

View the full interactive catalog on [padu.dev/agents](/agents).
