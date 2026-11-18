import type {
  AgentSession,
  ComposerDraft,
  EmbeddedNote,
  MessageAttachment,
  PendingSteer,
  Project,
  ProviderKind,
  RuntimeMode,
  SequencedEvent,
} from '@padu/client';

export interface MobileRuntimeClock {
  nowSeconds: () => number;
  randomUUID: () => string;
}

export interface NewSessionOptions {
  sessionId?: string;
  model?: string | null;
  reasoningEffort?: string | null;
  runtimeMode?: RuntimeMode;
  /** Base branch for an isolated worktree; null means the project default. */
  baseBranch?: string | null;
}

export interface CreateTaskResult {
  session: AgentSession;
  promptAccepted: boolean;
  error?: string;
}

export interface CreateTaskRecoveryDependencies {
  createDraft: () => AgentSession;
  cache: (session: AgentSession) => void;
  persist: (session: AgentSession) => Promise<AgentSession>;
  send: (session: AgentSession) => Promise<AgentSession>;
  latest: (fallback: AgentSession) => AgentSession;
  reportError: (sessionId: string, error: string) => void;
}

/**
 * Persist the task before starting its provider. A provider-start failure
 * returns the failed session but explicitly marks the prompt as unaccepted so
 * the New Task screen keeps its recoverable composer draft.
 */
export async function createTaskWithRecovery(
  dependencies: CreateTaskRecoveryDependencies,
): Promise<CreateTaskResult> {
  const draft = dependencies.createDraft();
  dependencies.cache(draft);
  const saved = await dependencies.persist(draft);
  try {
    return {
      session: await dependencies.send(saved),
      promptAccepted: true,
    };
  } catch (cause) {
    const error = cause instanceof Error && cause.message.trim()
      ? cause.message
      : String(cause);
    dependencies.reportError(saved.id, error);
    return {
      session: dependencies.latest(saved),
      promptAccepted: false,
      error,
    };
  }
}

export interface PromptTurnOptions {
  /** Provider-facing text after skill or command rendering. */
  providerPrompt?: string;
  attachments?: MessageAttachment[];
  embeddedNotes?: EmbeddedNote[];
}

/** One recoverable composer snapshot for daemon-backed draft storage. */
export function composerDraftSnapshot(
  text: string,
  attachments: readonly MessageAttachment[],
  embeddedNotes: readonly EmbeddedNote[],
): ComposerDraft | null {
  if (!text.trim() && !attachments.length && !embeddedNotes.length) return null;
  return {
    text,
    attachments: [...attachments],
    embedded_notes: [...embeddedNotes],
  };
}

export function beginTurn(
  session: AgentSession,
  prompt: string,
  clock: MobileRuntimeClock,
  options: PromptTurnOptions = {},
): AgentSession {
  const now = clock.nowSeconds();
  const turnId = clock.randomUUID();
  const visiblePrompt = prompt.trim();
  const providerPrompt = options.providerPrompt?.trim() || visiblePrompt;
  return {
    ...session,
    auto_title:
      session.messages.length === 0 && session.title === 'New task' && !session.auto_title
        ? promptTitle(visiblePrompt)
        : session.auto_title,
    status: 'connecting',
    updated_at: now,
    last_reply_at: now,
    messages: [
      ...session.messages,
      {
        id: clock.randomUUID(),
        turn_id: turnId,
        role: 'user',
        content: providerPrompt,
        display_content: providerPrompt !== visiblePrompt ? visiblePrompt : null,
        attachments: options.attachments ?? [],
        embedded_notes: options.embeddedNotes ?? [],
        created_at: now,
        streaming: false,
      },
    ],
    turns: [
      ...session.turns,
      {
        id: turnId,
        turn_count: session.turns.length + 1,
        status: 'running',
        provider_turn_started: false,
        provider_resume_at: null,
        started_at: now,
        completed_at: null,
        checkpoint: null,
      },
    ],
  };
}

export function createSession(
  projectId: string,
  provider: ProviderKind,
  isolated: boolean,
  clock: MobileRuntimeClock,
  options: NewSessionOptions = {},
): AgentSession {
  const now = clock.nowSeconds();
  return {
    id: options.sessionId ?? clock.randomUUID(),
    title: 'New task',
    auto_title: null,
    project_id: projectId,
    workspace: isolated
      ? { kind: 'newWorktree', baseBranch: options.baseBranch ?? null }
      : { kind: 'local' },
    provider,
    model: options.model ?? null,
    runtime_mode: options.runtimeMode ?? 'fullAccess',
    interaction_mode: 'build',
    reasoning_effort: options.reasoningEffort ?? null,
    service_tier: null,
    context_window: null,
    agent_preset: null,
    status: 'idle',
    created_at: now,
    updated_at: now,
    last_reply_at: null,
    provider_cursor: null,
    available_commands: [],
    context_usage: null,
    provider_session_id: null,
    messages: [],
    transcript_blocks: [],
    turns: [],
    queued_messages: [],
  };
}

