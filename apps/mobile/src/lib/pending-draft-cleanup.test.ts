import { describe, expect, test } from "bun:test";

import {
  classifyPendingSubmission,
  createPendingDraftCleanupStore,
  removePendingDraftCleanup,
  upsertPendingDraftCleanup,
  type PendingDraftCleanup,
} from "./pending-draft-cleanup";

const job: PendingDraftCleanup = {
  profileId: "host-a",
  profileName: "Build box",
  projectId: "project-a",
  sessionId: "session-a",
  phase: "cleanup",
  message: "cleanup failed",
};

describe("pending draft cleanup jobs", () => {
  test("upsert keeps at most one cleanup job per host", () => {
    const next = upsertPendingDraftCleanup([job], {
      ...job,
      projectId: "project-b",
      sessionId: "session-b",
      message: "retry failed",
    });
    expect(next).toEqual([{
      ...job,
      projectId: "project-b",
      sessionId: "session-b",
      message: "retry failed",
    }]);
  });

  test("different hosts keep separate cleanup jobs", () => {
    const next = upsertPendingDraftCleanup([job], {
      ...job,
      profileId: "host-b",
      profileName: "Laptop",
    });
    expect(next.map((entry) => entry.profileId)).toEqual(["host-a", "host-b"]);
  });

  test("removing a job leaves other hosts untouched", () => {
    const other = { ...job, profileId: "host-b" };
    expect(removePendingDraftCleanup([job, other], job)).toEqual([other]);
  });

  test("corrupt storage rejects instead of looking like an empty queue", async () => {
    const store = createPendingDraftCleanupStore({
      getItem: async () => "{not-json",
      setItem: async () => {},
    });
    await expect(store.load()).rejects.toThrow();
  });

  test("invalid jobs reject instead of being silently dropped", async () => {
    const store = createPendingDraftCleanupStore({
      getItem: async () => JSON.stringify([{ profileId: "missing-fields" }]),
      setItem: async () => {},
    });
    await expect(store.load()).rejects.toThrow("invalid job");
  });

  test("an explicit reset makes corrupt cleanup storage usable again", async () => {
    let raw = "{not-json";
    const store = createPendingDraftCleanupStore({
      getItem: async () => raw,
      setItem: async (_key, value) => {
        raw = value;
      },
    });

    await expect(store.load()).rejects.toThrow();
    await store.reset();
    await expect(store.load()).resolves.toEqual([]);
  });

  test("concurrent host mutations are serialized without dropping jobs", async () => {
    let raw: string | null = null;
    const store = createPendingDraftCleanupStore({
      getItem: async () => {
        await Bun.sleep(0);
        return raw;
      },
      setItem: async (_key, value) => {
        await Bun.sleep(0);
        raw = value;
      },
    });

    await Promise.all([
      store.persist(job),
      store.persist({ ...job, profileId: "host-b", profileName: "Laptop" }),
    ]);

    expect((await store.load()).map((entry) => entry.profileId).sort())
      .toEqual(["host-a", "host-b"]);
  });

  test("a missing persisted session restores the composer draft", () => {
    expect(classifyPendingSubmission(null)).toBe("restoreDraft");
  });

  test("idle and failed sessions restore the composer draft", () => {
    expect(classifyPendingSubmission({
      status: "idle",
      turns: [{ provider_turn_started: false }],
    })).toBe("restoreDraft");
    expect(classifyPendingSubmission({
      status: "failed",
      turns: [{ provider_turn_started: false }],
    })).toBe("restoreDraft");
  });

  test("live sessions open without submitting the prompt twice", () => {
    expect(classifyPendingSubmission({
      status: "connecting",
      turns: [{ provider_turn_started: false }],
    }))
      .toBe("openAcceptedTask");
    expect(classifyPendingSubmission({
      status: "working",
      turns: [{ provider_turn_started: true }],
    }))
      .toBe("openAcceptedTask");
    expect(classifyPendingSubmission({
      status: "waiting",
      turns: [{ provider_turn_started: true }],
    }))
      .toBe("openAcceptedTask");
  });

  test("finished tasks with a provider-started turn stay accepted", () => {
    expect(classifyPendingSubmission({
      status: "idle",
      turns: [{ provider_turn_started: true }],
    })).toBe("openAcceptedTask");
    expect(classifyPendingSubmission({
      status: "failed",
      turns: [{ provider_turn_started: true }],
    })).toBe("openAcceptedTask");
  });
});
