import type {
  AgentSession,
  BranchSnapshot,
  ComposerDraftChange,
  DaemonSettings,
  Project,
  ProviderKind,
  ProviderProbe,
  ResponsePayload,
  PaduClient,
  SidebarGroupView,
  SidebarGrouping,
  SidebarOrdering,
  WorkspaceResult,
} from '@padu/client';

export type TaskState = Extract<ResponsePayload, { type: 'taskState' }>;
export type DaemonDirectory = Extract<WorkspaceResult, { type: 'directory' }>;

export const daemonKeys = {
  taskState: (profileId: string) => ['daemon', profileId, 'task-state'] as const,
  session: (profileId: string, sessionId: string) => [
    'daemon',
    profileId,
    'session',
    sessionId,
  ] as const,
  settings: (profileId: string) => ['daemon', profileId, 'settings'] as const,
  provider: (profileId: string, provider: ProviderKind) => [
    'daemon',
    profileId,
    'provider',
    provider,
  ] as const,
  directory: (profileId: string, path: string | null) => [
    'daemon',
    profileId,
    'directory',
    path ?? 'home',
  ] as const,
  branches: (profileId: string, cwd: string) => [
    'daemon',
    profileId,
    'branches',
    cwd,
  ] as const,
  sidebarGroups: (
    profileId: string,
    grouping: SidebarGrouping,
    ordering: SidebarOrdering,
    dayKey: string,
    revealedOlder: Record<string, number>,
  ) => [
    'daemon',
    profileId,
    'sidebar-groups',
    grouping,
    ordering,
    dayKey,
    JSON.stringify(revealedOlder),
  ] as const,
};

export async function loadTaskState(client: PaduClient): Promise<TaskState> {
  return expectResponse(await client.request({ type: 'loadTaskState' }), 'taskState');
}

export interface SidebarGroupsParams {
  grouping: SidebarGrouping;
  ordering: SidebarOrdering;
  /** Client-local calendar day the daemon buckets `updated` groups against. */
  today: Date;
  nowSecs: number;
  /** Client-local UTC offset in seconds; the daemon may run in another zone. */
  localUtcOffsetSecs: number;
  /** Per-group count of older sessions the user revealed through "show more". */
  revealedOlder?: Record<string, number>;
}

/**
 * Daemon-owned sidebar sort & grouping. Ordering and date-bucket grouping
 * run on the daemon over its authoritative task state; callers only render
 * the returned groups.
 */
export async function fetchSidebarGroups(
  client: PaduClient,
  params: SidebarGroupsParams,
): Promise<SidebarGroupView[]> {
  const response = expectResponse(
    await client.request({
      type: 'getSidebarGroups',
      grouping: params.grouping,
      ordering: params.ordering,
      todayYear: params.today.getFullYear(),
      todayMonth: params.today.getMonth() + 1,
      todayDay: params.today.getDate(),
      nowSecs: params.nowSecs,
      localUtcOffsetSecs: params.localUtcOffsetSecs,
      revealedOlder: params.revealedOlder ?? {},
    }),
    'sidebarGroups',
  );
  return response.groups;
}

export async function hydrateSession(
  client: PaduClient,
  sessionId: string,
): Promise<AgentSession | null> {
  const response = expectResponse(
    await client.request({ type: 'hydrateSession', sessionId }),
    'session',
  );
  return response.session;
}

export async function attachDaemonSession(
  client: PaduClient,
  sessionId: string,
): Promise<{ runtimeId: string; supportsSteer: boolean } | null> {
  const response = expectResponse(
    await client.request({ type: 'attachSession' }, sessionId),
    'sessionRuntime',
  );
  return response.runtimeId
    ? { runtimeId: response.runtimeId, supportsSteer: response.supportsSteer }
    : null;
}

export async function loadDaemonSettings(client: PaduClient): Promise<DaemonSettings> {
  const response = expectResponse(await client.request({ type: 'getSettings' }), 'settings');
  return {
    ...response.settings,
    provider_binary_overrides: response.settings.provider_binary_overrides ?? {},
  };
}

export async function probeProvider(
  client: PaduClient,
  provider: ProviderKind,
  settings: DaemonSettings,
  options: { discoverModels?: boolean; probeVersion?: boolean } = {},
): Promise<ProviderProbe & { version: string | null }> {
  const response = expectResponse(
    await client.request({
      type: 'probeProvider',
      provider,
      binaryOverride: settings.provider_binary_overrides?.[provider] ?? null,
      discoverModels: options.discoverModels ?? true,
      probeVersion: options.probeVersion ?? false,
    }),
    'providerProbe',
  );
  return { ...response.probe, version: response.version ?? null };
}

export async function browseDaemonDirectory(
  client: PaduClient,
  path: string | null,
): Promise<DaemonDirectory> {
  const response = expectResponse(
    await client.request({ type: 'workspace', operation: { type: 'browseDirectory', path } }),
    'workspace',
  );
  if (response.result.type !== 'directory') {
    throw new Error('The daemon returned an unexpected directory response');
  }
  return response.result;
}

