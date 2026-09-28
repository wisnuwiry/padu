import type {
  AgentSession,
  AgentTurn,
  Checkpoint,
  Message,
  Project,
  ProviderKind,
  RuntimeMode,
  SidebarGroupView,
  TranscriptBlock,
} from '@padu/client';
import { turnAnswerStart, turnFoldLabel } from '@padu/client/transcript-presentation';

export type SessionGroupId = 'pinned' | 'today' | 'yesterday' | 'week' | 'older';

export type SessionGroupKind = 'pinned' | 'updated' | 'project' | 'projectless';

export interface SessionListItem {
  session: AgentSession;
  projectName: string;
  timestamp: number;
}

export interface SessionGroup {
  /** Daemon group id: `pinned`, `updated:<bucket>`, `project:<uuid>`, `projectless`. */
  id: string;
  kind: SessionGroupKind;
  title: string;
  data: SessionListItem[];
  hasMore: boolean;
}

export type TranscriptRow =
  | { kind: 'message'; key: string; message: Message; footerTimestamp: number | null }
  | { kind: 'activities'; key: string; block: TranscriptBlock; live: boolean }
  | { kind: 'fold'; key: string; turn: AgentTurn; label: string; expanded: boolean }
  | { kind: 'changed'; key: string; checkpoint: Checkpoint };

const DATE_SECTION_TITLES: Record<SessionGroupId, string> = {
  pinned: 'Pinned',
  today: 'Today',
  yesterday: 'Yesterday',
  week: 'Previous 7 Days',
  older: 'Earlier',
};

/** Daemon `updated` buckets beyond the last week fold into "Earlier". */
const DAEMON_BUCKET_TO_SECTION: Record<string, SessionGroupId> = {
  today: 'today',
  yesterday: 'yesterday',
  week: 'week',
  month: 'older',
  year: 'older',
  more: 'older',
};

export function displaySessionTitle(session: AgentSession): string {
  if (session.title !== 'New task' && session.title.trim()) return session.title.trim();
  return session.auto_title?.trim() || 'New Task';
}

export function sessionHasStarted(session: AgentSession): boolean {
  return Boolean(
    session.turns.length || session.messages.length || session.provider_cursor || session.last_reply_at,
  );
}

export function sessionTimestamp(session: AgentSession): number {
  return session.last_reply_at ?? session.created_at;
}

/**
 * Turn daemon-owned sidebar groups into mobile sections. Sort and grouping
 * run on the daemon through `getSidebarGroups` (updated grouping); this only
 * resolves display names and folds daemon buckets into the mobile section
 * set (`month`/`year`/`more` → "Earlier"). Section order follows the daemon
 * response, so `oldest` ordering reverses date sections. Sessions missing
 * from the snapshot (e.g. search-filtered) are skipped.
 */
export function daemonGroupsToSessionSections(
  daemonGroups: SidebarGroupView[],
  sessionsById: ReadonlyMap<string, AgentSession>,
  projects: Project[],
  visibleSessionIds?: ReadonlySet<string>,
): SessionGroup[] {
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  const sections: SessionGroup[] = [];
  const indexes = new Map<string, number>();
  const itemFor = (session: AgentSession): SessionListItem => ({
    session,
    projectName: projectNames.get(session.project_id) || 'Unknown project',
    timestamp: sessionTimestamp(session),
  });
  for (const view of daemonGroups) {
    const items: SessionListItem[] = [];
    for (const sessionId of view.sessionIds) {
      if (visibleSessionIds && !visibleSessionIds.has(sessionId)) continue;
      const session = sessionsById.get(sessionId);
      if (!session) continue;
      items.push(itemFor(session));
    }
    if (view.kind === 'pinned') {
      sections.push({
        id: view.id,
        kind: 'pinned',
        title: DATE_SECTION_TITLES.pinned,
        data: items,
        hasMore: false,
      });
      continue;
    }
    const sectionId: SessionGroupId =
      DAEMON_BUCKET_TO_SECTION[view.dateGroup ?? 'today'] ?? 'older';
    const index = indexes.get(sectionId);
    if (index === undefined) {
      indexes.set(sectionId, sections.length);
      sections.push({
        id: view.id,
        kind: 'updated',
        title: DATE_SECTION_TITLES[sectionId],
        data: items,
        hasMore: false,
      });
    } else {
      sections[index]!.data.push(...items);
    }
  }
  return sections.filter((section) => section.data.length > 0);
}

