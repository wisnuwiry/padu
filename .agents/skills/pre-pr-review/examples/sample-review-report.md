# Example: Pre-PR Code Review Report

Realistic report showing scope, checks, findings, and PR description.
Branch `feature/reasoning-veil-stride` vs `origin/main` — verdict ⚠️ ACTION REQUIRED.

```markdown
# 🔍 Pre-PR Code Review Report

**Branch:** `feature/reasoning-veil-stride` · **Base:** `origin/main`
**Verdict:** ⚠️ ACTION REQUIRED

## 1. Scope

| Domain | Files |
| :--- | :---: |
| Rust native | 4 |
| Web client | 2 |

**Parity**: both surfaces updated. ✅

## 2. Automated Checks

| St | Check | Time |
| :---: | :--- | ---: |
| ✔ | Rust formatting / check / test (87 passed) | 39s |
| ✔ | Protocol sync · client typecheck+test (52) · web typecheck+test (140) | 14s |
| — | Mobile typecheck | skipped |

**Total: 53s · 0 errors · 1 warning**

## 3. Anti-Pattern Scan

Clean except 1 bare `.unwrap()` — `src/ui/motion.rs:247` on `lease.stride()`
(always `Some` here, but `.unwrap_or(1)` documents the invariant).

## 4. Findings

### 🟡 Warnings

1. **`src/ui/motion.rs:247`** — bare `.unwrap()` → `.unwrap_or(1)`.
2. **`src/ui/motion.rs:312`** — `fast_stride` misnamed for a 15 Hz
   (`Pulse::every(2)`) reasoning veil → rename `reasoning_stride`.

### 🟢 Suggestions

1. **`apps/web/.../transcript-presentation.ts:89`** — ternary → `?? 2`.

## 5. Performance / Accessibility

Perf ✅ (strided pulse clock, no new triggers, island-local rebuild).
A11y ✅ (no new controls; veil dissolve skips on `reduce_motion`).

## 6. Generated PR Description

### Summary

Stride the reasoning-veil dissolve at ~15 Hz instead of 30 Hz on the
transcript pane (~3% streaming CPU reduction in debug builds).

### Changes

- `src/ui/motion.rs`, `src/app/transcript_view.rs`: `reasoning_stride`.
- `apps/web/.../transcript-presentation.ts` (+ test): stride parameter.

### Checks

`cargo fmt` ✅ · `check` ✅ · `test` ✅ · `protocol:check` ✅ ·
`client` ✅ · `web` ✅ · manual debug-app validation ✅

### Limitations

Release-build measurement pending.
```

## Next steps

1. Fix the `.unwrap()`; 2. rename; 3. re-run checks, open PR.
