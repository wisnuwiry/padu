import { describe, expect, test } from "bun:test";

import {
  claimNewTaskSubmission,
  hasSameNewTaskDraftTarget,
  isNewTaskDraftReady,
  newTaskDraftScopeKey,
  planNewTaskDraftTransition,
  saveNewTaskDraftOrThrow,
} from "./new-task-draft";

describe("new task draft target scoping", () => {
  test("a project switch saves the outgoing prompt and hydrates the destination", () => {
    const plan = planNewTaskDraftTransition({
      previous: {
        profileId: "host-a",
        projectId: "project-a",
        prompt: "  keep project A  ",
      },
      nextProfileId: "host-a",
      sameClient: true,
      destinationText: "project B draft",
    });

    expect(plan.previousDraft).toEqual({
      projectId: "project-a",
      text: "keep project A",
    });
    expect(plan.destinationText).toBe("project B draft");
  });

  test("a host switch never carries the old prompt onto the destination host", () => {
    const plan = planNewTaskDraftTransition({
      previous: {
        profileId: "host-a",
        projectId: "project-a",
        prompt: "do not send this to host B",
      },
      nextProfileId: "host-b",
      sameClient: false,
      destinationText: "host B persisted draft",
    });

    expect(plan.previousDraft).toBeNull();
    expect(plan.destinationText).toBe("host B persisted draft");
  });

  test("scope keys include both host and project", () => {
    expect(newTaskDraftScopeKey("host-a", "project-one"))
      .toBe("host-a:project-one");
    expect(newTaskDraftScopeKey("host-b", "project-one"))
      .toBe("host-b:project-one");
    expect(newTaskDraftScopeKey(null, "project-one")).toBeNull();
  });

  test("readiness includes the client identity, not only host and project", () => {
    const firstClient = {};
    const replacementClient = {};
    const readiness = { key: "host-a:project-one", client: firstClient };

    expect(isNewTaskDraftReady(readiness, "host-a:project-one", firstClient))
      .toBe(true);
    expect(isNewTaskDraftReady(readiness, "host-a:project-one", replacementClient))
      .toBe(false);
  });

  test("returning to the current target can restore readiness without reloading", () => {
    const client = {};
    const currentTarget = { key: "host-a:project-a", client };

    expect(hasSameNewTaskDraftTarget(currentTarget, "host-a:project-a", client))
      .toBe(true);
    expect(hasSameNewTaskDraftTarget(currentTarget, "host-a:project-b", client))
      .toBe(false);
  });

  test("two synchronous Start calls claim only one submission slot", () => {
    const guard = { current: false };

    expect(claimNewTaskSubmission(guard)).toBe(true);
    expect(claimNewTaskSubmission(guard)).toBe(false);
    expect(guard.current).toBe(true);
  });

  test("a failed draft save prevents the provider start", async () => {
    let providerStarted = false;

    await expect((async () => {
      await saveNewTaskDraftOrThrow(async () => {
        throw new Error("daemon write failed");
      });
      providerStarted = true;
    })()).rejects.toThrow("Couldn’t save your draft before starting");
    expect(providerStarted).toBe(false);
  });
});
