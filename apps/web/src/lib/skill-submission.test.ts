import { describe, expect, test } from 'bun:test'
import type { AgentSession, EmbeddedNote, MessageAttachment } from '@padu/client'
import type { SkillBinaryIdentity } from '@padu/client'
import {
  SKILL_CAPABILITY_PROBE_VERSION,
  SKILL_CATALOG_UNAVAILABLE_WARNING,
  type ProviderSkillCapability,
  type SkillCommandCatalog,
} from '@padu/client/composer-skills'
import { beginTurn } from './daemon-api'
import { queueSubmission } from './runtime-submissions'
import {
  resolveWebProviderPrompt,
  resolveWebProviderPromptInStableContext,
  WebPromptContextChangedError,
  type WebPromptResolutionContext,
} from './skill-submission'

const PATH = '/home/remote/.agents/skills/deploy/SKILL.md'
const VISIBLE_PROMPT = '$deploy explain this'
const IDENTITY: SkillBinaryIdentity = {
  canonicalPath: '/opt/codex/bin/codex',
  contentSha256: 'a'.repeat(64),
}
const OTHER_IDENTITY: SkillBinaryIdentity = {
  canonicalPath: '/opt/codex/bin/codex',
  contentSha256: 'b'.repeat(64),
}
const ATTACHMENTS: MessageAttachment[] = [{
  path: '/workspace/src/file.txt',
  mention: 'src/file.txt',
  name: 'file.txt',
  is_dir: false,
  is_image: false,
}]
const EMBEDDED_NOTE: EmbeddedNote = {
  id: 'note-1',
  title: 'Deployment checklist',
  content: 'Check the canary before promoting the release.',
  revision: 3,
}
const capability: ProviderSkillCapability = {
  nativeSyntax: 'dollar',
  leadingInvocation: true,
  inlineMentions: true,
  repeatedMentions: true,
}

describe('web skill submission', () => {
  test('cached refetch failures leave a known mention unresolved with a warning', async () => {
    const staleCatalog = catalog('deploy', capability)
    const result = await resolveWebProviderPrompt({
      visiblePrompt: '$deploy',
      provider: 'codex',
      catalog: staleCatalog,
      binaryVersion: '1.2.3',
      binaryIdentity: IDENTITY,
      sessionCommands: [],
      attachments: [],
      embeddedNotes: [],
      refetchBinary: async () => ({
        isError: true,
        data: { version: '1.2.3', binaryIdentity: IDENTITY },
      }),
      refetchCatalog: async () => ({
        isError: true,
        data: staleCatalog,
      }),
    })

    expect(result.providerPrompt).toBeUndefined()
    expect(result.warnings).toEqual([SKILL_CATALOG_UNAVAILABLE_WARNING])
  })

  test('idle rewind sends fresh provider text and persists visible text plus notes', async () => {
    const result = await rewindPromptResult()
    const session = rewindSession('idle')
    const started = beginTurn(
      session,
      VISIBLE_PROMPT,
      ATTACHMENTS,
      [EMBEDDED_NOTE],
      result.providerPrompt,
    )

    expect(result.providerPrompt).toContain('[mandatory! open skill: ')
    expect(result.providerPrompt).toContain('src/file.txt')
    expect(result.providerPrompt).toContain(EMBEDDED_NOTE.content)
    expect(started.messages.at(-1)).toMatchObject({
      content: result.providerPrompt!,
      display_content: VISIBLE_PROMPT,
      attachments: ATTACHMENTS,
      embedded_notes: [EMBEDDED_NOTE],
    })
  })

  test('busy rewind queues provider text while restoring visible text and notes', async () => {
    const result = await rewindPromptResult()
    const session = rewindSession('working')
    const queued = queueSubmission(
      session,
      VISIBLE_PROMPT,
      result.providerPrompt!,
      ATTACHMENTS,
      [EMBEDDED_NOTE],
    )
    const message = queued.queued_messages?.at(-1)

    expect(queued.status).toBe('working')
    expect(result.providerPrompt).toContain(EMBEDDED_NOTE.content)
    expect(message).toMatchObject({
      content: result.providerPrompt!,
      display_content: VISIBLE_PROMPT,
      attachments: ATTACHMENTS,
      embedded_notes: [EMBEDDED_NOTE],
    })
  })

  test('native evidence requires the exact current binary identity', async () => {
    const result = await resolveWebProviderPrompt({
      visiblePrompt: '$deploy',
      provider: 'codex',
      catalog: catalog('deploy', capability),
      binaryVersion: '1.2.3',
      binaryIdentity: OTHER_IDENTITY,
      sessionCommands: [],
      attachments: [],
      embeddedNotes: [],
      refetchBinary: async () => ({
        isError: false,
        data: { version: '1.2.3', binaryIdentity: OTHER_IDENTITY },
      }),
      refetchCatalog: async () => ({
        isError: false,
        data: catalog('deploy', capability),
      }),
    })

    expect(result.providerPrompt).toBe(`[mandatory! open skill: ${PATH}]`)
  })

  test('rejects a probe result when task context changes during resolution', async () => {
    const initial = promptContext()
    let current = initial

    await expect(resolveWebProviderPromptInStableContext(
      initial,
      () => current,
      async () => {
        current = {
          ...current,
          sessionId: 'other-session',
          provider: 'claude',
          projectId: 'other-project',
          projectRoot: '/workspace/other',
          hostAddress: 'wss://other.example',
        }
        return {
          providerPrompt: `[mandatory! open skill: ${PATH}] explain this`,
          warnings: [],
        }
      },
    )).rejects.toBeInstanceOf(WebPromptContextChangedError)
  })

  test('rejects a probe result when binary identity changes during resolution', async () => {
    const initial = promptContext()
    let current = initial

    await expect(resolveWebProviderPromptInStableContext(
      initial,
      () => current,
      async () => {
        current = { ...current, binaryIdentity: OTHER_IDENTITY }
        return { providerPrompt: '$deploy', warnings: [] }
      },
    )).rejects.toBeInstanceOf(WebPromptContextChangedError)
  })

  test('rejects a probe result when model selection changes during resolution', async () => {
    const initial = promptContext()
    let current = initial

    await expect(resolveWebProviderPromptInStableContext(
      initial,
      () => current,
      async () => {
        current = { ...current, model: 'other-model' }
        return { providerPrompt: '$deploy', warnings: [] }
      },
    )).rejects.toBeInstanceOf(WebPromptContextChangedError)
  })
})

