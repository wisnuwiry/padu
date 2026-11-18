import type {
  EmbeddedNote,
  MessageAttachment,
  ProviderKind,
  ReportedCommand,
  SkillBinaryIdentity,
} from '@padu/client'
import {
  catalogSkillCapability,
  findSkillMentions,
  refetchedBinaryIdentity,
  refetchedSkillCatalog,
  resolveFreshSkillCatalog,
  sameSkillBinaryIdentity,
  skillSubmissionWarnings,
  type SkillCommandCatalog,
  type SkillRefetchResult,
} from '@padu/client/composer-skills'
import { useCallback, useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { markdownFileReference } from '@/lib/inline-file-references'
import {
  mergeComposerCommands,
  renderedComposerSubmission,
} from '@/lib/composer-autocomplete'

export interface WebSkillCommandQuery {
  data: SkillCommandCatalog | undefined
  binaryVersion: string | null
  binaryIdentity: SkillBinaryIdentity | null
  refetchBinary: () => Promise<
    SkillRefetchResult<{
      version?: string | null
      binaryIdentity?: SkillBinaryIdentity | null
    } | null>
  >
  refetch: () => Promise<SkillRefetchResult<SkillCommandCatalog>>
}

export interface WebProviderPromptInput {
  visiblePrompt: string
  provider: ProviderKind
  catalog: SkillCommandCatalog | undefined
  binaryVersion: string | null
  binaryIdentity: SkillBinaryIdentity | null
  sessionCommands: readonly ReportedCommand[]
  attachments: readonly MessageAttachment[]
  embeddedNotes: readonly EmbeddedNote[]
  refetchBinary: WebSkillCommandQuery['refetchBinary']
  refetchCatalog: WebSkillCommandQuery['refetch']
}

export interface WebProviderPromptResult {
  providerPrompt: string | undefined
  warnings: string[]
}

export interface WebPromptResolutionContext {
  sessionId: string
  provider: ProviderKind
  projectId: string
  projectRoot: string
  binaryOverride: string | null
  binaryVersion: string | null
  binaryIdentity: SkillBinaryIdentity | null
  model: string | null
  hostAddress: string | null
}

export interface WebResolvedProviderPrompt extends WebProviderPromptResult {
  context: WebPromptResolutionContext
  assertCurrent: () => void
}

export class WebPromptContextChangedError extends Error {
  constructor() {
    super('The task context changed while resolving skills. Submit again to use the new context.')
    this.name = 'WebPromptContextChangedError'
  }
}

export function sameWebPromptResolutionContext(
  left: WebPromptResolutionContext,
  right: WebPromptResolutionContext,
): boolean {
  return left.sessionId === right.sessionId
    && left.provider === right.provider
    && left.projectId === right.projectId
    && left.projectRoot === right.projectRoot
    && left.binaryOverride === right.binaryOverride
    && left.binaryVersion === right.binaryVersion
    && sameSkillBinaryIdentity(left.binaryIdentity, right.binaryIdentity)
    && left.model === right.model
    && left.hostAddress === right.hostAddress
}

export function assertWebPromptResolutionContext(
  expected: WebPromptResolutionContext,
  current: WebPromptResolutionContext,
): void {
  if (!sameWebPromptResolutionContext(expected, current)) {
    throw new WebPromptContextChangedError()
  }
}

/**
 * The probe and catalog refetch may take long enough for the selected task,
 * provider, project, binary, or host to change. Do not return a rendered
 * prompt from that old snapshot; callers keep the draft recoverable and can
 * submit again against the new context.
 */
export async function resolveWebProviderPromptInStableContext(
  context: WebPromptResolutionContext,
  currentContext: () => WebPromptResolutionContext,
  resolve: () => Promise<WebProviderPromptResult>,
): Promise<WebProviderPromptResult> {
  const result = await resolve()
  assertWebPromptResolutionContext(context, currentContext())
  return result
}

export async function resolveWebProviderPrompt({
  visiblePrompt,
  provider,
  catalog: initialCatalog,
  binaryVersion: initialBinaryVersion,
  binaryIdentity: initialBinaryIdentity,
  sessionCommands,
  attachments,
  embeddedNotes,
  refetchBinary,
  refetchCatalog,
}: WebProviderPromptInput): Promise<WebProviderPromptResult> {
  const submittedPrompt = visiblePrompt.trim()
  const hasSkillMentions = findSkillMentions(submittedPrompt).length > 0
  let catalog = initialCatalog
  let binaryVersion = initialBinaryVersion
  let binaryIdentity = initialBinaryIdentity
  let catalogUnavailable = false
  if (hasSkillMentions) {
    const fresh = await resolveFreshSkillCatalog(
      { catalog, binaryVersion, binaryIdentity },
      true,
      async () => refetchedBinaryIdentity(await refetchBinary()),
      async () => refetchedSkillCatalog(await refetchCatalog()),
    )
    catalog = fresh.catalog
    binaryVersion = fresh.binaryVersion
    binaryIdentity = fresh.binaryIdentity
    catalogUnavailable = fresh.catalogUnavailable
  }

  const commands = mergeComposerCommands(
    catalog?.commands ?? [],
    [...sessionCommands],
  )
  const rendered = renderedComposerSubmission(
    provider,
    submittedPrompt,
    commands,
    catalogSkillCapability(catalog, provider, binaryVersion, binaryIdentity),
  )
  const warnings = skillSubmissionWarnings(
    submittedPrompt,
    rendered.text,
    rendered.warnings,
    hasSkillMentions && catalogUnavailable,
  )
  const providerPrompt = rendered.text === submittedPrompt
    && attachments.length === 0
    && embeddedNotes.length === 0
    ? undefined
    : [
        rendered.text,
        attachments
          .map((attachment) => markdownFileReference(attachment.mention))
          .join(' '),
        ...embeddedNotes.map((note) => `${note.title || 'Untitled note'}\n${note.content}`),
      ].filter(Boolean).join('\n\n')

  return { providerPrompt, warnings }
}

export function useWebProviderPromptResolver(
  query: WebSkillCommandQuery,
  provider: ProviderKind,
  sessionCommands: readonly ReportedCommand[],
  context: WebPromptResolutionContext,
) {
  const latest = useRef({ query, provider, sessionCommands, context })
  const mounted = useRef(true)
  latest.current = { query, provider, sessionCommands, context }
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  return useCallback(async (
    visiblePrompt: string,
    attachments: readonly MessageAttachment[] = [],
    embeddedNotes: readonly EmbeddedNote[] = [],
  ): Promise<WebResolvedProviderPrompt> => {
    const submitted = latest.current
    const resolved = await resolveWebProviderPromptInStableContext(
      submitted.context,
      () => latest.current.context,
      () => resolveWebProviderPrompt({
        visiblePrompt,
        provider: submitted.provider,
        catalog: submitted.query.data,
        binaryVersion: submitted.query.binaryVersion,
        binaryIdentity: submitted.query.binaryIdentity,
        sessionCommands: submitted.sessionCommands,
        attachments,
        embeddedNotes,
        refetchBinary: submitted.query.refetchBinary,
        refetchCatalog: submitted.query.refetch,
      }),
    )
    for (const warning of new Set(resolved.warnings)) toast.warning(warning)
    return {
      ...resolved,
      context: submitted.context,
      assertCurrent: () => {
        if (!mounted.current) throw new WebPromptContextChangedError()
        assertWebPromptResolutionContext(submitted.context, latest.current.context)
      },
    }
  }, [])
}