/** Mirrors the web client's queueSubmission: a prompt sent while a turn is
 * live becomes a persisted QueuedMessage. `content` carries the provider
 * payload resolved on this host; `display_content` keeps the authored text. */
export function queueSubmission(
  session: AgentSession,
  prompt: string,
  clock: MobileRuntimeClock,
  providerPrompt = prompt,
  attachments: MessageAttachment[] = [],
  embeddedNotes: EmbeddedNote[] = [],
): AgentSession {
  const now = clock.nowSeconds();
  const visiblePrompt = prompt.trim();
  const renderedPrompt = providerPrompt.trim() || visiblePrompt;
  return {
    ...session,
    updated_at: now,
    queued_messages: [
      ...(session.queued_messages ?? []),
      {
        id: clock.randomUUID(),
        content: renderedPrompt,
        display_content: renderedPrompt !== visiblePrompt ? visiblePrompt : null,
        attachments,
        embedded_notes: embeddedNotes,
        created_at: now,
      },
    ],
  };
}

export interface MobileQueuedMessageMetadata {
  visiblePrompt: string;
  providerPrompt: string;
  attachments: MessageAttachment[];
  embeddedNotes: EmbeddedNote[];
}

export interface MobileQueuedDrainDependencies {
  readCurrent: () => AgentSession | undefined;
  send: (
    session: AgentSession,
    metadata: MobileQueuedMessageMetadata,
  ) => Promise<void>;
  cache: (session: AgentSession) => void;
  persist: (session: AgentSession) => Promise<unknown>;
}

type QueuedMessage = NonNullable<AgentSession['queued_messages']>[number];

export type QueuedSubmissionAttempt =
  | { accepted: true; session: AgentSession }
  | { accepted: false; error: unknown; session: AgentSession };

export interface MobilePendingSteerMetadata {
  displayContent: string;
  attachments: MessageAttachment[];
  embeddedNotes: EmbeddedNote[];
}

export function queuedMessageMetadata(
  message: QueuedMessage,
): MobileQueuedMessageMetadata {
  return {
    visiblePrompt: queuedVisiblePrompt(message),
    providerPrompt: queuedProviderPrompt(message),
    attachments: message.attachments ?? [],
    embeddedNotes: message.embedded_notes ?? [],
  };
}

/** Remove only the accepted queued row and leave the rest of the queue intact. */
export function removeQueuedSubmission(
  session: AgentSession,
  messageId: string,
): AgentSession {
  return {
    ...session,
    queued_messages: (session.queued_messages ?? []).filter(
      (message) => message.id !== messageId,
    ),
  };
}

/** Put the exact original row first without rebuilding its saved metadata. */
export function restoreQueuedSubmission(
  session: AgentSession,
  message: QueuedMessage,
): AgentSession {
  return {
    ...session,
    queued_messages: [
      message,
      ...(session.queued_messages ?? []).filter(
        (candidate) => candidate.id !== message.id,
      ),
    ],
  };
}

/**
 * Keep the queued row in the session until the provider prompt is accepted.
 * A disconnect, runtime-start failure, or preparation failure restores that
 * exact row at the front with its visible/provider text and attachments/notes.
 */
export async function attemptQueuedSubmission(
  session: AgentSession,
  message: QueuedMessage,
  submit: (
    session: AgentSession,
    metadata: MobileQueuedMessageMetadata,
  ) => Promise<void>,
): Promise<QueuedSubmissionAttempt> {
  try {
    await submit(session, queuedMessageMetadata(message));
    return {
      accepted: true,
      session: removeQueuedSubmission(session, message.id),
    };
  } catch (error) {
    return {
      accepted: false,
      error,
      session: restoreQueuedSubmission(session, message),
    };
  }
}

/**
 * Run the production drain transition after a settled turn.
 *
 * A disconnect, runtime-start failure, or preparation failure restores the
 * exact original row at the front of the newest session. Success removes only
 * the accepted row ID, preserving later rows and unrelated cache updates.
 */
