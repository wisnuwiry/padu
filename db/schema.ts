/**
 * Padu local state schema.
 *
 * Drizzle is a build-time tool here: `bun run db:generate` diffs this file and
 * writes plain SQL into `db/migrations`, which the Rust app applies at startup
 * (see `apply_migrations` in `src/persistence.rs`). drizzle-orm never ships in
 * the binary — Rust owns every query.
 *
 * Session history is kept out of the `sessions` row: the row holds only what
 * the session list renders, so listing is a scan over narrow rows. The
 * transcript lives in `session_details` and messages in `messages`, both
 * fetched only when a session is opened.
 */

import { index, integer, sqliteTable, text, uniqueIndex } from "drizzle-orm/sqlite-core";

export const projects = sqliteTable("projects", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  path: text("path").notNull(),
  /** Order shown in the sidebar. */
  position: integer("position").notNull(),
  /** When the project was added, unix seconds. */
  createdAt: integer("created_at").notNull(),
  /** Project scripts / actions, JSON serialized. */
  scripts: text("scripts").notNull().default("[]"),
  /**
   * Linked GitHub/GitLab repo, JSON serialized `{ provider, owner, repo }`.
   * Null means not connected (PRD §5.4 extension, Phase 2).
   */
  linkedRepo: text("linked_repo"),
});

