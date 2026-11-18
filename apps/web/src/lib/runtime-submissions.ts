import type { AgentSession, EmbeddedNote, MessageAttachment } from '@padu/client'

type QueuedMessage = NonNullable<AgentSession['queued_messages']>[number]

export interface QueuedSubmissionMetadata {
  visiblePrompt: string
  providerPrompt: string
  attachments: MessageAttachment[]
  embeddedNotes: EmbeddedNote[]
}

export interface QueuedDrainDependencies {
  readCurrent: () => AgentSession | undefined
  send: (
    session: AgentSession,
    metadata: QueuedSubmissionMetadata,
  ) => Promise<void>
  cache: (session: AgentSession) => void
  persist: (session: AgentSession) => Promise<unknown>
}

export type QueuedSubmissionAttempt =
  | { accepted: true; session: AgentSession }
  | { accepted: false; error: unknown; session: AgentSession }

/**
 * Keep a persisted composer draft until the daemon has accepted its prompt.
 * A host switch can unmount the composer while the request is still in
 * flight; removing the stored draft before acceptance would then leave the
 * user with no recoverable text if the request fails.
 */
export async function awaitComposerSubmissionAcceptance(
  pending: Promise<unknown>,
  onAccepted: () => void,
): Promise<void> {
  await pending
  onAccepted()
}

export function queueSubmission(
  session: AgentSession,
  displayContent: string,
  providerPrompt: string,
  attachments: MessageAttachment[],
  embeddedNotes: EmbeddedNote[] = [],
): AgentSession {
  return {
    ...session,
    updated_at: Math.floor(Date.now() / 1_000),
    queued_messages: [
      ...(session.queued_messages ?? []),
      {
        id: crypto.randomUUID(),
        content: providerPrompt,
        display_content: attachments.length || providerPrompt !== displayContent
          ? displayContent
          : null,
        attachments,
        embedded_notes: embeddedNotes,
        created_at: Math.floor(Date.now() / 1_000),
      },
    ],
  }
}

export function queuedSubmissionMetadata(message: QueuedMessage): QueuedSubmissionMetadata {
  return {
    visiblePrompt: message.display_content ?? message.content,
    providerPrompt: message.content,
    attachments: message.attachments ?? [],
    embeddedNotes: message.embedded_notes ?? [],
  }
}

/**
 * Remove only the accepted queued item. Other queued messages keep their
 * original metadata and order.
 */
export function removeQueuedSubmission(
  session: AgentSession,
  messageId: string,
): AgentSession {
  return {
    ...session,
    queued_messages: (session.queued_messages ?? []).filter(
      (message) => message.id !== messageId,
    ),
  }
}

/**
 * Restore the exact queued item at the front. The queued row already carries
 * the visible/provider split and provider-visible attachments and notes, so
 * recovery must not rebuild it from partial metadata.
 */
export function restoreQueuedSubmission(
  session: AgentSession,
  message: QueuedMessage,
): AgentSession {
  return {
    ...session,
    queued_messages: [
      message,
      ...(session.queued_messages ?? []).filter((candidate) => candidate.id !== message.id),
    ],
  }
}

/**
 * A queued item remains in the session while the daemon starts the runtime and
 * accepts the prompt. Only a successful submit removes it. A disconnect,
 * startup error, or preparation error leaves the exact item at the front.
 */
export async function attemptQueuedSubmission(
  session: AgentSession,
  message: QueuedMessage,
  submit: (
    session: AgentSession,
    metadata: QueuedSubmissionMetadata,
  ) => Promise<void>,
): Promise<QueuedSubmissionAttempt> {
  try {
    await submit(session, queuedSubmissionMetadata(message))
    return {
      accepted: true,
      session: removeQueuedSubmission(session, message.id),
    }
  } catch (error) {
    return {
      accepted: false,
      error,
      session: restoreQueuedSubmission(session, message),
    }
  }
}

/**
 * The production transition used after a settled turn releases the queue.
 *
 * The send can fail while connecting, starting the runtime, or preparing the
 * provider turn. Every failure restores the exact queued row at the front of
 * the newest cached session. A successful send removes only that row's ID, so
 * rows added or reordered while the request was in flight stay intact.
 */
export async function drainQueuedSubmission(
  queuedSession: AgentSession,
  message: QueuedMessage,
  dependencies: QueuedDrainDependencies,
): Promise<AgentSession> {
  try {
    await dependencies.send(queuedSession, queuedSubmissionMetadata(message))
  } catch (error) {
    const current = dependencies.readCurrent() ?? queuedSession
    const restored = restoreQueuedSubmission(current, message)
    dependencies.cache(restored)
    await dependencies.persist(restored).catch(() => undefined)
    throw error
  }

  const current = dependencies.readCurrent() ?? queuedSession
  const accepted = removeQueuedSubmission(current, message.id)
  dependencies.cache(accepted)
  await dependencies.persist(accepted)
  return accepted
}
