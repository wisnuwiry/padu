# Example: Adding a Protocol Event (`CustomModelStatus`)

```rust
// 1. crates/padu-protocol/src/lib.rs
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CustomModelStatus {
    pub provider_id: String,
    pub is_available: bool,
    pub latency_ms: Option<u64>,
}
// + ServerMessage::CustomModelStatus(CustomModelStatus) if daemon-emitted
```

```bash
# 2. Codegen → packages/padu-client/src/generated/CustomModelStatus.ts
bun run protocol:generate && bun run protocol:check
```

```typescript
// 3. event-reducer.ts (+ event-reducer.test.ts), then:
// bun run --filter @padu/client test
export function reduceCustomModelStatus(state: SessionState, event: CustomModelStatus): SessionState {
  return { ...state,
    modelStatuses: { ...state.modelStatuses,
      [event.provider_id]: { isAvailable: event.is_available, latencyMs: event.latency_ms ?? null } } };
}
```

4. **Desktop** (`src/ui/model_picker.rs`): status indicator, keyboard focus
   preserved. 5. **Web** (`ModelPicker.tsx`): identical Tailwind badge.
6. `.agents/skills/protocol-parity-sync/scripts/verify-parity.sh`
