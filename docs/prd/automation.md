# PRD: Kanban, Automation & Git Integration — Padu

**Product:** Padu (padu.dev) · **Doc version:** 3.0 (Final) · **Date:** 25 September 2026 · **Status:** Final — scope locked, ready for implementation

**Supersedes:** v2.0 (19 Sep 2026, Indonesian draft). This v3.0 grounds the same four-feature system in the actual Padu codebase (daemon–client, `padu-protocol` v8, 14 providers, `AgentSession`/`Project`/`DaemonSettings` as they exist today) and resolves the two remaining open questions.

---

## 1. Executive Summary

Padu is a native desktop orchestrator (Rust/GPUI, daemon–client over versioned WebSocket in `crates/padu-protocol`) for multi-agent AI coding, with workspace isolation (`SessionWorkspace::Local` / `NewWorktree` / `Worktree`), queue-and-steer (`QueuedMessage`), and git-checkpoint rewind (`refs/padu/session-*`). Notes exist; projectless workspaces live on the daemon host under `~/.padu/projects/<date>/<slug>`.

This PRD defines four connected features as **one system**:

1. **Kanban Board** — one global board (filter per project/agent/status) bound to the real `AgentSession` lifecycle.
2. **Agent Profile** — registry over all 14 `ProviderKind`s with role tags, cost tier, priority — the decision base for automation, replacing per-rule hardcoded agent names.
3. **Automation** — local event-driven rule engine running as a daemon background job (no third-party cloud dependency).
4. **GitHub/GitLab Integration** — two-way sync: import issues, and **create issues and PRs/MRs** from Padu (manual or via automation).

Connection loop: **Git ⇄ Kanban ⇄ Automation ⇄ Agent Profile ⇄ Git** (see §9). Build order: **Agent Profile + Kanban (data-model foundation) → Git read+write → Automation**.

---

## 2. Background & Problem

Padu already has strong multi-agent orchestration (queue, steer, checkpoint rewind), notes, worktrees, and provider-native session resume — but no:

- Visual way to see many tasks/agents at once.
- Automatic response to events (checkpoint failure streak, CI failure, PR merged) without manual intervention.
- Native bridge to real developer work sources (issues, PRs/MRs) — tasks are still created manually.

Competitors (multi-agent features in Cursor/Kilo Code and similar orchestrators) focus on raw orchestration; none unify project management + automation + git in one local-daemon native app. That is Padu's moat.

---

## 3. Goals

- Make Padu the "control tower": task management + agent execution in one place, not just an agent runner.
- Cut friction on the path issue → assign to agent → agent finishes → open PR → review.
- Keep the moat local: automation + git run **through `padu-daemon` on the daemon host**, no third-party cloud service.

### Non-Goals (v1)

- Not a full Jira/Linear replacement (no sprints, story points, velocity).
- Single-user board first; no multi-user real-time collaboration.
- `github.com` and `gitlab.com` only. Self-hosted GitHub Enterprise / GitLab self-managed → post-v1 (see §14, resolved).
- One Padu `Project` = one local checkout = at most one `linked_repo`. Monorepo → multiple Padu projects mapping is post-v1.
- No webhook receiver in v1 (no public endpoint requirement); Git state arrives via authenticated polling. Webhooks are a post-v1 optimization.
- No mobile-specific Kanban/Automation UI in v1; mobile uses the same daemon contract but presentation parity is desktop + web only.

---

## 4. Target Users

- Solo developers / small teams already orchestrating multiple coding agents in Padu.
- Power users who want fewer context switches between tracker, terminal/IDE, and PR review.

---

## 5. Feature 1 — Kanban Board

### 5.1 Description

One global board whose columns map to the real workspace/agent lifecycle — not generic columns. **Card = new daemon entity `Task`** (see §5.4), distinct from `AgentSession`: a task is the unit of work; a session is one execution attempt. A task links to at most one active session at a time (`session_id: null` while in Backlog).

