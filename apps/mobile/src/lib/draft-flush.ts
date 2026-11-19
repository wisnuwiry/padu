export const COMPOSER_DRAFT_DEBOUNCE_MS = 800;

type DraftSave = () => Promise<void>;

export function createBeforeDisconnectCoordinator<Owner>() {
  const flushes = new Map<symbol, {
    owner: Owner;
    key: string;
    flush: DraftSave;
  }>();
  return {
    register(owner: Owner, logicalKey: string, flush: DraftSave) {
      for (const [id, entry] of flushes) {
        if (entry.owner === owner && entry.key === logicalKey) {
          flushes.delete(id);
        }
      }
      const id = Symbol("before-disconnect");
      flushes.set(id, { owner, key: logicalKey, flush });
      return () => {
        const current = flushes.get(id);
        if (current?.owner === owner && current.key === logicalKey) {
          flushes.delete(id);
        }
      };
    },

    async flush(owner: Owner) {
      const results = await Promise.allSettled(
        [...flushes.values()]
          .filter((entry) => entry.owner === owner)
          .map((entry) => entry.flush()),
      );
      const rejected = results.find(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      );
      if (rejected) throw rejected.reason;
    },

    retire(owner: Owner) {
      for (const [key, entry] of flushes) {
        if (entry.owner === owner) flushes.delete(key);
      }
    },
  };
}

export async function flushBeforeUnregister(
  flush: DraftSave,
  unregister: () => void,
): Promise<void> {
  await flush();
  unregister();
}

/**
 * Debounces composer writes while still allowing a host switch to force the
 * latest write immediately. Saves run one at a time so a forced flush cannot
 * race an older debounced write against the same daemon draft.
 */
export function createDraftFlushScheduler(
  delayMs = COMPOSER_DRAFT_DEBOUNCE_MS,
) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending: { key: string; save: DraftSave } | null = null;
  let tail = Promise.resolve();
  const errors = new Map<string, unknown>();
  let draining: Promise<void> | null = null;

  function enqueue(key: string, save: DraftSave): Promise<void> {
    const operation = tail.then(save, save);
    tail = operation.then(
      () => {
        errors.delete(key);
      },
      (error) => {
        errors.set(key, error);
      },
    );
    return operation;
  }

  function cancelTimer() {
    if (!timer) return;
    clearTimeout(timer);
    timer = null;
  }

  return {
    schedule(key: string, save: DraftSave) {
      cancelTimer();
      if (pending && pending.key !== key) {
        const older = pending;
        pending = null;
        void enqueue(older.key, older.save).catch(() => {});
      }
      pending = { key, save };
      timer = setTimeout(() => {
        timer = null;
        const next = pending;
        pending = null;
        if (next) void enqueue(next.key, next.save).catch(() => {});
      }, delayMs);
    },

    /**
     * Cancel the debounce and wait for the newest pending write plus any older
     * write that was already in flight.
     */
    flush(): Promise<void> {
      if (draining) return draining;
      const operation = (async () => {
        while (true) {
          cancelTimer();
          const next = pending;
          pending = null;
          if (next) await enqueue(next.key, next.save);
          else await tail;
          // A composer may schedule a newer snapshot while the network write
          // is in flight. Keep draining until the queue is actually empty.
          if (!pending) break;
        }
        const failure = errors.values().next();
        if (!failure.done) throw failure.value;
      })().finally(() => {
        draining = null;
      });
      draining = operation;
      return operation;
    },

    /**
     * Drop a draft that was successfully accepted and deleted remotely. Its
     * old write error must not poison the next host switch.
     */
    discard(key?: string) {
      if (!key || pending?.key === key) {
        cancelTimer();
        pending = null;
      }
      if (key) {
        errors.delete(key);
      } else {
        errors.clear();
      }
    },
  };
}
