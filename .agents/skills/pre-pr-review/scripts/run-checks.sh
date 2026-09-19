#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# Pre-PR Automated Check Suite for Padu
#
# Runs formatting, compilation, tests, protocol sync, anti-pattern scans, and
# client checks. Smart-scopes expensive steps to the files actually changed.
#
# Usage:
#   .agents/skills/pre-pr-review/scripts/run-checks.sh [--base <ref>] [--full]
#
# Options:
#   --base <ref>   Compare against this ref (default: origin/main)
#   --full         Run every check regardless of changed files
# ──────────────────────────────────────────────────────────────────────────────

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT_DIR"

BASE_REF="origin/main"
FULL_MODE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --base) BASE_REF="$2"; shift 2 ;;
        --full) FULL_MODE=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

ERRORS=0
WARNINGS=0
TOTAL_START=$SECONDS
declare -a CHECK_RESULTS=()

# ── Helpers ──────────────────────────────────────────────────────────────────

# Output policy: one line per passing check; failing checks print the last
# 30 lines of captured output so the cause is visible without re-running.
CHECK_LOG="$(mktemp)"
trap 'rm -f "$CHECK_LOG"' EXIT

run_check() {
    local name="$1"
    local cmd="$2"
    local start=$SECONDS
    echo -e "${BLUE}▶ ${name}${NC}"
    if eval "$cmd" >"$CHECK_LOG" 2>&1; then
        local elapsed=$(( SECONDS - start ))
        echo -e "${GREEN}  ✔ Passed${DIM} (${elapsed}s)${NC}"
        CHECK_RESULTS+=("✔|${name}|${elapsed}s")
    else
        local elapsed=$(( SECONDS - start ))
        echo -e "${RED}  ✖ Failed${DIM} (${elapsed}s) — error tail:${NC}"
        tail -30 "$CHECK_LOG" | sed 's/^/    /'
        CHECK_RESULTS+=("✖|${name}|${elapsed}s")
        ERRORS=$((ERRORS + 1))
    fi
}

run_warn_check() {
    local name="$1"
    local cmd="$2"
    local start=$SECONDS
    echo -e "${BLUE}▶ ${name}${NC}"
    local output
    output=$(eval "$cmd" 2>&1) || true
    local elapsed=$(( SECONDS - start ))
    if [ -n "$output" ]; then
        echo -e "${YELLOW}  ⚠ Findings${DIM} (${elapsed}s)${NC}"
        echo "$output" | head -30
        CHECK_RESULTS+=("⚠|${name}|${elapsed}s")
        WARNINGS=$((WARNINGS + 1))
    else
        echo -e "${GREEN}  ✔ Clean${DIM} (${elapsed}s)${NC}"
        CHECK_RESULTS+=("✔|${name}|${elapsed}s")
    fi
}

# ── 0. Determine Scope ──────────────────────────────────────────────────────

echo -e "${BOLD}${BLUE}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}${BLUE}║   Padu Pre-PR Automated Check Suite         ║${NC}"
echo -e "${BOLD}${BLUE}╚══════════════════════════════════════════════╝${NC}\n"

echo -e "${BOLD}Base ref:${NC} ${BASE_REF}"

# Collect changed files relative to base (fall back to worktree-vs-HEAD when
# the base ref does not exist, e.g. fresh clones without origin/main).
if git rev-parse --verify --quiet "${BASE_REF}" >/dev/null; then
    DIFF_RANGE="${BASE_REF}...HEAD"
else
    DIFF_RANGE="HEAD"
    echo -e "${YELLOW}  Base '${BASE_REF}' not found — diffing worktree against HEAD${NC}"
fi
CHANGED_FILES=$(git diff --name-only ${DIFF_RANGE} 2>/dev/null || git diff --name-only HEAD)
if [ -z "$CHANGED_FILES" ]; then
    CHANGED_FILES=$(git diff --name-only HEAD)
fi

RUST_CHANGED=false
WEB_CHANGED=false
MOBILE_CHANGED=false
PROTOCOL_CHANGED=false
CLIENT_PKG_CHANGED=false

if echo "$CHANGED_FILES" | grep -qE '^(apps/desktop/|crates/|Cargo\.|build\.rs)'; then
    RUST_CHANGED=true
fi
if echo "$CHANGED_FILES" | grep -qE '^apps/web/'; then
    WEB_CHANGED=true
fi
if echo "$CHANGED_FILES" | grep -qE '^apps/mobile/'; then
    MOBILE_CHANGED=true
fi
if echo "$CHANGED_FILES" | grep -qE '^crates/padu-protocol/'; then
    PROTOCOL_CHANGED=true
