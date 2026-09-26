# Tasks: Kanban, Automation & Git Integration — Padu

**Source:** `docs/prd/automation.md` v3.0 (Final) · **Date:** 25 September 2026
**Build order:** Phase 1 (Profile + Kanban) → Phase 2 (Git read) → Phase 3 (Git write manual) → Phase 4 (Automation core) → Phase 5 (Automation + Git write)
**Per-phase rule:** protocol bump + `bun run protocol:generate` + `bun run protocol:check` + desktop & web parity + `cargo fmt/check/test` + client/web checks + Padu Debug validation + pre-PR review (`.agents/skills/pre-pr-review/SKILL.md`).

Conventions used below: daemon = `crates/padu-core` + `crates/padu-daemon`; wire = `crates/padu-protocol` (+ `packages/padu-client/src/generated`, `@padu/client` reducers); desktop = `apps/desktop`; web = `apps/web`. All daemon I/O off the UI thread (`cx.background_executor().spawn` / tokio tasks); no blocking work in `render`; virtualized `list()` for board/history surfaces; keyboard + `focus_visible` + reduce-motion + `tr!` locales for every new string.

---

## Phase 0 — Scaffolding & contracts (unblocks all phases)

- [x] **P0-01** Add wire modules skeleton: `crates/padu-protocol/src/{kanban,agent_profile,automation,git_integration}.rs` with `TS`-derived empty shells + `export_types` wiring; `bun run protocol:generate` passes with no behavior change.
- [x] **P0-02** Daemon storage plan: SQLite tables `tasks`, `agent_profiles`, `rules`, `pending_approvals`, `automation_outbox`, `audit_log` (+ `projects.linked_repo` nullable column); migration + rollback test.
- [ ] **P0-03** Event catalog: define canonical daemon events (`task_queued`, `workspace_started`, `agent_completed`, `checkpoint_failed`, `card_updated`, `rule_triggered`, `approval_requested`, `issue_created`, `pr_opened`, `pr_merged`/`mr_merged`, `issue_closed`) as `ServerMessage` variants with replay/dedup semantics documented.
- [ ] **P0-04** Idempotency + audit helpers: `idempotency_key = hash(rule_id, trigger_event_id)` dedup layer + append-only audit writer + keychain token-store abstraction (macOS Keychain / Win Credential Manager / Linux Secret Service); unit tests for dedup + redaction (no secrets in logs/DB).

## Phase 1 — Kanban + Agent Profile (protocol 8 → 9)

**Agent Profile (PRD §6)**
- [ ] **P1-01** Seed all 14 `ProviderKind`s (`agy, amp, claude, codex, cursor, deepseek, fx, opencode, grok, kimi, commandcode, ohmypi, pi, qoder`) with display names from `ProviderKind::display_name()`, default `role_tags`/`cost_tier`/`priority`/`max_retry_before_escalate=3`; migration imports legacy `DaemonSettings.disabled_providers` → `enabled=false` once.
- [ ] **P1-02** Daemon CRUD: `ListAgentProfiles` / `UpdateAgentProfile` commands + `expected_version` guard; disabled agent excluded from candidacy (in-flight sessions untouched; queued-unstarted tasks re-resolve); `cargo test` for seed + import + exclusion.
- [ ] **P1-03** Profile UI (desktop + web): reorder priority (drag + keyboard up/down), enable/disable toggle with tooltip, capability notes (Kimi/Fx no rollback/fork → skipped in rewind-dependent chains); strings via locales.

