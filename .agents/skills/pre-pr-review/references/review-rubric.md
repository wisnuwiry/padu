# Pre-PR Code Review Rubric

Checklist per domain: what to look for plus the grep that finds it.
Performance model source of truth:
[Cadence Invariants](../../gpui-perf-audit/references/cadence-invariants.md)
and [docs/performance.md](../../../../docs/performance.md).

---

## 1. Performance & UI Thread Safety

One blocking call in `render()` is a user-visible hitch. Invariants in one
line: no I/O reached from `render()`; no `request_animation_frame` /
`window.refresh()` during streaming (never `with_animation().repeat()` —
use the pulse clock); commits ≤ ~8.3 Hz; pulse ≤ ~30 Hz with stride on
costly surfaces; virtualize with `list()`, remeasure tail rows only, hash
fingerprints at display granularity; scrollbars hold at constant opacity.

**Detection greps** (run on the diff):

```bash
# Blocking I/O reached from render
git diff origin/main...HEAD -- '*.rs' | grep -n '^\+' | grep -E '(std::fs::|File::open|read_to_string|metadata\(\)|canonicalize\(\))'
git diff origin/main...HEAD -- '*.rs' | grep -n '^\+' | grep -E '(Command::new|std::process::)'
git diff origin/main...HEAD -- '*.rs' | grep -n '^\+' | grep -E '(\.lock\(\)|\.read\(\)|\.write\(\))' | grep -v 'RwLock.*async'
git diff origin/main...HEAD -- '*.rs' | grep -n '^\+' | grep -E '(reqwest::blocking|TcpStream::connect)'

# Frame-trigger abuse (verify non-streaming use)
git diff origin/main...HEAD -- '*.rs' | grep -n '^\+' | grep -E '(request_animation_frame|window\.refresh|with_animation.*repeat)'
```

**Fix**: `cx.background_executor().spawn(...)` → store on entity →
`cx.notify()`; a miss renders a placeholder. New panes must follow the
`PaduPane` + `Entity::cached` island pattern.

---

## 2. Client Parity & Protocol Sync

Desktop (`src/app/`, `src/ui/`, `src/input/`) ↔ web (`apps/web/`) must move
together unless platform-exclusive (window chrome, OS integrations); same
for user-facing `apps/mobile/` changes. Item boxes:
[parity checklist](../../protocol-parity-sync/references/parity-checklist.md).

```bash
# Rust UI changed, no web counterpart
git diff --name-only origin/main...HEAD | grep -E '^src/(app|ui|input)' && \
  ! git diff --name-only origin/main...HEAD | grep -qE '^apps/web/' && \
  echo "⚠ Parity warning"
```

Wire types changed → `bun run protocol:generate`, commit `generated/`,
`bun run protocol:check` exits 0.

---

## 3. Accessibility

| Requirement | Check |
| :--- | :--- |
| Keyboard operable | `track_focus`, `tab_index`/`tab_group`/`tab_stop`; arrows, Home/End, Enter/Space, Escape |
| Visible focus | `focus_visible` treatment |
| Reduce-motion | Decorative `request_animation_frame` gated on `cx.reduce_motion()` (`with_animation` already respects it) |
| No color-only meaning | Status paired with icon/text; hover-revealed content also focus-reachable |
| Legibility | Both themes contrast; sufficient hit area |

---

## 4. Code Quality

```bash
# Bare .unwrap() (ok only in tests or with `// safe:`)
git diff origin/main...HEAD -- '*.rs' | grep -n '^\+' | grep '\.unwrap()' | grep -v 'test' | grep -v '// safe:'

# Debug leftovers
git diff origin/main...HEAD -- '*.rs' | grep -n '^\+' | grep -E '(dbg!\(|println!\(|eprintln!\()'
git diff origin/main...HEAD -- '*.ts' '*.tsx' | grep -n '^\+' | grep -E '(console\.(log|debug|warn)\(|debugger;)'

# Secrets / sensitive files
git diff origin/main...HEAD | grep -niE '(api[_-]?key|secret[_-]?key|password|token|credential)\s*[:=]\s*["\x27]'
git status --short | grep -iE '\.(env|pem|key|p12|pfx)$'
```

Provider invariants: preserve event ordering (citations, reasoning, tool
events); never leak private provider control markers into the transcript.
Keep existing comments unless deliberately updating; document non-obvious
architecture inline.

---

## 5. Verification baseline (run by the check script)

`cargo fmt --check` · `cargo check` · `cargo test` ·
`bun run protocol:check` · `@padu/client` check+test ·
`@padu/web` typecheck+test (if web changed) ·
`@padu/mobile` typecheck (if mobile changed).

Desktop crate name is `padu` (`apps/desktop/Cargo.toml`), not `padu-desktop` —
scope desktop-only runs as `cargo check|test -p padu`.
