import { describe, expect, test } from 'bun:test'
import type { AgentSession, TaskSummary } from '@padu/client'
import {
  formatDurationShort,
  formatTokens,
  taskLiveBadges,
  type TranslateFn,
} from './task-live-badges'

function testSession(overrides: Partial<AgentSession> = {}): AgentSession {
  return {
    id: 'session',
    title: 'Test session',
    project_id: 'project',
    provider: 'codex',
    runtime_mode: 'fullAccess',
    interaction_mode: 'build',
    status: 'working',
    created_at: 1_000,
    updated_at: 2_000,
    provider_cursor: null,
    messages: [],
    transcript_blocks: [],
    turns: [],
    ...overrides,
  }
}

const en: TranslateFn = (key, params = {}) => {
  const count = params.count ?? ''
  switch (key) {
    case 'duration.seconds_short': return `${count}s`
    case 'duration.minutes_short': return `${count}m`
    case 'duration.hours_short': return `${count}h`
    case 'duration.two_units': return `${params.first} ${params.second}`
    default: return key
  }
}

describe('taskLiveBadges', () => {
  test('absent without a session', () => {
    expect(taskLiveBadges(undefined, 5_000)).toEqual({})
    expect(taskLiveBadges(null, 5_000)).toEqual({})
  })

  test('hides zero usage but ticks the busy wall clock', () => {
    expect(taskLiveBadges(testSession(), 5_000)).toEqual({ durationSecs: 4_000 })
  })

  test('reads context usage and sums turns with the live turn', () => {
    const session = testSession({
      context_usage: { tokens: 12_500, window: 200_000 },
      turns: [
        {
          id: 't1', turn_count: 1, status: 'completed', provider_turn_started: false,
          started_at: 1_000, completed_at: 1_060, checkpoint: null,
        },
        {
          id: 't2', turn_count: 2, status: 'running', provider_turn_started: false,
          started_at: 2_000, completed_at: null, checkpoint: null,
        },
      ],
    })
    expect(taskLiveBadges(session, 2_030)).toEqual({ tokens: 12_500, durationSecs: 90 })
  })

  test('prefers the larger goal ledger and falls back to the idle wall clock', () => {
    const session = testSession({
      status: 'idle',
      context_usage: { tokens: 100 },
      thread_goal: {
        objective: 'goal', status: 'active', tokenBudget: 50_000,
        tokensUsed: 4_000, timeUsedSeconds: 0,
      },
    })
    expect(taskLiveBadges(session, 9_000)).toEqual({ tokens: 4_000, durationSecs: 1_000 })
  })

  test('uses goal time when there are no turns', () => {
    const session = testSession({
      thread_goal: {
        objective: 'goal', status: 'active',
        tokensUsed: 0, timeUsedSeconds: 165,
      },
    })
    expect(taskLiveBadges(session, 9_000)).toEqual({ durationSecs: 165 })
  })
})

describe('formatTokens', () => {
  test('mirrors the protocol compact formatting', () => {
    expect(formatTokens(0)).toBe('0')
    expect(formatTokens(999)).toBe('999')
    expect(formatTokens(1_000)).toBe('1.0k')
    expect(formatTokens(12_500)).toBe('12.5k')
    expect(formatTokens(999_500)).toBe('1.0M')
  })
})

describe('formatDurationShort', () => {
  test('renders compact elapsed labels', () => {
    expect(formatDurationShort(9, en)).toBe('9s')
    expect(formatDurationShort(60, en)).toBe('1m')
    expect(formatDurationShort(65, en)).toBe('1m 5s')
    expect(formatDurationShort(3_600, en)).toBe('1h')
    expect(formatDurationShort(3_720, en)).toBe('1h 2m')
  })
})

describe('board interaction at 500 cards', () => {
  test('session join plus per-card lookup stays interactive', () => {
    const nowSec = 1_000_000
    const sessions: AgentSession[] = []
    const summaries: TaskSummary[] = []
    for (let index = 0; index < 500; index += 1) {
      const sessionId = `session-${index}`
      sessions.push(
        testSession({
          id: sessionId,
          context_usage: { tokens: 1_000 + index },
          turns: [
            {
              id: `turn-${index}`, turn_count: 1, status: 'completed',
              provider_turn_started: false, started_at: nowSec - 300,
              completed_at: nowSec - 240, checkpoint: null,
            },
          ],
        }),
      )
      summaries.push({
        id: `task-${index}`,
        projectId: 'project',
        title: `task ${index}`,
        descriptionPreview: '',
        status: 'running',
        sessionId,
        labels: [],
        needsAttention: false,
        updatedAt: 2_000,
        version: 1,
        archived: false,
      })
    }

    const started = performance.now()
    // One pass per render, then O(1) lookups per visible row — the same
    // shape as the board page memo.
    const liveBySession = new Map(sessions.map((s) => [s.id, taskLiveBadges(s, nowSec)]))
    let rendered = 0
    for (const summary of summaries) {
      const badges = summary.sessionId ? liveBySession.get(summary.sessionId) : undefined
      if (badges?.tokens !== undefined) rendered += 1
    }
    const elapsedMs = performance.now() - started

    expect(rendered).toBe(500)
    expect(elapsedMs).toBeLessThan(500)
  })
})
