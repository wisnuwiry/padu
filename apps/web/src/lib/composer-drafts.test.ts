import { describe, expect, test } from 'bun:test'
import type { ComposerDraftChange, ComposerDrafts } from '@padu/client'
import {
  composerDraftChanges,
  composerDraftFor,
  flushComposerDraftSnapshot,
  moveComposerDraftToEmpty,
  setComposerDraft,
  waitForComposerDraftSource,
} from './composer-drafts'

describe('composer drafts', () => {
  test('updates one key without replacing unrelated client drafts', () => {
    const drafts: ComposerDrafts = {
      sessions: { other: { text: 'from desktop' } },
    }
    const change = setComposerDraft(
      drafts,
      { type: 'newSession', projectId: 'padu' },
      { text: 'from web' },
    )

    expect(change).toEqual({
      target: { type: 'newSession', projectId: 'padu' },
      draft: { text: 'from web', attachments: [], embedded_notes: [] },
    })
    expect(drafts.sessions?.other?.text).toBe('from desktop')
  })

  test('moves a new-task draft only when the destination is empty', () => {
    const drafts: ComposerDrafts = {
      new_sessions: {
        first: { text: 'carry me' },
        occupied: { text: 'keep me' },
      },
    }
    expect(moveComposerDraftToEmpty(
      drafts,
      { type: 'newSession', projectId: 'first' },
      { type: 'newSession', projectId: 'second' },
    )).toHaveLength(2)
    expect(composerDraftFor(drafts, { type: 'newSession', projectId: 'second' }).text)
      .toBe('carry me')

    expect(moveComposerDraftToEmpty(
      drafts,
      { type: 'newSession', projectId: 'second' },
      { type: 'newSession', projectId: 'occupied' },
    )).toEqual([])
    expect(composerDraftFor(drafts, { type: 'newSession', projectId: 'occupied' }).text)
      .toBe('keep me')
  })

  test('keeps embedded notes in drafts and flushes every recoverable target', () => {
    const note = {
      id: 'note-1',
      title: 'Context',
      content: 'Keep this note',
      revision: 2,
    }
    const drafts: ComposerDrafts = {}

    const change = setComposerDraft(
      drafts,
      { type: 'session', sessionId: 'session-1' },
      { text: '', attachments: [], embedded_notes: [note] },
    )

    expect(change).toEqual({
      target: { type: 'session', sessionId: 'session-1' },
      draft: { text: '', attachments: [], embedded_notes: [note] },
    })
    expect(composerDraftFor(
      drafts,
      { type: 'session', sessionId: 'session-1' },
    ).embedded_notes).toEqual([note])
    expect(composerDraftChanges(drafts)).toEqual([{
      target: { type: 'session', sessionId: 'session-1' },
      draft: { text: '', attachments: [], embedded_notes: [note] },
    }])
  })

  test('flushes the submitted notes after older writes before a host switch', async () => {
    const note = {
      id: 'note-2',
      title: 'Submitted context',
      content: 'Survive the host switch',
      revision: 1,
    }
    let drafts: ComposerDrafts = {
      sessions: {
        submitted: { text: '$review this', embedded_notes: [note] },
      },
    }
    let generation = 0
    const order: string[] = []

    await flushComposerDraftSnapshot(
      () => drafts,
      () => [],
      () => generation,
      async () => {
        order.push('pending-write')
      },
      async (changes) => {
        order.push('flush')
        if (generation === 0) {
          expect(changes).toEqual([{
            target: { type: 'session', sessionId: 'submitted' },
            draft: { text: '$review this', attachments: [], embedded_notes: [note] },
          }])
          drafts = {
            sessions: {
              submitted: {
                text: '$review this and this',
                embedded_notes: [note],
              },
            },
          }
          generation = 1
        } else {
          expect(changes).toEqual([{
            target: { type: 'session', sessionId: 'submitted' },
            draft: {
              text: '$review this and this',
              attachments: [],
              embedded_notes: [note],
            },
          }])
        }
      },
    )

    expect(order).toEqual(['pending-write', 'flush', 'pending-write', 'flush'])
  })

  test('flushes cleared drafts as tombstones before disconnect', async () => {
    const target = { type: 'session' as const, sessionId: 'cleared' }
    const changes: ComposerDraftChange[] = []

    await flushComposerDraftSnapshot(
      () => ({}),
      () => [target],
      () => 0,
      async () => {},
      async (next) => {
        changes.push(...next)
      },
    )

    expect(changes).toEqual([{ target, draft: null }])
  })

  test('waits for an older queued write before snapshotting the newer draft', async () => {
    const drafts: ComposerDrafts = {
      sessions: { raced: { text: 'v3' } },
    }
    const generation = 1
    let queue = Promise.resolve()
    const writes: string[] = []

    queue = queue.then(() => {
      writes.push('v2')
    })

    await flushComposerDraftSnapshot(
      () => drafts,
      () => [],
      () => generation,
      async () => queue,
      async (changes) => {
        queue = queue.then(() => {
          const text = changes[0]?.draft?.text
          if (text) writes.push(text)
        })
        await queue
      },
    )

    expect(writes).toEqual(['v2', 'v3'])
  })

  test('does not flush a host until that host draft source hydrates', async () => {
    let source: string | null = null
    let waits = 0

    await waitForComposerDraftSource(
      () => source,
      'wss://host-a',
      async () => {
        waits += 1
        if (waits === 2) source = 'wss://host-a'
      },
    )

    expect(waits).toBe(2)
  })
})
