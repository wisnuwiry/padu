CREATE TABLE `agent_profiles` (
	`agent_id` text PRIMARY KEY NOT NULL,
	`role_tags` text DEFAULT '[]' NOT NULL,
	`cost_tier` text NOT NULL,
	`priority` integer NOT NULL,
	`max_retry_before_escalate` integer DEFAULT 3 NOT NULL,
	`enabled` integer DEFAULT true NOT NULL
);
--> statement-breakpoint
CREATE INDEX `agent_profiles_by_priority` ON `agent_profiles` (`priority`);--> statement-breakpoint
CREATE TABLE `audit_log` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`rule_id` text,
	`task_id` text,
	`event` text NOT NULL,
	`detail` text,
	`created_at` integer NOT NULL
);
--> statement-breakpoint
CREATE INDEX `audit_log_by_rule` ON `audit_log` (`rule_id`,`created_at`);--> statement-breakpoint
CREATE INDEX `audit_log_by_task` ON `audit_log` (`task_id`,`created_at`);--> statement-breakpoint
CREATE TABLE `automation_outbox` (
	`id` text PRIMARY KEY NOT NULL,
	`rule_id` text NOT NULL,
	`idempotency_key` text NOT NULL,
	`action` text NOT NULL,
	`status` text NOT NULL,
	`attempts` integer DEFAULT 0 NOT NULL,
	`next_retry_at` integer,
	`created_at` integer NOT NULL,
	`updated_at` integer NOT NULL
);
--> statement-breakpoint
CREATE UNIQUE INDEX `automation_outbox_idempotency_key` ON `automation_outbox` (`idempotency_key`);--> statement-breakpoint
CREATE INDEX `automation_outbox_by_status` ON `automation_outbox` (`status`,`next_retry_at`);--> statement-breakpoint
CREATE TABLE `pending_approvals` (
	`id` text PRIMARY KEY NOT NULL,
	`rule_id` text NOT NULL,
	`trigger_event_id` text NOT NULL,
	`action` text DEFAULT '[]' NOT NULL,
	`status` text NOT NULL,
	`requested_at` integer NOT NULL,
	`decided_at` integer
);
--> statement-breakpoint
CREATE INDEX `pending_approvals_by_rule` ON `pending_approvals` (`rule_id`,`requested_at`);--> statement-breakpoint
CREATE INDEX `pending_approvals_by_status` ON `pending_approvals` (`status`,`requested_at`);--> statement-breakpoint
CREATE TABLE `rules` (
	`id` text PRIMARY KEY NOT NULL,
	`name` text NOT NULL,
	`enabled` integer DEFAULT true NOT NULL,
	`priority` integer NOT NULL,
	`trigger` text NOT NULL,
	`condition` text,
	`action` text DEFAULT '[]' NOT NULL,
	`requires_approval` integer DEFAULT false NOT NULL,
	`cooldown_secs` integer DEFAULT 60 NOT NULL,
	`max_chain_depth` integer DEFAULT 5 NOT NULL,
	`created_at` integer NOT NULL,
	`updated_at` integer NOT NULL
);
--> statement-breakpoint
CREATE INDEX `rules_by_priority` ON `rules` (`priority`);--> statement-breakpoint
CREATE TABLE `tasks` (
	`id` text PRIMARY KEY NOT NULL,
	`project_id` text NOT NULL,
	`title` text NOT NULL,
	`description` text DEFAULT '' NOT NULL,
	`labels` text DEFAULT '[]' NOT NULL,
	`status` text NOT NULL,
	`assigned_agent` text,
	`session_id` text,
	`workspace_kind` text,
	`linked_issue` text,
	`linked_pr` text,
	`needs_attention` integer DEFAULT false NOT NULL,
	`sync_failed` text,
	`idempotency_keys` text DEFAULT '[]' NOT NULL,
	`version` integer DEFAULT 1 NOT NULL,
	`created_at` integer NOT NULL,
	`updated_at` integer NOT NULL,
	`archived` integer DEFAULT false NOT NULL
);
--> statement-breakpoint
CREATE INDEX `tasks_by_project` ON `tasks` (`project_id`,`updated_at`);--> statement-breakpoint
CREATE INDEX `tasks_by_status` ON `tasks` (`status`,`updated_at`);--> statement-breakpoint
CREATE INDEX `tasks_by_session` ON `tasks` (`session_id`);--> statement-breakpoint
ALTER TABLE `projects` ADD `linked_repo` text;