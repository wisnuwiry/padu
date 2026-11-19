import type {
  InteractionMode,
  ProviderKind,
  RuntimeMode,
  SessionWorkspace,
  SkillBinaryIdentity,
} from '@padu/client';
import {
  catalogSkillCapability,
  findSkillMentions,
  refetchedBinaryIdentity,
  refetchedSkillCatalog,
  renderSkillMentions,
  resolveFreshSkillCatalog,
  sameSkillBinaryIdentity,
  skillSubmissionWarnings,
  SKILL_CATALOG_UNAVAILABLE_WARNING,
  type SkillCommandCatalog,
  type SkillRefetchResult,
} from '@padu/client/composer-skills';
import { useCallback, useEffect, useRef } from 'react';

export { SKILL_CATALOG_UNAVAILABLE_WARNING };

export interface SkillSubmissionInput {
  visiblePrompt: string;
  provider: ProviderKind;
  catalog: SkillCommandCatalog | undefined;
  binaryVersion: string | null;
  binaryIdentity: SkillBinaryIdentity | null;
  refetchBinary: () => Promise<SkillRefetchResult<{
    version?: string | null;
    binaryIdentity?: SkillBinaryIdentity | null;
  } | null>>;
  refetchCatalog: () => Promise<SkillRefetchResult<SkillCommandCatalog>>;
}

export interface SkillSubmissionResult {
  providerText: string;
  warnings: string[];
}

export interface MobilePromptSelectionContext {
  model: string | null;
  reasoningEffort: string | null;
  runtimeMode: RuntimeMode;
  interactionMode: InteractionMode;
  agentPreset: string | null;
  serviceTier: string | null;
  isolated: boolean;
  baseBranch: string | null;
  workspaceKind: SessionWorkspace['kind'] | null;
  workspacePath: string | null;
  workspaceBranch: string | null;
}

export interface MobilePromptResolutionContext {
  hostId: string | null;
  hostAddress: string | null;
  sessionId: string | null;
  projectId: string;
  projectRoot: string;
  provider: ProviderKind;
  binaryOverride: string | null;
  binaryVersion: string | null;
  binaryIdentity: SkillBinaryIdentity | null;
  selection: MobilePromptSelectionContext;
}

export interface StableMobilePromptResolution<T> {
  result: T;
  context: MobilePromptResolutionContext;
  assertCurrent: () => void;
}

export class MobilePromptContextChangedError extends Error {
  constructor() {
    super('The task context changed while resolving skills. Submit again to use the new context.');
    this.name = 'MobilePromptContextChangedError';
  }
}

export function mobilePromptSelectionContext(
  workspace: SessionWorkspace | undefined,
  selection: {
    model: string | null;
    reasoningEffort: string | null;
    runtimeMode: RuntimeMode;
    interactionMode: InteractionMode;
    agentPreset?: string | null;
    serviceTier?: string | null;
    isolated?: boolean;
    baseBranch?: string | null;
  },
): MobilePromptSelectionContext {
  return {
    model: selection.model,
    reasoningEffort: selection.reasoningEffort,
    runtimeMode: selection.runtimeMode,
    interactionMode: selection.interactionMode,
    agentPreset: selection.agentPreset ?? null,
    serviceTier: selection.serviceTier ?? null,
    isolated: selection.isolated ?? workspace?.kind === 'newWorktree',
    baseBranch: selection.baseBranch
      ?? (workspace?.kind === 'newWorktree' ? workspace.baseBranch ?? null : null),
    workspaceKind: workspace?.kind ?? null,
    workspacePath: workspace?.kind === 'worktree' ? workspace.path : null,
    workspaceBranch: workspace?.kind === 'worktree' ? workspace.branch : null,
  };
}

