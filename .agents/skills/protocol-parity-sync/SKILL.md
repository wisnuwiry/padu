---
name: protocol-parity-sync
description: >-
  Sync wire-protocol changes to TypeScript bindings, @padu/client reducers,
  and desktop/web/mobile UI. Use when touching crates/padu-protocol.
---

# Protocol & Cross-Client Parity Skill

Pipeline: Rust types → `protocol:generate` → `@padu/client` reducer →
desktop + web (+ mobile unless platform-exclusive) → verify script.
Item-level boxes: [checklist](./references/parity-checklist.md). Worked
example: [adding a protocol event](./examples/adding-protocol-event.md).

## When to use

Changing `crates/padu-protocol/`, adding daemon events/commands, mirroring a
feature across clients, or fixing stale `packages/padu-client/src/generated/`.

## Workflow

1. **Rust type** — derive `Serialize, Deserialize, TS` + `#[ts(export)]`;
   register new root types in `export_types.rs`.
2. **Codegen** — `bun run protocol:generate && bun run protocol:check`;
   commit `packages/padu-client/src/generated/`.
3. **Shared client** — handle in `event-reducer.ts` (+ presentation helpers),
   add `*.test.ts` cases,
   `bun run --filter @padu/client check && bun run --filter @padu/client test`.
4. **Desktop** — `apps/desktop/src/ui/` or `app/`; GPUI idioms, keyboard
   focus, zero blocking I/O on UI thread.
5. **Web + mobile** — mirror in `apps/web/src/` and `apps/mobile/src/`;
   reuse `@padu/client` state, then run their typechecks/tests.
6. **Verify** — `.agents/skills/protocol-parity-sync/scripts/verify-parity.sh`.
