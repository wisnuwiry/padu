import { describe, expect, test } from 'bun:test'
import type { AgentSession, Project } from '@padu/client'
import {
  dateGroup,
  formatTimeAgo,
  formatWorkingElapsed,
  groupSessions,
  nextSidebarUpdateDelay,
  sessionHasStarted,
  sessionTimeLabel,
  sidebarRows,
  sortSidebarSessions,
} from './sidebar-presentation'

describe('desktop sidebar presentation', () => {
  test('uses a Monday-based current week instead of a rolling seven days', () => {
    const wednesday = new Date(2026, 7, 12, 12)
    expect(dateGroup(atLocalNoon(2026, 7, 12), wednesday)).toBe('today')
    expect(dateGroup(atLocalNoon(2026, 7, 11), wednesday)).toBe('yesterday')
    expect(dateGroup(atLocalNoon(2026, 7, 10), wednesday)).toBe('week')
    expect(dateGroup(atLocalNoon(2026, 7, 9), wednesday)).toBe('month')
  })

  test('keeps Add Project in an empty history through the first group header', () => {
    expect(sidebarRows([], new Set())).toEqual([
      { kind: 'search', key: 'search' },
      {
        kind: 'group',
        key: 'group:updated:today',
        group: {
          id: 'updated:today',
          kind: 'updated',
          dateGroup: 'today',
          label: 'Today',
          sessions: [],
        },
        collapsed: false,
        first: true,
      },
    ])
  })

  test('matches desktop settled and live time labels', () => {
    expect(formatTimeAgo(0)).toBe('just now')
    expect(formatTimeAgo(604_800)).toBe('7d')
    expect(formatWorkingElapsed(65)).toBe('1m 5s')
    expect(formatWorkingElapsed(3_720)).toBe('1h 2m')

    const live = session({
      status: 'working',
      turns: [{
        id: 'turn',
        turn_count: 1,
        status: 'running',
        provider_turn_started: true,
        started_at: 100,
        completed_at: null,
        checkpoint: null,
      }],
    })
    expect(sessionTimeLabel(live, 165)).toBe('Working for 1m 5s')
    expect(nextSidebarUpdateDelay([live], 165)).toBe(1)
  })

  test('does not invent a reply time and keeps cursor-only resumed tasks', () => {
    const resumed = session({ provider_cursor: { provider: 'codex', value: {} } as never })
    expect(sessionHasStarted(resumed)).toBe(true)
    expect(sessionTimeLabel(resumed, 1_000)).toBeNull()
  })

  test('presents the projectless sentinel with the localized desktop name', () => {
    const project: Project = {
      id: 'project',
      name: 'No project',
      path: '/home/me/.padu/projects/session',
      created_at: 1,
    }
    const now = new Date(2026, 7, 15, 12)
    const nowSeconds = Math.floor(now.getTime() / 1000)
    const groups = groupSessions(
      [project],
      [session({ created_at: nowSeconds, messages: [{ id: 'message' } as never] })],
      now,
      'Unknown project',
      'プロジェクトなし',
    )
    expect(groups[0]?.sessions[0]?.projectName).toBe('プロジェクトなし')
  })

  test('sorts sessions by newest or oldest timestamp', () => {
    const s1 = session({ id: 's1', created_at: 100, last_reply_at: 100, messages: [{ id: 'm1' } as never] })
    const s2 = session({ id: 's2', created_at: 200, last_reply_at: 200, messages: [{ id: 'm2' } as never] })
    const s3 = session({ id: 's3', created_at: 300, last_reply_at: 300, messages: [{ id: 'm3' } as never] })

    const newest = sortSidebarSessions([s2, s1, s3], 'newest')
    expect(newest.map((s) => s.id)).toEqual(['s3', 's2', 's1'])

    const oldest = sortSidebarSessions([s2, s1, s3], 'oldest')
    expect(oldest.map((s) => s.id)).toEqual(['s1', 's2', 's3'])
  })

  test('puts pinned sessions first and omits archived sessions', () => {
    const pinned = session({ id: 'pinned', created_at: 100, last_reply_at: 100, pinned_at: 1, messages: [{ id: 'm' } as never] })
    const normal = session({ id: 'normal', created_at: 300, last_reply_at: 300, messages: [{ id: 'm' } as never] })
    const archived = session({ id: 'archived', created_at: 400, last_reply_at: 400, archived_at: 1, messages: [{ id: 'm' } as never] })

    expect(sortSidebarSessions([normal, pinned], 'newest').map((item) => item.id)).toEqual(['pinned', 'normal'])
    const groups = groupSessions([], [normal, pinned, archived], new Date(2026, 7, 15, 12), 'Unknown', 'No project', 'updated')
    expect(groups.flatMap((group) => group.sessions.map((item) => item.session.id))).toEqual(['pinned', 'normal'])
  })

  test('groups sessions by project and applies 7-day recent cutoff with pagination', () => {
    const now = new Date(2026, 7, 15, 12)
    const nowSeconds = Math.floor(now.getTime() / 1000)
    const eightDaysAgo = nowSeconds - 8 * 86_400
    const oneDayAgo = nowSeconds - 1 * 86_400

    const p1: Project = { id: 'p1', name: 'Project Alpha', path: '/home/alpha', created_at: 1 }
    const p2: Project = { id: 'p2', name: 'Project Beta', path: '/home/beta', created_at: 1 }

    const sRecentAlpha = session({ id: 's1', project_id: 'p1', created_at: oneDayAgo, last_reply_at: oneDayAgo, messages: [{ id: 'm1' } as never] })
    const sOlderAlpha = session({ id: 's2', project_id: 'p1', created_at: eightDaysAgo, last_reply_at: eightDaysAgo, messages: [{ id: 'm2' } as never] })
    const sRecentBeta = session({ id: 's3', project_id: 'p2', created_at: oneDayAgo, last_reply_at: oneDayAgo, messages: [{ id: 'm3' } as never] })

    // Without revealing older sessions: Alpha has 1 visible and hasMore: true
    const groups = groupSessions([p1, p2], [sRecentAlpha, sOlderAlpha, sRecentBeta], now, 'Unknown', 'No project', 'project', 'newest', {})
    expect(groups.length).toBe(2)

    const alphaGroup = groups.find((g) => g.projectId === 'p1')!
    expect(alphaGroup.sessions.length).toBe(1)
    expect(alphaGroup.sessions[0]?.session.id).toBe('s1')
    expect(alphaGroup.hasMore).toBe(true)

    // With reveal count >= 1 for Alpha: both sessions visible and hasMore: false
    const groupsRevealed = groupSessions([p1, p2], [sRecentAlpha, sOlderAlpha, sRecentBeta], now, 'Unknown', 'No project', 'project', 'newest', { 'project:p1': 5 })
    const alphaRevealed = groupsRevealed.find((g) => g.projectId === 'p1')!
    expect(alphaRevealed.sessions.length).toBe(2)
    expect(alphaRevealed.hasMore).toBe(false)
  })

  test('reverses date group order in updated grouping when oldest ordering is selected', () => {
    const now = new Date(2026, 7, 15, 12)
    const nowSeconds = Math.floor(now.getTime() / 1000)
    const today = session({ id: 'today', created_at: nowSeconds, last_reply_at: nowSeconds, messages: [{ id: 'm' } as never] })
    const month = session({ id: 'month', created_at: nowSeconds - 10 * 86_400, last_reply_at: nowSeconds - 10 * 86_400, messages: [{ id: 'm' } as never] })

    const groupsNewest = groupSessions([], [today, month], now, 'Unknown', 'No project', 'updated', 'newest')
    expect(groupsNewest.map((g) => g.dateGroup)).toEqual(['today', 'month'])

    const groupsOldest = groupSessions([], [today, month], now, 'Unknown', 'No project', 'updated', 'oldest')
    expect(groupsOldest.map((g) => g.dateGroup)).toEqual(['month', 'today'])
  })

  test('handles time labels and elapsed counters across different time deltas', () => {
    expect(formatTimeAgo(30)).toBe('just now')
    expect(formatTimeAgo(90)).toBe('1m')
    expect(formatTimeAgo(3_600)).toBe('1h')
    expect(formatTimeAgo(7_200)).toBe('2h')
    expect(formatTimeAgo(86_400)).toBe('1d')
    expect(formatTimeAgo(172_800)).toBe('2d')
  })

  test('sidebarRows respects collapsed state and inserts spacers between groups', () => {
    const now = new Date(2026, 7, 15, 12)
    const nowSeconds = Math.floor(now.getTime() / 1000)
    const s1 = session({ id: 's1', created_at: nowSeconds, last_reply_at: nowSeconds, messages: [{ id: 'm1' } as never] })
    const s2 = session({ id: 's2', created_at: nowSeconds - 10 * 86_400, last_reply_at: nowSeconds - 10 * 86_400, messages: [{ id: 'm2' } as never] })

    const groups = groupSessions([], [s1, s2], now, 'Unknown', 'No project', 'updated', 'newest')
    expect(groups.length).toBe(2)

    const uncollapsedRows = sidebarRows(groups, new Set())
    expect(uncollapsedRows.some((r) => r.kind === 'session' && r.item.session.id === 's1')).toBe(true)
    expect(uncollapsedRows.some((r) => r.kind === 'session' && r.item.session.id === 's2')).toBe(true)

    // When group 1 is collapsed, session 1 is hidden
    const collapsedRows = sidebarRows(groups, new Set([groups[0]!.id]))
    expect(collapsedRows.some((r) => r.kind === 'session' && r.item.session.id === 's1')).toBe(false)
    expect(collapsedRows.some((r) => r.kind === 'session' && r.item.session.id === 's2')).toBe(true)
  })

  test('sessionHasStarted accurately detects active, replied, or persisted sessions', () => {
    const blank = session({ messages: [], turns: [], provider_cursor: null })
    expect(sessionHasStarted(blank)).toBe(false)

    const withMessage = session({ messages: [{ id: 'm1' } as never] })
    expect(sessionHasStarted(withMessage)).toBe(true)

    const withTurn = session({ turns: [{ id: 't1' } as never] })
    expect(sessionHasStarted(withTurn)).toBe(true)
  })

  test('nextSidebarUpdateDelay returns 1s for working session and midnight delta for idle sessions', () => {
    const idle = session({ status: 'idle', turns: [], last_reply_at: null })
    expect(nextSidebarUpdateDelay([idle], 100)).toBeGreaterThan(0)

    const working = session({
      status: 'working',
      turns: [{
        id: 'turn',
        turn_count: 1,
        status: 'running',
        provider_turn_started: true,
        started_at: 100,
        completed_at: null,
        checkpoint: null,
      }],
    })
    expect(nextSidebarUpdateDelay([working], 150)).toBe(1)
  })

  test('group headers and session items support keyboard interaction invariants', () => {
    const groups = groupSessions([], [], new Date(), 'Unknown', 'No project', 'project', 'newest')
    const rows = sidebarRows(groups, new Set())
    expect(rows[0]?.kind).toBe('search')
    expect(rows[0]?.key).toBe('search')
  })
})

function atLocalNoon(year: number, month: number, day: number): number {
  return Math.floor(new Date(year, month, day, 12).getTime() / 1_000)
}

function session(patch: Partial<AgentSession>): AgentSession {
  return {
    id: 'session',
    title: 'New Task',
    project_id: 'project',
    provider: 'codex',
    model: null,
    runtime_mode: 'ask',
    interaction_mode: 'build',
    status: 'idle',
    created_at: 10,
    updated_at: 10,
    provider_cursor: null,
    messages: [],
    transcript_blocks: [],
    turns: [],
    ...patch,
  }
}