async function rewindPromptResult() {
  return resolveWebProviderPrompt({
    visiblePrompt: VISIBLE_PROMPT,
    provider: 'codex',
    catalog: catalog('deploy'),
    binaryVersion: '1.2.3',
    binaryIdentity: IDENTITY,
    sessionCommands: [],
    attachments: ATTACHMENTS,
    embeddedNotes: [EMBEDDED_NOTE],
    refetchBinary: async () => ({
      isError: false,
      data: { version: '1.2.4', binaryIdentity: IDENTITY },
    }),
    refetchCatalog: async () => ({
      isError: false,
      data: catalog('deploy'),
    }),
  })
}

function promptContext(): WebPromptResolutionContext {
  return {
    sessionId: 'session',
    provider: 'codex',
    projectId: 'project',
    projectRoot: '/workspace/padu',
    binaryOverride: null,
    binaryVersion: '1.2.3',
    binaryIdentity: IDENTITY,
    model: 'gpt-5-codex',
    hostAddress: 'wss://daemon.example',
  }
}

function catalog(
  name: string,
  nativeCapability?: ProviderSkillCapability,
): SkillCommandCatalog {
  return {
    commands: [{
      name,
      description: '',
      scope: 'Skill',
      argument_hint: null,
      template: null,
      skill_file: PATH,
    }],
    skill_capability: nativeCapability
      ? {
          binaryVersion: '1.2.3',
          binaryIdentity: IDENTITY,
          probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
          capability: nativeCapability,
        }
      : undefined,
  }
}

function rewindSession(status: AgentSession['status']): AgentSession {
  return {
    id: 'session',
    title: 'New task',
    auto_title: null,
    project_id: 'project',
    workspace: { kind: 'local' },
    provider: 'codex',
    runtime_mode: 'fullAccess',
    interaction_mode: 'build',
    status,
    created_at: 1,
    updated_at: 1,
    last_reply_at: null,
    provider_cursor: null,
    messages: [],
    transcript_blocks: [],
    turns: [],
  }
}
