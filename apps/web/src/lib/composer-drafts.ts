import type {
  ComposerDraft,
  ComposerDraftChange,
  ComposerDrafts,
  ComposerDraftTarget,
} from '@padu/client'

export function composerDraftId(target: ComposerDraftTarget): string {
  return target.type === 'newSession'
    ? `new:${target.projectId}`
    : `session:${target.sessionId}`
}

export function composerDraftFor(
  drafts: ComposerDrafts,
  target: ComposerDraftTarget,
): ComposerDraft {
  const draft = target.type === 'newSession'
    ? drafts.new_sessions?.[target.projectId]
    : drafts.sessions?.[target.sessionId]
  return {
    text: draft?.text ?? '',
    attachments: draft?.attachments ?? [],
    embedded_notes: draft?.embedded_notes ?? [],
  }
}

export function composerDraftChanges(
  drafts: ComposerDrafts,
  tombstones: ComposerDraftTarget[] = [],
): ComposerDraftChange[] {
  const changes: ComposerDraftChange[] = []
  const included = new Set<string>()
  for (const [projectId, draft] of Object.entries(drafts.new_sessions ?? {})) {
    if (!draftEmpty(draft)) {
      const target: ComposerDraftTarget = { type: 'newSession', projectId }
      changes.push({
        target,
        draft: normalizeDraft(draft),
      })
      included.add(composerDraftId(target))
    }
  }
  for (const [sessionId, draft] of Object.entries(drafts.sessions ?? {})) {
    if (!draftEmpty(draft)) {
      const target: ComposerDraftTarget = { type: 'session', sessionId }
      changes.push({
        target,
        draft: normalizeDraft(draft),
      })
      included.add(composerDraftId(target))
    }
  }
  for (const target of tombstones) {
    const id = composerDraftId(target)
    if (!included.has(id)) {
      changes.push({ target, draft: null })
      included.add(id)
    }
  }
  return changes
}

export async function flushComposerDraftSnapshot(
  readDrafts: () => ComposerDrafts,
  readTombstones: () => ComposerDraftTarget[],
  readGeneration: () => number,
  waitForPendingWrite: () => Promise<void>,
  write: (changes: ComposerDraftChange[]) => Promise<void>,
): Promise<void> {
  while (true) {
    const generation = readGeneration()
    await waitForPendingWrite()
    const changes = composerDraftChanges(readDrafts(), readTombstones())
    if (changes.length) await write(changes)
    if (readGeneration() === generation) return
  }
}

export async function waitForComposerDraftSource(
  readSource: () => string | null,
  expected: string,
  wait: () => Promise<void>,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (readSource() !== expected) {
    if (Date.now() >= deadline) {
      throw new Error(`draft store for ${expected} did not hydrate before the host switch`)
    }
    await wait()
  }
}

export function setComposerDraft(
  drafts: ComposerDrafts,
  target: ComposerDraftTarget,
  draft: ComposerDraft,
): ComposerDraftChange | null {
  const normalized = normalizeDraft(draft)
  const current = composerDraftFor(drafts, target)
  if (draftsEqual(current, normalized)) return null
  const entries = target.type === 'newSession'
    ? (drafts.new_sessions ??= {})
    : (drafts.sessions ??= {})
  const id = target.type === 'newSession' ? target.projectId : target.sessionId
  if (draftEmpty(normalized)) delete entries[id]
  else entries[id] = normalized
  return { target, draft: draftEmpty(normalized) ? null : normalized }
}

export function moveComposerDraftToEmpty(
  drafts: ComposerDrafts,
  source: ComposerDraftTarget,
  destination: ComposerDraftTarget,
): ComposerDraftChange[] {
  if (composerDraftId(source) === composerDraftId(destination)) return []
  const sourceDraft = composerDraftFor(drafts, source)
  if (draftEmpty(sourceDraft) || !draftEmpty(composerDraftFor(drafts, destination))) return []
  const changes: ComposerDraftChange[] = []
  const destinationChange = setComposerDraft(drafts, destination, sourceDraft)
  const sourceChange = setComposerDraft(drafts, source, { text: '', attachments: [] })
  if (destinationChange) changes.push(destinationChange)
  if (sourceChange) changes.push(sourceChange)
  return changes
}

function normalizeDraft(draft: ComposerDraft): ComposerDraft {
  return {
    text: draft.text ?? '',
    attachments: draft.attachments ?? [],
    embedded_notes: draft.embedded_notes ?? [],
  }
}

function draftEmpty(draft: ComposerDraft): boolean {
  return !(draft.text?.length || draft.attachments?.length || draft.embedded_notes?.length)
}

function draftsEqual(left: ComposerDraft, right: ComposerDraft): boolean {
  return left.text === right.text
    && JSON.stringify(left.attachments) === JSON.stringify(right.attachments)
    && JSON.stringify(left.embedded_notes) === JSON.stringify(right.embedded_notes)
}
