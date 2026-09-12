import type { AgentSession, Project } from '@padu/client'
import { isProjectlessProject, projectDisplayName } from './project-presentation'

export type SidebarGrouping = 'project' | 'updated'
export type SidebarOrdering = 'newest' | 'oldest'

export type DateGroup = 'today' | 'yesterday' | 'week' | 'month' | 'year' | 'more'

export interface SessionItem {
  session: AgentSession
  projectName: string
  timestamp: number
}

export interface SessionGroup {
  id: string
  kind: 'pinned' | 'updated' | 'project' | 'projectless'
  dateGroup?: DateGroup
  projectId?: string
  project?: Project
  label: string
  sessions: SessionItem[]
  hasMore?: boolean
}

export type SidebarListRow =
  | { kind: 'search'; key: 'search' }
  | { kind: 'notes'; key: 'notes' }
  | { kind: 'group'; key: string; group: SessionGroup; collapsed: boolean; first: boolean }
  | { kind: 'session'; key: string; item: SessionItem }
  | { kind: 'showMore'; key: string; groupId: string }
  | { kind: 'spacer'; key: string }
  | { kind: 'separator'; key: string }

export const GROUP_LABELS: Record<DateGroup, string> = {
  today: 'Today',
  yesterday: 'Yesterday',
  week: 'This Week',
  month: 'This Month',
  year: 'This Year',
  more: 'More',
}

export const GROUP_ORDER_NEWEST: DateGroup[] = ['today', 'yesterday', 'week', 'month', 'year', 'more']
export const GROUP_ORDER_OLDEST: DateGroup[] = ['more', 'year', 'month', 'week', 'yesterday', 'today']

export const SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS = 7 * 24 * 60 * 60 // 604_800 seconds (7 days)

export function sidebarRows(
  groups: SessionGroup[],
  collapsed: ReadonlySet<string>,
): SidebarListRow[] {
  const rows: SidebarListRow[] = [
    { kind: 'search', key: 'search' },
    { kind: 'notes', key: 'notes' },
    { kind: 'separator', key: 'actions-separator' },
    { kind: 'spacer', key: 'actions-spacer' },
  ]
  const visibleGroups = groups.length
    ? groups
    // Desktop keeps the first header so Add Project never disappears merely
    // because there is no task history yet.
    : [{
        id: 'updated:today',
        kind: 'updated' as const,
        dateGroup: 'today' as const,
        label: GROUP_LABELS.today,
        sessions: [],
      }]
  visibleGroups.forEach((group, index) => {
    const isCollapsed = collapsed.has(group.id)
    rows.push({
      kind: 'group',
      key: `group:${group.id}`,
      group,
      collapsed: isCollapsed,
      first: group.kind !== 'pinned' && visibleGroups.slice(0, index).every((candidate) => candidate.kind === 'pinned'),
    })
    if (!isCollapsed) {
      rows.push(...group.sessions.map((item) => ({
        kind: 'session' as const,
        key: `session:${item.session.id}`,
        item,
      })))
      if (group.hasMore) {
        rows.push({
          kind: 'showMore',
          key: `showMore:${group.id}`,
          groupId: group.id,
        })
      }
    }
    if (groups.length) {
      if (group.kind === 'pinned') {
        rows.push({ kind: 'separator', key: 'separator:pinned' })
      } else {
        rows.push({ kind: 'spacer', key: `spacer:${group.id}` })
      }
    }
  })
  return rows
}

export function sortSidebarSessions(
  sessions: AgentSession[],
  ordering: SidebarOrdering = 'newest',
): AgentSession[] {
  return [...sessions].sort((left, right) => {
    const pinnedOrder = Number(Boolean(right.pinned_at)) - Number(Boolean(left.pinned_at))
    if (pinnedOrder) return pinnedOrder
    const leftTime = sessionTimestamp(left)
    const rightTime = sessionTimestamp(right)
    return ordering === 'oldest' ? leftTime - rightTime : rightTime - leftTime
  })
}

export function readSidebarGrouping(): SidebarGrouping {
  if (typeof window === 'undefined') return 'project'
  return window.localStorage.getItem('padu:sidebar_grouping') === 'updated'
    ? 'updated'
    : 'project'
}

export function readSidebarOrdering(): SidebarOrdering {
  if (typeof window === 'undefined') return 'newest'
  return window.localStorage.getItem('padu:sidebar_ordering') === 'oldest'
    ? 'oldest'
    : 'newest'
}

/**
 * Sessions in the order the sidebar renders them: grouped (pinned, then
 * per-project or per-date groups) with the user's grouping/ordering applied.
 * Adjacent-session stepping must follow this sequence — a flat newest-first
 * list diverges from it whenever projects interleave by recency (or the user
 * picks oldest-first), making ↑/↓ land on visually wrong rows.
 */
