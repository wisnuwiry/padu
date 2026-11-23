# Padu OpenCode v2, Context, and Session Rename Plan

**Date:** 2026-11-23
**Status:** Implementation plan only. No feature implementation is included on this branch.
**Scope:** OpenCode v1/v2 provider separation, session rename propagation, and effective context-window accounting.
**Excluded:** SSH remote work is maintained on its separate branch and is not part of this plan.

## 16. Scope and Status

This part defines the implementation plan for three changes:

1. Add first-class support for OpenCode v2 while preserving the existing
   OpenCode v1 provider.
2. Propagate user-driven Padu session renames to supported harnesses.
3. Correct Claude Code context-window accounting for non-Claude and custom
   models whose effective context budget is below 1M tokens.

The plan is based on the repository state and harness versions observed on
2026-11-23:

| Item | Observed state |
| --- | --- |
| OpenCode binary | `v2.3.2` |
| OpenCode v2 API info | API version `2.1.3` |
| OpenCode v2 event stream | SSE at `/api/event` |
| OpenCode model catalog | `/api/model` |
| Claude Code | `2.1.330` |
| Claude compact-window environment | `CLAUDE_CODE_AUTO_COMPACT_WINDOW=400000` |

No implementation is part of this plan branch. Runtime behavior, protocol
changes, provider data, and harness configuration remain unchanged until a
separate implementation batch is approved.

## 17. Product Decisions

### 17.1 OpenCode

OpenCode v1 and OpenCode v2 will be separate public providers. This avoids a
single driver that must support two incompatible API and event contracts.

| Decision | Final behavior |
| --- | --- |
| Provider model | Two public providers: `OpenCode v1` and `OpenCode v2` |
| Existing provider identity | `ProviderKind::OpenCode` remains v1 with serialized ID `openCode` |
| New provider identity | Add `ProviderKind::OpenCodeV2` with serialized ID `openCodeV2` |
| Command | Both providers use the `opencode` command, with independent binary overrides |
| Native session cursor | v1 keeps `ProviderResumeCursor::OpenCode`; v2 uses a separate cursor variant |
| Driver implementation | Separate v1 and v2 drivers; no shared compatibility parser |
| Unsupported binary | The incompatible provider remains visible in Settings as unavailable with a reason |
| Existing v1 sessions | Stay v1; there is no automatic or manual upgrade to v2 |
| Missing v1 native data | Mark the session as not resumable and ask the user to provide the v1 data path |
| Rename support | v1 stores the title only in Padu; v2 uses the native v2 rename API |

### 17.2 OpenCode v2 storage and settings isolation

OpenCode v2 owns the standard OpenCode directories currently present on the
machine. OpenCode v1 receives isolated directories so the two providers do not
share models, credentials, sessions, state, skills, MCP configuration, or cache.

| Data | OpenCode v2 | OpenCode v1 |
| --- | --- | --- |
| Config | `~/.config/opencode` | Dedicated Padu-owned v1 XDG root |
| Auth and sessions | `~/.local/share/opencode` | Dedicated Padu-owned v1 XDG root |
| State and service files | `~/.local/state/opencode` | Dedicated Padu-owned v1 XDG root |
| Cache | `~/.cache/opencode` | Dedicated Padu-owned v1 XDG root |

