import { useQueryClient } from '@tanstack/react-query';
import type {
  AgentSession,
  PendingPermission,
  PendingUserInput,
  ProviderKind,
  UserInputAnswer,
} from '@padu/client';
import { reduceRuntimeEvent } from '@padu/client/event-reducer';
import { writeProviderProbeCache } from '@padu/client/provider-probe-cache';
import * as Crypto from 'expo-crypto';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';

import {
  attachDaemonSession,
  daemonKeys,
  hydrateSession,
  loadDaemonSettings,
  loadTaskState,
  materializeWorktree,
  persistSession,
  probeProvider,
  removeDaemonSession,
  setSessionArchived,
  setSessionPinned,
  type TaskState,
} from './daemon-api';
import { persistentStorageSync } from './composer-preferences-store';
import { useDaemon } from './daemon-context';
import {
  applySessionOptions,
  beginTurn,
  createSession,
  queueSubmission,
  sessionBusy,
  sessionCwd,
  shouldApplyRuntimeEvent,
  type NewSessionOptions,
  type SessionOptionChanges,
} from './mobile-runtime';

export interface MobileRuntime {
  runtimeId: string;
  supportsSteer: boolean;
  starting: boolean;
}

interface RuntimeEntry extends MobileRuntime {
  lastDriverError: string | null;
  unsubscribe: () => void;
}

interface RuntimeContextValue {
  runtimes: Record<string, MobileRuntime | undefined>;
  permissions: Record<string, PendingPermission | undefined>;
  userInputs: Record<string, PendingUserInput | undefined>;
  errors: Record<string, string | undefined>;
  attachSession: (session: AgentSession) => Promise<boolean>;
  sendPrompt: (session: AgentSession, prompt: string) => Promise<AgentSession>;
  steerPrompt: (session: AgentSession, prompt: string) => Promise<void>;
  createTask: (
    projectId: string,
    provider: ProviderKind,
    isolated: boolean,
    prompt: string,
    options?: NewSessionOptions,
  ) => Promise<AgentSession>;
  cancel: (sessionId: string) => Promise<void>;
  respond: (sessionId: string, requestId: string, optionId: string) => Promise<void>;
  respondUserInput: (
    sessionId: string,
    requestId: string,
    answers: UserInputAnswer[],
  ) => Promise<void>;
  updateSessionOptions: (sessionId: string, changes: SessionOptionChanges) => Promise<void>;
  renameSession: (sessionId: string, title: string) => Promise<void>;
  setSessionPinned: (sessionId: string, pinned: boolean) => Promise<void>;
  setSessionArchived: (sessionId: string, archived: boolean) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  removeQueuedMessage: (sessionId: string, messageId: string) => Promise<void>;
  dismissError: (sessionId: string) => void;
}

const RuntimeContext = createContext<RuntimeContextValue | null>(null);
const clock = {
  nowSeconds: () => Math.floor(Date.now() / 1_000),
  nowMillis: () => Date.now(),
  randomUUID: Crypto.randomUUID,
};