fi
if echo "$CHANGED_FILES" | grep -qE '^packages/padu-client/'; then
    CLIENT_PKG_CHANGED=true
fi

echo -e "${BOLD}Changed areas:${NC} rust=$RUST_CHANGED web=$WEB_CHANGED mobile=$MOBILE_CHANGED protocol=$PROTOCOL_CHANGED client-pkg=$CLIENT_PKG_CHANGED"
echo -e "${DIM}$(echo "$CHANGED_FILES" | wc -l | tr -d ' ') file(s) changed${NC}\n"

if $FULL_MODE; then
    RUST_CHANGED=true; WEB_CHANGED=true; MOBILE_CHANGED=true
    PROTOCOL_CHANGED=true; CLIENT_PKG_CHANGED=true
    echo -e "${YELLOW}  --full mode: running all checks${NC}\n"
fi

# ── 1. Git Hygiene ──────────────────────────────────────────────────────────

echo -e "\n${BOLD}━━━ 1. Git Hygiene ━━━${NC}"
echo -e "${DIM}Uncommitted / untracked:${NC}"
git status --short
echo ""

# ── 2. Anti-Pattern Scans ───────────────────────────────────────────────────

echo -e "${BOLD}━━━ 2. Anti-Pattern Scans ━━━${NC}"

# Debug code in Rust
run_warn_check "Debug macros in Rust (dbg!, println! in apps/desktop/)" \
    "git diff ${DIFF_RANGE} -- '*.rs' | grep -n '^\+' | grep -E '(dbg!\(|println!\(|eprintln!\()' | grep -v '// keep' | grep -v '#\[cfg(test)\]' | head -20"

# Debug code in TypeScript
run_warn_check "Debug code in TS/TSX (console.log, debugger)" \
    "git diff ${DIFF_RANGE} -- '*.ts' '*.tsx' | grep -n '^\+' | grep -E '(console\.(log|debug|warn)\(|debugger;)' | head -20"

# Bare .unwrap() in non-test Rust code
run_warn_check "Bare .unwrap() in changed Rust files (non-test)" \
    "git diff ${DIFF_RANGE} -- '*.rs' | grep -n '^\+' | grep -v '#\[test\]' | grep -v 'mod tests' | grep -v '// safe:' | grep '\.unwrap()' | head -20"

# Secrets & credentials
run_warn_check "Potential secrets or credentials" \
    "git diff ${DIFF_RANGE} | grep -niE '(api[_-]?key|secret[_-]?key|password|token|credential)\\s*[:=]\\s*[\"'\\'']' | head -10"

# Performance anti-patterns: request_animation_frame in streaming paths
run_warn_check "request_animation_frame usage (verify non-streaming)" \
    "git diff ${DIFF_RANGE} -- '*.rs' | grep -n '^\+' | grep 'request_animation_frame' | head -10"

# Performance anti-patterns: window.refresh() (expensive, bypasses pane cache)
run_warn_check "window.refresh() usage (bypasses pane cache)" \
    "git diff ${DIFF_RANGE} -- '*.rs' | grep -n '^\+' | grep 'window\.refresh\|\.refresh()' | head -10"

# Leftover temp files
run_warn_check "Tracked temp/OS files (.DS_Store, .env, *.orig)" \
    "echo \"$CHANGED_FILES\" | grep -iE '(\.DS_Store|\.env$|\.orig$|\.bak$|\.swp$|~$)' | head -10"

# ── 3. Rust Checks ─────────────────────────────────────────────────────────

if $RUST_CHANGED; then
    echo -e "\n${BOLD}━━━ 3. Rust Checks ━━━${NC}"
    run_check "Rust formatting" \
        "cargo fmt --package padu --package padu-protocol --package padu-client --package padu-core --package padu-daemon -- --check"
    run_check "Cargo check (compilation)" \
        "cargo check"
    run_check "Cargo test" \
        "cargo test"
else
    echo -e "\n${BOLD}━━━ 3. Rust Checks ━━━${NC}"
    echo -e "${DIM}  Skipped (no Rust files changed)${NC}"
fi

# ── 4. Protocol Sync ───────────────────────────────────────────────────────

if $PROTOCOL_CHANGED || $RUST_CHANGED; then
    echo -e "\n${BOLD}━━━ 4. Protocol Sync ━━━${NC}"
    run_check "Wire protocol types sync" \
        "cargo run -p padu-protocol --bin export_types -- --check"
else
    echo -e "\n${BOLD}━━━ 4. Protocol Sync ━━━${NC}"
    echo -e "${DIM}  Skipped (no protocol files changed)${NC}"
fi

# ── 5. Client Package ──────────────────────────────────────────────────────