Padu sets provider-specific `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
`XDG_STATE_HOME`, and `XDG_CACHE_HOME` values for the child process. OpenCode
continues to append its own `opencode` directory according to its normal
convention.

Padu must not write persistent OpenCode configuration. Settings and
credentials are managed outside Padu. Provider Settings will show the resolved
config, data, state, and cache paths as read-only, copyable text.

Padu may inject ephemeral process environment for Padu-owned integration
features such as MCP or Computer Use. Those values must not be persisted into
the user's OpenCode configuration.

### 17.3 Session rename

Manual titles remain authoritative in Padu. Provider-generated titles continue
to populate `auto_title` and must never overwrite a manual title.

| Provider | Native rename behavior |
| --- | --- |
| Codex | Use Codex's native thread name operation |
| Claude Code | Use Claude's native session rename behavior when a live runtime exists |
| OpenCode v2 | `PATCH /api/session/{sessionID}` with the new title |
| OpenCode v1 | Padu-only title; no native rename |
| All other providers | Padu-only title |

Rename behavior is intentionally one-shot:

1. The user commits a rename in Padu.
2. Padu updates and persists the manual title immediately.
3. If a compatible runtime is active, Padu performs one native rename attempt.
4. If the native operation fails, Padu shows one warning and stops.
5. Padu does not retry while the runtime stays active.
6. Padu does not retry when the session or application is opened again.

No pending-rename queue is persisted. Reopening Padu must not send a rename
request that the user did not initiate in that session.

### 17.4 Claude context budget

Padu must not use `modelUsage[*].contextWindow` as the primary budget for a
Claude session with a custom model. It also must not fall back to the largest
window in the result map, because that may select a title-generation model or a
different provider entry.

The effective budget is resolved in this order:

1. Read `CLAUDE_CODE_AUTO_COMPACT_WINDOW` from the same environment used to
   launch the Claude child process.
2. Parse a plain integer or a value with a `k` or `M` suffix.
3. If the environment value is absent or invalid, inspect the effective model
   identifier.
4. A model identifier containing `[1m]` resolves to `1,000,000` tokens.
5. A model identifier without `[1m]` resolves to `250,000` tokens.
6. If no model identifier can be determined, the budget is `Unknown`.

The current machine observes `CLAUDE_CODE_AUTO_COMPACT_WINDOW=400000`, so the
effective Claude budget is 400K tokens.

Padu does not set or overwrite `CLAUDE_CODE_AUTO_COMPACT_WINDOW`,
`CLAUDE_CODE_MAX_CONTEXT_TOKENS`, `CLAUDE_CODE_DISABLE_1M_CONTEXT`, or any
other harness setting. Users configure those values outside Padu.

The Padu-side Claude context-window selector `200k / 1m` is removed. Existing
stored `context_window` values on Claude sessions are ignored so old Padu
settings cannot silently alter a harness-defined effective budget.

The context meter will be labeled `Effective context` across providers. Its
tooltip reports the source of the denominator:

| Source | Tooltip meaning |
| --- | --- |
| Harness environment | `CLAUDE_CODE_AUTO_COMPACT_WINDOW` |
| Claude model suffix | `Model name ([1m])` |
| Claude non-1M fallback | `Default for non-[1m] model` |
| Provider stream or catalog | `Provider-reported context window` |
| No usable model or value | `Context budget could not be determined` |

When the token count exceeds the effective budget, the visual gauge remains
full at 100 percent. The raw token count and budget remain visible, and the
text must communicate that the budget is exceeded instead of hiding the
overflow.

## 18. Protocol and Persistent Model Changes

The implementation will update the wire protocol and regenerate TypeScript
bindings with `bun run protocol:generate` and `bun run protocol:check`.

### 18.1 Provider identity and cursors

Add:

- `ProviderKind::OpenCodeV2`
- `ProviderResumeCursor::OpenCodeV2 { session_id }`

Preserve:

- `ProviderKind::OpenCode` serialized as `openCode`
- `ProviderResumeCursor::OpenCode { session_id }`

The legacy identity must remain stable so existing sessions, profiles, pins,
archives, and provider settings are not silently migrated.

### 18.2 Context usage source

Extend context usage with a serialized source discriminator. The exact name may
be `ContextWindowSource`, and the generated TypeScript must expose the same
values in camelCase.

Suggested variants:

- `HarnessEnvironment`
- `ClaudeModelOneMillion`
- `ClaudeModelDefault`
- `Provider`
- `Unknown`

The source field must survive event reduction and session hydration so the
meter can explain why a denominator exists or why it is unknown.

### 18.3 Rename command and warning

Add a daemon-owned command such as:

```rust
RenameSession {
    session_id: Uuid,
    title: String,
}
```

The command validates and trims the title, persists the manual title, and
returns the updated session metadata. If native synchronization fails, the
daemon returns a non-fatal warning result or emits a dedicated warning event;
the Padu title must remain saved.

The wire contract must not create a persistent retry token. One client action
means at most one native attempt.

## 19. Daemon and Driver Design

### 19.1 Provider registry

Add one agent profile for OpenCode v2. Both OpenCode profiles receive the same
curated role tags and cost tier, but their runtime, cursor, model catalog, and
settings are independent.

Provider probing must validate the binary's major version:

| Binary | OpenCode v1 | OpenCode v2 |
| --- | --- | --- |
| v1.x | Available | Unavailable with reason |
| v2.x | Unavailable with reason | Available |
| Future binary supporting both | Available | Available |

The runtime performs a second guard against `/api/info` before accepting a v2
session. Binary discovery alone is not sufficient evidence that the expected
API is reachable.

### 19.2 OpenCode v1 driver

The current `driver/opencode.rs` implementation remains the v1 driver. It
continues to use the existing v1 routes and event parser. It must receive the
new isolated v1 XDG roots and the v1 cursor.

OpenCode v1 rename remains Padu-only.

### 19.3 OpenCode v2 driver

Create a separate v2 driver with direct routes for:

| Operation | v2 route |
| --- | --- |
| Health and capability detection | `GET /api/info` |
| Model catalog | `GET /api/model` |
| Create session | `POST /api/session` |
| Read session | `GET /api/session/{sessionID}` |
| Rename | `PATCH /api/session/{sessionID}` |
| Prompt | `POST /api/session/{sessionID}/prompt` |
| Steering | Prompt with `delivery: "steer"` |
| Interrupt | `POST /api/session/{sessionID}/interrupt` |
| Messages | `GET /api/session/{sessionID}/message` |
| Session context | `GET /api/session/{sessionID}/context` |
| Permission list | `GET /api/session/{sessionID}/permission` |
| Permission reply | `POST /api/session/{sessionID}/permission/{requestID}/reply` |
| Form list | `GET /api/session/{sessionID}/form` |
| Form reply | `POST /api/session/{sessionID}/form/{formID}/reply` |
| Event stream | `GET /api/event` |

The v2 event stream is an SSE contract and remains volatile by definition. A
disconnect can miss events, so the driver must reconnect and refresh session
messages/context before continuing live event reduction.

The v2 event translator must cover at least:

- `server.connected`
- `session.text.started`, `session.text.delta`, `session.text.ended`
- `session.reasoning.started`, `session.reasoning.delta`,
  `session.reasoning.ended`
- `session.tool.input.started`, `session.tool.input.delta`,
  `session.tool.input.ended`
- `session.tool.called`, `session.tool.progress`, `session.tool.success`,
  `session.tool.failed`
- `session.step.started`, `session.step.ended`, `session.step.failed`
- `session.execution.started`, `session.execution.succeeded`,
  `session.execution.failed`, `session.execution.interrupted`
- `session.renamed`
- `permission.asked`, `permission.replied`
- `form.created`, `form.replied`, `form.cancelled`
- `session.error`

Unknown event types are ignored, retained in debug-level diagnostics, and must
not terminate the session.

### 19.4 Claude driver

The Claude driver will:

1. Resolve the effective context budget from the child-process environment and
   model identifier.
2. Emit `Effective context` source metadata with token and window updates.
3. Stop using the largest `modelUsage.contextWindow` fallback.
4. Ignore the legacy Padu Claude context-window selector.
5. Leave all harness environment and model configuration untouched.

The native rename operation must use one attempt and must not be replayed on
startup or resume.

### 19.5 Codex and other rename-capable harnesses

Codex keeps its existing native thread-name operation. The daemon must route
the manual title through the live driver once rather than generating an
automatic title.

Harnesses without a proven native rename operation receive a Padu-only title
and no warning beyond the generic capability behavior.

## 20. Client and UI Parity

All three clients must present the same provider and session semantics.

### 20.1 Desktop

- Add the OpenCode v2 provider icon, label, availability reason, and settings
  row.
- Show read-only OpenCode v1/v2 config, data, state, and cache paths.
- Remove the Claude `200K / 1M` selector.
- Label the usage lane `Effective context` and display the source in the
  tooltip.
- Clamp the visual gauge to 100 percent while preserving raw token values.
- Send rename changes through the new daemon command.
- Show one non-blocking warning when native rename fails.
- Keep keyboard focus, focus-visible treatment, and reduce-motion behavior
  correct for new controls.

### 20.2 Web

Mirror desktop behavior for:

- OpenCode v1/v2 provider availability and settings.
- Read-only configuration paths.
- Claude context selector removal.
- Effective-context label and source tooltip.
- Rename command, one-shot native attempt, and warning.
- Gauge clamping and unknown-budget state.

### 20.3 Mobile

Mirror the same provider capability and context semantics using adaptive
layouts at the required phone, fold, and tablet widths. Every new control must
have an accessibility role, label, and at least a 44pt target.

## 21. Migration and Compatibility

### 21.1 Provider identity

No automatic session migration occurs. Existing `openCode` rows remain v1.
Users who want OpenCode v2 start new sessions under `openCodeV2`.

### 21.2 Native OpenCode data

The standard OpenCode directories are assigned to v2 because the observed
database uses the v2 session schema. v1 receives isolated directories.

If a v1 session exists without a usable v1 native store, Padu:

1. Keeps the Padu transcript and metadata.
2. Marks the session as not resumable at the native harness layer.
3. Shows the expected v1 config/data/state/cache paths.
4. Asks the user to supply or restore the v1 data path.

Padu must not copy, move, or rewrite the standard v2 directories while
handling a v1 session.

### 21.3 Claude context

No harness setting migration is performed. Padu reads the effective setting
that already exists outside Padu. Stored Padu `context_window` values for
Claude are ignored but may be cleared during a safe later migration.

## 22. Implementation Sequence

### Phase 1 — Protocol and persistent model

- Add OpenCode v2 provider and cursor identity.
- Add context-window source metadata.
- Add the rename command and non-fatal warning contract.
- Regenerate and verify TypeScript bindings.
- Add protocol tests for serialization, deserialization, and generated type
  parity.

### Phase 2 — Daemon and drivers

- Add OpenCode v2 provider registry/profile metadata.
- Add major-version probing and unavailable reasons.
- Configure isolated v1 and standard v2 environment roots.
- Keep the v1 driver separate.
- Implement the v2 driver and event translator.
- Implement Claude effective-budget resolution.
- Implement one-shot rename routing and native capability checks.

### Phase 3 — Clients

- Add provider availability and path displays.
- Remove the Claude window selector.
- Add effective-context source presentation.
- Add rename command integration and one-shot warning behavior.
- Keep desktop, web, and mobile behavior synchronized.

### Phase 4 — Verification

The implementation batch must run the project checks required by the changed
surfaces:

| Area | Required verification |
| --- | --- |
| Protocol | `bun run protocol:generate` and `bun run protocol:check` |
| Rust | `cargo fmt`, `cargo check`, and affected tests |
| Client | Typecheck and tests for `@padu/client` |
| Web | Typecheck and tests |
| Mobile | Typecheck and tests |
| Desktop | Affected Rust tests and interactive smoke validation |

## 23. Acceptance Criteria

### OpenCode

- OpenCode v1 and v2 appear as separate providers.
- A v1 binary makes v1 available and marks v2 unavailable with a reason.
- A v2 binary makes v2 available and marks v1 unavailable with a reason.
- v2 uses the `/api/*` routes listed in Section 19.3.
- v1 and v2 do not share config, auth, sessions, state, cache, models, skills,
  or MCP settings.
- Padu does not persist configuration for either provider.
- Paths are visible as read-only values.
- Existing v1 sessions remain v1 and are not upgraded automatically.
- A missing v1 native store produces an explicit not-resumable state.

### Rename

- Rename in Padu always updates the manual title.
- Manual titles keep precedence over provider-generated titles.
- Codex, Claude, and OpenCode v2 receive at most one native rename attempt.
- OpenCode v1 and unsupported harnesses remain Padu-only.
- A failed native rename keeps the Padu title and shows one warning.
- Reopening Padu does not retry the rename.

### Context

- Claude uses `CLAUDE_CODE_AUTO_COMPACT_WINDOW` when valid.
- A `[1m]` model resolves to 1,000,000 tokens when the environment value is
  absent or invalid.
- A non-`[1m]` model resolves to 250,000 tokens when the environment value is
  absent or invalid.
- A missing model produces `Unknown`.
- Padu no longer falls back to the largest `modelUsage.contextWindow`.
- Padu does not modify Claude or other harness settings.
- The UI labels the meter `Effective context` and exposes the source.
- Visual percentage is clamped to 100 percent while raw values remain visible.

## 24. Non-Goals

- No implementation in this plan branch.
- No automatic OpenCode v1 to OpenCode v2 session upgrade.
- No direct editing of unsupported harness session files.
- No persistent provider configuration writes from Padu.
- No background rename retries.
- No Padu-owned Claude context or compaction settings.
- No changes to delegated context compaction in this plan; its implementation
  and review are tracked separately on `feat/compaction`.

## 25. Related Work

- [Transcript compaction implementation plan](20261120_2041_transcript-compaction-plan.md)
- [Provider integrations](providers.md)
- [Streaming render performance](performance.md)
- [Linux installation and SSH remote hosts](linux.md)
