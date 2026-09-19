# Cross-Client Parity & Protocol Checklist

## 1. Wire protocol (`crates/padu-protocol/`)

- [ ] Type derives `Serialize, Deserialize, TS` + `#[ts(export)]`; new root
      types registered in `export_types.rs`.
- [ ] `bun run protocol:generate` run, `generated/` committed,
      `bun run protocol:check` exits 0.

## 2. Shared client (`packages/padu-client/`)

- [ ] Event handled in `event-reducer.ts`; presentation helpers updated
      (`transcript-presentation.ts`, `composer-preferences.ts`, …).
- [ ] Unit tests added; `bun run --filter @padu/client check` + `test` pass.

## 3. Desktop (`apps/desktop/src/`)

- [ ] GPUI idioms, theme colors; zero blocking I/O in `render()` (work via
      `cx.background_executor().spawn`); `track_focus`/`tab_index`/`focus_visible`;
      `Entity::cached` where appropriate.

## 4. Web (`apps/web/`) + Mobile (`apps/mobile/`)

- [ ] Mirrored components reusing `@padu/client` state (Tailwind tokens on
      web, ≥44pt touch targets on mobile); skip only if platform-exclusive.
- [ ] `bun run --filter @padu/web typecheck` + `test`,
      `bun run --filter @padu/mobile typecheck` pass.