**Kanban Task model + board (PRD §5)**
- [ ] **P1-04** `Task` entity per §5.4 (`session_id` nullable link, `version`, `needs_attention`, `sync_failed`, `idempotency_keys`, `archived`) + `Project.linked_repo` nullable extension; merge-only save semantics (stale snapshots never delete others' rows; explicit delete only).
- [ ] **P1-05** Daemon commands: `ListTasks` (summary projection, no checkpoint bodies/logs), `CreateTask`, `UpdateTask`/`MoveTask` (with `expected_version` reject + re-fetch), `DeleteTask` (explicit), `HydrateTask` (on-demand session/checkpoint/log); `card_updated` broadcast on every transition.
- [ ] **P1-06** Lifecycle wiring: Backlog→Queued creates `AgentSession` honoring worktree policy (`NewWorktree{base_branch}`/`Local`); Queued→Running on `workspace_started`; Running→Review on `agent_completed` (checkpoint `Ready`) or `checkpoint_failed` streak (stays in Review + `needs_attention`); Review→Done only on external merge signal or explicit Mark Done; manual drag into Running rejected with reason.
- [ ] **P1-07** Board UI (desktop + web): global board, per-project/agent/status/label/flag filters, virtualized cards, detail panel (checkpoint scrubber reuse, hydrate-on-open log/rewind/fork, linked issue/PR slots), optimistic drag + daemon reconciliation, empty/error states (filter-empty, disconnected-cached).
- [ ] **P1-08** Live badges: cost/token/duration via existing usage events (≤ commit cadence ~8.3 Hz); no UI polling; 500-card interaction test.
- [ ] **P1-09** Phase 1 gates: §5.5 + §6.4 acceptance pass; protocol bump + codegen + reducers + web types; full checks green; pre-PR review.

## Phase 2 — Git read (protocol → 10)

- [ ] **P2-01** Auth chain on daemon host: `gh`/`glab auth status` → `auth token` (zero-friction) → OAuth device flow (minimal scopes: GitHub `repo, read:user`; GitLab `api`, group-scoping documented) → PAT paste with immediate `whoami` validation; tokens to OS keychain only.
- [ ] **P2-02** `Project.linked_repo` connect/disconnect UI (desktop + web) + per-connection status (`healthy` / `needs re-auth` on 401, polling paused); disconnect clears handle, marks pending writes only.
- [ ] **P2-03** Import issue → Backlog `Task` (title/body/labels exact, `linked_issue` set, local-only Padu backlink metadata; never edits remote body).
- [ ] **P2-04** Background sync: one generation-guarded repo pass per tick (60s focused / 5min idle), ETag/`If-None-Match`, `X-RateLimit-Remaining` tracking (<10% slows polling, user-initiated first); emits `issue_closed` / `pr_merged` / check-status events; no per-card polling, no UI-thread network.
- [ ] **P2-05** Card surfaces: CI/check summary + link-out on `linked_pr` cards; remote-close → card move via default (overridable) rule; `sync_failed` + Retry on permanent errors (403/404/422).
- [ ] **P2-06** Phase 2 gates: read half of §8.6; stubbed-API tests for import mapping + 401→`needs re-auth` + rate-limit slowdown; protocol + parity + checks + pre-PR review.

## Phase 3 — Git write, manual (same protocol 10)

- [ ] **P3-01** `CreateIssue` command (idempotent key, optimistic UI → daemon confirm with canonical URL → `linked_issue`): "New Issue" on cards with `linked_issue == null`, editable title/body/labels pre-submit, Padu backlink in body (no private provider markers).
- [ ] **P3-02** `CreatePr/CreateMr` command: push task worktree branch from daemon, base = repo default (overridable), body generated from checkpoint history (per-turn summaries + file stats), editable pre-submit when manual; success sets `linked_pr` + opens browser link from card.
- [ ] **P3-03** Failure UX: transient (timeout/5xx/rate-limit) → backoff retry 1s/4s/16s ×3 on background executor → `sync_failed` + Retry; permanent → immediate `sync_failed` with reason; never silent; duplicate-retry test with stubbed API must create exactly one remote object.
- [ ] **P3-04** PR `draft → ready_for_review` explicit button (auto-follow of card position default OFF in v1).
- [ ] **P3-05** Phase 3 gates: write half of §8.6; protocol regen + parity + checks + pre-PR review.

## Phase 4 — Automation core, no auto-create (protocol → 11)

- [ ] **P4-01** Rule store + engine skeleton: `Rule` model (§7.2, AND-combined dropdown conditions), `ListRules`/`SaveRule`/`ToggleRule`/priority-reorder commands, evaluation on daemon event bus (sequential by `priority`, stable `rule_id` tiebreak; failed action aborts that rule only).
- [ ] **P4-02** Core triggers: `task_queued`, `workspace_started`, `agent_completed`, `checkpoint_failed(streak>=N)`, `kanban_card_moved`, `pr_merged`/`mr_merged`, `issue_closed`, `ci_check_failed/passed`, `workspace_idle(N min)` — each mapped to real daemon/Git-poll events.
- [ ] **P4-03** Core actions: `spawn_agent` (specific or `next_available` via Profile tags+priority+enabled), `move_card`, `create_card`, `notify_desktop`, `escalate` (capability-aware fallback chain, `max_retry_before_escalate` → `needs_attention`), `archive_workspace` (existing safe-cleanup semantics). No `create_issue`/`create_pr` in this phase.
- [ ] **P4-04** Safety base: per-rule `cooldown_secs` (default 60), chain depth cap 5 with chain-id propagation + halt + audit entry, persist + outbox replay across daemon restart (no loss/dup), append-only audit per rule + per card.
- [ ] **P4-05** Rule UI (desktop + web): dropdown builder, list with toggles + priority reorder (keyboard accessible), per-rule execution history (time, event, actions, outcome, audit link); ship 2 disabled-by-default templates (stuck-task notify, PR-merged archive).
- [ ] **P4-06** Phase 4 gates: scenarios §7.5 (1–3); loop-halt + restart-replay + ordering tests; protocol + parity + checks + pre-PR review.

## Phase 5 — Automation + Git write (same protocol 11)

- [ ] **P5-01** Auto-create actions: `create_issue` / `create_pr` via automation with trigger-instance `idempotency_key`, card `linked_*` set, issued-keys recorded; forced-retry test proves zero duplicates.
- [ ] **P5-02** Approval queue: `requires_approval` default true on any new rule containing `create_issue`/`create_pr`/cross-tier `escalate`; `approval_requested` event → notification + in-app queue (approve/deny, keyboard operable, denial audited); opt-out per rule only after one approved run. Ship 3rd template (`agent_completed_without_pr` → PR) disabled by default.
- [ ] **P5-03** `agent_completed_without_pr` trigger + `agent_reported_new_issue` conservative classifier (§7.8: precision-first patterns + diff-scope; low confidence → `needs_attention` suggestion, never auto-issue); thresholds logged, unit-tested.
- [ ] **P5-04** Rule JSON export/import (validates on import) as v1 power-user path; conditions `agent.cost_tier/id`, `pr.state`, `time.hour` range included.
- [ ] **P5-05** Metrics instrumentation from daemon events/audit: import share, Padu-created issue/PR share, active rules/user, approval accept-rate, `sync_failed`/100 writes, duplicate-create count (=0), loop-halt count.
- [ ] **P5-06** Final gates: scenarios §7.5 (4–5) + full §7.9; end-to-end issue→agent→PR→merge→Done on a scratch repo via CLI-auth; protocol + parity + full checks + pre-PR review; update PRD status to Shipped.

## Cross-cutting checklist (every phase)

- [ ] Protocol: `bun run protocol:generate` + `bun run protocol:check`, generated files committed, `@padu/client` reducers + web types updated (`.agents/skills/protocol-parity-sync`).
- [ ] Perf: no I/O in `render`/row builders; `docs/performance.md` re-read if touching pump/pulse/veils/scrollbars/pane cache; virtualized surfaces only.
- [ ] A11y/i18n: keyboard path + `focus_visible` + reduce-motion + icon+text status for each new control; strings through locales.
- [ ] Security: keychain-only tokens, minimal scopes shown at connect, audit redaction covered by test, loopback-default unchanged, remote-daemon path rule respected.
- [ ] Docs: PRD status line + roadmap checkboxes updated; user-facing copy reviewed (no private provider markers in remote bodies).