### 5.2 Default Columns (customizable: rename, add, archive; deletion only when empty)

| Column | Meaning | Source of truth |
| --- | --- | --- |
| Backlog | Task not yet assigned / no session | `Task.status = backlog` |
| Queued | Task has a session draft queued for an agent | `AgentSession` created, not yet started (or `QueuedMessage` pending) |
| Agent Running | Workspace active, provider process running | `SessionStatus::Connecting \| Working \| Waiting` |
| Awaiting Review | Agent settled with a valid checkpoint | Last `AgentTurn` completed + `Checkpoint.status = Ready` |
| Merged/Done | PR/MR merged, or manually closed | External `pr_merged`/`mr_merged` signal, or explicit Mark Done |

### 5.2.1 Transition Criteria (objective, daemon-emitted — never client-local)

| Transition | Trigger event (daemon) |
| --- | --- |
| Backlog → Queued | `task_queued` (manual drag or automation `create_card` + auto-queue creates the `AgentSession`) |
| Queued → Agent Running | `workspace_started` (worktree materialized + provider process spawned) |
| Agent Running → Awaiting Review | `agent_completed` (provider settled **and** checkpoint `Ready`) |
| Agent Running → Awaiting Review (attention) | `checkpoint_failed` streak ≥ threshold → same column, `needs_attention = true`, never silently to Done |
| Awaiting Review → Merged/Done | `pr_merged` / `mr_merged` / `issue_closed`, **or** explicit user Mark Done. `agent_completed` alone MUST NOT close a task |
| Any → Backlog (reopen) | Manual reopen or automation `reopen` action; clears `needs_attention`, keeps history |

Every transition is a daemon broadcast (`card_updated` / `task_state_changed`-class event); clients are view/cache only (§10.1).

### 5.3 Key Behaviors