export async function drainQueuedSubmission(
  queuedSession: AgentSession,
  message: QueuedMessage,
  dependencies: MobileQueuedDrainDependencies,
): Promise<AgentSession> {
  try {
    await dependencies.send(queuedSession, queuedMessageMetadata(message));
  } catch (error) {
    const current = dependencies.readCurrent() ?? queuedSession;
    const restored = restoreQueuedSubmission(current, message);
    dependencies.cache(restored);
    await dependencies.persist(restored).catch(() => undefined);
    throw error;
  }

  const current = dependencies.readCurrent() ?? queuedSession;
  const accepted = removeQueuedSubmission(current, message.id);
  dependencies.cache(accepted);
  await dependencies.persist(accepted);
  return accepted;
}

/** Keep provider-visible attachments and notes when a queued message is
 * edited on mobile, even if the original message was authored on web. */
export function composeMobileProviderPrompt(
  providerText: string,
  attachments: readonly MessageAttachment[],
  embeddedNotes: readonly EmbeddedNote[],
): string {
  return [
    providerText.trim(),
    attachments.map((attachment) => `@${attachment.mention}`).join(' '),
    ...embeddedNotes.map((note) => `${note.title || 'Untitled note'}\n${note.content}`),
  ].filter(Boolean).join('\n\n');
}

export function applyAcceptedSteer(
  session: AgentSession,
  pending: PendingSteer<MobilePendingSteerMetadata>,
  eventPrompt: string | undefined,
  clock: MobileRuntimeClock,
): AgentSession {
  const providerPrompt = eventPrompt ?? pending.providerPrompt;
  const metadata = pending.value;
  return {
    ...session,
    messages: [
      ...session.messages,
      {
        id: clock.randomUUID(),
        turn_id: session.turns.at(-1)?.id ?? null,
        role: 'user',
        content: providerPrompt,
        display_content:
          providerPrompt !== metadata.displayContent
            ? metadata.displayContent
            : null,
        attachments: metadata.attachments,
        embedded_notes: metadata.embeddedNotes,
        created_at: clock.nowSeconds(),
        streaming: false,
      },
    ],
  };
}

export function restoreRejectedSteer(
  session: AgentSession,
  pending: PendingSteer<MobilePendingSteerMetadata>,
  clock: MobileRuntimeClock,
): AgentSession {
  return queueSubmission(
    session,
    pending.value.displayContent,
    clock,
    pending.providerPrompt,
    pending.value.attachments,
    pending.value.embeddedNotes,
  );
}

export function queuedVisiblePrompt(
  message: Pick<NonNullable<AgentSession['queued_messages']>[number], 'content' | 'display_content'>,
): string {
  return message.display_content ?? message.content;
}

/**
 * Queued content is the provider payload resolved on the catalog's host.
 * Drain it as-is instead of resolving a fallback path a second time.
 */
export function queuedProviderPrompt(
  message: Pick<NonNullable<AgentSession['queued_messages']>[number], 'content'>,
): string {
  return message.content;
}

export function sessionBusy(session: Pick<AgentSession, 'status'>): boolean {
  return (
    session.status === 'connecting' || session.status === 'working' || session.status === 'waiting'
  );
}

export interface SessionOptionChanges {
  model?: string | null;
  reasoningEffort?: string | null;
  runtimeMode?: RuntimeMode;
}

export function applySessionOptions(
  session: AgentSession,
  changes: SessionOptionChanges,
  clock: MobileRuntimeClock,
): AgentSession {
  return {
    ...session,
    model: changes.model !== undefined ? changes.model : session.model,
    reasoning_effort:
      changes.reasoningEffort !== undefined ? changes.reasoningEffort : session.reasoning_effort,
    runtime_mode: changes.runtimeMode ?? session.runtime_mode,
    updated_at: clock.nowSeconds(),
  };
}

export function sessionCwd(session: AgentSession, project: Project): string {
  return session.workspace?.kind === 'worktree' ? session.workspace.path : project.path;
}

export function runtimeEventAlreadyApplied(session: AgentSession, event: SequencedEvent): boolean {
  const cursor = session.runtime_event_cursor;
  return Boolean(
    cursor && cursor.runtime_id === event.runtimeId && cursor.epoch === event.epoch &&
      cursor.sequence >= event.sequence,
  );
}

export function shouldApplyRuntimeEvent(
  session: AgentSession,
  event: SequencedEvent,
): boolean {
  if (!runtimeEventAlreadyApplied(session, event)) return true;
  return session.status === 'waiting' && (
    event.event.kind === 'permission' || event.event.kind === 'userInputRequested'
  );
}

function promptTitle(prompt: string): string | null {
  let title = prompt.split(/\s+/u).filter(Boolean).slice(0, 7).join(' ');
  if (!title) return null;
  if ([...title].length > 54) title = `${[...title].slice(0, 53).join('')}…`;
  return title;
}
