# Draft PR: Add project Notes

## Summary

Adds project-scoped Notes backed by daemon persistence and exposes a full Notes workspace in the native desktop client and web client. Notes support Markdown editing/preview, project metadata, CRUD operations, assistant-response capture, and embedded note snapshots in chat state.

## Changes

### Native Desktop (`apps/desktop/`)

- Added daemon-backed Notes loading, creation, update, and deletion.
- Added Notes navigation to the sidebar footer.
- Added command-palette navigation with `Cmd/Ctrl+Shift+N`.
- Notes now replace the conversation center while preserving the existing sidebar.
- Selecting a session returns to the Conversation workspace.
- Removed Notes from Settings navigation.
- Added Markdown modes:
  - Edit/source only.
  - Side by Side editor and preview.
  - Preview/read-only.
- Reused the file-preview Markdown rendering and selection/scrolling infrastructure.
- Added project, created-at, and updated-at metadata.
- Preserved keyboard operation and visible focus treatments.

### Web Client (`apps/web/`)

- Added `/notes` route and project-scoped daemon CRUD.
- Added Notes sidebar navigation.
- Added Markdown edit/preview UI.
- Added assistant-response capture into Notes.
- Added full-width embedded-note presentation in transcript messages.
- Added project-aware navigation from the composer Notes entry point.

### Protocol / Shared (`crates/`, `packages/`, `db/`)

- Added project-scoped Note and NoteSummary protocol types.
- Added list/get/create/update/delete Notes commands and responses.
- Added optimistic revision checks.
- Added SQLite schema and migrations.
- Added embedded note snapshots to messages, queued messages, and composer drafts.
- Regenerated TypeScript bindings.
- Bumped protocol version to `8`.

## Checks

| Check | Status |
| :--- | :--- |
| `cargo fmt --manifest-path apps/desktop/Cargo.toml -- --check` | ✅ |
| `cargo check --workspace` | ✅ |
| `cargo check --manifest-path apps/desktop/Cargo.toml` | ✅ |
| Focused Notes protocol tests | ✅ |
| `cargo test --workspace` | ❌ 376 passed, 7 failed due `Operation not permitted` in existing provider/server socket/process tests |
| `bun run protocol:generate` | ✅ |
| `bun run protocol:check` | ✅ |
| `bun run --filter @padu/client check` | ✅ |
| `bun run --filter @padu/client test` | ✅ 15 passed |
| `bun run --filter @padu/web typecheck` | ✅ |
| `bun run --filter @padu/web test` | ❌ 158 passed, 1 existing i18n catalog synchronization failure |
| `git diff --check` | ✅ |
| Pre-PR anti-pattern scans | ✅ |
| Manual debug-app validation | ➖ Not run; visual validation was not requested |

## Performance

- Notes CRUD and daemon requests run off the UI thread.
- Render paths read cached note/editor state only.
- Markdown preview uses cached Markdown view state and existing scroll/selection infrastructure.
- No new streaming animation or `request_animation_frame` path was introduced.

## Accessibility

- Notes navigation and actions are keyboard-operable.
- Edit, Side by Side, Preview, Save, Delete, and note-list controls have focus-visible states.
- Enter/Space activate Notes controls.
- Note attachment presentation includes text labels in addition to icon/color treatment.

## Parity

- Desktop and web Notes surfaces were updated together.
- Protocol bindings were regenerated and checked.
- `PaduClient` sends `PROTOCOL_VERSION` and rejects any daemon version that does not match exactly. Protocol version `8` therefore requires synchronized client and daemon releases; existing clients must be updated to complete the handshake rather than remaining compatible with the old version.

## Limitations & Follow-ups

- The full structured `/note` composer picker and rich staged note attachment flow still needs to be completed across every desktop submission path. The protocol snapshot fields and transcript rendering are in place, but the current composer behavior remains partially wired.
- Notes invalidation is refreshed through explicit list/detail loads; a dedicated `NotesChanged` broadcast event should be added for live multi-client cache invalidation.
- Full workspace tests need an environment that permits the existing socket/process tests.
- Translated catalog entries for the new Notes strings have been added to `zh-CN.yml`, `ja.yml`, and `id.yml`; the locale synchronization test stays green.

## Related Issues

- TBD
