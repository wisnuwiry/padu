import type {
  AgentSession,
  PaduClient,
  Project,
  SidebarDateGroup,
  SidebarGroupView,
  SidebarGrouping,
  SidebarOrdering,
} from '@padu/client'
import { projectDisplayName } from './project-presentation'
import { fetchSidebarGroups } from './daemon-api'

export type { SidebarDateGroup, SidebarGroupView, SidebarGrouping, SidebarOrdering }

/**
 * Calendar bucket for `updated` grouping. Alias of the daemon's
 * `SidebarDateGroup`: the daemon owns bucketing through
 * `getSidebarGroups`, clients only render these values.
 */
export type DateGroup = SidebarDateGroup

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
  | { kind: 'board'; key: 'board' }
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

export function sidebarRows(
  groups: SessionGroup[],
  collapsed: ReadonlySet<string>,
): SidebarListRow[] {
  const rows: SidebarListRow[] = [
    { kind: 'search', key: 'search' },
    { kind: 'board', key: 'board' },
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

export function readSidebarShowProvider(storage?: Pick<Storage, 'getItem'> | null): boolean {
  const store = storage ?? (typeof window !== 'undefined' ? window.localStorage : null)
  if (!store) return true
  const item = store.getItem('padu.sidebar_show_provider')
  return item === null ? true : item !== 'false'
}

/**
 * Client-local UTC offset in seconds for `getSidebarGroups`. The daemon may
 * run in another timezone, so the request carries the client's offset and
 * today for correct date buckets.
 */
export function localUtcOffsetSecs(at = new Date()): number {
  return -at.getTimezoneOffset() * 60
}

/**
 * Turn daemon-owned sidebar groups into renderable session groups. Sort,
 * bucketing, and pagination already happened on the daemon; this only
 * resolves display names and recency timestamps from the local task snapshot.
 */
export function daemonGroupsToSessionGroups(
  daemonGroups: SidebarGroupView[],
  sessionsById: ReadonlyMap<string, AgentSession>,
  projects: Project[],
  unknownProject = 'Unknown project',
  projectlessName = 'No project',
): SessionGroup[] {
  const projectMap = new Map(projects.map((p) => [p.id, p]))
  const projectNames = new Map(projects.map((project) => [
    project.id,
    projectDisplayName(project, projectlessName),
  ]))
  const groups: SessionGroup[] = []
  for (const view of daemonGroups) {
    const items: SessionItem[] = []
    for (const sessionId of view.sessionIds) {
      const session = sessionsById.get(sessionId)
      if (!session) continue
      items.push({
        session,
        projectName: projectNames.get(session.project_id) ?? unknownProject,
        timestamp: sessionTimestamp(session),
      })
    }
    if (view.kind === 'pinned') {
      groups.push({ id: view.id, kind: 'pinned', label: 'Pinned', sessions: items })
      continue
    }
    if (view.kind === 'updated') {
      const dateGroup = view.dateGroup ?? 'today'
      groups.push({
        id: view.id,
        kind: 'updated',
        dateGroup,
        label: GROUP_LABELS[dateGroup],
        sessions: items,
      })
      continue
    }
    if (view.kind === 'projectless') {
      groups.push({
        id: view.id,
        kind: 'projectless',
        label: projectlessName,
        sessions: items,
        hasMore: view.hasMore || undefined,
      })
      continue
    }
    const project = view.projectId ? projectMap.get(view.projectId) : undefined
    groups.push({
      id: view.id,
      kind: 'project',
      projectId: view.projectId,
      project,
      label: project ? (projectNames.get(project.id) ?? unknownProject) : unknownProject,
      sessions: items,
      hasMore: view.hasMore || undefined,
    })
  }
  return groups
}

/**
 * Flat visual session order from daemon groups for adjacent-session
 * stepping. Matches the rendered sidebar sequence: pinned first, then each
 * group in order.
 */
export function daemonGroupsToVisualSessions(
  daemonGroups: SidebarGroupView[],
  sessionsById: ReadonlyMap<string, AgentSession>,
): AgentSession[] {
  const ordered: AgentSession[] = []
  for (const view of daemonGroups) {
    for (const sessionId of view.sessionIds) {
      const session = sessionsById.get(sessionId)
      if (session) ordered.push(session)
    }
  }
  return ordered
}

export interface SidebarViewQuery {
  grouping: SidebarGrouping
  ordering: SidebarOrdering
  unknownProject: string
  projectlessName: string
  revealedOlderCounts?: Record<string, number>
  now?: Date
}

/**
 * Fetch the daemon-owned sidebar view and map it to renderable groups in one
 * step. Used by adjacent-session stepping, which needs the same visual order
 * the sidebar renders.
 */
export async function fetchSidebarSessionGroups(
  client: PaduClient,
  projects: Project[],
  sessions: AgentSession[],
  query: SidebarViewQuery,
): Promise<SessionGroup[]> {
  const now = query.now ?? new Date()
  const daemonGroups = await fetchSidebarGroups(client, {
    grouping: query.grouping,
    ordering: query.ordering,
    today: now,
    nowSecs: Math.floor(now.getTime() / 1_000),
    localUtcOffsetSecs: localUtcOffsetSecs(now),
    revealedOlder: query.revealedOlderCounts ?? {},
  })
  return daemonGroupsToSessionGroups(
    daemonGroups,
    new Map(sessions.map((session) => [session.id, session])),
    projects,
    query.unknownProject,
    query.projectlessName,
  )
}

/**
 * Fetch the daemon-owned flat visual session order for adjacent-session
 * stepping (↑/↓ must follow the rendered sidebar sequence).
 */
export async function fetchSidebarVisualSessions(
  client: PaduClient,
  sessions: AgentSession[],
  query: Omit<SidebarViewQuery, 'unknownProject' | 'projectlessName'>,
): Promise<AgentSession[]> {
  const now = query.now ?? new Date()
  const daemonGroups = await fetchSidebarGroups(client, {
    grouping: query.grouping,
    ordering: query.ordering,
    today: now,
    nowSecs: Math.floor(now.getTime() / 1_000),
    localUtcOffsetSecs: localUtcOffsetSecs(now),
    revealedOlder: query.revealedOlderCounts ?? {},
  })
  return daemonGroupsToVisualSessions(
    daemonGroups,
    new Map(sessions.map((session) => [session.id, session])),
  )
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

/**
 * Display recency for a session row. The daemon owns ordering/grouping off
 * this same timestamp; rows only read it for display.
 */
function sessionTimestamp(session: AgentSession): number {
  return session.last_reply_at ?? session.created_at
}

function secondsUntilLocalMidnight(nowSeconds: number): number {
  const now = new Date(nowSeconds * 1_000)
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1)
  return Math.max(1, Math.ceil((tomorrow.getTime() - now.getTime()) / 1_000))
}
