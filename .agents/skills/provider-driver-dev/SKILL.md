---
name: provider-driver-dev
description: >-
  Add or fix provider drivers in padu-core/src/driver via ACP registration or
  native DriverControl; verify with the provider:test CLI.
---

# Provider Driver Development Skill

Two integration paths — pick one, then follow it. Architecture background:
[driver-architecture.md](./references/driver-architecture.md) (read only if
designing a new transport, not for routine registration).

## 1. When to use

New agent CLI, new native driver, or debugging subprocess lifecycle /
streaming demux / tool-call ordering. Test entry: `bun run provider:test`.

## 2. ACP vs native

- **Speaks ACP** (Cline, Goose, Gemini, Qwen, Cursor, …) → **Path A**:
  no driver code; register CLI args + model catalog only.
- **Bespoke protocol** (Claude, Codex, OpenCode, Pi, Amp) → **Path B**:
  new module implementing `DriverControl` in `driver/<name>.rs`.

## 3. Path A — ACP registration

Scaffold exact snippets first (replaces manual copy-paste):

```bash
bun .agents/skills/provider-driver-dev/scripts/scaffold-provider.ts <id> "<DisplayName>" <binary> --acp
```

Then: `ProviderKind` variant in `crates/padu-protocol/src/model.rs`
(+ `ALL`, `id`, `display_name`, `command`, capability flags) →
`launch_for()` args in `driver/acp.rs` + `start_local()` routing in
`driver/mod.rs` → fallback models in `model_catalog.rs` →
`bun run protocol:generate && bun run protocol:check`.
Full annotated diff: [sample-acp-registration.rs](./examples/sample-acp-registration.rs).

## 4. Path B — Native driver

Copy [sample-native-driver.rs](./examples/sample-native-driver.rs) to
`driver/<provider>.rs`, implement `prompt` / `cancel` / `respond` /
`apply_options` / `rollback` / `fork`, and normalize output per
[event-normalization.md](./references/event-normalization.md)
(`TextDelta` → text, `ReasoningDelta` → thinking with tags stripped,
`RichActivity` → tool calls, `TurnFinished`/`ProcessExited` → settlement).
Spawn via `crate::command_env::command()`, parse on background reader
threads, forward via `DriverEventSender`. Register `mod` + `start_local()`
arm in `driver/mod.rs`.

## 5. Verify

```bash
bun run provider:test probe|models|connect <id>
bun run provider:test turn <id> "Reply with PONG"
bun run provider:test suite <id>          # full matrix before PR
.agents/skills/provider-driver-dev/scripts/verify-driver.sh
```

## 6. Invariants

No subprocess I/O on render paths; stream commits ≤ ~8.3 Hz; decorative
motion ≤ ~30 Hz via pulse clock, honoring `reduce_motion`. PR also needs
desktop + web icons, landing catalog (`apps/landing/src/data/agent-pages.ts`),
and `protocol:generate` output committed.