if $CLIENT_PKG_CHANGED || $PROTOCOL_CHANGED || $RUST_CHANGED; then
    echo -e "\n${BOLD}━━━ 5. Client Package (@padu/client) ━━━${NC}"
    run_check "@padu/client typecheck" \
        "bun run --filter @padu/client check"
    run_check "@padu/client tests" \
        "bun run --filter @padu/client test"
else
    echo -e "\n${BOLD}━━━ 5. Client Package (@padu/client) ━━━${NC}"
    echo -e "${DIM}  Skipped (no client package files changed)${NC}"
fi

# ── 6. Web Client ──────────────────────────────────────────────────────────

if $WEB_CHANGED || $CLIENT_PKG_CHANGED || $PROTOCOL_CHANGED; then
    echo -e "\n${BOLD}━━━ 6. Web Client (@padu/web) ━━━${NC}"
    run_check "Web typecheck" \
        "bun run --filter @padu/web typecheck"
    run_check "Web tests" \
        "bun run --filter @padu/web test"
else
    echo -e "\n${BOLD}━━━ 6. Web Client (@padu/web) ━━━${NC}"
    echo -e "${DIM}  Skipped (no web files changed)${NC}"
fi

# ── 7. Mobile Client ───────────────────────────────────────────────────────

if $MOBILE_CHANGED; then
    echo -e "\n${BOLD}━━━ 7. Mobile Client (@padu/mobile) ━━━${NC}"
    run_check "Mobile typecheck" \
        "bun run --filter @padu/mobile typecheck"
    if bun run --filter @padu/mobile test --help > /dev/null 2>&1; then
        run_check "Mobile tests" \
            "bun run --filter @padu/mobile test"
    fi
else
    echo -e "\n${BOLD}━━━ 7. Mobile Client (@padu/mobile) ━━━${NC}"
    echo -e "${DIM}  Skipped (no mobile files changed)${NC}"
fi

# ── 8. Parity Check ────────────────────────────────────────────────────────

echo -e "\n${BOLD}━━━ 8. Parity Heuristic ━━━${NC}"

if $RUST_CHANGED && ! $WEB_CHANGED; then
    # Check if any of the changed Rust files have UI/feature implications
    UI_RUST=$(echo "$CHANGED_FILES" | grep -E '^apps/desktop/src/(app|ui|input|browser|terminal)' || true)
    if [ -n "$UI_RUST" ]; then
        echo -e "${YELLOW}  ⚠ Rust UI files changed but no web files updated — verify parity:${NC}"
        echo "$UI_RUST" | sed 's/^/    /'
        WARNINGS=$((WARNINGS + 1))
        CHECK_RESULTS+=("⚠|Parity: Rust UI changed, no web counterpart|—")
    else
        echo -e "${GREEN}  ✔ Non-UI Rust changes, parity N/A${NC}"
    fi
elif $WEB_CHANGED && ! $RUST_CHANGED; then
    echo -e "${YELLOW}  ⚠ Web files changed but no Rust files updated — verify parity${NC}"
    WARNINGS=$((WARNINGS + 1))
    CHECK_RESULTS+=("⚠|Parity: Web changed, no Rust counterpart|—")
else
    echo -e "${GREEN}  ✔ Both surfaces updated or non-UI change${NC}"
fi

# ── Summary ─────────────────────────────────────────────────────────────────

TOTAL_ELAPSED=$(( SECONDS - TOTAL_START ))

echo -e "\n${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║   Summary                                    ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"

printf "\n  %-4s %-42s %s\n" "St" "Check" "Time"
printf "  %-4s %-42s %s\n" "──" "──────────────────────────────────────────" "────"
for result in "${CHECK_RESULTS[@]}"; do
    IFS='|' read -r status name elapsed <<< "$result"
    case "$status" in
        "✔") color="$GREEN" ;;
        "✖") color="$RED" ;;
        "⚠") color="$YELLOW" ;;
        *)   color="$NC" ;;
    esac
    printf "  ${color}%-4s${NC} %-42s ${DIM}%s${NC}\n" "$status" "$name" "$elapsed"
done

echo ""
echo -e "  ${BOLD}Total time:${NC} ${TOTAL_ELAPSED}s"
echo -e "  ${BOLD}Errors:${NC}     ${ERRORS}"
echo -e "  ${BOLD}Warnings:${NC}   ${WARNINGS}"
echo ""

if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}${BOLD}  ✔ All checks passed. Ready for code inspection.${NC}\n"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}${BOLD}  ⚠ Passed with ${WARNINGS} warning(s). Review before opening PR.${NC}\n"
    exit 0
else
    echo -e "${RED}${BOLD}  ✖ ${ERRORS} check(s) failed. Fix before opening PR.${NC}\n"
    exit 1
fi
