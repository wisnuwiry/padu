import AsyncStorage from "@react-native-async-storage/async-storage";

const STORAGE_KEY = "padu.mobile.pending-draft-cleanup.v1";

export type PendingNewTaskPhase = "submitting" | "cleanup";

export interface PendingDraftCleanup {
  profileId: string;
  profileName: string;
  projectId: string;
  sessionId: string;
  phase: PendingNewTaskPhase;
  message: string;
}

export type PendingSubmissionRecovery = "restoreDraft" | "openAcceptedTask";

export interface PendingSubmissionSession {
  status: "idle" | "connecting" | "working" | "waiting" | "failed";
  turns: Array<{
    provider_turn_started: boolean;
  }>;
}

interface PendingStorage {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
}

export function upsertPendingDraftCleanup(
  jobs: readonly PendingDraftCleanup[],
  job: PendingDraftCleanup,
): PendingDraftCleanup[] {
  return [
    ...jobs.filter((current) => current.profileId !== job.profileId),
    job,
  ];
}

export function removePendingDraftCleanup(
  jobs: readonly PendingDraftCleanup[],
  job: Pick<PendingDraftCleanup, "profileId">,
): PendingDraftCleanup[] {
  return jobs.filter((current) => current.profileId !== job.profileId);
}

/**
 * A persisted task can exist before its provider accepted the prompt. Only a
 * live session means submission crossed that boundary; idle or failed
 * sessions belong back in the recoverable composer draft.
 */
export function classifyPendingSubmission(
  session: PendingSubmissionSession | null,
): PendingSubmissionRecovery {
  if (!session) return "restoreDraft";
  if (session.turns.some((turn) => turn.provider_turn_started)) {
    return "openAcceptedTask";
  }
  return session.status === "connecting"
      || session.status === "working"
      || session.status === "waiting"
    ? "openAcceptedTask"
    : "restoreDraft";
}

function isPendingDraftCleanup(value: unknown): value is PendingDraftCleanup {
  return (
    typeof value === "object"
    && value !== null
    && "profileId" in value
    && typeof value.profileId === "string"
    && "profileName" in value
    && typeof value.profileName === "string"
    && "projectId" in value
    && typeof value.projectId === "string"
    && "sessionId" in value
    && typeof value.sessionId === "string"
    && "phase" in value
    && (value.phase === "submitting" || value.phase === "cleanup")
    && "message" in value
    && typeof value.message === "string"
  );
}

function parseJobs(raw: string | null): PendingDraftCleanup[] {
  if (raw === null) return [];
  const parsed: unknown = JSON.parse(raw);
  if (!Array.isArray(parsed)) {
    throw new Error("Pending draft cleanup storage is not a JSON array");
  }
  if (!parsed.every(isPendingDraftCleanup)) {
    throw new Error("Pending draft cleanup storage contains an invalid job");
  }
  return parsed;
}

export function createPendingDraftCleanupStore(storage: PendingStorage) {
  let mutationTail = Promise.resolve();

  function serialize<T>(operation: () => Promise<T>): Promise<T> {
    const next = mutationTail.then(operation, operation);
    mutationTail = next.then(
      () => undefined,
      () => undefined,
    );
    return next;
  }

  async function readJobs(): Promise<PendingDraftCleanup[]> {
    return parseJobs(await storage.getItem(STORAGE_KEY));
  }

  async function writeJobs(jobs: PendingDraftCleanup[]): Promise<void> {
    await storage.setItem(STORAGE_KEY, JSON.stringify(jobs));
  }

  return {
    load(): Promise<PendingDraftCleanup[]> {
      return serialize(readJobs);
    },

    persist(job: PendingDraftCleanup): Promise<void> {
      return serialize(async () => {
        await writeJobs(upsertPendingDraftCleanup(await readJobs(), job));
      });
    },

    clear(job: Pick<PendingDraftCleanup, "profileId">): Promise<void> {
      return serialize(async () => {
        await writeJobs(removePendingDraftCleanup(await readJobs(), job));
      });
    },

    reset(): Promise<void> {
      return serialize(async () => {
        await writeJobs([]);
      });
    },
  };
}

const store = createPendingDraftCleanupStore(AsyncStorage);

export function loadPendingDraftCleanups(): Promise<PendingDraftCleanup[]> {
  return store.load();
}

export function persistPendingDraftCleanup(
  job: PendingDraftCleanup,
): Promise<void> {
  return store.persist(job);
}

export function clearPendingDraftCleanup(
  job: Pick<PendingDraftCleanup, "profileId">,
): Promise<void> {
  return store.clear(job);
}

export function resetPendingDraftCleanups(): Promise<void> {
  return store.reset();
}