export const notes = sqliteTable(
  "notes",
  {
    id: text("id").primaryKey(),
    projectId: text("project_id").notNull(),
    title: text("title").notNull(),
    content: text("content").notNull(),
    revision: integer("revision").notNull(),
    createdAt: integer("created_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
  },
  (table) => [index("notes_by_project").on(table.projectId, table.updatedAt)],
);

export const sessions = sqliteTable(
  "sessions",
  {
    id: text("id").primaryKey(),
    projectId: text("project_id").notNull(),
    /** Explicit user title; "New task" means the automatic fallback is active. */
    title: text("title").notNull(),
    /** Provider-generated title, with the first prompt as a local fallback. */
    autoTitle: text("auto_title"),
    provider: text("provider").notNull(),
    model: text("model"),
    status: text("status").notNull(),
    /** Session creation time, unix seconds. */
    createdAt: integer("created_at").notNull(),
    /** Any mutation, unix seconds — including title edits and truncation. */
    updatedAt: integer("updated_at").notNull(),
    /** Completion of the most recent assistant turn, unix seconds. */
    lastReplyAt: integer("last_reply_at"),
    /** When the session was pinned, unix seconds. */
    pinnedAt: integer("pinned_at"),
    /** When the session was archived, unix seconds. */
    archivedAt: integer("archived_at"),
  },
  (table) => [
    index("sessions_by_project").on(table.projectId, table.updatedAt),
    index("sessions_by_updated_at").on(table.updatedAt),
    index("sessions_by_last_reply_at").on(table.lastReplyAt),
    index("sessions_by_archive_pin").on(table.archivedAt, table.pinnedAt),
    index("sessions_by_archive_updated_at").on(
      table.archivedAt,
      table.updatedAt,
    ),
  ],
);

/**
 * Conversation messages, one row each.
 *
 * Split out of `sessions.data` so appending to a long conversation writes one
 * small row instead of rewriting the whole history, and so a message can be
 * read or counted without deserializing a transcript.
 */
export const messages = sqliteTable(
  "messages",
  {
    id: text("id").primaryKey(),
    sessionId: text("session_id").notNull(),
    turnId: text("turn_id"),
    /** Ordinal within the session; conversation order, not wall-clock. */
    position: integer("position").notNull(),
    role: text("role").notNull(),
    content: text("content").notNull(),
    /** User-visible text before provider-facing attachment mentions. */
    displayContent: text("display_content"),
    /** JSON-serialized MessageAttachment array. */
    attachments: text("attachments").notNull().default("[]"),
    /** JSON-serialized immutable EmbeddedNote snapshot array. */
    embeddedNotes: text("embedded_notes").notNull().default("[]"),
    createdAt: integer("created_at").notNull(),
    streaming: integer("streaming", { mode: "boolean" }).notNull(),
  },
  (table) => [index("messages_by_session").on(table.sessionId, table.position)],
);

/**
 * The rest of `AgentSession` as JSON — transcript blocks, turns, provider
 * cursor.
 *
 * Split from `sessions` because it is large and rarely read: keeping it in the
 * row would mean listing sessions pages through every transcript, and every
 * title edit rewrites a transcript-sized row.
 */
export const sessionDetails = sqliteTable("session_details", {
  sessionId: text("session_id").primaryKey(),
  data: text("data").notNull(),
});

/**
 * Kanban tasks (PRD §5.4). A task is the unit of work; a session is one
 * execution attempt. Phase 0 storage plan only — DAOs land in Phase 1.
 */
export const tasks = sqliteTable(
  "tasks",
  {
    id: text("id").primaryKey(),
    projectId: text("project_id").notNull(),
    title: text("title").notNull(),
    /** Markdown description. */
    description: text("description").notNull().default(""),
    /** Free-form labels, JSON-serialized string array. */
    labels: text("labels").notNull().default("[]"),
    /** backlog | queued | running | review | done (PRD §5.2). */
    status: text("status").notNull(),
    /** AgentProfile.agent_id this task is assigned to, if any. */
    assignedAgent: text("assigned_agent"),
    /** Active AgentSession id; null in backlog / done-without-run. */
    sessionId: text("session_id"),
    /** SessionWorkspace kind at queue time: local | new_worktree | worktree. */
    workspaceKind: text("workspace_kind"),
    /** Remote issue handle, JSON-serialized `{ provider, id, url }`. */
    linkedIssue: text("linked_issue"),
    /** Remote PR/MR handle, JSON `{ provider, id, url, state }`. */
    linkedPr: text("linked_pr"),
    needsAttention: integer("needs_attention", { mode: "boolean" })
      .notNull()
      .default(false),
    /** Human-readable failure reason; null when healthy. */
    syncFailed: text("sync_failed"),
    /** Issued create_issue/create_pr idempotency keys, JSON array. */
    idempotencyKeys: text("idempotency_keys").notNull().default("[]"),
    /** Optimistic-concurrency guard, checked on every update. */
    version: integer("version").notNull().default(1),
    createdAt: integer("created_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
    archived: integer("archived", { mode: "boolean" })
      .notNull()
      .default(false),
  },
  (table) => [
    index("tasks_by_project").on(table.projectId, table.updatedAt),
    index("tasks_by_status").on(table.status, table.updatedAt),
    index("tasks_by_session").on(table.sessionId),
  ],
);

/**
 * Agent Profile registry (PRD §6.2), seeded from ProviderKind::ALL in
 * Phase 1. Phase 0 storage plan only.
 */
export const agentProfiles = sqliteTable(
  "agent_profiles",
  {
    /** ProviderKind value, e.g. "codex". */
    agentId: text("agent_id").primaryKey(),
    /** Role tags, JSON-serialized string array. */
    roleTags: text("role_tags").notNull().default("[]"),
    /** low | medium | high. */
    costTier: text("cost_tier").notNull(),
    /** Lower runs first in "next available" / fallback chains. */
    priority: integer("priority").notNull(),
    maxRetryBeforeEscalate: integer("max_retry_before_escalate")
      .notNull()
      .default(3),
    enabled: integer("enabled", { mode: "boolean" }).notNull().default(true),
    /**
     * Optimistic-concurrency guard for UpdateAgentProfile (P1-02).
     * Bumped on every write; clients send `expected_version` and re-fetch
     * on conflict. Existing rows backfill to 1.
     */
    version: integer("version").notNull().default(1),
  },
  (table) => [index("agent_profiles_by_priority").on(table.priority)],
);

/**
 * Automation rules (PRD §7.2). Engine lands in Phase 4; Phase 0 storage
 * plan only.
 */
export const rules = sqliteTable(
  "rules",
  {
    id: text("id").primaryKey(),
    name: text("name").notNull(),
    enabled: integer("enabled", { mode: "boolean" }).notNull().default(true),
    /** Lower runs first when several rules match one event. */
    priority: integer("priority").notNull(),
    /** Trigger descriptor, JSON-serialized. */
    trigger: text("trigger").notNull(),
    /** AND-combined predicates, JSON array; null means always. */
    condition: text("condition"),
    /** Sequential actions, JSON-serialized array. */
    action: text("action").notNull().default("[]"),
    requiresApproval: integer("requires_approval", { mode: "boolean" })
      .notNull()
      .default(false),
    /** Per-rule rate limit, seconds. */
    cooldownSecs: integer("cooldown_secs").notNull().default(60),
    maxChainDepth: integer("max_chain_depth").notNull().default(5),
    createdAt: integer("created_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
  },
  (table) => [index("rules_by_priority").on(table.priority)],
);

/**
 * Pending human approvals for gated automation actions (PRD §7.7).
 * Phase 5 behavior; Phase 0 storage plan only.
 */
export const pendingApprovals = sqliteTable(
  "pending_approvals",
  {
    id: text("id").primaryKey(),
    ruleId: text("rule_id").notNull(),
    /** The event instance that triggered the rule. */
    triggerEventId: text("trigger_event_id").notNull(),
    /** Proposed actions, JSON-serialized array. */
    action: text("action").notNull().default("[]"),
    /** pending | approved | denied. */
    status: text("status").notNull(),
    requestedAt: integer("requested_at").notNull(),
    decidedAt: integer("decided_at"),
  },
  (table) => [
    index("pending_approvals_by_rule").on(table.ruleId, table.requestedAt),
    index("pending_approvals_by_status").on(table.status, table.requestedAt),
  ],
);

/**
 * Durable outbox for automation actions: un-acked rows replay with the same
 * idempotency key across daemon restarts (PRD §7.7). Phase 4 behavior;
 * Phase 0 storage plan only.
 */
export const automationOutbox = sqliteTable(
  "automation_outbox",
  {
    id: text("id").primaryKey(),
    ruleId: text("rule_id").notNull(),
    /** hash(rule_id, trigger_event_id); retries never duplicate. */
    idempotencyKey: text("idempotency_key").notNull(),
    /** Action payload, JSON-serialized. */
    action: text("action").notNull(),
    /** pending | in_flight | done | failed. */
    status: text("status").notNull(),
    attempts: integer("attempts").notNull().default(0),
    nextRetryAt: integer("next_retry_at"),
    createdAt: integer("created_at").notNull(),
    updatedAt: integer("updated_at").notNull(),
  },
  (table) => [
    uniqueIndex("automation_outbox_idempotency_key").on(table.idempotencyKey),
    index("automation_outbox_by_status").on(table.status, table.nextRetryAt),
  ],
);

/**
 * Append-only audit trail: every rule evaluation + execution lands here,
 * user-visible per rule and per card (PRD §7.7). Phase 4 behavior; Phase 0
 * storage plan only.
 */
export const auditLog = sqliteTable(
  "audit_log",
  {
    id: integer("id").primaryKey({ autoIncrement: true }),
    ruleId: text("rule_id"),
    /** Kanban task id this entry concerns, if any. */
    taskId: text("task_id"),
    /** e.g. rule_triggered, action_executed, approval_requested. */
    event: text("event").notNull(),
    /** Thresholds, outcomes, reasons — JSON-serialized. */
    detail: text("detail"),
    createdAt: integer("created_at").notNull(),
  },
  (table) => [
    index("audit_log_by_rule").on(table.ruleId, table.createdAt),
    index("audit_log_by_task").on(table.taskId, table.createdAt),
  ],
);
