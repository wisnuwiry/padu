import { describe, expect, test } from 'bun:test'
import type {
  AgentSession,
  EmbeddedNote,
  MessageAttachment,
} from '@padu/client'

import {
  attemptQueuedSubmission,
  awaitComposerSubmissionAcceptance,
  drainQueuedSubmission,
  queuedSubmissionMetadata,
} from './runtime-submissions'

const ATTACHMENTS: MessageAttachment[] = [{
  path: '/workspace/src/web.ts',
  mention: 'src/web.ts',
  name: 'web.ts',
  is_dir: false,
  is_image: false,
}]
const NOTES: EmbeddedNote[] = [{
  id: 'note-1',
  title: 'Web context',
  content: 'Keep the visible and provider text split.',
  revision: 4,
}]

describe('web queued submission recovery', () => {
  test('removes only the accepted row after the daemon accepts the prompt', async () => {
    const first = queuedMessage('first')
    const later = queuedMessage('later')
    const current = session([first, later])

    const attempt = await attemptQueuedSubmission(current, first, async (_queued, metadata) => {
      expect(metadata).toMatchObject({
        visiblePrompt: first.display_content,
        providerPrompt: first.content,
        attachments: ATTACHMENTS,
        embeddedNotes: NOTES,
      })
    })

    expect(attempt.accepted).toBe(true)
    if (!attempt.accepted) throw new Error('expected acceptance')
    expect(attempt.session.queued_messages).toEqual([later])
    expect(attempt.session.queued_messages?.[0]).toBe(later)
  })

  for (const [phase, error] of [
    ['disconnect', new Error('daemon disconnected')],
    ['startup', new Error('runtime failed to start')],
    ['preparation', new Error('provider preparation failed')],
  ] as const) {
    test(`restores the exact row at the front after ${phase} failure`, async () => {
      const first = queuedMessage('first')
      const later = queuedMessage('later')
      const current = session([first, later])

      const attempt = await attemptQueuedSubmission(current, first, async () => {
        throw error
      })

      expect(attempt.accepted).toBe(false)
      expect(attempt.accepted ? undefined : attempt.error).toBe(error)
      expect(attempt.session.queued_messages?.[0]).toBe(first)
      expect(attempt.session.queued_messages?.[1]).toBe(later)
      expect(queuedSubmissionMetadata(attempt.session.queued_messages![0]!)).toEqual({
        visiblePrompt: '$deploy explain this',
        providerPrompt: '[mandatory! open skill: /remote/SKILL.md] explain this',
        attachments: ATTACHMENTS,
        embeddedNotes: NOTES,
      })
    })
  }
})