export function sidebarVisualSessions(
  projects: Project[],
  sessions: AgentSession[],
  grouping: SidebarGrouping,
  ordering: SidebarOrdering,
  unknownProject: string,
  projectlessName: string,
): AgentSession[] {
  return groupSessions(projects, sessions, new Date(), unknownProject, projectlessName, grouping, ordering)
    .flatMap((group) => group.sessions.map((item) => item.session))
}

export function groupSessions(
  projects: Project[],
  sessions: AgentSession[],
  now = new Date(),
  unknownProject = 'Unknown project',
  projectlessName = 'No project',
  grouping: SidebarGrouping = 'project',
  ordering: SidebarOrdering = 'newest',
  revealedOlderCounts: Record<string, number> = {},
): SessionGroup[] {
  const projectMap = new Map(projects.map((p) => [p.id, p]))
  const projectNames = new Map(projects.map((project) => [
    project.id,
    projectDisplayName(project, projectlessName),
  ]))
  const started = sortSidebarSessions(
    sessions.filter((session) => !session.archived_at && sessionHasStarted(session)),
    ordering,
  )

  const pinned = started
    .filter((session) => session.pinned_at)
    .map((session) => ({
        session,
        projectName: projectNames.get(session.project_id) ?? unknownProject,
        timestamp: sessionTimestamp(session),
      }))

  if (grouping === 'updated') {
    const grouped = new Map<DateGroup, SessionItem[]>()
    for (const session of started.filter((session) => !session.pinned_at)) {
      const id = dateGroup(sessionTimestamp(session), now)
      const items = grouped.get(id) ?? []
      items.push({
        session,
        projectName: projectNames.get(session.project_id) ?? unknownProject,
        timestamp: sessionTimestamp(session),
      })
      grouped.set(id, items)
    }
    const order = ordering === 'oldest' ? GROUP_ORDER_OLDEST : GROUP_ORDER_NEWEST
    return [
      ...(pinned.length > 0
        ? [{ id: 'pinned', kind: 'pinned' as const, label: 'Pinned', sessions: pinned }]
        : []),
      ...order
      .filter((id) => grouped.has(id))
      .map((id) => ({
        id: `updated:${id}`,
        kind: 'updated' as const,
        dateGroup: id,
        label: GROUP_LABELS[id],
        sessions: grouped.get(id)!,
      })),
    ]
  }

  // grouping === 'project'
  const nowSeconds = Math.floor(now.getTime() / 1000)
  const recentCutoff = nowSeconds - SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS

  const projectGroups: SessionGroup[] = []
  const projectIndexMap = new Map<string, number>()
  const projectlessSessions: SessionItem[] = []

  for (const session of started.filter((session) => !session.pinned_at)) {
    const project = projectMap.get(session.project_id)
    const isProjectless = !project || isProjectlessProject(project)
    const item: SessionItem = {
      session,
      projectName: projectNames.get(session.project_id) ?? unknownProject,
      timestamp: sessionTimestamp(session),
    }

    if (isProjectless) {
      projectlessSessions.push(item)
      continue
    }

    let groupIndex = projectIndexMap.get(session.project_id)
    if (groupIndex === undefined) {
      groupIndex = projectGroups.length
      projectIndexMap.set(session.project_id, groupIndex)
      projectGroups.push({
        id: `project:${session.project_id}`,
        kind: 'project',
        projectId: session.project_id,
        project,
        label: projectNames.get(session.project_id) ?? unknownProject,
        sessions: [],
      })
    }
    projectGroups[groupIndex]!.sessions.push(item)
  }

  if (projectlessSessions.length > 0) {
    projectGroups.push({
      id: 'projectless',
      kind: 'projectless',
      label: projectlessName,
      sessions: projectlessSessions,
    })
  }

  // Apply recent cutoff and pagination for each project group
  const visibleProjectGroups = projectGroups.map((group) => {
    const allSessions = group.sessions
    const revealedOlder = revealedOlderCounts[group.id] ?? 0

    const visible: SessionItem[] = []
    let olderSeen = 0

    for (const item of allSessions) {
      const recent = item.session.pinned_at != null || item.timestamp >= recentCutoff
      if (recent || olderSeen < revealedOlder) {
        visible.push(item)
      }
      if (!recent) {
        olderSeen++
      }
    }

    const hasMore = olderSeen > revealedOlder

    return {
      ...group,
      sessions: visible,
      hasMore,
    }
  })
  return [
    ...(pinned.length > 0
      ? [{ id: 'pinned', kind: 'pinned' as const, label: 'Pinned', sessions: pinned }]
      : []),
    ...visibleProjectGroups,
  ]
}

export function sessionHasStarted(session: AgentSession): boolean {
  return Boolean(
    session.turns.length
      || session.messages.length
      || session.provider_cursor
      || session.last_reply_at,
  )
}

