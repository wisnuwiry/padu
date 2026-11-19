import { describe, expect, test } from "bun:test";

import { PendingSteerQueue } from "./pending-steers";

describe("pending steer queue", () => {
  test("a failed request cannot poison the next accepted steer", () => {
    const queue = new PendingSteerQueue<{ visible: string }>();
    const failed = queue.enqueue("session", "provider failed", { visible: "failed" });
    queue.enqueue("session", "provider accepted", { visible: "accepted" });

    expect(queue.remove("session", failed)).toBe(true);
    expect(queue.take("session", "provider accepted")?.value).toEqual({ visible: "accepted" });
    expect(queue.take("session", "provider accepted")).toBeUndefined();
  });

  test("events consume the matching provider prompt rather than the oldest stale item", () => {
    const queue = new PendingSteerQueue<{ visible: string }>();
    queue.enqueue("session", "first", { visible: "first" });
    queue.enqueue("session", "second", { visible: "second" });

    expect(queue.take("session", "second")?.value).toEqual({ visible: "second" });
    expect(queue.take("session", "first")?.value).toEqual({ visible: "first" });
  });
});
