# Provider Driver Architecture in Padu

How AI coding-agent CLIs are driven from `crates/padu-core/src/driver/`.
Padu spawns the agent CLI locally and speaks to it over stdio — never calls
remote LLM APIs directly.

## Flow

Clients (Desktop/Web) ⇄ `padu-daemon` (WebSocket JSON-RPC) → `DriverControl`
→ `AcpDriver` (ACP SDK) or native driver → child process → reader threads →
`DriverEventSender` → unbounded event queue + bounded(1) wake channel (events
never drop, UI wakes coalesce).

## `DriverControl` trait

Required: `prompt`, `cancel`, `respond`, `rollback`. Also `fork`,
`apply_options` (in-place? else recreate), `steer`/`supports_steer`,
`respond_user_input`, background-work and computer-tool hooks — all defaulted
except the required four (see `driver/mod.rs` for exact signatures).

## Path A: ACP (preferred)

Agents speaking [ACP](https://agentclientprotocol.com) (Cursor, Grok, Kimi,
Cline, Goose, Gemini, …) run through `driver/acp.rs`; the SDK owns framing,
correlation, cancellation, errors. New ACP agent = register CLI args in
`launch_for()` only.

## Path B: Native driver

Bespoke protocols (Claude, Codex, OpenCode, Pi, Amp): new module
`driver/<name>.rs`, spawn via `crate::command_env::command()`, parse stdout
on background threads into `DriverEvent`, emit `ProcessExited` on exit.

## Lifecycle invariants

1. `unblock_sigchld_for_current_thread()` before spawn; inherit env via
   `shell_environment()`, pass `cwd`.
2. No subprocess I/O on render paths — background threads only.
3. Forward reasoning/text/tool deltas in exact arrival order; strip
   proprietary tags (`<thought>`, `<ant_thought>`).