export function createProject(
  rawPath: string,
  id: string,
  createdAt = Math.floor(Date.now() / 1_000),
): Project {
  const input = rawPath.trim();
  if (!input.startsWith('/') && !/^[a-z]:[\\/]/i.test(input)) {
    throw new Error('Enter an absolute path on the daemon host');
  }
  const path = input === '/' ? input : input.replace(/[\\/]+$/, '');
  const name = path.split(/[\\/]/).filter(Boolean).at(-1) ?? 'Project';
  return { id, name, path, created_at: createdAt };
}

export async function persistProject(
  client: PaduClient,
  candidate: Project,
): Promise<{ project: Project; taskState: TaskState }> {
  const current = await loadTaskState(client);
  const existing = current.projects.find((project) => project.path === candidate.path);
  if (existing) return { project: existing, taskState: current };
  const projects = [...current.projects, candidate];
  expectResponse(
    await client.request({
      type: 'saveTaskState',
      projects,
      liveSessionIds: current.sessions.map((session) => session.id),
      sessions: [],
    }),
    'taskStateSaved',
  );
  return { project: candidate, taskState: { ...current, projects } };
}

export async function createProjectlessWorkspace(client: PaduClient): Promise<string> {
  const response = expectResponse(
    await client.request({
      type: 'workspace',
      operation: { type: 'createProjectlessWorkspace', prompt: null },
    }),
    'workspace',
  );
  if (response.result.type !== 'projectlessWorkspace') {
    throw new Error('The daemon returned an unexpected workspace response');
  }
  return response.result.cwd;
}

export async function materializeWorktree(
  client: PaduClient,
  session: AgentSession,
  projectPath: string,
  prompt: string,
): Promise<AgentSession> {
  if (session.workspace?.kind !== 'newWorktree') return session;
  const response = expectResponse(
    await client.request({
      type: 'workspace',
      operation: {
        type: 'createWorktree',
        project_path: projectPath,
        project_id: session.project_id,
        session_id: session.id,
        prompt,
        base_branch: session.workspace.baseBranch ?? null,
      },
    }),
    'workspace',
  );
  if (response.result.type !== 'worktreeCreated') {
    throw new Error('The daemon returned an unexpected worktree result');
  }
  return {
    ...session,
    workspace: {
      kind: 'worktree',
      path: response.result.worktree.path,
      branch: response.result.worktree.branch,
    },
  };
}

export async function loadComposerDrafts(
  client: PaduClient,
): Promise<Extract<ResponsePayload, { type: 'composerDrafts' }>['drafts']> {
  const response = expectResponse(
    await client.request({ type: 'loadComposerDrafts' }),
    'composerDrafts',
  );
  return response.drafts;
}

export async function applyComposerDraftChanges(
  client: PaduClient,
  changes: ComposerDraftChange[],
): Promise<void> {
  expectResponse(
    await client.request({ type: 'applyComposerDraftChanges', changes }),
    'ack',
  );
}

export async function inspectBranches(
  client: PaduClient,
  cwd: string,
): Promise<BranchSnapshot | null> {
  const response = expectResponse(
    await client.request({ type: 'workspace', operation: { type: 'inspectBranches', cwd } }),
    'workspace',
  );
  if (response.result.type !== 'branches') {
    throw new Error('The daemon returned an unexpected branches response');
  }
  return response.result.snapshot;
}

export async function removeDaemonSession(
  client: PaduClient,
  sessionId: string,
): Promise<void> {
  expectResponse(await client.request({ type: 'removeSession' }, sessionId), 'ack');
}

export async function setSessionPinned(
  client: PaduClient,
  sessionId: string,
  pinned: boolean,
): Promise<AgentSession> {
  const response = expectResponse(
    await client.request({ type: 'setSessionPinned', pinned }, sessionId),
    'sessionMetadataUpdated',
  );
  return response.session;
}

/**
 * Reads daemon blob bytes on the global path (these commands bail on the
 * session-scoped path). Returns base64, mirroring web's attachments reader.
 */
export async function readDaemonBlob(
  client: PaduClient,
  reference: string,
): Promise<string> {
  const response = expectResponse(
    await client.request({ type: 'readBlob', reference }),
    'blobData',
  );
  return response.bytes;
}

export async function setSessionArchived(
  client: PaduClient,
  sessionId: string,
  archived: boolean,
): Promise<AgentSession> {
  const response = expectResponse(
    await client.request({ type: 'setSessionArchived', archived }, sessionId),
    'sessionMetadataUpdated',
  );
  return response.session;
}

export async function persistSession(
  client: PaduClient,
  session: AgentSession,
): Promise<AgentSession> {
  const response = expectResponse(
    await client.request({
      type: 'saveTaskState',
      projects: [],
      liveSessionIds: [session.id],
      sessions: [session],
    }),
    'taskStateSaved',
  );
  return response.sessions.find((item) => item.id === session.id) ?? session;
}

function expectResponse<T extends ResponsePayload['type']>(
  response: ResponsePayload,
  expected: T,
): Extract<ResponsePayload, { type: T }> {
  if (response.type !== expected) {
    throw new Error(`Expected daemon response ${expected}, received ${response.type}`);
  }
  return response as Extract<ResponsePayload, { type: T }>;
}
