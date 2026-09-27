import type { AgentSession } from '@padu/client'

/**
 * Live usage badges for a board card (P1-08, web half).
 *
 * Mirrors the desktop `task_live_badges` helper in
 * `apps/desktop/src/app/board.rs`: each card's `sessionId` joins the
 * in-memory `AgentSession` catalog, which the daemon context already
 * refreshes on `task-state` / `card-updated` events — themselves driven by
 * the daemon's sequenced stream at commit cadence (≤ ~8.3 Hz). Badges
 * therefore stay live with no UI polling, and a missing session simply
 * means "not known yet" (no badges).
 *
 * Tokens prefer `context_usage.tokens`, falling back to the Codex
 * `threadGoal` ledger (whichever is larger). Duration sums settled turn
 * spans plus the live running turn via `nowSec`, falling back to the goal's
 * `timeUsedSeconds` and finally the session wall clock. There is no live
 * USD cost on the wire (costs only exist in the scanned usage history), so
 * tokens are the cost proxy.
 */
export interface TaskLiveBadges {
  tokens?: number
  durationSecs?: number
}

const BUSY_STATUSES = new Set(['connecting', 'working', 'waiting'])

export function taskLiveBadges(
  session: AgentSession | undefined | null,
  nowSec: number,
): TaskLiveBadges {
  if (!session) return {}

  let tokens = session.context_usage?.tokens
  const goal = session.thread_goal
  if (goal) {
    const goalTokens = Math.max(0, goal.tokensUsed)
    tokens = tokens === undefined ? goalTokens : Math.max(tokens, goalTokens)
  }
  const badges: TaskLiveBadges = {}
  if (tokens !== undefined && tokens > 0) badges.tokens = tokens

  let durationSecs: number | undefined
  if (session.turns.length > 0) {
    let total = 0
    for (const turn of session.turns) {
      const end = turn.completed_at ?? nowSec
      total += Math.max(0, end - turn.started_at)
    }
    durationSecs = total
  } else if (goal && goal.timeUsedSeconds > 0) {
    durationSecs = goal.timeUsedSeconds
  } else {
    const end = BUSY_STATUSES.has(session.status)
      ? nowSec
      : (session.last_reply_at ?? session.updated_at)
    durationSecs = Math.max(0, end - session.created_at)
  }
  if (durationSecs !== undefined && durationSecs > 0) {
    badges.durationSecs = durationSecs
  }
  return badges
}

/** Compact token count mirroring `format_tokens` in `padu-protocol::usage`. */
export function formatTokens(tokens: number): string {
  if (tokens >= 999_500) return `${(tokens / 1_000_000).toFixed(1)}M`
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}k`
  return String(Math.floor(tokens))
}

export type TranslateFn = (key: string, params?: Record<string, string | number>) => string

/**
 * Compact elapsed label mirroring desktop `format_working_elapsed`:
 * "9s", "1m 5s", "1h 2m" (localized via the shared `duration.*` keys).
 */
export function formatDurationShort(totalSeconds: number, t: TranslateFn): string {
  const seconds = Math.max(0, Math.floor(totalSeconds))
  if (seconds < 60) return t('duration.seconds_short', { count: seconds })
  if (seconds < 3600) {
    const minutes = Math.floor(seconds / 60)
    const rest = seconds % 60
    if (rest === 0) return t('duration.minutes_short', { count: minutes })
    return t('duration.two_units', {
      first: t('duration.minutes_short', { count: minutes }),
      second: t('duration.seconds_short', { count: rest }),
    })
  }
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  if (minutes === 0) return t('duration.hours_short', { count: hours })
  return t('duration.two_units', {
    first: t('duration.hours_short', { count: hours }),
    second: t('duration.minutes_short', { count: minutes }),
  })
}
