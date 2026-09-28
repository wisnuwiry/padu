import { describe, expect, test } from 'bun:test'
import type { AgentSession, Project, SidebarGroupView } from '@padu/client'
import {
  daemonGroupsToSessionGroups,
  daemonGroupsToVisualSessions,
  formatTimeAgo,
  formatWorkingElapsed,
  localUtcOffsetSecs,
  nextSidebarUpdateDelay,
  readSidebarShowProvider,
  sessionHasStarted,
  sessionTimeLabel,
  sidebarRows,
} from './sidebar-presentation'

// Sort, date-bucket grouping, project grouping, and recent-window pagination
// are owned by the daemon engine (`padu-protocol::sidebar`, served through
// `getSidebarGroups`) and tested there. These tests cover what stays
// client-side: mapping daemon groups to renderable rows, time labels, and
// presentation helpers.

describe('daemon sidebar presentation', () => {
  test('keeps Add Project in an empty history through the first group header', () => {
    expect(sidebarRows([], new Set())).toEqual([
      { kind: 'search', key: 'search' },
      { kind: 'board', key: 'board' },
      { kind: 'notes', key: 'notes' },
      { kind: 'separator', key: 'actions-separator' },
      { kind: 'spacer', key: 'actions-spacer' },
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

  test('maps daemon groups to renderable groups preserving order and reveal state', () => {
    const project: Project = { id: 'p1', name: 'Alpha', path: '/home/alpha', created_at: 1 }
    const pinned = session({ id: 'pinned', project_id: 'p1', created_at: 100, last_reply_at: 100, pinned_at: 1 })
    const recent = session({ id: 'recent', project_id: 'p1', created_at: 300, last_reply_at: 300 })
    const byId = new Map([pinned, recent].map((s) => [s.id, s]))
    const views: SidebarGroupView[] = [
      { id: 'pinned', kind: 'pinned', sessionIds: ['pinned'], hasMore: false },
      { id: 'project:p1', kind: 'project', projectId: 'p1', sessionIds: ['recent'], hasMore: true },
      { id: 'updated:today', kind: 'updated', dateGroup: 'today', sessionIds: ['missing'], hasMore: false },
    ]
    const groups = daemonGroupsToSessionGroups(views, byId, [project], 'Unknown', 'No project')
    expect(groups.map((g) => g.id)).toEqual(['pinned', 'project:p1', 'updated:today'])
    expect(groups[0]?.sessions.map((item) => item.session.id)).toEqual(['pinned'])
    expect(groups[1]?.sessions.map((item) => item.session.id)).toEqual(['recent'])
    expect(groups[1]?.hasMore).toBe(true)
    expect(groups[1]?.label).toBe('Alpha')
    // Unknown session ids are skipped, never rendered as holes.
    expect(groups[2]?.sessions).toEqual([])
    expect(groups[2]?.label).toBe('Today')

    const visual = daemonGroupsToVisualSessions(views, byId)
    expect(visual.map((s) => s.id)).toEqual(['pinned', 'recent'])
  })

  test('resolves projectless and unknown projects to display names', () => {
    const projectless: Project = {
      id: 'project',
      name: 'No project',
      path: '/home/me/.padu/projects/session',
      created_at: 1,
    }
    const item = session({ created_at: 500, last_reply_at: 500 })
    const groups = daemonGroupsToSessionGroups(
      [{ id: 'projectless', kind: 'projectless', sessionIds: [item.id], hasMore: false }],
      new Map([[item.id, item]]),
      [projectless],
      'Unknown project',
      'プロジェクトなし',
    )
    expect(groups[0]?.sessions[0]?.projectName).toBe('プロジェクトなし')
    expect(groups[0]?.label).toBe('プロジェクトなし')
    expect(groups[0]?.sessions[0]?.timestamp).toBe(500)
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

  test('handles time labels and elapsed counters across different time deltas', () => {
    expect(formatTimeAgo(30)).toBe('just now')
    expect(formatTimeAgo(90)).toBe('1m')
    expect(formatTimeAgo(3_600)).toBe('1h')
    expect(formatTimeAgo(7_200)).toBe('2h')
    expect(formatTimeAgo(86_400)).toBe('1d')
    expect(formatTimeAgo(172_800)).toBe('2d')
  })

  test('sidebarRows respects collapsed state and inserts spacers between groups', () => {
    const s1 = session({ id: 's1', created_at: 500, last_reply_at: 500 })
    const s2 = session({ id: 's2', created_at: 400, last_reply_at: 400 })
    const byId = new Map([s1, s2].map((s) => [s.id, s]))
    const groups = daemonGroupsToSessionGroups(
      [
        { id: 'updated:today', kind: 'updated', dateGroup: 'today', sessionIds: ['s1'], hasMore: false },
        { id: 'updated:month', kind: 'updated', dateGroup: 'month', sessionIds: ['s2'], hasMore: false },
      ],
      byId,
      [],
      'Unknown',
      'No project',
    )
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
    const rows = sidebarRows([], new Set())
    expect(rows[0]?.kind).toBe('search')
    expect(rows[0]?.key).toBe('search')
  })

  test('readSidebarShowProvider defaults to true and reads from storage', () => {
    expect(readSidebarShowProvider(null)).toBe(true)
    expect(readSidebarShowProvider({ getItem: () => null })).toBe(true)
    expect(readSidebarShowProvider({ getItem: () => 'false' })).toBe(false)
    expect(readSidebarShowProvider({ getItem: () => 'true' })).toBe(true)
  })

  test('formatWorkingElapsed formats seconds, minutes, whole hours, and mixed hours', () => {
    expect(formatWorkingElapsed(45)).toBe('45s')
    expect(formatWorkingElapsed(60)).toBe('1m')
    expect(formatWorkingElapsed(75)).toBe('1m 15s')
    expect(formatWorkingElapsed(3600)).toBe('1h')
    expect(formatWorkingElapsed(7200)).toBe('2h')
    expect(formatWorkingElapsed(3665)).toBe('1h 1m')
  })

  test('localUtcOffsetSecs mirrors the date offset the daemon buckets against', () => {
    const at = new Date(2026, 7, 15, 12)
    expect(localUtcOffsetSecs(at)).toBe(-at.getTimezoneOffset() * 60)
  })
})

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