/**
 * Turn daemon-owned project groups into mobile sections, mirroring the
 * desktop sidebar: pinned tasks first, then one collapsible section per
 * project (projectless tasks trailing), each carrying the daemon's `hasMore`
 * pager state for its "show more" row.
 */
export function daemonGroupsToProjectSections(
  daemonGroups: SidebarGroupView[],
  sessionsById: ReadonlyMap<string, AgentSession>,
  projects: Project[],
  visibleSessionIds?: ReadonlySet<string>,
  projectlessName = 'No project',
): SessionGroup[] {
  const projectNames = new Map(projects.map((project) => [project.id, project.name]));
  const itemFor = (session: AgentSession): SessionListItem => ({
    session,
    projectName: projectNames.get(session.project_id) || 'Unknown project',
    timestamp: sessionTimestamp(session),
  });
  const sections: SessionGroup[] = [];
  for (const view of daemonGroups) {
    const items: SessionListItem[] = [];
    for (const sessionId of view.sessionIds) {
      if (visibleSessionIds && !visibleSessionIds.has(sessionId)) continue;
      const session = sessionsById.get(sessionId);
      if (!session) continue;
      items.push(itemFor(session));
    }
    if (view.kind === 'pinned') {
      sections.push({
        id: view.id,
        kind: 'pinned',
        title: DATE_SECTION_TITLES.pinned,
        data: items,
        hasMore: false,
      });
      continue;
    }
    if (view.kind === 'projectless') {
      if (items.length === 0 && !view.hasMore) continue;
      sections.push({
        id: view.id,
        kind: 'projectless',
        title: projectlessName,
        data: items,
        hasMore: view.hasMore,
      });
      continue;
    }
    if (view.kind !== 'project') continue;
    if (items.length === 0 && !view.hasMore) continue;
    sections.push({
      id: view.id,
      kind: 'project',
      title: (view.projectId && projectNames.get(view.projectId)) || 'Unknown project',
      data: items,
      hasMore: view.hasMore,
    });
  }
  return sections;
}

export function relativeSessionTime(timestamp: number, now = Date.now()): string {
  const elapsed = Math.max(0, Math.floor(now / 1_000) - timestamp);
  if (elapsed < 60) return 'Now';
  if (elapsed < 3_600) return `${Math.floor(elapsed / 60)}m`;
  if (elapsed < 86_400) return `${Math.floor(elapsed / 3_600)}h`;
  if (elapsed < 604_800) return `${Math.floor(elapsed / 86_400)}d`;
  return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(
    new Date(timestamp * 1_000),
  );
}

export function providerLabel(provider: ProviderKind): string {
  const labels: Record<ProviderKind, string> = {
    agy: 'Antigravity',
    amp: 'Amp',
    claude: 'Claude',
    codex: 'Codex',
    commandCode: 'Command Code',
    cursor: 'Cursor',
    deepSeek: 'DeepSeek',
    fx: 'Fx',
    openCode: 'OpenCode',
    grok: 'Grok',
    kimi: 'Kimi',
    ohMyPi: 'Oh My Pi',
    pi: 'Pi',
    qoder: 'Qoder',
  };
  return labels[provider];
}

export function runtimeModeLabel(mode: RuntimeMode): string {
  const labels: Record<RuntimeMode, string> = {
    plan: 'Plan',
    ask: 'Ask first',
    autoAcceptEdits: 'Accept edits',
    auto: 'Auto',
    fullAccess: 'Full access',
  };
  return labels[mode];
}

export function contextPercent(session: AgentSession): number | null {
  const usage = session.context_usage;
  if (!usage || !usage.window) return null;
  return Math.max(0, Math.min(100, Math.round((usage.tokens / usage.window) * 100)));
}