export function sessionTimeLabel(
  session: AgentSession,
  nowSeconds = Math.floor(Date.now() / 1_000),
  t?: Translator,
): string | null {
  const turn = session.turns.at(-1)
  if (
    (session.status === 'connecting' || session.status === 'working' || session.status === 'waiting')
      && turn?.status === 'running'
  ) {
    const elapsed = Math.max(0, nowSeconds - turn.started_at)
    return t
      ? t('sidebar.working', { elapsed: formatWorkingElapsedLocalized(elapsed, t) })
      : `Working for ${formatWorkingElapsed(elapsed)}`
  }
  if (session.last_reply_at == null) return null
  const elapsed = Math.max(0, nowSeconds - session.last_reply_at)
  return t ? formatTimeAgoLocalized(elapsed, t) : formatTimeAgo(elapsed)
}

export function nextSidebarUpdateDelay(
  sessions: AgentSession[],
  nowSeconds = Math.floor(Date.now() / 1_000),
): number {
  let next = secondsUntilLocalMidnight(nowSeconds)
  for (const session of sessions) {
    const turn = session.turns.at(-1)
    if (
      (session.status === 'connecting' || session.status === 'working' || session.status === 'waiting')
        && turn?.status === 'running'
    ) {
      return 1
    }
    if (session.last_reply_at == null) continue
    const elapsed = Math.max(0, nowSeconds - session.last_reply_at)
    const step = elapsed < 3_600 ? 60 : elapsed < 86_400 ? 3_600 : 86_400
    const remaining = Math.max(1, step - elapsed % step)
    next = Math.min(next, remaining)
  }
  return next
}

export function dateGroup(timestamp: number, now = new Date()): DateGroup {
  const date = new Date(timestamp * 1_000)
  const today = localDateStart(now)
  const sessionDay = localDateStart(date)
  if (sessionDay >= today) return 'today'

  const yesterday = new Date(today)
  yesterday.setDate(yesterday.getDate() - 1)
  if (sessionDay.getTime() === yesterday.getTime()) return 'yesterday'

  const weekStart = new Date(today)
  weekStart.setDate(weekStart.getDate() - ((weekStart.getDay() + 6) % 7))
  if (sessionDay >= weekStart) return 'week'

  if (
    sessionDay.getFullYear() === today.getFullYear()
      && sessionDay.getMonth() === today.getMonth()
  ) return 'month'
  if (sessionDay.getFullYear() === today.getFullYear()) return 'year'
  return 'more'
}

export function formatTimeAgo(seconds: number): string {
  if (seconds < 60) return 'just now'
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m`
  if (seconds < 86_400) return `${Math.floor(seconds / 3_600)}h`
  return `${Math.floor(seconds / 86_400)}d`
}

export function formatWorkingElapsed(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3_600) {
    const minutes = Math.floor(seconds / 60)
    const remainder = seconds % 60
    return remainder ? `${minutes}m ${remainder}s` : `${minutes}m`
  }
  const hours = Math.floor(seconds / 3_600)
  const minutes = Math.floor((seconds % 3_600) / 60)
  return minutes ? `${hours}h ${minutes}m` : `${hours}h`
}

function formatTimeAgoLocalized(seconds: number, t: Translator): string {
  if (seconds < 60) return t('sidebar.just_now')
  if (seconds < 3_600) return t('sidebar.minutes_ago', { count: Math.floor(seconds / 60) })
  if (seconds < 86_400) return t('sidebar.hours_ago', { count: Math.floor(seconds / 3_600) })
  return t('sidebar.days_ago', { count: Math.floor(seconds / 86_400) })
}

function formatWorkingElapsedLocalized(seconds: number, t: Translator): string {
  if (seconds < 60) return t('duration.seconds_short', { count: seconds })
  if (seconds < 3_600) {
    const minutes = Math.floor(seconds / 60)
    const remainder = seconds % 60
    const first = t('duration.minutes_short', { count: minutes })
    return remainder
      ? t('duration.two_units', { first, second: t('duration.seconds_short', { count: remainder }) })
      : first
  }
  const hours = Math.floor(seconds / 3_600)
  const minutes = Math.floor((seconds % 3_600) / 60)
  const first = t('duration.hours_short', { count: hours })
  return minutes
    ? t('duration.two_units', { first, second: t('duration.minutes_short', { count: minutes }) })
    : first
}

type Translator = (key: string, params?: Record<string, string | number>) => string

function sessionTimestamp(session: AgentSession): number {
  return session.last_reply_at ?? session.created_at
}

function localDateStart(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

function secondsUntilLocalMidnight(nowSeconds: number): number {
  const now = new Date(nowSeconds * 1_000)
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1)
  return Math.max(1, Math.ceil((tomorrow.getTime() - now.getTime()) / 1_000))
}