export function sameMobilePromptResolutionContext(
  left: MobilePromptResolutionContext,
  right: MobilePromptResolutionContext | null,
): boolean {
  if (!right) return false;
  return left.hostId === right.hostId
    && left.hostAddress === right.hostAddress
    && left.sessionId === right.sessionId
    && left.projectId === right.projectId
    && left.projectRoot === right.projectRoot
    && left.provider === right.provider
    && left.binaryOverride === right.binaryOverride
    && left.binaryVersion === right.binaryVersion
    && sameSkillBinaryIdentity(left.binaryIdentity, right.binaryIdentity)
    && left.selection.model === right.selection.model
    && left.selection.reasoningEffort === right.selection.reasoningEffort
    && left.selection.runtimeMode === right.selection.runtimeMode
    && left.selection.interactionMode === right.selection.interactionMode
    && left.selection.agentPreset === right.selection.agentPreset
    && left.selection.serviceTier === right.selection.serviceTier
    && left.selection.isolated === right.selection.isolated
    && left.selection.baseBranch === right.selection.baseBranch
    && left.selection.workspaceKind === right.selection.workspaceKind
    && left.selection.workspacePath === right.selection.workspacePath
    && left.selection.workspaceBranch === right.selection.workspaceBranch;
}

export function assertMobilePromptResolutionContext(
  expected: MobilePromptResolutionContext,
  current: MobilePromptResolutionContext | null,
  mounted = true,
): void {
  if (!mounted || !sameMobilePromptResolutionContext(expected, current)) {
    throw new MobilePromptContextChangedError();
  }
}

/**
 * Capture the complete submission context before refreshing the provider
 * version and catalog. The returned result is valid only while that exact
 * host/session/project/provider/binary/selection snapshot is still current.
 */
export async function resolveMobilePromptInStableContext<T>(
  context: MobilePromptResolutionContext,
  currentContext: () => MobilePromptResolutionContext | null,
  isMounted: () => boolean,
  resolve: (context: MobilePromptResolutionContext) => Promise<T>,
): Promise<StableMobilePromptResolution<T>> {
  const result = await resolve(context);
  const assertCurrent = () => {
    assertMobilePromptResolutionContext(context, currentContext(), isMounted());
  };
  assertCurrent();
  return { result, context, assertCurrent };
}

/**
 * React wrapper that tracks the latest context and unmount state. Each call
 * captures the context at invocation time, then validates it after the
 * version/catalog refresh and again immediately before createTask/sendPrompt.
 */
export function useMobilePromptResolutionGuard(
  context: MobilePromptResolutionContext | null,
) {
  const latest = useRef<MobilePromptResolutionContext | null>(context);
  const mounted = useRef(true);
  latest.current = context;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  return useCallback(<T>(
    resolve: (context: MobilePromptResolutionContext) => Promise<T>,
  ): Promise<StableMobilePromptResolution<T>> => {
    const captured = latest.current;
    if (!captured) return Promise.reject(new MobilePromptContextChangedError());
    return resolveMobilePromptInStableContext(
      captured,
      () => latest.current,
      () => mounted.current,
      resolve,
    );
  }, []);
}

/**
 * Render one mobile submission from a freshly resolved capability pair. Both
 * session and new-task composers use this path so neither can authorize native
 * syntax from a stale version/catalog cache.
 */
export async function renderSkillSubmissionPrompt({
  visiblePrompt,
  provider,
  catalog: initialCatalog,
  binaryVersion: initialBinaryVersion,
  binaryIdentity: initialBinaryIdentity,
  refetchBinary,
  refetchCatalog,
}: SkillSubmissionInput): Promise<SkillSubmissionResult> {
  const hasMentions = findSkillMentions(visiblePrompt).length > 0;
  if (!hasMentions) {
    return { providerText: visiblePrompt, warnings: [] };
  }

  const fresh = await resolveFreshSkillCatalog(
    {
      catalog: initialCatalog,
      binaryVersion: initialBinaryVersion,
      binaryIdentity: initialBinaryIdentity,
    },
    true,
    async () => refetchedBinaryIdentity(await refetchBinary()),
    async () => refetchedSkillCatalog(await refetchCatalog()),
  );
  const rendered = renderSkillMentions(
    visiblePrompt,
    provider,
    fresh.catalog?.commands ?? [],
    {
      capability: catalogSkillCapability(
        fresh.catalog,
        provider,
        fresh.binaryVersion,
        fresh.binaryIdentity,
      ),
    },
  );
  const warnings = skillSubmissionWarnings(
    visiblePrompt,
    rendered.providerText,
    rendered.warnings,
    fresh.catalogUnavailable,
  );
  return { providerText: rendered.providerText, warnings };
}
