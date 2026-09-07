ALTER TABLE `sessions` ADD `pinned_at` integer;--> statement-breakpoint
ALTER TABLE `sessions` ADD `archived_at` integer;--> statement-breakpoint
CREATE INDEX `sessions_by_archive_pin` ON `sessions` (`archived_at`,`pinned_at`);--> statement-breakpoint
CREATE INDEX `sessions_by_archive_updated_at` ON `sessions` (`archived_at`,`updated_at`);