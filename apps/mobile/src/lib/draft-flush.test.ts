import { describe, expect, test } from "bun:test";

import {
  createBeforeDisconnectCoordinator,
  createDraftFlushScheduler,
  flushBeforeUnregister,
} from "./draft-flush";

describe("mobile composer draft flush", () => {
  test("a host switch flushes immediately before the debounce expires", async () => {
    const scheduler = createDraftFlushScheduler(60_000);
    const switchCoordinator = createBeforeDisconnectCoordinator<string>();
    let saves = 0;

    scheduler.schedule("draft", async () => {
      saves += 1;
    });
    switchCoordinator.register("host", "draft", () => scheduler.flush());
    await switchCoordinator.flush("host");

    expect(saves).toBe(1);
  });

  test("a forced flush waits for an older write before saving the newest draft", async () => {
    const scheduler = createDraftFlushScheduler(60_000);
    const order: string[] = [];
    let releaseFirst!: () => void;
    const firstBlocked = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });

    scheduler.schedule("draft", async () => {
      order.push("older-start");
      await firstBlocked;
      order.push("older-end");
    });
    const flushingOlder = scheduler.flush();
    scheduler.schedule("draft", async () => {
      order.push("newest");
    });
    const flushingNewest = scheduler.flush();
    releaseFirst();
    await Promise.all([flushingOlder, flushingNewest]);

    expect(order).toEqual(["older-start", "older-end", "newest"]);
  });

  test("a newer project target does not cancel the previous target's pending save", async () => {
    const scheduler = createDraftFlushScheduler(60_000);
    const order: string[] = [];

    scheduler.schedule("profile:project-a", async () => {
      order.push("project-a");
    });
    scheduler.schedule("profile:project-b", async () => {
      order.push("project-b");
    });
    await scheduler.flush();

    expect(order).toEqual(["project-a", "project-b"]);
  });

  test("one coordinator flush drains a newer draft scheduled during the first write", async () => {
    const scheduler = createDraftFlushScheduler(60_000);
    const switchCoordinator = createBeforeDisconnectCoordinator<string>();
    const order: string[] = [];
    let releaseFirst!: () => void;
    const firstBlocked = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });

    scheduler.schedule("draft", async () => {
      order.push("older-start");
      await firstBlocked;
      order.push("older-end");
    });
    switchCoordinator.register("host", "draft", () => scheduler.flush());
    const switching = switchCoordinator.flush("host");
    await Bun.sleep(0);
    scheduler.schedule("draft", async () => {
      order.push("newest");
    });
    releaseFirst();
    await switching;

    expect(order).toEqual(["older-start", "older-end", "newest"]);
  });

  test("unmount keeps the registration until its pending flush settles", async () => {
    const scheduler = createDraftFlushScheduler(60_000);
    const switchCoordinator = createBeforeDisconnectCoordinator<string>();
    let saves = 0;
    scheduler.schedule("draft", async () => {
      saves += 1;
    });
    const unregister = switchCoordinator.register("host", "draft", () => scheduler.flush());

    // Simulate a screen unmount immediately before the debounce.
    void flushBeforeUnregister(() => scheduler.flush(), unregister);
    await switchCoordinator.flush("host");

    expect(saves).toBe(1);
  });

  test("a failed unmount flush stays registered so a later host switch retries", async () => {
    const switchCoordinator = createBeforeDisconnectCoordinator<string>();
    const failure = new Error("draft write failed");
    const unregister = switchCoordinator.register("host", "draft", async () => {
      throw failure;
    });

    await expect(
      flushBeforeUnregister(async () => {
        throw failure;
      }, unregister),
    ).rejects.toBe(failure);
    await expect(switchCoordinator.flush("host")).rejects.toBe(failure);
  });

  test("a successful retry retires an unmounted callback", async () => {
    const switchCoordinator = createBeforeDisconnectCoordinator<string>();
    let fail = true;
    let calls = 0;
    const failure = new Error("draft write failed");
    const unregister = switchCoordinator.register("host", "draft", async () => {
      calls += 1;
      if (fail) throw failure;
    });

    await expect(
      flushBeforeUnregister(async () => {
        calls += 1;
        throw failure;
      }, unregister),
    ).rejects.toBe(failure);

    fail = false;
    await switchCoordinator.flush("host");
    switchCoordinator.retire("host");
    await switchCoordinator.flush("host");

    expect(calls).toBe(2);
  });

  test("remounting one logical draft replaces the stale callback", async () => {
    const switchCoordinator = createBeforeDisconnectCoordinator<string>();
    let staleCalls = 0;
    let freshCalls = 0;
    switchCoordinator.register("host", "session:task", async () => {
      staleCalls += 1;
    });
    switchCoordinator.register("host", "session:task", async () => {
      freshCalls += 1;
    });

    await switchCoordinator.flush("host");

    expect(staleCalls).toBe(0);
    expect(freshCalls).toBe(1);
  });

  test("a failed debounced write still blocks a host switch", async () => {
    const scheduler = createDraftFlushScheduler(0);
    const failure = new Error("daemon rejected the draft");
    scheduler.schedule("draft", async () => {
      throw failure;
    });
    await Bun.sleep(1);

    await expect(scheduler.flush()).rejects.toBe(failure);
  });

  test("discarding an accepted draft clears its old failure", async () => {
    const scheduler = createDraftFlushScheduler(0);
    scheduler.schedule("draft", async () => {
      throw new Error("old write failed");
    });
    await Bun.sleep(1);
    await expect(scheduler.flush()).rejects.toThrow("old write failed");

    scheduler.discard();
    await expect(scheduler.flush()).resolves.toBeUndefined();
  });

  test("discarding one target preserves another host's pending draft", async () => {
    const scheduler = createDraftFlushScheduler(60_000);
    const saved: string[] = [];

    scheduler.schedule("host-a:project", async () => {
      saved.push("host-a");
    });
    await scheduler.flush();
    scheduler.schedule("host-b:project", async () => {
      saved.push("host-b");
    });
    scheduler.discard("host-a:project");
    await scheduler.flush();

    expect(saved).toEqual(["host-a", "host-b"]);
  });

  test("a successful save cannot hide another target's failed save", async () => {
    const scheduler = createDraftFlushScheduler(60_000);

    scheduler.schedule("host-a:project", async () => {
      throw new Error("host A draft failed");
    });
    await expect(scheduler.flush()).rejects.toThrow("host A draft failed");

    scheduler.schedule("host-b:project", async () => {});
    await expect(scheduler.flush()).rejects.toThrow("host A draft failed");

    scheduler.discard("host-a:project");
    await expect(scheduler.flush()).resolves.toBeUndefined();
  });
});
