export interface NewTaskDraftPrevious {
  profileId: string;
  projectId: string;
  prompt: string;
}

export interface NewTaskDraftTransitionPlan {
  previousDraft: {
    projectId: string;
    text: string;
  } | null;
  destinationText: string | null;
}

export interface NewTaskDraftReadiness<Client> {
  key: string;
  client: Client;
}

export interface NewTaskSubmissionGuard {
  current: boolean;
}

export function claimNewTaskSubmission(
  guard: NewTaskSubmissionGuard,
): boolean {
  if (guard.current) return false;
  guard.current = true;
  return true;
}

export async function saveNewTaskDraftOrThrow(
  save: () => Promise<void>,
): Promise<void> {
  try {
    await save();
  } catch (cause) {
    throw new Error(
      `Couldn’t save your draft before starting: ${
        cause instanceof Error ? cause.message : String(cause)
      }`,
    );
  }
}

export function newTaskDraftScopeKey(
  profileId: string | null,
  projectId: string | null,
): string | null {
  if (!profileId || !projectId) return null;
  return `${profileId}:${projectId}`;
}

export function isNewTaskDraftReady<Client>(
  readiness: NewTaskDraftReadiness<Client> | null,
  key: string | null,
  client: Client | null,
): boolean {
  return Boolean(key && readiness?.key === key && readiness.client === client);
}

export function hasSameNewTaskDraftTarget<Client>(
  target: NewTaskDraftReadiness<Client> | null,
  key: string,
  client: Client,
): boolean {
  return target?.key === key && target.client === client;
}

/**
 * Decide which draft belongs to the outgoing target and which text may be
 * hydrated for the incoming one. A host change never carries the old prompt
 * onto the new host; that host's persisted draft wins.
 */
export function planNewTaskDraftTransition(input: {
  previous: NewTaskDraftPrevious | null;
  nextProfileId: string;
  sameClient: boolean;
  destinationText: string | null | undefined;
}): NewTaskDraftTransitionPlan {
  const previousText = input.previous?.prompt.trim() ?? "";
  return {
    previousDraft: input.previous
      && input.sameClient
      && input.previous.profileId === input.nextProfileId
      && previousText
      ? {
          projectId: input.previous.projectId,
          text: previousText,
        }
      : null,
    destinationText: input.destinationText?.length
      ? input.destinationText
      : null,
  };
}
