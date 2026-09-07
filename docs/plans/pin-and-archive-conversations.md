# Pin and archive conversations

## Goal

Add durable pin and archive controls for Padu conversations so important active threads stay easy to reach and completed threads can leave the normal workflow without being deleted.

The state must be owned by the daemon and persisted in SQLite so desktop and web remain consistent across restarts, host switches, and simultaneous clients.

## Recommended product behavior

### Pin

- Add `Pin conversation` / `Unpin conversation` to each session context menu.
- A pinned conversation remains active and selectable.
- In **Project** grouping, pinned conversations appear first inside their existing project group; pinned and unpinned subsets each retain the selected newest/oldest ordering.
- In **Updated** grouping, add one leading `Pinned` group, then keep unpinned conversations in the existing calendar groups. This makes pinning useful instead of leaving a pinned conversation inside an old date bucket.
- Show a small pin icon on pinned rows. Do not rely on color or hover alone to communicate pinned state.
- Do not add manual pinned ordering in the first version. `pinned_at` leaves room for it later, but v1 uses the existing recency ordering within the pinned subset.

### Archive

- Add `Archive conversation` to the session context menu below pin/unpin and above rename/delete.
- Archiving preserves the session, transcript, checkpoints, provider cursor, worktree references, and message-search data.
- Archived conversations are excluded from the normal sidebar, previous/next session navigation, task switcher, and default command-palette results.
- Add an `Archived conversations` management surface to both desktop and web settings. It lists archived rows newest-archived first and supports `Open`, `Unarchive`, and permanent `Delete`.
- Opening an archived conversation from settings should unarchive it first, then select it. This avoids a selected conversation that is invisible in normal navigation.
- Archiving atomically clears `pinned_at`; unarchiving does not restore the old pin.
- Disable archive while a session is `Connecting`, `Working`, or `Waiting`. The menu item should explain that the active turn must finish or be cancelled first. This avoids hiding a live runtime and its pending input.
- If the selected idle/failed conversation is archived, select the nearest visible active conversation using sidebar order. If none exists, create/select the normal new-task draft for the current project.
- Permanent deletion remains the only destructive action and keeps its existing confirmation dialog.

### Keyboard and focus behavior

- Keep `Shift+F10`/context-menu-key access on desktop and web.
- Context menu items must be operable with arrows and Enter/Space through the existing menu primitives.
- After pin/unpin, return focus to the same row and keep it visible if its position changes.
- After archive removes a row, move focus to the selected replacement row; if there is no active row, move focus to the new-task control.
- The archived settings list must support normal tab navigation, visible focus, and keyboard activation for open/unarchive/delete.
- Do not assign a global pin/archive shortcut in v1. Add commands to the command palette only after deciding on stable keybindings.

## Data model and migration

### Shared protocol model