export function RuntimeProvider({ children }: { children: ReactNode }) {
  const daemon = useDaemon();
  const queryClient = useQueryClient();
  const [runtimes, setRuntimes] = useState<Record<string, MobileRuntime | undefined>>({});
  const [permissions, setPermissions] = useState<Record<string, PendingPermission | undefined>>({});
  const [userInputs, setUserInputs] = useState<Record<string, PendingUserInput | undefined>>({});
  const [errors, setErrors] = useState<Record<string, string | undefined>>({});
  const entries = useRef(new Map<string, RuntimeEntry>());
  const attachRequests = useRef(new Map<string, Promise<boolean>>());
  const persistTails = useRef(new Map<string, Promise<AgentSession>>());
  const persistTimers = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const pendingSteers = useRef(new Map<string, string[]>());
  const drainingQueues = useRef(new Set<string>());
  const sendPromptRef = useRef<
    ((session: AgentSession, prompt: string) => Promise<AgentSession>) | null
  >(null);

  const cacheSession = useCallback((session: AgentSession) => {
    const profileId = daemon.activeProfile?.id;
    if (!profileId) return;
    queryClient.setQueryData(daemonKeys.session(profileId, session.id), session);
    queryClient.setQueryData<TaskState>(daemonKeys.taskState(profileId), (current) => {
      if (!current) return current;
      const sessions = current.sessions.some((item) => item.id === session.id)
        ? current.sessions.map((item) => item.id === session.id ? session : item)
        : [...current.sessions, session];
      return { ...current, sessions };
    });
  }, [daemon.activeProfile?.id, queryClient]);

  /** Full session from the query cache, hydrating from the daemon when only
   * the list skeleton is known. A skeleton must never be persisted back — it
   * would blank the stored transcript. */
  const loadFullSession = useCallback(async (sessionId: string): Promise<AgentSession> => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId) throw new Error('Padu daemon is disconnected');
    const cached = queryClient.getQueryData<AgentSession>(
      daemonKeys.session(profileId, sessionId),
    );
    if (cached) return cached;
    const hydrated = await hydrateSession(client, sessionId);
    if (!hydrated) throw new Error('This task no longer exists on the daemon');
    queryClient.setQueryData(daemonKeys.session(profileId, sessionId), hydrated);
    return hydrated;
  }, [daemon.activeProfile?.id, daemon.client, queryClient]);

  const persistOrdered = useCallback((session: AgentSession): Promise<AgentSession> => {
    const client = daemon.client;
    if (!client) return Promise.reject(new Error('Padu daemon is disconnected'));
    const previous = persistTails.current.get(session.id);
    const operation = (previous ?? Promise.resolve(session))
      .catch(() => session)
      .then(() => persistSession(client, session))
      .then((saved) => {
        cacheSession(saved);
        return saved;
      });
    persistTails.current.set(session.id, operation);
    void operation.finally(() => {
      if (persistTails.current.get(session.id) === operation) persistTails.current.delete(session.id);
    }).catch(() => {});
    return operation;
  }, [cacheSession, daemon.client]);

  const schedulePersist = useCallback((sessionId: string) => {
    const profileId = daemon.activeProfile?.id;
    if (!profileId) return;
    const previous = persistTimers.current.get(sessionId);
    if (previous) clearTimeout(previous);
    const timer = setTimeout(() => {
      persistTimers.current.delete(sessionId);
      const latest = queryClient.getQueryData<AgentSession>(
        daemonKeys.session(profileId, sessionId),
      );
      if (latest) void persistOrdered(latest).catch((cause) => {
        setErrors((current) => ({ ...current, [sessionId]: errorMessage(cause) }));
      });
    }, 500);
    persistTimers.current.set(sessionId, timer);
  }, [daemon.activeProfile?.id, persistOrdered, queryClient]);

  const removeRuntime = useCallback((sessionId: string) => {
    const entry = entries.current.get(sessionId);
    entry?.unsubscribe();
    entries.current.delete(sessionId);
    pendingSteers.current.delete(sessionId);
    setRuntimes((current) => removeKey(current, sessionId));
  }, []);

  /** After a settled turn is persisted, submit the oldest queued message as
   * the next turn. Guarded per session so one settle drains one message. */
  const drainQueue = useCallback(async (sessionId: string) => {
    const profileId = daemon.activeProfile?.id;
    if (!profileId || drainingQueues.current.has(sessionId)) return;
    const latest = queryClient.getQueryData<AgentSession>(
      daemonKeys.session(profileId, sessionId),
    );
    if (!latest || latest.status !== 'idle') return;
    const next = latest.queued_messages?.[0];
    if (!next) return;
    drainingQueues.current.add(sessionId);
    try {
      const dequeued = {
        ...latest,
        queued_messages: latest.queued_messages?.slice(1),
      };
      cacheSession(dequeued);
      const persisted = await persistOrdered(dequeued);
      await sendPromptRef.current?.(persisted, next.display_content ?? next.content);
    } catch (cause) {
      setErrors((values) => ({ ...values, [sessionId]: errorMessage(cause) }));
    } finally {
      drainingQueues.current.delete(sessionId);
    }
  }, [cacheSession, daemon.activeProfile?.id, persistOrdered, queryClient]);

  const subscribe = useCallback((
    session: AgentSession,
    runtimeId: string,
    supportsSteer = false,
    starting = true,
  ): RuntimeEntry => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId) throw new Error('Padu daemon is disconnected');
    const existing = entries.current.get(session.id);
    if (existing) return existing;

    const entry: RuntimeEntry = {
      runtimeId,
      supportsSteer,
      starting,
      lastDriverError: null,
      unsubscribe: () => {},
    };
    entries.current.set(session.id, entry);
    cacheSession(session);
    setRuntimes((current) => ({ ...current, [session.id]: publicRuntime(entry) }));
    const unsubscribe = client.subscribe(session.id, runtimeId, (event) => {
      if (entries.current.get(session.id) !== entry) return;
      const key = daemonKeys.session(profileId, session.id);
      let current = queryClient.getQueryData<AgentSession>(key) ?? session;
      if (!shouldApplyRuntimeEvent(current, event)) return;
      if (event.event.kind === 'connected' || event.event.kind === 'turnStarted' || event.event.kind === 'turnFinished') {
        entry.lastDriverError = null;
      } else if (event.event.kind === 'error' && typeof event.event.payload === 'string') {
        entry.lastDriverError = event.event.payload;
      }
      if (event.event.kind === 'steerAccepted') {
        const payload = event.event.payload as { message?: string };
        const pending = pendingSteers.current.get(session.id)?.shift();
        if (pending) {
          current = {
            ...current,
            messages: [
              ...current.messages,
              {
                id: clock.randomUUID(),
                turn_id: current.turns.at(-1)?.id ?? null,
                role: 'user',
                content: payload.message ?? pending,
                created_at: clock.nowSeconds(),
                streaming: false,
              },
            ],
          };
        }
      } else if (event.event.kind === 'steerRejected') {
        const pending = pendingSteers.current.get(session.id)?.shift();
        if (pending) {
          current = queueSubmission(current, pending, clock);
          cacheSession(current);
          void persistOrdered(current).catch(() => {});
          setErrors((values) => ({
            ...values,
            [session.id]: 'The agent couldn’t take that mid-turn, so it was queued for the next turn.',
          }));
        }
      }
      const result = reduceRuntimeEvent(current, event, clock, entry.lastDriverError);
      cacheSession(result.session);
      schedulePersist(session.id);
      if (result.permission !== undefined) {
        setPermissions((values) => ({ ...values, [session.id]: result.permission ?? undefined }));
      }
      if (result.userInput !== undefined) {
        setUserInputs((values) => ({ ...values, [session.id]: result.userInput ?? undefined }));
      }
      if (result.error) setErrors((values) => ({ ...values, [session.id]: result.error }));
      if (result.settled) {
        const timer = persistTimers.current.get(session.id);
        if (timer) clearTimeout(timer);
        persistTimers.current.delete(session.id);
        void persistOrdered(result.session)
          .then(() => queryClient.invalidateQueries({ queryKey: daemonKeys.taskState(profileId) }))
          .then(() => drainQueue(session.id))
          .catch((cause) => {
            setErrors((values) => ({ ...values, [session.id]: errorMessage(cause) }));
          });
      }
      if (result.removeRuntime) removeRuntime(session.id);
    });
    if (entries.current.get(session.id) === entry) entry.unsubscribe = unsubscribe;
    else unsubscribe();
    return entry;
  }, [cacheSession, daemon.activeProfile?.id, daemon.client, drainQueue, persistOrdered, queryClient, removeRuntime, schedulePersist]);

  const attachSession = useCallback((session: AgentSession): Promise<boolean> => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId || daemon.phase !== 'connected') return Promise.resolve(false);
    if (entries.current.has(session.id)) return Promise.resolve(true);
    const pending = attachRequests.current.get(session.id);
    if (pending) return pending;

    const request = (async () => {
      const attached = await attachDaemonSession(client, session.id);
      if (!client.connected || !attached) return false;
      const current = queryClient.getQueryData<AgentSession>(
        daemonKeys.session(profileId, session.id),
      ) ?? session;
      subscribe(current, attached.runtimeId, attached.supportsSteer, false);
      return true;
    })().finally(() => {
      if (attachRequests.current.get(session.id) === request) attachRequests.current.delete(session.id);
    });
    attachRequests.current.set(session.id, request);
    return request;
  }, [daemon.activeProfile?.id, daemon.client, daemon.phase, queryClient, subscribe]);

  const sendPrompt = useCallback(async (
    inputSession: AgentSession,
    rawPrompt: string,
  ): Promise<AgentSession> => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId || daemon.phase !== 'connected') {
      throw new Error('Padu daemon is disconnected');
    }
    const prompt = rawPrompt.trim();
    if (!prompt) return inputSession;
    let current = queryClient.getQueryData<AgentSession>(
      daemonKeys.session(profileId, inputSession.id),
    ) ?? inputSession;
    if (sessionBusy(current)) {
      const queued = queueSubmission(current, prompt, clock);
      cacheSession(queued);
      return persistOrdered(queued);
    }

    if (!entries.current.has(current.id)) await attachSession(current);
    let runtime = entries.current.get(current.id);
    let startup: { binary: string; cwd: string } | null = null;
    if (!runtime) {
      const state = await loadTaskState(client);
      const project = state.projects.find((item) => item.id === current.project_id);
      if (!project) throw new Error('This task’s project is no longer available on the daemon');
      if (current.workspace?.kind === 'newWorktree') {
        current = await materializeWorktree(client, current, project.path, prompt);
        cacheSession(current);
        current = await persistOrdered(current);
      }
      const settings = await loadDaemonSettings(client);
      const probe = await probeProvider(client, current.provider, settings);
      if (!probe.installed || !probe.path) {
        throw new Error(`${current.provider} is not installed on the daemon host`);
      }
      if (daemon.activeProfile) {
        writeProviderProbeCache(
          persistentStorageSync(),
          daemon.activeProfile.address,
          current.provider,
          settings.provider_binary_overrides?.[current.provider] ?? null,
          probe,
        );
      }
      startup = { binary: probe.path, cwd: sessionCwd(current, project) };
    }

    current = beginTurn(current, prompt, clock);
    cacheSession(current);
    current = await persistOrdered(current);
    try {
      if (!runtime) {
        const runtimeId = Crypto.randomUUID();
        runtime = subscribe(current, runtimeId);
        const response = await client.request({
          type: 'start',
          options: {
            provider: current.provider,
            binary: startup!.binary,
            cwd: startup!.cwd,
            mode: current.runtime_mode,
            interactionMode: current.interaction_mode,
            model: current.model ?? null,
            reasoningEffort: current.reasoning_effort ?? null,
            serviceTier: current.service_tier ?? null,
            contextWindow: current.context_window ?? null,
            agentPreset: current.agent_preset ?? null,
            computerUseEnabled: false,
            providerCursor: current.provider_cursor as never,
          },
        }, current.id, runtimeId);
        if (response.type !== 'started') {
          throw new Error(`Expected daemon response started, received ${response.type}`);
        }
        runtime.supportsSteer = response.supportsSteer;
        runtime.starting = false;
        setRuntimes((values) => ({ ...values, [current.id]: publicRuntime(runtime!) }));
      }
      await client.request({ type: 'prompt', prompt }, current.id, runtime.runtimeId);
      setErrors((values) => removeKey(values, current.id));
      return current;
    } catch (cause) {
      if (runtime?.starting) removeRuntime(current.id);
      const latest = queryClient.getQueryData<AgentSession>(
        daemonKeys.session(profileId, current.id),
      ) ?? current;
      const cursor = latest.runtime_event_cursor;
      const failed = reduceRuntimeEvent(latest, {
        sessionId: latest.id,
        runtimeId: runtime?.runtimeId ?? Crypto.randomUUID(),
        epoch: cursor?.epoch ?? `mobile-${Date.now()}`,
        sequence: (cursor?.sequence ?? 0) + 1,
        event: { kind: 'turnFinished', payload: { success: false, summary: errorMessage(cause) } },
      }, clock).session;
      cacheSession(failed);
      await persistOrdered(failed).catch(() => failed);
      throw cause;
    }
  }, [attachSession, cacheSession, daemon.activeProfile?.id, daemon.client, daemon.phase, persistOrdered, queryClient, removeRuntime, subscribe]);

  useEffect(() => {
    sendPromptRef.current = sendPrompt;
  }, [sendPrompt]);

  /** Inject a prompt into the running turn when the provider supports it;
   * otherwise fall through to sendPrompt, which queues while busy. */
  const steerPrompt = useCallback(async (session: AgentSession, rawPrompt: string) => {
    const client = daemon.client;
    const prompt = rawPrompt.trim();
    if (!prompt) return;
    if (!client || daemon.phase !== 'connected') throw new Error('Padu daemon is disconnected');
    const runtime = entries.current.get(session.id);
    if (
      !runtime || !runtime.supportsSteer ||
      session.status === 'connecting' || session.status === 'idle' || session.status === 'failed'
    ) {
      await sendPrompt(session, prompt);
      return;
    }
    const pending = pendingSteers.current.get(session.id) ?? [];
    pending.push(prompt);
    pendingSteers.current.set(session.id, pending);
    await client.request({ type: 'steer', prompt }, session.id, runtime.runtimeId);
  }, [daemon.client, daemon.phase, sendPrompt]);

  const createTask = useCallback(async (
    projectId: string,
    provider: ProviderKind,
    isolated: boolean,
    prompt: string,
    options: NewSessionOptions = {},
  ): Promise<AgentSession> => {
    const profileId = daemon.activeProfile?.id;
    if (!profileId || !daemon.client || daemon.phase !== 'connected') {
      throw new Error('Padu daemon is disconnected');
    }
    const draft = createSession(projectId, provider, isolated, clock, options);
    cacheSession(draft);
    const saved = await persistOrdered(draft);
    try {
      return await sendPrompt(saved, prompt);
    } catch (cause) {
      setErrors((values) => ({ ...values, [saved.id]: errorMessage(cause) }));
      return queryClient.getQueryData<AgentSession>(
        daemonKeys.session(profileId, saved.id),
      ) ?? saved;
    }
  }, [cacheSession, daemon.activeProfile?.id, daemon.client, daemon.phase, persistOrdered, queryClient, sendPrompt]);

  const cancel = useCallback(async (sessionId: string) => {
    const client = daemon.client;
    if (!client) throw new Error('Padu daemon is disconnected');
    const runtime = entries.current.get(sessionId);
    if (!runtime) throw new Error('This task has no live agent runtime');
    await client.request({ type: 'cancel' }, sessionId, runtime.runtimeId);
  }, [daemon.client]);

  const markWorking = useCallback((sessionId: string) => {
    const profileId = daemon.activeProfile?.id;
    if (!profileId) return;
    const key = daemonKeys.session(profileId, sessionId);
    const session = queryClient.getQueryData<AgentSession>(key);
    if (!session) return;
    const updated = { ...session, status: 'working' as const };
    cacheSession(updated);
    void persistOrdered(updated).catch((cause) => {
      setErrors((values) => ({ ...values, [sessionId]: errorMessage(cause) }));
    });
  }, [cacheSession, daemon.activeProfile?.id, persistOrdered, queryClient]);

  const respond = useCallback(async (sessionId: string, requestId: string, optionId: string) => {
    const client = daemon.client;
    const runtime = entries.current.get(sessionId);
    if (!client || !runtime) throw new Error('This task has no live agent runtime');
    await client.request({ type: 'respond', requestId, optionId }, sessionId, runtime.runtimeId);
    setPermissions((values) => ({ ...values, [sessionId]: undefined }));
    markWorking(sessionId);
  }, [daemon.client, markWorking]);

  const respondUserInput = useCallback(async (
    sessionId: string,
    requestId: string,
    answers: UserInputAnswer[],
  ) => {
    const client = daemon.client;
    const runtime = entries.current.get(sessionId);
    if (!client || !runtime) throw new Error('This task has no live agent runtime');
    await client.request(
      { type: 'respondUserInput', requestId, answers },
      sessionId,
      runtime.runtimeId,
    );
    setUserInputs((values) => ({ ...values, [sessionId]: undefined }));
    markWorking(sessionId);
  }, [daemon.client, markWorking]);

  /** Change model / effort / access mode. Applied live via
   * applyOptions when a runtime exists; a runtime that can't take the change
   * is closed so the next prompt restarts with the new options. */
  const updateSessionOptions = useCallback(async (
    sessionId: string,
    changes: SessionOptionChanges,
  ) => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId || daemon.phase !== 'connected') {
      throw new Error('Padu daemon is disconnected');
    }
    const current = await loadFullSession(sessionId);
    const next = applySessionOptions(current, changes, clock);
    cacheSession(next);
    try {
      const runtime = entries.current.get(sessionId);
      if (runtime && !runtime.starting) {
        const response = await client.request({
          type: 'applyOptions',
          options: {
            mode: next.runtime_mode,
            interactionMode: next.interaction_mode,
            model: next.model ?? null,
            reasoningEffort: next.reasoning_effort ?? null,
            serviceTier: next.service_tier ?? null,
            contextWindow: next.context_window ?? null,
          },
        }, sessionId, runtime.runtimeId);
        if (response.type !== 'optionsApplied' || !response.applied) {
          await client.request({ type: 'closeSession' }, sessionId, runtime.runtimeId);
          removeRuntime(sessionId);
        }
      }
      await persistOrdered(next);
    } catch (cause) {
      cacheSession(current);
      throw cause;
    }
  }, [cacheSession, daemon.activeProfile?.id, daemon.client, daemon.phase, loadFullSession, persistOrdered, removeRuntime]);

  const renameSession = useCallback(async (sessionId: string, title: string) => {
    const current = await loadFullSession(sessionId);
    const trimmed = title.trim();
    const next = {
      ...current,
      title: trimmed || 'New task',
      updated_at: clock.nowSeconds(),
    };
    cacheSession(next);
    await persistOrdered(next);
  }, [cacheSession, loadFullSession, persistOrdered]);

  /** Pin / archive flags are daemon-owned metadata. Optimistically flip the
   * flag in both the task list and the hydrated session, then reconcile with
   * the daemon response; roll back on failure. Archiving also clears a live
   * runtime's pin, mirroring web. */
  const updateSessionFlag = useCallback(async (
    sessionId: string,
    flag: 'pinned_at' | 'archived_at',
    enabled: boolean,
  ) => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId || daemon.phase !== 'connected') {
      throw new Error('Padu daemon is disconnected');
    }
    const timestamp = enabled ? clock.nowSeconds() : null;
    const applyFlag = (session: AgentSession): AgentSession => ({
      ...session,
      [flag]: timestamp,
      ...(flag === 'archived_at' && enabled ? { pinned_at: null } : {}),
    });
    const previousState = queryClient.getQueryData<TaskState>(daemonKeys.taskState(profileId));
    const previousSession = queryClient.getQueryData<AgentSession>(
      daemonKeys.session(profileId, sessionId),
    );
    queryClient.setQueryData<TaskState>(daemonKeys.taskState(profileId), (current) => (
      current
        ? { ...current, sessions: current.sessions.map((item) => item.id === sessionId ? applyFlag(item) : item) }
        : current
    ));
    if (previousSession) {
      queryClient.setQueryData(
        daemonKeys.session(profileId, sessionId),
        applyFlag(previousSession),
      );
    }
    try {
      const updated = flag === 'pinned_at'
        ? await setSessionPinned(client, sessionId, enabled)
        : await setSessionArchived(client, sessionId, enabled);
      queryClient.setQueryData<TaskState>(daemonKeys.taskState(profileId), (current) => (
        current
          ? {
            ...current,
            sessions: current.sessions.map((item) => item.id === sessionId
              ? { ...item, pinned_at: updated.pinned_at, archived_at: updated.archived_at }
              : item),
          }
          : current
      ));
      if (previousSession || queryClient.getQueryData<AgentSession>(daemonKeys.session(profileId, sessionId))) {
        queryClient.setQueryData<AgentSession>(
          daemonKeys.session(profileId, sessionId),
          (current) => current
            ? { ...current, pinned_at: updated.pinned_at, archived_at: updated.archived_at }
            : current,
        );
      }
      // The list renders daemon-owned sidebar groups, not the task state
      // directly — without this the tabs never recompute and the mutation
      // looks like a no-op.
      void queryClient.invalidateQueries({ queryKey: daemonKeys.taskState(profileId) });
      void queryClient.invalidateQueries({ queryKey: ['daemon', profileId, 'sidebar-groups'] });
    } catch (cause) {
      if (previousState) queryClient.setQueryData(daemonKeys.taskState(profileId), previousState);
      if (previousSession) {
        queryClient.setQueryData(daemonKeys.session(profileId, sessionId), previousSession);
      }
      throw cause;
    }
  }, [daemon.activeProfile?.id, daemon.client, daemon.phase, queryClient]);

  const setPinned = useCallback(async (sessionId: string, pinned: boolean) => {
    await updateSessionFlag(sessionId, 'pinned_at', pinned);
  }, [updateSessionFlag]);

  const setArchived = useCallback(async (sessionId: string, archived: boolean) => {
    await updateSessionFlag(sessionId, 'archived_at', archived);
  }, [updateSessionFlag]);

  const deleteSession = useCallback(async (sessionId: string) => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId || daemon.phase !== 'connected') {
      throw new Error('Padu daemon is disconnected');
    }
    const runtime = entries.current.get(sessionId);
    if (runtime) {
      await client.request({ type: 'closeSession' }, sessionId, runtime.runtimeId).catch(() => {});
      removeRuntime(sessionId);
    }
    const timer = persistTimers.current.get(sessionId);
    if (timer) clearTimeout(timer);
    persistTimers.current.delete(sessionId);
    await persistTails.current.get(sessionId)?.catch(() => {});
    await removeDaemonSession(client, sessionId);
    queryClient.removeQueries({ queryKey: daemonKeys.session(profileId, sessionId) });
    queryClient.setQueryData<TaskState>(daemonKeys.taskState(profileId), (current) => (
      current
        ? { ...current, sessions: current.sessions.filter((item) => item.id !== sessionId) }
        : current
    ));
    setPermissions((values) => removeKey(values, sessionId));
    setUserInputs((values) => removeKey(values, sessionId));
    setErrors((values) => removeKey(values, sessionId));
  }, [daemon.activeProfile?.id, daemon.client, daemon.phase, queryClient, removeRuntime]);

  const removeQueuedMessage = useCallback(async (sessionId: string, messageId: string) => {
    const profileId = daemon.activeProfile?.id;
    if (!profileId) throw new Error('Padu daemon is disconnected');
    const current = queryClient.getQueryData<AgentSession>(
      daemonKeys.session(profileId, sessionId),
    );
    if (!current) return;
    const next = {
      ...current,
      queued_messages: (current.queued_messages ?? []).filter((item) => item.id !== messageId),
    };
    cacheSession(next);
    await persistOrdered(next);
  }, [cacheSession, daemon.activeProfile?.id, persistOrdered, queryClient]);

  const dismissError = useCallback((sessionId: string) => {
    setErrors((values) => removeKey(values, sessionId));
  }, []);

  // Another client (the desktop, the web app) persisting a session announces
  // itself through taskStateChanged. Refetch any hydrated session we are not
  // already following live, so watching a desktop-driven task stays current.
  useEffect(() => {
    const client = daemon.client;
    const profileId = daemon.activeProfile?.id;
    if (!client || !profileId) return;
    return client.subscribeTaskState(() => {
      void queryClient.invalidateQueries({
        queryKey: ['daemon', profileId, 'session'],
        predicate: (query) => {
          const sessionId = query.queryKey[3];
          return typeof sessionId === 'string' && !entries.current.has(sessionId);
        },
      });
      // Pin / archive / delete from another client move sessions between
      // sidebar groups — refetch the list inputs too, not just sessions.
      void queryClient.invalidateQueries({ queryKey: daemonKeys.taskState(profileId) });
      void queryClient.invalidateQueries({ queryKey: ['daemon', profileId, 'sidebar-groups'] });
    });
  }, [daemon.activeProfile?.id, daemon.client, queryClient]);

  useEffect(() => {
    setRuntimes({});
    setPermissions({});
    setUserInputs({});
    setErrors({});
    return () => {
      for (const entry of entries.current.values()) entry.unsubscribe();
      entries.current.clear();
      attachRequests.current.clear();
      persistTails.current.clear();
      pendingSteers.current.clear();
      drainingQueues.current.clear();
      for (const timer of persistTimers.current.values()) clearTimeout(timer);
      persistTimers.current.clear();
    };
  }, [daemon.activeProfile?.id, daemon.client]);

  return (
    <RuntimeContext.Provider value={{
      runtimes,
      permissions,
      userInputs,
      errors,
      attachSession,
      sendPrompt,
      steerPrompt,
      createTask,
      cancel,
      respond,
      respondUserInput,
      updateSessionOptions,
      renameSession,
      setSessionPinned: setPinned,
      setSessionArchived: setArchived,
      deleteSession,
      removeQueuedMessage,
      dismissError,
    }}>
      {children}
    </RuntimeContext.Provider>
  );
}

export function useRuntime() {
  const context = useContext(RuntimeContext);
  if (!context) throw new Error('useRuntime must be used inside RuntimeProvider');
  return context;
}

function publicRuntime(entry: RuntimeEntry): MobileRuntime {
  return {
    runtimeId: entry.runtimeId,
    supportsSteer: entry.supportsSteer,
    starting: entry.starting,
  };
}

function removeKey<T>(record: Record<string, T | undefined>, key: string): Record<string, T | undefined> {
  const next = { ...record };
  delete next[key];
  return next;
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error && cause.message.trim() ? cause.message : String(cause);
}
