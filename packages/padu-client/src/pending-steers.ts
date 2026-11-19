export interface PendingSteer<T> {
  providerPrompt: string;
  value: T;
}

/**
 * Tracks optimistic steer metadata until the provider emits its corresponding
 * accepted/rejected event. Request failures remove only the exact optimistic
 * entry they created, so a later event cannot consume stale metadata.
 */
export class PendingSteerQueue<T> {
  private readonly bySession = new Map<string, PendingSteer<T>[]>();

  enqueue(
    sessionId: string,
    providerPrompt: string,
    value: T,
  ): PendingSteer<T> {
    const pending = { providerPrompt, value };
    const queue = this.bySession.get(sessionId) ?? [];
    queue.push(pending);
    this.bySession.set(sessionId, queue);
    return pending;
  }

  remove(sessionId: string, pending: PendingSteer<T>): boolean {
    const queue = this.bySession.get(sessionId);
    if (!queue) return false;
    const index = queue.indexOf(pending);
    if (index === -1) return false;
    queue.splice(index, 1);
    if (!queue.length) this.bySession.delete(sessionId);
    return true;
  }

  take(sessionId: string, providerPrompt?: string): PendingSteer<T> | undefined {
    const queue = this.bySession.get(sessionId);
    if (!queue?.length) return undefined;
    const index = providerPrompt === undefined
      ? 0
      : queue.findIndex((pending) => pending.providerPrompt === providerPrompt);
    if (index === -1) return undefined;
    const [pending] = queue.splice(index, 1);
    if (!queue.length) this.bySession.delete(sessionId);
    return pending;
  }

  clearSession(sessionId: string): void {
    this.bySession.delete(sessionId);
  }

  clear(): void {
    this.bySession.clear();
  }
}
