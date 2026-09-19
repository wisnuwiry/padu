---
name: pre-pr-review
description: >-
  Review changes before a PR or prepare a PR: scoped automated checks, diff
  audit (performance, parity, accessibility, security), review report.
---

# Pre-PR Code Review Skill

Diff-scoped review: establish scope, run the check script, audit only what
changed, then report. Deep rules live in [the rubric](./references/review-rubric.md) —
read the section matching the changed domains, not the whole file.

## Step 0 — Scope

```bash
git diff --name-only origin/main...HEAD
```

| Domain | Paths | Activates |
| :--- | :--- | :--- |
| **Rust native** | `apps/desktop/`, `crates/`, `Cargo.*`, `build.rs` | Rust checks + perf audit (§1) |
| **Wire protocol** | `crates/padu-protocol/` | Protocol sync (§2b) |
| **Client / Web / Mobile** | `packages/padu-client/`, `apps/web/`, `apps/mobile/` | Respective checks + parity (§2) |

UI change on only one of `apps/desktop/` ↔ `apps/web/` = **parity warning**.

## Step 1 — Automated checks

```bash
.agents/skills/pre-pr-review/scripts/run-checks.sh [--base <ref>] [--full]
```

Smart-scoped to changed files (git hygiene, anti-pattern scans, Rust,
protocol sync, client/web/mobile, parity heuristic). Fallback:
[CONTRIBUTING.md](../../../CONTRIBUTING.md#checks).

## Step 2 — Manual audit (diff only)

For each changed file, apply the matching rubric section:

- **Rust UI/streaming touched** → rubric §1 (zero blocking I/O in `render()`,
  no `request_animation_frame`/`window.refresh()` during streaming,
  commits ≤ ~8.3 Hz, pulse ≤ ~30 Hz). Full model:
  [cadence invariants](../gpui-perf-audit/references/cadence-invariants.md).
- **User-facing change** → rubric §2 (desktop ↔ web ↔ mobile parity;
  `bun run protocol:generate` if wire types changed).
- **New/changed controls** → rubric §3 (keyboard, `focus_visible`,
  `reduce_motion`, no color-only meaning).
- **Any logic** → rubric §4 (no bare `.unwrap()`, no debug code/secrets,
  provider event ordering).

Fix pattern for render-path I/O: `cx.background_executor().spawn(...)` →
store on entity → `cx.notify()`; a miss renders a placeholder.

## Step 3 — Report

Verdict (✅ READY / ⚠️ ACTION REQUIRED / 🔴 BLOCKED), scope table, checks
table, findings grouped as 🔴 Blockers / 🟡 Warnings / 🟢 Suggestions (each
with `path:line`), plus a PR description per
[pr-template.md](./references/pr-template.md). Layout:
[sample-review-report.md](./examples/sample-review-report.md).

## Subagent

```
TypeName: pre-pr-reviewer — review current branch vs origin/main:
run checks, audit diffs (perf, parity, a11y, security), emit report.
```
