# Event Normalization Reference for Provider Drivers

Normalize every provider output (ACP notifications, JSON-RPC/SSE lines,
custom stdout) into `DriverEvent` before dispatch.

## Variants

| Variant | Payload | UI effect |
| :--- | :--- | :--- |
| `Connected` | `provider_cursor?` | Session active, cursor updated |
| `TurnStarted` | — | Busy spinner, composer disabled |
| `TextDelta` | `String` | Streaming markdown append |
| `ReasoningDelta` | `String` | Collapsible Thought fold append |
| `RichActivity` | `ActivityItem` | Tool badge + output |
| `Activity` | `{ id, kind, title, detail, complete }` | Progress status |
| `PermissionRequested` | `{ request_id, options, … }` | Allow/Deny modal |
| `UserInputRequested` | `{ request_id, questions }` | Choice modal |
| `TurnFinished` | `{ success, summary? }` | Checkpoint, composer enabled |
| `Error` | `String` | Error banner |
| `ProcessExited` | — | Session disconnected |
| `SteerAccepted` / `SteerRejected` | message (+ reason) | Steering indicator / revert |

## Rules

- **Text**: emit chunks as they arrive; strip ANSI escapes; never buffer
  whole paragraphs.
- **Reasoning**: high-reasoning models stream thinking separately → strip
  bounding tags (`<thinking>`, `<thought>`, `<<THOUGHT>>`).
- **Tool calls**: `ActivityItem { id, kind: Command|FileRead|FileEdit|Search|Plan|Custom, title, detail?, output?, complete }` —
  send `complete: false` at start, update with `output` + `complete: true`.
- **Cadence**: state/disk commits ≤ ~8.3 Hz; decorative motion ≤ ~30 Hz;
  zero allocation-heavy parsing, no disk/Git/locks on render paths
  (see `docs/performance.md`).