Update `AgentSession` in `crates/padu-protocol/src/model.rs` with lightweight list metadata:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub pinned_at: Option<u64>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub archived_at: Option<u64>,
```

Use timestamps rather than booleans because they:

- encode the state without extra columns;
- support deterministic archived-list ordering;
- leave room for pin recency/manual-order migration later;
- deserialize old state as active and unpinned.

Initialize both fields to `None` in `AgentSession::new`, preserve them in `list_projection`, and include serde compatibility tests.

### SQLite

Update `db/schema.ts` and generate a new migration with `bun run db:generate`:

- nullable integer `sessions.pinned_at`;
- nullable integer `sessions.archived_at`;
- index `(archived_at, pinned_at)` for active/pinned filtering;
- index `(archived_at, updated_at)` for active and archived ordering.

Do not put these values only in `session_details`: sidebar startup intentionally reads narrow `sessions` rows without deserializing transcript JSON.

Update `crates/padu-core/src/persistence.rs` in all promoted-column paths:

- startup `SELECT` and row tuple;
- `session_skeleton` construction;
- `session_params`;
- `UPSERT_SESSION` insert/update columns;
- persistence fixtures and migration tests.

Hydration should continue treating the narrow row as authoritative for these fields; it should not overwrite them from potentially older `session_details` JSON.

## Protocol and daemon ownership

### Commands

Add dedicated commands in `crates/padu-protocol/src/protocol.rs`:

```rust
SetSessionPinned { pinned: bool }
SetSessionArchived { archived: bool }
```

Use the request envelope's `session_id`, as `RemoveSession` does. The daemon assigns timestamps from its own clock.

Return a new response containing the updated list projection, for example:

```rust
SessionMetadataUpdated { session: AgentSession }
```

A dedicated mutation is preferable to sending a whole hydrated `AgentSession` through `SaveTaskState`: a stale desktop or browser snapshot must not overwrite a pin/archive change made by another client, and metadata changes should not require transcript hydration.

### Daemon handling

In `crates/padu-core/src/daemon.rs`:

1. Resolve the target session or return the existing not-found error.
2. For pin:
   - reject `pinned = true` when the session is archived;
   - set `pinned_at = Some(unix_time())` or `None`;
   - avoid a write/notification when state is unchanged.
3. For archive:
   - reject `archived = true` while `session.is_busy()`;
   - set `archived_at = Some(unix_time())` or `None`;
   - clear `pinned_at` in the same locked mutation when archiving.
4. Mark only that session dirty and persist it.
5. Return its `list_projection`.

Register both commands as task-catalog changes in `crates/padu-core/src/server.rs` so other connected clients receive `TaskStateChanged` and invalidate their snapshots.

Also update stale-session merge behavior in `crates/padu-core/src/daemon.rs`. Pin/archive fields must remain daemon-authoritative during generic `SaveTaskState`; a stale full-session save must not undo a newer dedicated mutation. Add an explicit concurrency regression test analogous to `stale_projection_cannot_resurrect_a_removed_session`.

### Generated clients

Run:

```sh
bun run protocol:generate
bun run protocol:check
```

Commit the generated changes in `packages/padu-client/src/generated/`. Add small typed helpers in `apps/web/src/lib/daemon-api.ts` for pin/archive commands and response validation.

No event-reducer change is expected because this is catalog metadata, not a streaming runtime event.

## Desktop integration

### State mutation helpers

Add focused methods on `Padu`, preferably outside rendering code:

- `set_session_pinned(session_id, pinned, cx)`;
- `set_session_archived(session_id, archived, window, cx)`.

They should issue the dedicated daemon command, merge the returned projection into `self.state.sessions`, invalidate `sidebar_rows_fingerprint`, update selection/focus when archiving, and notify. Use existing toast/error handling. A one-shot menu action may perform synchronous RPC under the current project rule, but no persistence or IPC may be reached from row building/rendering.

### Sidebar rows and cache

Update `apps/desktop/src/app/sidebar.rs`:

- Add `SidebarGroup::Pinned` for Updated grouping only.
- Filter archived sessions before grouping.
- Project grouping: stable-partition each project's sorted sessions into pinned then unpinned.
- Updated grouping: collect pinned sessions into the leading Pinned group and date-group only unpinned sessions.
- Include `pinned_at` and `archived_at` in `sidebar_rows_cached`'s fingerprint. Skip all other work for archived rows.
- Ensure project `Show more` counts only unpinned active history; pinned sessions must never disappear behind the seven-day reveal cutoff.
- Extend `element_key`, fingerprint mixing, labels, row height logic, collapse-all behavior, and tests for the new group.
- Keep all row construction in memory and proportional to the current session catalog; no DB calls or RPC from `sidebar_rows`, `sidebar_row`, or `render_sidebar_session_item`.

### Session row and menu

In `render_sidebar_session_item`:

- render a legible pin icon adjacent to the title/status when pinned;
- add pin/unpin and archive items to the context menu;
- disable archive for busy sessions and expose an explanatory label/tooltip using the menu primitive's supported disabled treatment;
- retain rename and destructive delete separation;
- after pinning reorders a row, reveal it and restore focus on the next frame.

Use existing or newly added SVG assets under the desktop icon directory rather than drawing a meaning-bearing state with color alone.

### Other desktop surfaces

Audit and filter archived sessions in:

- previous/next and first/last session navigation;
- task switcher;
- command-palette default session results and message-search result presentation;
- startup/restored selection (`ensure_runtime_session` and equivalent selection repair);
- project cleanup logic, which must still count archived sessions so archiving the last projectless conversation does not orphan/remove its project metadata.

Add an Archived conversations settings page/section using a virtualized or bounded list. It must operate only on already-loaded list projections; opening a row can hydrate after unarchiving/selecting through the existing background/session-selection path.

## Web integration

### Sidebar presentation

Update `apps/web/src/lib/sidebar-presentation.ts` in parity with desktop:

- exclude archived sessions from normal groups;
- add the leading Pinned group for Updated grouping;
- pin first within project groups;
- exclude pinned sessions from project reveal pagination;
- add unit tests mirroring desktop ordering and visibility cases.

Update `apps/web/src/components/sidebar.tsx`:

- add `onPinSession` and `onArchiveSession` props;
- show the pin indicator;
- add pin/unpin/archive context-menu items in the same order and enabled states as desktop;
- preserve Shift+F10/context-menu-key behavior and focus restoration when rows reorder/disappear.

### Mutation flow

In `apps/web/src/components/padu-app.tsx`:

- call the dedicated daemon helpers;
- optimistically patch both task-state and hydrated-session query caches;
- roll back on failure and display localized errors;
- use the daemon-returned projection as the final value so client clock skew cannot alter timestamps;
- invalidate/reconcile the task-state query after success.

Do not hydrate and resave the whole transcript for these metadata changes; the current rename flow is not the concurrency model to copy.

### Archived settings surface

Add the same Archived conversations page/section to web settings with open/unarchive/delete actions and responsive behavior. Filter the normal task switcher, command palette, and restored selection exactly as on desktop.

If `apps/mobile` consumes `AgentSession` or task-state catalogs, ensure generated-type compatibility and exclude archived sessions from its normal conversation list. If it has no conversation-management UI yet, document pin/archive actions as deferred UI while still preventing archived rows from reappearing.

## Localization and iconography

Add matching translation keys for all supported locales, including:

- `sidebar.pinned`;
- `session.pin`, `session.unpin`;
- `session.archive`, `session.unarchive`;
- `session.archive_busy`;
- `settings.archived_conversations`;
- empty-state, open, and mutation error strings.

Use the same labels and icon concepts on desktop and web. Ensure pin/archive icons exist in both icon registries and have accessible names where they are interactive or status-bearing.

## Test plan

### Protocol/model

- Old JSON without either field deserializes to `None`.
- New sessions start active and unpinned.
- `list_projection` preserves both timestamps.
- Generated TypeScript compiles with the new fields and commands.

### Persistence/migrations

- Fresh DB applies the new migration once.
- Existing rows migrate to `NULL`/active/unpinned.
- Pin/archive timestamps round-trip through narrow session rows.
- Updating a skeleton session writes metadata without rewriting transcript/messages.
- Archived sessions remain hydratable and searchable.

### Daemon/concurrency

- Pin, unpin, archive, and unarchive mutate one session and return its projection.
- Archiving clears pin atomically.
- Busy sessions cannot be archived.
- A stale `SaveTaskState` cannot undo pin/archive state.
- A second websocket client receives `TaskStateChanged` for both commands.
- Permanent removal still deletes archived sessions and associated rows.

### Desktop sidebar

- Archived rows never appear in either grouping mode.
- Project grouping places pinned rows first and retains selected ordering within each subset.
- Updated grouping has one leading Pinned group and no duplicate date-group row.
- Pinned old sessions bypass project `Show more`; archived sessions do not affect reveal counts.
- Sidebar cache fingerprint changes for pin/archive mutations.
- Keyboard navigation skips archived sessions and remains correct after a row moves/disappears.
- Archiving selected first/middle/last/only active session chooses the expected fallback.

### Web

- Mirror desktop grouping/order tests in `sidebar-presentation.test.ts`.
- Context-menu pin/archive actions are keyboard accessible.
- Optimistic pin/archive cache patches roll back on command failure.
- Archived settings can open (unarchive + select), unarchive, and delete a session.
- Normal command palette/task switcher do not expose archived sessions.

## Validation sequence

Run focused checks first, then parity and workspace checks:

```sh
cargo fmt --all -- --check
cargo test -p padu-protocol
cargo test -p padu-core persistence
cargo test -p padu-core daemon
cargo test -p padu-desktop sidebar
bun run protocol:generate
bun run protocol:check
bun run --filter @padu/client check
bun run --filter @padu/client test
bun run --filter @padu/web typecheck
bun run --filter @padu/web test
bun run --filter @padu/mobile typecheck
.agents/skills/protocol-parity-sync/scripts/verify-parity.sh
```

During implementation, rely on the existing `bun ./scripts/dev.ts` watcher. After successful rebuild/relaunch, manually validate the exact pin/archive flows in the fresh signed debug app only when visual validation is requested or scheduled for the implementation task.

## Suggested implementation sequence and commits

1. `feat: persist conversation pin and archive metadata`
   - protocol model, DB schema/migration, narrow-row persistence, model/persistence tests.
2. `feat: add daemon conversation disposition commands`
   - dedicated commands/responses, conflict-safe daemon handlers, notifications, concurrency tests, generated TS.
3. `feat: add desktop pin and archive workflows`
   - sidebar ordering/menu/status, selection repair, archived settings, desktop tests and translations/assets.
4. `feat: add web pin and archive workflows`
   - presentation rules, mutations, sidebar, archived settings, tests and translations/assets.
5. `chore: verify pin and archive client parity`
   - mobile compatibility if applicable, parity fixes, documentation, full validation results.

Keep each commit buildable and do not combine schema/protocol generation with unrelated sidebar refactors.

## Acceptance criteria

- Pin/archive state survives daemon and client restarts.
- Desktop and web connected to the same daemon converge after either client changes a session.
- Pinned conversations are visibly identified and predictably ordered in both grouping modes.
- Archived conversations disappear from every normal navigation surface without data loss.
- Archived conversations can be found, unarchived/opened, and permanently deleted from settings.
- A stale client cannot undo a newer pin/archive mutation.
- No render or row-builder path performs database, filesystem, network, subprocess, blocking-lock, or synchronous IPC work.
- Mouse and keyboard users can complete every pin/archive/unarchive flow with visible focus and non-color status cues.