interface TaggedRow {
  row: TranscriptRow;
  turnId: string | null;
  foldable: boolean;
  answerText: boolean;
}

/**
 * Interleaves messages with activity blocks and, for every settled turn,
 * hides everything before the terminal answer — thoughts, tool work, and
 * intermediate text parts alike — behind a "Worked for X" fold anchored where
 * the hidden work began. This mirrors the desktop transcript's turnFolds:
 * only the trailing run of answer text stays visible. Hidden rows are
 * re-emitted for turn ids present in `expandedFolds`.
 */
export function buildTranscriptRows(
  session: AgentSession,
  expandedFolds: ReadonlySet<string> = new Set(),
): TranscriptRow[] {
  const runningTurnId = session.turns.find((turn) => turn.status === 'running')?.id ?? null;
  const latestBlock = session.transcript_blocks.at(-1);

  const tagged: TaggedRow[] = [];
  const blocks = session.transcript_blocks.map((block, index) => ({ block, index }));
  for (let messageIndex = 0; messageIndex <= session.messages.length; messageIndex += 1) {
    for (const { block, index } of blocks) {
      if (block.after_message !== messageIndex) continue;
      tagged.push({
        row: {
          kind: 'activities',
          key: `activity:${index}:${block.turn_id ?? 'none'}:${block.after_message}`,
          block,
          live: Boolean(
            runningTurnId && block.turn_id === runningTurnId && block === latestBlock &&
              block.after_message === session.messages.length,
          ),
        },
        turnId: block.turn_id,
        foldable: true,
        answerText: false,
      });
    }
    const message = session.messages[messageIndex];
    if (message) {
      tagged.push({
        row: {
          kind: 'message',
          key: `message:${message.id}`,
          message,
          footerTimestamp: null,
        },
        turnId: message.turn_id,
        foldable: message.role === 'assistant',
        answerText: message.role === 'assistant' && Boolean(message.content.trim()),
      });
    }
  }

  // Per settled turn: which rows hide behind the fold and where it anchors.
  const hidden = new Set<TaggedRow>();
  const anchors = new Map<TaggedRow, AgentTurn>();
  const lastRowByTurn = new Map<string, TaggedRow>();
  for (const item of tagged) {
    if (item.turnId) lastRowByTurn.set(item.turnId, item);
  }
  for (const turn of session.turns) {
    if (turn.status === 'running') continue;
    const turnRows = tagged.filter((item) => item.turnId === turn.id && item.foldable);
    const answerStart = turnAnswerStart(turnRows, (item) => item.answerText);
    const work = turnRows.slice(0, answerStart);
    if (!work.length) continue;
    anchors.set(work[0]!, turn);
    for (const item of work) hidden.add(item);
  }

  const turnsById = new Map(session.turns.map((turn) => [turn.id, turn]));
  const rows: TranscriptRow[] = [];
  const answerRowByTurn = new Map<string, Extract<TranscriptRow, { kind: 'message' }>>();
  for (const item of tagged) {
    const anchorTurn = anchors.get(item);
    if (anchorTurn) {
      rows.push({
        kind: 'fold',
        key: `fold:${anchorTurn.id}`,
        turn: anchorTurn,
        label: turnFoldLabel(anchorTurn),
        expanded: expandedFolds.has(anchorTurn.id),
      });
    }
    if (!hidden.has(item) || (item.turnId && expandedFolds.has(item.turnId))) {
      rows.push(item.row);
      if (!hidden.has(item) && item.answerText && item.row.kind === 'message' && item.turnId) {
        answerRowByTurn.set(item.turnId, item.row);
      }
    }
    if (item.turnId && lastRowByTurn.get(item.turnId) === item) {
      const turn = turnsById.get(item.turnId);
      if (turn && turn.status !== 'running' && turn.checkpoint?.files.length) {
        rows.push({ kind: 'changed', key: `changed:${turn.id}`, checkpoint: turn.checkpoint });
      }
    }
  }
  for (const [turnId, row] of answerRowByTurn) {
    const turn = turnsById.get(turnId);
    if (turn?.completed_at && !row.message.streaming) row.footerTimestamp = turn.completed_at;
  }
  return rows;
}