describe('web production queue drain transition', () => {
  test('removes only the exact accepted ID after the send updates the queue', async () => {
    const first = queuedMessage('first')
    const later = queuedMessage('later', {
      content: 'later provider text',
      display_content: '$later visible prompt',
      created_at: 20,
    })
    const addedDuringSend = queuedMessage('added-during-send', {
      content: 'newer provider text',
      display_content: '$newer visible prompt',
      created_at: 30,
    })
    const queuedSession = session([first, later])
    const current = {
      ...session([later, first, addedDuringSend]),
      status: 'connecting' as const,
    }
    let cached: AgentSession | undefined
    const persisted: AgentSession[] = []

    const drained = await drainQueuedSubmission(queuedSession, first, {
      readCurrent: () => current,
      send: async (sessionAtDrain, metadata) => {
        expect(sessionAtDrain).toBe(queuedSession)
        expect(metadata).toEqual(queuedSubmissionMetadata(first))
      },
      cache: (session) => {
        cached = session
      },
      persist: async (session) => {
        persisted.push(session)
        return session
      },
    })

    expect(drained.queued_messages?.map((message) => message.id)).toEqual([
      'later',
      'added-during-send',
    ])
    expect(drained.queued_messages?.[0]).toBe(later)
    expect(drained.queued_messages?.[1]).toBe(addedDuringSend)
    expect(cached).toBe(drained)
    expect(persisted).toEqual([drained])
  })

  test('persists the exact original row after the send accepted the prompt', async () => {
    const first = queuedMessage('first')
    const later = queuedMessage('later', {
      content: 'later provider text',
      display_content: '$later visible prompt',
      created_at: 20,
    })
    const queuedSession = session([first, later])
    let cached: AgentSession | undefined
    const persisted: AgentSession[] = []

    const drained = await drainQueuedSubmission(queuedSession, first, {
      readCurrent: () => queuedSession,
      send: async () => undefined,
      cache: (session) => {
        cached = session
      },
      persist: async (session) => {
        persisted.push(session)
        return session
      },
    })

    expect(drained.queued_messages).toEqual([later])
    expect(drained.queued_messages?.[0]).toBe(later)
    expect(cached).toBe(drained)
    expect(persisted).toEqual([drained])
  })

  for (const [phase, error] of [
    ['disconnect', new Error('daemon disconnected')],
    ['startup', new Error('runtime failed to start')],
    ['preparation', new Error('provider preparation failed')],
  ] as const) {
    test(`restores the exact row into the newest cache after ${phase} failure`, async () => {
      const first = queuedMessage('first')
      const later = queuedMessage('later', {
        content: 'later provider text',
        display_content: '$later visible prompt',
        created_at: 20,
      })
      const staleFirst = queuedMessage('first', {
        content: 'stale provider text',
        display_content: '$stale visible prompt',
        created_at: 99,
      })
      const queuedSession = session([first, later])
      const current = {
        ...session([later, staleFirst]),
        updated_at: 200,
      }
      let cached: AgentSession | undefined
      const persisted: AgentSession[] = []

      await expect(drainQueuedSubmission(queuedSession, first, {
        readCurrent: () => current,
        send: async (_sessionAtDrain, metadata) => {
          expect(metadata).toEqual(queuedSubmissionMetadata(first))
          throw error
        },
        cache: (session) => {
          cached = session
        },
        persist: async (session) => {
          persisted.push(session)
          return session
        },
      })).rejects.toBe(error)

      expect(cached?.queued_messages?.map((message) => message.id)).toEqual([
        'first',
        'later',
      ])
      expect(cached?.queued_messages?.[0]).toBe(first)
      expect(cached?.queued_messages?.[1]).toBe(later)
      expect(cached?.queued_messages?.[0]).toMatchObject({
        id: first.id,
        content: first.content,
        display_content: first.display_content,
        attachments: ATTACHMENTS,
        embedded_notes: NOTES,
        created_at: first.created_at,
      })
      if (!cached) throw new Error('expected the restored session to be cached')
      expect(persisted).toEqual([cached])
    })
  }
})

describe('web composer draft acceptance', () => {
  for (const [phase, error] of [
    ['disconnect', new Error('daemon disconnected')],
    ['startup', new Error('runtime failed to start')],
    ['preparation', new Error('provider preparation failed')],
  ] as const) {
    test(`keeps the persisted draft after ${phase} failure and unmount`, async () => {
      let reject!: (error: unknown) => void
      const pending = new Promise<void>((_resolve, rejectPending) => {
        reject = rejectPending
      })
      let removed = false
      const accepted = awaitComposerSubmissionAcceptance(pending, () => {
        removed = true
      })

      reject(error)

      await expect(accepted).rejects.toBe(error)
      expect(removed).toBe(false)
    })
  }

  test('removes the persisted draft only after the send resolves', async () => {
    let resolve!: () => void
    const pending = new Promise<void>((resolvePending) => {
      resolve = resolvePending
    })
    let removed = false
    const accepted = awaitComposerSubmissionAcceptance(pending, () => {
      removed = true
    })

    await Promise.resolve()
    expect(removed).toBe(false)

    resolve()
    await accepted
    expect(removed).toBe(true)
  })
})

function queuedMessage(
  id: string,
  overrides: Partial<NonNullable<AgentSession['queued_messages']>[number]> = {},
): NonNullable<AgentSession['queued_messages']>[number] {
  return {
    id,
    content: '[mandatory! open skill: /remote/SKILL.md] explain this',
    display_content: '$deploy explain this',
    attachments: ATTACHMENTS,
    embedded_notes: NOTES,
    created_at: 10,
    ...overrides,
  }
}

function session(
  queuedMessages: NonNullable<AgentSession['queued_messages']>,
): AgentSession {
  return {
    id: 'session',
    title: 'New task',
    auto_title: null,
    project_id: 'project',
    workspace: { kind: 'local' },
    provider: 'codex',
    runtime_mode: 'fullAccess',
    interaction_mode: 'build',
    status: 'idle',
    created_at: 1,
    updated_at: 1,
    last_reply_at: null,
    provider_cursor: null,
    messages: [],
    transcript_blocks: [],
    turns: [],
    queued_messages: queuedMessages,
  }
}