- Card content: title, description (Markdown, convertible to/from existing notes), assignee (`agent_profile_id`), labels/tags (free-form: `bug`, `feature`, `refactor`…), linked issue/PR, live cost/token/duration (from `usage` / `ContextUsage` events), `needs_attention` / `sync_failed` flags, last checkpoint summary.
- Drag Backlog → Queued: daemon creates the `AgentSession` (honoring the project's worktree policy: `SessionWorkspace::NewWorktree { base_branch }` or `Local`) and enqueues the first turn. No extra manual step.
- Running → Review movement is system-driven (event), but manual drag into Running is rejected with an explanation (only the daemon can start a provider process).
- Click card → detail panel: checkpoint timeline scrubber (reuse existing rewind UI), agent log (hydrate on demand via `HydrateSession`), rewind/fork buttons, linked issue/PR actions, automation history for this card.
- Board is global; filters: project, agent (`ProviderKind`), status, label, `needs_attention` / `sync_failed`. Filter state is client presentation state (kept out of the wire protocol, but persisted per-client like other UI prefs).
- Empty / error states: explicit "No tasks match filters", "Git disconnected — showing cached cards", never a blank board.

### 5.4 Data Model (new, additive — existing `Project`/`AgentSession` unchanged)

```rust
// New daemon entity. Additive migration; existing sessions without a task keep working.
Task {
  id: Uuid,
  project_id: Uuid,                 // existing Project.id
  title: String,
  description: String,              // markdown
  labels: String[],
  status: enum(backlog, queued, running, review, done),
  assigned_agent: ProviderKind | null,  // references AgentProfile.agent_id (§6)
  session_id: Uuid | null,          // active AgentSession, null in backlog/done-without-run
  workspace_kind: enum(local, new_worktree, worktree), // mirrors SessionWorkspace at queue time
  linked_issue: { provider: enum(github, gitlab), id: String, url: String } | null,
  linked_pr:    { provider: enum(github, gitlab), id: String, url: String, state: enum(open, draft, merged, closed) } | null,
  needs_attention: bool,
  sync_failed: String | null,       // human-readable reason, null when healthy
  idempotency_keys: String[],       // issued create_issue/create_pr keys for this task
  version: u64,                     // optimistic-concurrency guard (§10.1)
  created_at: u64, updated_at: u64,
  archived: bool,
}

// Extension to existing Project (additive, optional):
Project.linked_repo: { provider: enum(github, gitlab), owner: String, repo: String } | null
```

Terminology lock: **project** = repo/codebase unit mapped to one `linked_repo`. **workspace** = per-task execution directory (`SessionWorkspace`, projectless path `~/.padu/projects/<date>/<slug>` on the daemon host, or a Git worktree). The old `~/.padu/<date>/<slug>` layout keeps its existing first-load migration.

### 5.5 Acceptance Criteria

- User can create, edit, delete, drag cards between allowed columns; disallowed drags explain why.
- Backlog → Queued creates an `AgentSession` + queue entry with zero extra steps; failure surfaces on the card (`sync_failed`-style error, retry button), never silent.
- Cost/token/duration on a running card updates via daemon events (no UI polling); board stays interactive with 500+ cards (virtualized, §10.3).
- Every mouse action has a keyboard equivalent (§10.4); strings are localized via `tr!`/locale files.

---

## 6. Feature 2 — Agent Profile

### 6.1 Description

Central registry over **all 14 providers Padu supports** — `agy` (Antigravity), `amp`, `claude` (Claude Code), `codex` (Codex CLI), `cursor` (Cursor CLI), `deepseek`, `fx`, `opencode`, `grok` (Grok Build), `kimi`, `commandcode`, `ohmypi`, `pi`, `qoder` — with role tags, cost tier, and priority. Single place to reorder/disable agents; kanban assignment and automation resolution both read from here. Display names MUST match `ProviderKind::display_name()`.

### 6.2 Data Model (new daemon table, seeded from `ProviderKind::ALL`)

```rust
AgentProfile {
  agent_id: ProviderKind,           // "codex", "claude", …
  role_tags: String[],              // ["fix", "refactor", "docs", "test", "review", "plan"]
  cost_tier: enum(low, medium, high),
  priority: u32,                    // lower runs first in "next available" / fallback chain
  max_retry_before_escalate: u32,   // default 3
  enabled: bool,                    // default true when CLI probed present; see §6.3
}
```

Seed defaults (editable, shipped as migration): sensible `role_tags` per provider capability (e.g. strong-code providers get `fix`/`refactor`, fast ones `docs`/`test`), `cost_tier` from catalog knowledge, `priority` = probe order. `supports_conversation_rollback/fork` limitations (Kimi/Fx) are surfaced in UI so fallback chains skip impossible hops (e.g. never escalate a rewind-dependent recovery to Fx).

### 6.3 Key Behaviors

- Reorder priority / enable-disable from one screen; no per-rule edits needed.
- `Task.assigned_agent` references `agent_id` (not free text). Disabling an agent removes it from assignee dropdowns and from automation candidacy immediately (in-flight sessions are unaffected; queued-but-unstarted tasks re-resolve).
- Relationship to existing `DaemonSettings.disabled_providers`: `AgentProfile.enabled` becomes the source of truth; on migration, `disabled_providers` seeds `enabled = false` once, then `disabled_providers` is derived (kept for wire-compat until protocol bump, then removed).
- Binary overrides (`provider_binary_overrides`) stay in `DaemonSettings`; profiles never store paths.

### 6.4 Acceptance Criteria

- Fresh install seeds 14 profiles; user edits persist across restarts (daemon SQLite).
- Disabled agent is unselectable in kanban + automation, with a tooltip explaining where to re-enable.
- `cargo test` covers seed migration + disabled-provider import.

---

## 7. Feature 3 — Automation

### 7.1 Description

Event-driven rule engine as a daemon background job (tokio task in `padu-daemon`, beside the session actors — never in GPUI UI thread, never a cloud service). Rules persist in daemon SQLite, evaluate on the daemon event bus, and execute via the same command path as manual actions (so idempotency, audit, and permissions are shared).

### 7.2 Rule Structure

```rust
Rule {
  id: Uuid,
  name: String,
  enabled: bool,
  priority: u32,                    // lower runs first when >1 rule matches one event
  trigger: Trigger,
  condition: Condition | null,      // AND-combined predicate list in v1
  action: Action[],                 // executed sequentially in listed order
  requires_approval: bool,          // true = hold + notify user before executing
  cooldown_secs: u32,               // per-rule rate limit (default 60)
  max_chain_depth: u32,             // default 5 (§7.7)
  created_at: u64, updated_at: u64,
}

Condition examples (v1, dropdown-built):
  card.labels contains "hotfix" · card.project == <project-id>
  agent.cost_tier == low · agent.id == codex
  pr.state == open · time.hour between 9 and 18 · task.status == review

when [TRIGGER] and [CONDITION…] then [ACTION…]   // sequential, priority-ordered
```

### 7.3 Triggers (v1 — each maps to an existing or new daemon event)

`task_queued`, `workspace_started`, `agent_completed`, `checkpoint_failed` (with `streak >= N`), `agent_completed_without_pr` (settled + no `linked_pr`), `agent_reported_new_issue` (provider output classifier, §7.8), `kanban_card_moved` (to a given column), `pr_opened` / `pr_merged` / `mr_merged`, `issue_closed`, `issue_opened` (poll-observed), `ci_check_failed` / `ci_check_passed` (from linked PR checks), `workspace_idle` (no provider output for `N` minutes).

Out of scope v1: cron/schedule triggers, file-watch triggers inside the repo (use CI triggers instead).

### 7.4 Actions (v1)

- `spawn_agent` (specific `agent_id` or `next_available` resolved via Agent Profile `role_tags` + `priority` + `enabled`).
- `move_card`, `create_card` (project + column + optional labels/assignee), `notify_desktop` (via existing desktop notification path).
- `create_issue` (title, body template, labels) → sets `linked_issue`.
- `create_pr` / `create_mr` (source branch = task worktree branch, body generated from checkpoint history, editable when manual) → sets `linked_pr`.
- `escalate` (fallback chain: next `enabled` profile by `priority` that supports the needed capability; gives up after `max_retry_before_escalate` and flags `needs_attention`).
- `archive_workspace` (daemon-owned worktree cleanup; projectless workspaces follow existing retention).

No destructive action in v1 besides `archive_workspace` (which follows existing safe-cleanup semantics). No force-push, no branch delete, no repo-settings mutation.

### 7.5 End-to-End Scenarios (must all work in v1)

1. `when ci_check_failed and task.status == review then spawn_agent(fix) + move_card(running)`
2. `when checkpoint_failed.streak >= 3 then notify("Task X stuck") + move_card(review, needs_attention=true)`
3. `when pr_merged then move_card(done) + archive_workspace`
4. `when agent_completed_without_pr then create_pr(auto_description) + move_card(review)`
5. `when agent_reported_new_issue then create_issue(label=bug) + create_card(backlog)`

### 7.6 UI (desktop + web, parity per §10.4)

- Dropdown-based rule builder (no free-syntax DSL in v1). Power-user compromise: JSON export/import per rule (validates on import).
- Rule list with on/off toggles, priority ordering (drag or up/down buttons, keyboard accessible), per-rule execution history (timestamp, triggering event, actions taken, outcome, link to audit entry).
- Ship with 3 disabled-by-default templates (scenarios 2–4 above) so first-run never auto-writes to a real repo.

### 7.7 Safety (normative)

- Rules persist in daemon; run with UI closed; survive daemon restart via persisted outbox (un-acked actions replay with same `idempotency_key`).
- Every evaluation + execution appends to the local transaction log (audit trail, user-visible per rule and per card).
- Loop protection: per-rule `cooldown_secs`, global `max_chain_depth = 5` per causal chain (chain id propagated on rule-caused events; depth exceeded → halt + `needs_attention` + audit entry). A rule never re-triggers itself on its own output event within the cooldown window.
- `create_issue` / `create_pr` are idempotent: `idempotency_key = hash(rule_id, trigger_event_id)` carried as the daemon command key; retries and daemon-restart replays never duplicate (see §10.1). Card also records issued keys for inspection.
- `requires_approval` defaults **true** for any new rule whose actions include `create_issue`/`create_pr`/`escalate-to-different-tier`. Approval surfaces as a desktop notification + in-app pending-approval queue (approve/deny with one click/keyboard); denial is audited. User may opt out per rule after first successful approved run.
- Multi-match ordering: sequential by `priority` (lower first), stable tiebreak by `rule_id`; never parallel/random. A failed action aborts the remaining actions in that rule (audited), but does not block other rules.

### 7.8 `agent_reported_new_issue` classifier (v1, conservative)

Heuristic over settled-turn transcript (provider-neutral, order-preserving): explicit "found a bug / out of scope" language + file/line reference, or a failing-test block unrelated to the task's touched files. Precision over recall: low-confidence findings produce a `needs_attention` suggestion on the card, NOT an auto-created issue. Classifier thresholds are logged for tuning; no LLM call required in v1 (pattern + diff-scope rules, unit-tested).

### 7.9 Acceptance Criteria

- Rules survive UI close + daemon restart without duplication or loss (outbox replay test).
- Audit log shows every trigger evaluation and action outcome; user can disable a noisy rule in ≤2 clicks.
- Loop-protection tests: self-triggering rule halts at depth/cooldown limits with visible flag.
- `create_*` approval default true; approved run is idempotent under forced retry.

---

## 8. Feature 4 — GitHub/GitLab Integration

### 8.1 Description

Two-way sync between Padu tasks and GitHub / GitLab.com repos — read (import, status, checks) and write (create issue, open PR/MR). All network I/O runs on the daemon host (never the UI thread, never interpreted client-side); results enter the board via the same event path as local transitions.

### 8.2 Read

- Import issue → new `Task` in Backlog with title/body/labels mapped, `linked_issue` set, backlink to Padu task in the imported card metadata (local-only; Padu never edits the remote issue body on import).
- Status sync: remote `issue_closed` / `pr_merged` observed by polling → daemon events → card moves (via default automation templates, user-overridable).
- CI/check status on cards with `linked_pr` (check-run summary + link-out; full log stays in browser).

### 8.3 Write

- **New issue** from a card (manual "New Issue" when `linked_issue == null`, or automation): title/body/labels editable pre-submit when manual; `linked_issue` set from the API response on success (optimistic UI reconciled by daemon event, §10.1). Body includes a Padu backlink (`Padu task <id>` — plain text, no private provider markers).
- **New PR/MR** from a card (manual "Open PR" or automation on `agent_completed_without_pr`): source = task worktree branch (pushed by daemon first), base = repo default (or user override), body generated from checkpoint history (per-turn summaries + file stats, reusing existing checkpoint data), editable pre-submit when manual.
- PR state transitions (`draft → ready_for_review`) follow card position when the user opts in per project (default off in v1 — explicit button instead).

### 8.4 Auth & Connection (priority order, zero-friction first)

1. **Existing CLI auth** — daemon probes `gh auth status` / `glab auth status`; on success reads the token via `gh auth token` / `glab auth token`. Zero new login.
2. **OAuth device flow** — Padu shows code + URL; user approves in browser; daemon polls for access+refresh tokens. Minimal scopes: GitHub `repo, read:user`; GitLab `api` (documented at connect time; user can scope down to one group, which limits import targets accordingly).
3. **PAT manual entry** — final fallback (and the path for Enterprise hostnames post-v1). Paste once; validated immediately with a `whoami` call.

Tokens from any source are stored in the OS keychain (macOS Keychain / Windows Credential Manager / Secret Service on Linux) via the daemon host — never plaintext files, never `settings.json`, never logs. Connection is per `Project.linked_repo`; all tasks under the project inherit it. Disconnect revokes cached token handle locally and marks cards `sync_failed: "needs re-auth"` only if a pending write exists (no alarmist banners otherwise).

### 8.5 Failure Handling (read + write, normative)

- Permanent failure (401-after-refresh → `needs re-auth`; 403/404/422): card gets `sync_failed` with the human-readable reason + Retry button. Never silent.
- Transient failure (rate limit, timeout, 5xx): automatic retry with exponential backoff (1s, 4s, 16s, max 3 attempts) on the daemon background executor, then `sync_failed`.
- Token revoked externally: next API call's 401 demotes connection to `needs re-auth` with a settings prompt; background polling for that connection pauses (no hot retry loop).
- Rate limits: daemon tracks `X-RateLimit-Remaining` (GitHub) / `RateLimit-Remaining` (GitLab); below 10% it slows polling and prioritizes user-initiated requests over background sync. Polling defaults: 60s per connected repo when window focused, 5min when idle/background; ETag/`If-None-Match` caching; no per-card polling (one pass per repo per tick, generation-guarded so superseded passes cannot overwrite newer state).

### 8.6 Acceptance Criteria

- Connect flow works via CLI token when present (no extra login), via device flow otherwise; PAT path validated.
- Imported issue → card mapping is exact (title, body markdown, labels); card links open the remote issue/PR in the browser.
- Manual + automated issue/PR creation land in the correct repo, set `linked_*` on the card, and never duplicate under retry (idempotency test with stubbed API).
- Every sync failure is visible on the card and/or settings; token-revocation produces `needs re-auth`, not a silent stall.

---

## 9. Dependencies Between Features

```text
Agent Profile ──references──▶ Kanban assignee + Automation spawn/escalate resolution
Git Integration ──imports──▶ Kanban cards (Backlog) + status/check signals
Kanban moves / new cards ──emit──▶ Automation triggers
Automation ──calls──▶ Git Integration (create issue/PR) + Agent queue (spawn, via Agent Profile)
Git signals (issue/PR open/close/merge) ──update──▶ Kanban cards
```

Build order: **Agent Profile + Kanban (foundation) → Git read+write → Automation wiring**. Each phase bumps the wire protocol and ships desktop + web together (§13).

---

## 10. Technical Considerations

### 10.1 Data Principle: Daemon Is Source of Truth (extends current architecture)

- Single source of truth in `padu-daemon` (SQLite for tasks/rules/profiles/audit/outbox; keychain for tokens). Clients (`apps/desktop`, `apps/web`) hold in-memory view caches only.
- Command–query separation: all mutations are explicit daemon commands (create/update card with `expected_version`, toggle rule, create issue/PR, edit profile). Client never writes daemon DB directly.
- Event-driven UI sync: daemon broadcasts `card_updated`, `rule_triggered`, `approval_requested`, `workspace_status_changed`, `issue_created`, `pr_opened` over the existing sequenced channel (`SequencedEvent`, replay cursors, dedup). External Git polling is the only polling in the system; UI never polls.
- Optimistic UI + reconciliation: drag/card-create/issue-create apply instantly client-side, then confirm or roll back on the daemon event (which carries canonical URLs/ids).
- Versioning: every `Task` carries `version`/`updated_at`; stale-version commands are rejected and the client re-fetches (same pattern as existing multi-agent conflict prevention).
- Batching: initial board load fetches task summaries (no checkpoint bodies/logs); detail hydrates on card open (`HydrateSession`-style on-demand fetch).
- Idempotency: every automation action and every Git write carries `idempotency_key` (trigger-instance scoped); daemon dedups on (key, command) before executing.
- Audit: append-only local transaction log (commands + events + rule evaluations) backs both the automation history UI and crash recovery (outbox replay).

### 10.2 Wire-Protocol & Storage Plan (normative for implementers)

- New `padu-protocol` modules: `kanban.rs` (`Task`, `TaskSummary`, `LinkedRepo`, `LinkedIssue/Pr`), `agent_profile.rs`, `automation.rs` (`Rule`, `Trigger`, `Action`, `PendingApproval`), `git_integration.rs` (`GitConnection`, check status). All `TS`-derived for codegen.
- New `Command`s (e.g. `ListTasks`, `CreateTask`, `UpdateTask`, `MoveTask`, `ListAgentProfiles`, `UpdateAgentProfile`, `ListRules`, `SaveRule`, `ToggleRule`, `ApprovePendingAction`, `ConnectGit`, `ImportIssue`, `CreateIssue`, `CreatePr`, `RetrySync`) with matching `ResponsePayload`s; new `ServerMessage` events listed in §10.1. `PROTOCOL_VERSION` bumps per phase (8 → 9 Kanban/Profile, → 10 Git, → 11 Automation).
- After any wire-type change: `bun run protocol:generate` + `bun run protocol:check`, commit `packages/padu-client/src/generated`, update `@padu/client` reducers + `apps/web` types in the same change set (see `.agents/skills/protocol-parity-sync/SKILL.md`).
- `SaveTaskState` stays merge-only; task/rule/profile saves follow the same rule: stale snapshots never delete rows another client created (`RemoveSession`-style explicit delete only).
- Remote-daemon correctness: all Git CLI/API work and worktree paths execute on the daemon host; clients never interpret daemon paths (local folder picker/PTY stay unavailable on remote connections until their dedicated endpoints land — existing limitation, unchanged).

### 10.3 Performance (product requirement, per `docs/performance.md` + AGENTS.md)

- Board renders via virtualized `list()`; row builders read cached in-memory summaries only — no subprocess, filesystem walk, network, blocking lock, or sync IPC in `render`, even if "cheap/cached".
- Git polling + rule evaluation run on `cx.background_executor().spawn` (desktop) / daemon tokio tasks (backend); results stored on entities + `cx.notify()`. One repo-wide pass per tick (generation-guarded), never per-card I/O.
- Live cost/token/duration ride existing event cadences (stream commits ≤ ~8.3 Hz, pulse ≤ ~30 Hz); card badges update at most once per commit, not per chunk.
- Read `docs/performance.md` before touching the event pump, pulse clock, veils, overlay scrollbars, or pane caching.

### 10.4 Client Parity, Accessibility, i18n (product requirements)

- Every feature ships in `apps/desktop/` (GPUI idioms) **and** `apps/web/` (web patterns) in the same change set, with consistent hierarchy, states, and interaction model. Platform-exclusive exceptions (macOS window chrome, OS keychain UI) are documented, not assumed.
- Keyboard: every drag has a keyboard path (grab with Space/Enter, move with arrows, drop with Enter, cancel with Esc); rule priority reorder, toggles, approval queue, and card actions are all tab-reachable with visible `focus_visible` treatment. Standard keys (`home`/`end`, `enter`/`space`, `escape`, arrows) behave conventionally.
- Reduce-motion honored (`cx.reduce_motion()` / `App::reduce_motion`); status never encoded by color/hover/motion alone (icon + text + sufficient contrast in both themes; generous hit areas).
- All user-facing strings via the existing locale pipeline (`tr!`, `locales/*.yml`); no hardcoded English in new UI.

---

## 11. Success Metrics (instrumented from daemon events/audit, not guesses)

- Share of tasks created via issue import (target > 40% within 60 days of Git phase).
- Share of issues/PRs created from Padu (manual + automated) vs. created in browser.
- Active automation rules per weekly-active user; approval accept-rate per rule template.
- Retention: weekly frequency / session length for kanban+automation users vs. orchestration-only baseline.
- Reliability: `sync_failed` rate per 100 Git writes; duplicate-create rate (must be 0); rule-loop halt count.

---

## 12. Risks & Mitigations

| Risk | Mitigation |
| --- | --- |
| Automation loops (rule chains) | Chain depth cap + per-rule cooldown + audit halt flag (§7.7) |
| UI overload (board + rules + git + profiles at once) | Phased rollout (§13); board-only mode until user connects git / enables automation |
| API rate limits | ETag caching, adaptive polling, user-initiated priority (§8.5) |
| Duplicate issues/PRs on retry | Mandatory `idempotency_key` + outbox dedup (§7.7, §10.1) |
| Silent auto-write to real repos | `requires_approval` default true; templates ship disabled (§7.6–7.7) |
| Revoked tokens silently stall sync | 401 → `needs re-auth`, polling paused (§8.5) |
| Stale-client overwrites (drag vs. rule race) | `expected_version` rejection + re-fetch (§10.1) |
| Secrets in logs/DB | Keychain-only tokens; audit log redacts credentials; `cargo test` asserts redaction |
| Board jank on long transcripts | Virtualized lists, summary-first fetch, event cadence caps (§10.3) |

---

## 13. Release Roadmap (each phase: protocol bump + codegen + desktop & web + checks)

- **Phase 1 — Kanban + Agent Profile (manual, no git/automation).** New `Task` store, board UI, 14 seeded profiles. Protocol 8 → 9. Exit: §5.5 + §6.4 pass; `cargo fmt/check/test`, `protocol:check`, web typecheck/test green; validated in relaunched Padu Debug.
- **Phase 2 — Git read.** Connect (CLI → device → PAT), `linked_repo` per project, import → Backlog, status/check polling. Protocol → 10. Exit: §8.6 read half.
- **Phase 3 — Git write (manual).** New-issue + Open-PR from cards, approval-free (explicit user click is the approval), idempotent, failure-visible. Exit: §8.6 write half with stubbed-API duplicate test.
- **Phase 4 — Automation core.** Engine + dropdown builder + list/history + triggers/actions minus auto-create (spawn/move/notify/escalate only). Protocol → 11. Exit: scenarios §7.5 (1–3) + §7.9 core.
- **Phase 5 — Automation + Git write.** Auto create issue/PR rules, fallback chains, cooldowns/depth caps, approval queue, audit UI, classifier (§7.8) with conservative thresholds. Exit: scenarios §7.5 (4–5) + full §7.9.
- **Post-v1 (explicitly out):** self-hosted Enterprise hostnames, webhook push, cron/file-watch triggers, advanced scripting mode, monorepo multi-project mapping, mobile-specific surfaces.

Pre-PR for every phase: `.agents/skills/pre-pr-review/SKILL.md` review + `cargo fmt`, `cargo check`, `cargo test`, `protocol:check`, client/web checks; diff audit for UI-thread I/O, parity, keyboard a11y, and error handling.

---

## 14. Open Questions — Resolved

1. **Self-hosted GitLab / GitHub Enterprise — which phase?** → **Post-v1.** v1 targets `github.com` + `gitlab.com` only. Rationale: device flow + `gh`/`glab` token paths differ per Enterprise hostname/SSO configuration; PAT fallback already unblocks Enterprise users manually without committing v1 to per-host QA. Track as the first post-v1 Git milestone.
2. **Does the rule builder need an "advanced" (scripting) mode at launch?** → **No. Deferred post-v1.** v1 ships dropdown builder + per-rule JSON export/import (validates on import) as the power-user escape hatch. Rationale: keeps v1 safety story (approval defaults, idempotency, audit) reviewable without a second expression language; revisit once rule-evaluation telemetry shows which predicates users actually compose.

No blocking open questions remain.
