import { useQuery } from '@tanstack/react-query';
import type { PaduClient, ProviderKind, RuntimeMode } from '@padu/client';
import { rememberComposerSession } from '@padu/client/composer-preferences';
import {
  detectSkillTrigger,
  filterSkillCommands,
  replaceSkillTrigger,
  SKILL_AUTOCOMPLETE_CAP,
} from '@padu/client/composer-skills';
import * as Haptics from 'expo-haptics';
import * as Crypto from 'expo-crypto';
import { router } from 'expo-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
  type TextInputKeyPressEvent,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { PaduIcon, type PaduIconName } from '@/components/padu-icon';
import {
  ComposerSkillAutocomplete,
  ComposerCard,
  ComposerIconButton,
  SendButton,
} from '@/components/mobile-composer';
import { RemoteProjectPicker } from '@/components/remote-project-picker';
import { ScreenHeader } from '@/components/screen-header';
import { AccessSheet, ModelPickerSheet } from '@/components/session-option-sheets';
import { SessionView } from '@/components/session-view';
import { Sheet, SheetRow } from '@/components/sheet';
import { Radius, Spacing } from '@/constants/theme';
import {
  useAllProviderModels,
  useComposerCommands,
  useProviderCatalog,
  useTaskState,
} from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import {
  applyComposerDraftChanges,
  daemonKeys,
  hydrateSession,
  inspectBranches,
  loadComposerDrafts,
  removeDaemonSession,
} from '@/lib/daemon-api';
import {
  loadComposerPreferences,
  loadNewTaskExtras,
  saveComposerPreferences,
  saveNewTaskExtras,
} from '@/lib/composer-preferences-store';
import { useDaemon } from '@/lib/daemon-context';
import {
  createDraftFlushScheduler,
} from '@/lib/draft-flush';
import {
  claimNewTaskSubmission,
  hasSameNewTaskDraftTarget,
  isNewTaskDraftReady,
  newTaskDraftScopeKey,
  planNewTaskDraftTransition,
  saveNewTaskDraftOrThrow,
} from '@/lib/new-task-draft';
import {
  classifyPendingSubmission,
  clearPendingDraftCleanup,
  loadPendingDraftCleanups,
  persistPendingDraftCleanup,
  resetPendingDraftCleanups,
  type PendingDraftCleanup,
} from '@/lib/pending-draft-cleanup';
import { useRuntime } from '@/lib/runtime-context';
import { providerLabel } from '@/lib/session-presentation';
import { composerDraftSnapshot } from '@/lib/mobile-runtime';
import {
  mobilePromptSelectionContext,
  renderSkillSubmissionPrompt,
  useMobilePromptResolutionGuard,
} from '@/lib/skill-submission';

type SheetKind = 'daemon' | 'project' | 'model' | 'workspace' | 'branch' | 'access';
type CleanupLookupState = 'loading' | 'ready' | 'error';
const TOUCH_TARGET = 44;

export default function NewTaskScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const taskState = useTaskState();
  const catalog = useProviderCatalog();
  const [projectId, setProjectId] = useState<string | null>(null);
  const [provider, setProvider] = useState<ProviderKind | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [reasoningEffort, setReasoningEffort] = useState<string | null>(null);
  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>('fullAccess');
  const [isolated, setIsolated] = useState(false);
  const [baseBranch, setBaseBranch] = useState<string | null>(null);
  const [prompt, setPrompt] = useState('');
  const [draftReadyScope, setDraftReadyScope] = useState<{
    key: string;
    client: PaduClient;
  } | null>(null);
  const [draftHydrationError, setDraftHydrationError] = useState<string | null>(null);
  const [draftHydrationAttempt, setDraftHydrationAttempt] = useState(0);
  const [cursor, setCursor] = useState(0);
  const [inputFocused, setInputFocused] = useState(false);
  const [skillWarnings, setSkillWarnings] = useState<string[]>([]);
  const [skillSelection, setSkillSelection] = useState({ key: '', index: 0 });
  const [dismissedSkillAutocomplete, setDismissedSkillAutocomplete] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [acceptedSessionId, setAcceptedSessionId] = useState<string | null>(null);
  const [createdSessionId, setCreatedSessionId] = useState<string | null>(null);
  const [openSheet, setOpenSheet] = useState<SheetKind | null>(null);
  const [projectPickerOpen, setProjectPickerOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [draftCleanupFailure, setDraftCleanupFailure] =
    useState<PendingDraftCleanup | null>(null);
  const [cleanupLookupState, setCleanupLookupState] =
    useState<CleanupLookupState>('loading');
  const [cleanupLookupError, setCleanupLookupError] = useState<string | null>(null);
  const [cleanupLookupAttempt, setCleanupLookupAttempt] = useState(0);
  const [cleanupLookupResetting, setCleanupLookupResetting] = useState(false);
  const startedRef = useRef(false);
  const submittingRef = useRef(false);
  const operationSequence = useRef(0);
  const activeProfileIdRef = useRef(daemon.activeProfile?.id ?? null);
  const activeClientRef = useRef(daemon.client);
  activeProfileIdRef.current = daemon.activeProfile?.id ?? null;
  activeClientRef.current = daemon.client;
  const installedProviders = useMemo(
    () => catalog.providers.filter((item) => item.installed).map((item) => item.id),
    [catalog.providers],
  );
  const modelCatalog = useAllProviderModels(installedProviders);
  const projects = taskState.data?.projects ?? [];
  const selectedProject = projects.find((item) => item.id === projectId);
  const draftProfileId = daemon.activeProfile?.id ?? null;
  const draftProjectId = selectedProject?.id ?? null;
  const draftScopeKey = newTaskDraftScopeKey(draftProfileId, draftProjectId);
  const draftReady = isNewTaskDraftReady(
    draftReadyScope,
    draftScopeKey,
    daemon.client,
  );
  useEffect(() => {
    const profileId = daemon.activeProfile?.id;
    const client = daemon.client;
    if (!profileId || !client || daemon.phase !== 'connected') return;
    let cancelled = false;
    operationSequence.current += 1;
    submittingRef.current = false;
    setSubmitting(false);
    setCleanupLookupState('loading');
    setCleanupLookupError(null);
    setDraftCleanupFailure(null);
    setAcceptedSessionId(null);
    setCleanupLookupResetting(false);
    startedRef.current = false;
    void (async () => {
      try {
        const jobs = await loadPendingDraftCleanups();
        if (cancelled) return;
        const job = jobs.find((entry) => entry.profileId === profileId);
        if (job?.phase === 'submitting') {
          const session = await hydrateSession(client, job.sessionId);
          if (
            cancelled
            || activeProfileIdRef.current !== profileId
            || activeClientRef.current !== client
          ) return;
          if (classifyPendingSubmission(session) === 'restoreDraft') {
            if (session) {
              await removeDaemonSession(client, job.sessionId);
            }
            await clearPendingDraftCleanup(job);
            if (
              cancelled
              || activeProfileIdRef.current !== profileId
              || activeClientRef.current !== client
            ) return;
            startedRef.current = false;
            setDone();
            return;
          }
          const cleanupJob: PendingDraftCleanup = {
            ...job,
            phase: 'cleanup',
            message: 'Task accepted; waiting for draft cleanup',
          };
          await persistPendingDraftCleanup(cleanupJob);
          if (
            cancelled
            || activeProfileIdRef.current !== profileId
            || activeClientRef.current !== client
          ) return;
          startedRef.current = true;
          setAcceptedSessionId(cleanupJob.sessionId);
          setDraftCleanupFailure(cleanupJob);
          setDraftReadyScope(null);
        } else if (job) {
          startedRef.current = true;
          setAcceptedSessionId(job.sessionId);
          setDraftCleanupFailure(job);
          setDraftReadyScope(null);
        }
        setDone();
      } catch (cause) {
        if (cancelled) return;
        setCleanupLookupError(
          cause instanceof Error
            ? cause.message
            : 'Couldn’t check for unfinished draft cleanup',
        );
        setCleanupLookupState('error');
      }
    })();

    function setDone() {
      setCleanupLookupState('ready');
    }

    return () => {
      cancelled = true;
    };
  }, [
    cleanupLookupAttempt,
    daemon.activeProfile?.id,
    daemon.client,
    daemon.phase,
  ]);
  const projectless = selectedProject?.name === 'No project';
  const composerCommands = useComposerCommands(provider, selectedProject?.path ?? null);
  const skillCatalog = composerCommands.data;
  const skillCommands = skillCatalog?.commands ?? [];
  const effectiveIsolated = isolated && !projectless;
  const promptResolutionContext = selectedProject && provider
    ? {
        hostId: daemon.activeProfile?.id ?? null,
        hostAddress: daemon.activeProfile?.address ?? null,
        sessionId: null,
        projectId: selectedProject.id,
        projectRoot: selectedProject.path,
        provider,
        binaryOverride: composerCommands.binaryOverride,
        binaryVersion: composerCommands.binaryVersion,
        binaryIdentity: composerCommands.binaryIdentity,
        selection: mobilePromptSelectionContext(
          effectiveIsolated
            ? { kind: 'newWorktree', baseBranch }
            : { kind: 'local' },
          {
            model,
            reasoningEffort,
            runtimeMode,
            interactionMode: 'build',
            isolated: effectiveIsolated,
            baseBranch: effectiveIsolated ? baseBranch : null,
          },
        ),
      }
    : null;
  const resolvePromptInStableContext = useMobilePromptResolutionGuard(
    promptResolutionContext,
  );
  const skillTrigger = inputFocused ? detectSkillTrigger(prompt, cursor) : null;
  const skillAutocompleteKey = skillTrigger
    ? `${skillTrigger.start}:${skillTrigger.end}:${skillTrigger.query}`
    : null;
  const skillRows = skillTrigger
    ? filterSkillCommands(skillCommands, skillTrigger.query, SKILL_AUTOCOMPLETE_CAP)
    : [];
  const skillAutocompleteLoading = Boolean(
    skillTrigger && (composerCommands.isPending || composerCommands.isFetching),
  );
  const skillAutocompleteVisible = Boolean(
    skillAutocompleteKey
      && skillAutocompleteKey !== dismissedSkillAutocomplete
      && (skillRows.length || skillAutocompleteLoading),
  );
  const skillAutocompleteOpen = skillAutocompleteVisible && skillRows.length > 0;
  const skillHighlight = skillSelection.key === skillAutocompleteKey
    ? Math.min(skillSelection.index, Math.max(0, skillRows.length - 1))
    : 0;
  const skillBlurTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const branches = useQuery({
    queryKey: daemonKeys.branches(
      daemon.activeProfile?.id ?? 'disconnected',
      selectedProject?.path ?? 'missing',
    ),
    queryFn: () => inspectBranches(daemon.client!, selectedProject!.path),
    enabled: daemon.phase === 'connected' && Boolean(daemon.client && selectedProject) && isolated,
    staleTime: 30_000,
  });

  useEffect(() => {
    if (!projectId || !projects.some((project) => project.id === projectId)) {
      setProjectId(projects[0]?.id ?? null);
    }
  }, [projectId, projects]);

  useEffect(() => {
    setSkillWarnings([]);
    setDismissedSkillAutocomplete(null);
    setSkillSelection({ key: '', index: 0 });
  }, [projectId, provider]);

  useEffect(() => () => {
    if (skillBlurTimer.current) clearTimeout(skillBlurTimer.current);
  }, []);

  useEffect(() => {
    if (projectless && isolated) setIsolated(false);
  }, [isolated, projectless]);

  useEffect(() => setBaseBranch(null), [projectId]);

  useEffect(() => {
    if (provider && installedProviders.includes(provider)) return;
    const preferred = installedProviders.includes('codex') ? 'codex' : installedProviders[0];
    if (preferred) setProvider(preferred);
  }, [installedProviders, provider]);

  // Restore the last-used composition (provider/model/effort via the shared
  // composer preferences, plus mobile extras) once per daemon.
  const [restoredAddress, setRestoredAddress] = useState<string | null>(null);
  const restoredFor = useRef<string | null>(null);
  useEffect(() => {
    const address = daemon.activeProfile?.address;
    if (!address || restoredFor.current === address) return;
    restoredFor.current = address;
    void (async () => {
      const [prefs, extras] = await Promise.all([
        loadComposerPreferences(address),
        loadNewTaskExtras(address),
      ]);
      setRuntimeMode(extras.runtimeMode);
      setIsolated(extras.isolated);
      if (extras.projectId) setProjectId(extras.projectId);
      setProvider(prefs.lastProvider);
      setModel(prefs.lastModel);
      setReasoningEffort(prefs.lastReasoningEffort);
      setRestoredAddress(address);
    })();
  }, [daemon.activeProfile?.address]);

  // Unlike the web SPA, this screen unmounts between visits, so every choice
  // persists as it is made — not only when a task is created.
  useEffect(() => {
    const address = daemon.activeProfile?.address;
    if (!address || restoredAddress !== address) return;
    const timer = setTimeout(() => {
      void loadComposerPreferences(address).then((prefs) => saveComposerPreferences(address, {
        ...prefs,
        ...(provider ? { lastProvider: provider } : {}),
        lastModel: model,
        lastReasoningEffort: reasoningEffort,
      })).catch(() => {});
      void saveNewTaskExtras(address, {
        runtimeMode,
        isolated,
        projectId,
      }).catch(() => {});
    }, 300);
    return () => clearTimeout(timer);
  }, [daemon.activeProfile?.address, isolated, model, projectId, provider, reasoningEffort, restoredAddress, runtimeMode]);

  // Cross-device draft: prefill from the daemon-persisted new-session draft
  // for this project, and persist edits back, debounced.
  const draftFlush = useRef(createDraftFlushScheduler()).current;
  const draftTransition = useRef(0);
  const draftReadyScopeRef = useRef(draftReadyScope);
  draftReadyScopeRef.current = draftReadyScope;
  const draftScopeRef = useRef<{
    key: string;
    profileId: string;
    projectId: string;
    client: PaduClient;
  } | null>(null);
  const promptRef = useRef(prompt);
  promptRef.current = prompt;
  useEffect(() => {
    const client = daemon.client;
    if (
      acceptedSessionId
      ||
      cleanupLookupState !== 'ready'
      ||
      !client
      || daemon.phase !== 'connected'
      || !draftProfileId
      || !draftProjectId
      || !draftScopeKey
    ) return;
    if (hasSameNewTaskDraftTarget(
      draftScopeRef.current,
      draftScopeKey,
      client,
    )) {
      setDraftHydrationError(null);
      if (
        draftReadyScopeRef.current?.key !== draftScopeKey
        || draftReadyScopeRef.current.client !== client
      ) {
        setDraftReadyScope({ key: draftScopeKey, client });
      }
      return;
    }

    const transition = ++draftTransition.current;
    setDraftReadyScope(null);
    setDraftHydrationError(null);
    void (async () => {
      const previous = draftScopeRef.current;
      const drafts = await loadComposerDrafts(client);
      if (draftTransition.current !== transition) return;
      const plan = planNewTaskDraftTransition({
        previous: previous
          ? {
              profileId: previous.profileId,
              projectId: previous.projectId,
              prompt: promptRef.current,
            }
          : null,
        nextProfileId: draftProfileId,
        sameClient: previous?.client === client,
        destinationText: drafts.new_sessions?.[draftProjectId]?.text,
      });
      const previousDraft = plan.previousDraft;
      if (previous && previousDraft) {
        draftFlush.schedule(previous.key, () => applyComposerDraftChanges(
          previous.client,
          [{
            target: {
              type: 'newSession',
              projectId: previousDraft.projectId,
            },
            draft: { text: previousDraft.text },
          }],
        ));
      }
      await draftFlush.flush();
      if (draftTransition.current !== transition) return;

      draftScopeRef.current = {
        key: draftScopeKey,
        profileId: draftProfileId,
        projectId: draftProjectId,
        client,
      };
      promptRef.current = '';
      setPrompt('');
      setCursor(0);
      setSkillWarnings([]);
      setDismissedSkillAutocomplete(null);
      setSkillSelection({ key: '', index: 0 });
      const text = plan.destinationText;
      if (text) {
        promptRef.current = text;
        setPrompt(text);
        setCursor(text.length);
      }
      setDraftReadyScope({ key: draftScopeKey, client });
    })().catch((cause) => {
      if (draftTransition.current !== transition) return;
      setDraftHydrationError(
        cause instanceof Error
          ? cause.message
          : 'Couldn’t save or load this project’s draft',
      );
    });

    return () => {
      if (draftTransition.current === transition) {
        draftTransition.current += 1;
      }
    };
  }, [
    daemon.client,
    daemon.phase,
    draftFlush,
    draftProfileId,
    draftProjectId,
    acceptedSessionId,
    cleanupLookupState,
    draftHydrationAttempt,
    draftScopeKey,
  ]);
  useEffect(() => {
    const client = daemon.client;
    const target = selectedProject?.id;
    if (
      !client
      || !target
      || !draftScopeKey
      || startedRef.current
      || acceptedSessionId
      || cleanupLookupState !== 'ready'
      || !draftReady
      || draftScopeRef.current?.key !== draftScopeKey
      || draftScopeRef.current.client !== client
    ) return;
    draftFlush.schedule(draftScopeKey, () => applyComposerDraftChanges(client, [{
        target: { type: 'newSession', projectId: target },
        draft: prompt.trim() ? { text: prompt } : null,
      }]));
  }, [
    daemon.client,
    acceptedSessionId,
    cleanupLookupState,
    draftReady,
    draftFlush,
    draftScopeKey,
    prompt,
    selectedProject?.id,
  ]);
  function pick(apply: () => void) {
    return () => {
      void Haptics.selectionAsync();
      apply();
      setOpenSheet(null);
    };
  }

  function acceptSkill(index = skillHighlight) {
    if (!skillTrigger) return;
    const command = skillRows[index];
    if (!command) return;
    const replacement = replaceSkillTrigger(prompt, skillTrigger, command);
    setPrompt(replacement.text);
    setCursor(replacement.cursor);
    setDismissedSkillAutocomplete(null);
    setSkillSelection({ key: '', index: 0 });
  }

  function moveSkill(direction: -1 | 1) {
    if (!skillAutocompleteKey || !skillRows.length) return;
    const next = (
      skillHighlight + direction + skillRows.length
    ) % skillRows.length;
    setSkillSelection({ key: skillAutocompleteKey, index: next });
  }

  function handleComposerKeyPress(
    event: TextInputKeyPressEvent,
  ) {
    if (!skillAutocompleteVisible) return;
    if (event.nativeEvent.key === 'Escape') {
      event.preventDefault();
      setDismissedSkillAutocomplete(skillAutocompleteKey);
      return;
    }
    if (!skillAutocompleteOpen) return;
    if (
      event.nativeEvent.key === 'ArrowDown'
      || event.nativeEvent.key === 'ArrowUp'
    ) {
      event.preventDefault();
      moveSkill(event.nativeEvent.key === 'ArrowDown' ? 1 : -1);
      return;
    }
    if (event.nativeEvent.key === 'Enter' || event.nativeEvent.key === 'Tab') {
      event.preventDefault();
      acceptSkill();
    }
  }

  async function renderProviderPrompt(visiblePrompt: string) {
    return resolvePromptInStableContext(async (context) => {
      const rendered = await renderSkillSubmissionPrompt({
        visiblePrompt,
        provider: context.provider,
        catalog: skillCatalog,
        binaryVersion: composerCommands.binaryVersion,
        binaryIdentity: composerCommands.binaryIdentity,
        refetchBinary: () => composerCommands.refetchBinary(),
        refetchCatalog: () => composerCommands.refetch(),
      });
      return rendered;
    });
  }

  async function start() {
    const value = prompt.trim();
    const taskClient = daemon.client;
    const taskProfile = daemon.activeProfile;
    if (
      !selectedProject
      || !provider
      || !value
      || submittingRef.current
      || !draftReady
      || !taskClient
      || !taskProfile
      || acceptedSessionId
      || cleanupLookupState !== 'ready'
    ) return;
    if (!claimNewTaskSubmission(submittingRef)) return;
    const startedProjectId = selectedProject.id;
    const taskDraftKey = newTaskDraftScopeKey(
      taskProfile.id,
      startedProjectId,
    );
    const operation = ++operationSequence.current;
    const isCurrentOperation = () => (
      operationSequence.current === operation
      && activeProfileIdRef.current === taskProfile.id
      && activeClientRef.current === taskClient
    );
    setSubmitting(true);
    setError(null);
    setSkillWarnings([]);
    let accepted = false;
    try {
      await saveNewTaskDraftOrThrow(() => (
        applyComposerDraftChanges(taskClient, [{
          target: { type: 'newSession', projectId: selectedProject.id },
          draft: composerDraftSnapshot(value, [], []),
        }])
      ));
      const resolution = await renderProviderPrompt(value);
      resolution.assertCurrent();
      setSkillWarnings(resolution.result.warnings);
      resolution.assertCurrent();
      const submissionId = Crypto.randomUUID();
      const submissionJob: PendingDraftCleanup = {
        profileId: taskProfile.id,
        profileName: taskProfile.name,
        projectId: startedProjectId,
        sessionId: submissionId,
        phase: 'submitting',
        message: 'Task submission is in progress',
      };
      try {
        await persistPendingDraftCleanup(submissionJob);
      } catch (cause) {
        throw new Error(
          `Couldn’t secure this submission before starting: ${
            cause instanceof Error ? cause.message : String(cause)
          }`,
        );
      }
      const result = await runtime.createTask(
        resolution.context.projectId,
        resolution.context.provider,
        resolution.context.selection.isolated,
        value,
        {
          sessionId: submissionId,
          model: resolution.context.selection.model,
          reasoningEffort: resolution.context.selection.reasoningEffort,
          runtimeMode: resolution.context.selection.runtimeMode,
          baseBranch: resolution.context.selection.baseBranch,
        },
        resolution.result.providerText,
      );
      if (!result.promptAccepted) {
        await clearPendingDraftCleanup(submissionJob).catch(() => {});
        throw new Error(
          result.error || 'The agent failed to start. Your draft was kept.',
        );
      }
      const session = result.session;
      accepted = true;
      if (isCurrentOperation()) {
        setAcceptedSessionId(session.id);
        startedRef.current = true;
        promptRef.current = '';
        setPrompt('');
        setCursor(0);
      }
      const cleanupJob: PendingDraftCleanup = {
        profileId: taskProfile.id,
        profileName: taskProfile.name,
        projectId: startedProjectId,
        sessionId: session.id,
        phase: 'cleanup',
        message: 'Task accepted; waiting for draft cleanup',
      };
      try {
        await persistPendingDraftCleanup(cleanupJob);
      } catch (cause) {
        if (isCurrentOperation()) {
          if (taskDraftKey) draftFlush.discard(taskDraftKey);
          setDraftCleanupFailure({
            ...cleanupJob,
            message: cause instanceof Error
              ? cause.message
              : 'Couldn’t persist the accepted-task cleanup job',
          });
        }
        return;
      }
      if (!isCurrentOperation()) return;

      // A failed debounced write must not prevent removing the accepted draft.
      await draftFlush.flush().catch(() => {});
      if (!isCurrentOperation()) return;
      try {
        await applyComposerDraftChanges(taskClient, [{
          target: { type: 'newSession', projectId: startedProjectId },
          draft: null,
        }]);
      } catch (cause) {
        cleanupJob.message = cause instanceof Error
          ? cause.message
          : 'Couldn’t remove the draft for the started task';
        try {
          await persistPendingDraftCleanup(cleanupJob);
        } catch (storageCause) {
          cleanupJob.message = `${
            storageCause instanceof Error ? storageCause.message : String(storageCause)
          }. ${cleanupJob.message}`;
        }
        // The accepted task owns cleanup now; old composer write errors must
        // not make this recovery screen impossible to leave.
        if (isCurrentOperation()) {
          if (taskDraftKey) draftFlush.discard(taskDraftKey);
          setDraftCleanupFailure(cleanupJob);
        }
        return;
      }
      if (!isCurrentOperation()) return;
      try {
        await clearPendingDraftCleanup(cleanupJob);
      } catch (cause) {
        if (isCurrentOperation()) {
          setDraftCleanupFailure({
            ...cleanupJob,
            message: cause instanceof Error
              ? cause.message
              : 'Couldn’t clear the finished draft cleanup record',
          });
        }
        return;
      }
      if (!isCurrentOperation()) return;
      if (taskDraftKey) draftFlush.discard(taskDraftKey);
      void Haptics.notificationAsync(
        Haptics.NotificationFeedbackType.Success,
      ).catch(() => {});
      const address = daemon.activeProfile?.address;
      if (address) {
        void loadComposerPreferences(address).then((prefs) => {
          let next = rememberComposerSession(prefs, session);
          if (!session.model) {
            next = {
              ...next,
              lastProvider: session.provider,
              lastModel: null,
              lastReasoningEffort: session.reasoning_effort ?? null,
            };
          }
          return saveComposerPreferences(address, next);
        }).catch(() => {});
        void saveNewTaskExtras(address, {
          runtimeMode,
          isolated: isolated && !projectless,
          projectId: selectedProject.id,
        }).catch(() => {});
      }
      // The page becomes the session in place — no navigation, matching the
      // desktop where the composer stays put and the transcript starts above.
      setCreatedSessionId(session.id);
    } catch (cause) {
      if (!accepted && isCurrentOperation()) {
        setError(cause instanceof Error ? cause.message : String(cause));
        void Haptics.notificationAsync(
          Haptics.NotificationFeedbackType.Error,
        ).catch(() => {});
      }
    } finally {
      if (operationSequence.current === operation) {
        submittingRef.current = false;
        if (isCurrentOperation()) setSubmitting(false);
      }
    }
  }

  async function retryDraftCleanup() {
    const pending = draftCleanupFailure;
    if (
      !pending
      || pending.phase !== 'cleanup'
      || submitting
      || submittingRef.current
    ) return;
    const client = daemon.client;
    const profile = daemon.activeProfile;
    if (!client || profile?.id !== pending.profileId) {
      setDraftCleanupFailure({
        ...pending,
        message: `Switch to ${pending.profileName} to finish removing this draft.`,
      });
      return;
    }
    if (!claimNewTaskSubmission(submittingRef)) return;
    const operation = ++operationSequence.current;
    const draftKey = newTaskDraftScopeKey(pending.profileId, pending.projectId);
    const isCurrentOperation = () => (
      operationSequence.current === operation
      && activeProfileIdRef.current === pending.profileId
      && activeClientRef.current === client
    );
    setSubmitting(true);
    try {
      await persistPendingDraftCleanup(pending);
      if (!isCurrentOperation()) return;
      await applyComposerDraftChanges(client, [{
        target: { type: 'newSession', projectId: pending.projectId },
        draft: null,
      }]);
      if (!isCurrentOperation()) return;
      await clearPendingDraftCleanup(pending);
      if (!isCurrentOperation()) return;
      setDraftCleanupFailure(null);
      if (draftKey) draftFlush.discard(draftKey);
      setCreatedSessionId(pending.sessionId);
    } catch (cause) {
      if (isCurrentOperation()) {
        const failed = {
          ...pending,
          message: cause instanceof Error
            ? cause.message
            : 'Couldn’t remove the draft for the started task',
        };
        await persistPendingDraftCleanup(failed).catch(() => {});
        if (isCurrentOperation()) setDraftCleanupFailure(failed);
      }
    } finally {
      if (operationSequence.current === operation) {
        submittingRef.current = false;
        if (isCurrentOperation()) setSubmitting(false);
      }
    }
  }

  async function resetCleanupLookup() {
    if (cleanupLookupResetting) return;
    setCleanupLookupResetting(true);
    try {
      await resetPendingDraftCleanups();
      setCleanupLookupAttempt((value) => value + 1);
    } catch (cause) {
      setCleanupLookupError(
        cause instanceof Error
          ? cause.message
          : 'Couldn’t reset unfinished draft cleanup records',
      );
    } finally {
      setCleanupLookupResetting(false);
    }
  }

  const providerModels = modelCatalog.find((entry) => entry.id === provider)?.models ?? [];
  const activeModel = model
    ? providerModels.find((item) => item.id === model)
    : providerModels.find((item) => item.is_default);
  const modelLabel = !provider
    ? catalog.isPending ? 'Checking agents…' : 'No agents installed'
    : activeModel?.name ?? model ?? providerLabel(provider);
  const branchLabel = baseBranch
    ?? branches.data?.default_branch
    ?? branches.data?.current
    ?? 'Default branch';
  const startDisabled = !selectedProject
    || !provider
    || !prompt.trim()
    || submitting
    || !draftReady
    || Boolean(acceptedSessionId)
    || cleanupLookupState !== 'ready';
  const hasComposerFeedback = skillWarnings.length > 0
    || Boolean(error)
    || cleanupLookupState === 'error'
    || Boolean(draftHydrationError)
    || Boolean(draftCleanupFailure);

  if (createdSessionId) {
    return (
      <SessionView
        initialSkillWarnings={skillWarnings}
        sessionId={createdSessionId}
      />
    );
  }

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      style={[styles.screen, { backgroundColor: theme.background }]}>
      <ScreenHeader title="New Task" />
      <View style={styles.spacer} />

      <View style={styles.rows}>
        <SelectorRow
          icon="laptop"
          label="Daemon"
          loading={daemon.phase === 'connecting' || daemon.phase === 'booting'}
          value={daemon.activeProfile?.name ?? 'Add a daemon'}
          onPress={() => setOpenSheet('daemon')}
        />
        <SelectorRow
          icon="folder"
          label="Project"
          loading={taskState.isPending}
          value={selectedProject?.name ?? 'Choose a project'}
          onPress={() => setOpenSheet('project')}
        />
        <SelectorRow
          icon="sparkle"
          label="Model"
          loading={catalog.isPending}
          value={modelLabel}
          onPress={() => setOpenSheet('model')}
        />
        <SelectorRow
          icon="laptop"
          label="Workspace"
          value={isolated ? 'Isolated worktree' : 'Work locally'}
          onPress={() => setOpenSheet('workspace')}
        />
        {isolated && (
          <SelectorRow
            icon="gitBranch"
            label="Base branch"
            loading={branches.isPending && branches.fetchStatus !== 'idle'}
            value={branchLabel}
            onPress={() => setOpenSheet('branch')}
          />
        )}
      </View>

      <View style={[styles.composerShell, { paddingBottom: Math.max(insets.bottom, 10) }]}>
        {skillAutocompleteVisible && (
          <ComposerSkillAutocomplete
            commands={skillRows}
            highlight={skillHighlight}
            loading={skillAutocompleteLoading}
            onAccept={acceptSkill}
            onHighlight={(index) => {
              if (skillAutocompleteKey) {
                setSkillSelection({ key: skillAutocompleteKey, index });
              }
            }}
          />
        )}
        {hasComposerFeedback && (
          <ScrollView
            accessibilityLabel="Task recovery messages"
            keyboardShouldPersistTaps="handled"
            nestedScrollEnabled
            style={styles.feedbackScroll}
            contentContainerStyle={styles.feedbackContent}>
            {skillWarnings.length > 0 && (
              <View
                accessibilityLiveRegion="polite"
                style={[styles.warning, { backgroundColor: theme.warningSoft }]}>
                <Text style={[styles.warningText, { color: theme.warning }]}>
                  {skillWarnings.join(' ')}
                </Text>
                <Pressable
                  accessibilityLabel="Dismiss skill warning"
                  accessibilityRole="button"
                  onPress={() => setSkillWarnings([])}
                  style={styles.warningDismiss}>
                  <PaduIcon name="x" size={14} tintColor={theme.warning} />
                </Pressable>
              </View>
            )}
            {error && (
              <View
                accessibilityLiveRegion="polite"
                style={[styles.error, { backgroundColor: theme.dangerSoft }]}>
                <Text style={[styles.errorText, { color: theme.danger }]}>{error}</Text>
              </View>
            )}
            {cleanupLookupState === 'error' && (
              <View
                accessibilityLiveRegion="polite"
                style={[styles.error, { backgroundColor: theme.dangerSoft }]}>
                <Text style={[styles.errorText, { color: theme.danger }]}>
                  {cleanupLookupError || 'Couldn’t check for unfinished draft cleanup'}
                </Text>
                <Text style={[styles.errorText, { color: theme.danger }]}>
                  Reset these unfinished-task records only if you accept that a
                  pending draft cleanup cannot be recovered automatically.
                </Text>
                <Pressable
                  accessibilityLabel="Retry checking unfinished draft cleanup"
                  accessibilityRole="button"
                  accessibilityState={{ busy: cleanupLookupResetting }}
                  disabled={cleanupLookupResetting}
                  onPress={() => setCleanupLookupAttempt((value) => value + 1)}
                  style={styles.retryAction}>
                  <Text style={[styles.errorText, { color: theme.accent }]}>Retry check</Text>
                </Pressable>
                <Pressable
                  accessibilityLabel="Reset unreadable unfinished draft cleanup records"
                  accessibilityRole="button"
                  accessibilityState={{ busy: cleanupLookupResetting }}
                  disabled={cleanupLookupResetting}
                  onPress={() => void resetCleanupLookup()}
                  style={styles.retryAction}>
                  <Text style={[styles.errorText, { color: theme.accent }]}>
                    {cleanupLookupResetting ? 'Resetting…' : 'Reset cleanup records'}
                  </Text>
                </Pressable>
              </View>
            )}
            {draftHydrationError && (
              <View
                accessibilityLiveRegion="polite"
                style={[styles.error, { backgroundColor: theme.dangerSoft }]}>
                <Text style={[styles.errorText, { color: theme.danger }]}>
                  {draftHydrationError}
                </Text>
                <Pressable
                  accessibilityLabel="Retry loading the draft"
                  accessibilityRole="button"
                  onPress={() => {
                    setDraftHydrationError(null);
                    setDraftHydrationAttempt((value) => value + 1);
                  }}
                  style={styles.retryAction}>
                  <Text style={[styles.errorText, { color: theme.accent }]}>Retry</Text>
                </Pressable>
              </View>
            )}
            {draftCleanupFailure && (
              <View
                accessibilityLiveRegion="polite"
                style={[styles.error, { backgroundColor: theme.dangerSoft }]}>
                <Text style={[styles.errorText, { color: theme.danger }]}>
                  Task started, but its old draft could not be removed: {draftCleanupFailure.message}
                </Text>
                <Pressable
                  accessibilityLabel="Retry removing the started task draft"
                  accessibilityRole="button"
                  accessibilityState={{ busy: submitting, disabled: submitting }}
                  disabled={submitting}
                  onPress={() => void retryDraftCleanup()}
                  style={styles.retryAction}>
                  <Text style={[styles.errorText, { color: theme.accent }]}>Retry cleanup</Text>
                </Pressable>
              </View>
            )}
          </ScrollView>
        )}
        <ComposerCard
          accessibilityLabel="Task prompt"
          autoFocus
          editable={
            !submitting
            && draftReady
            && !acceptedSessionId
            && cleanupLookupState === 'ready'
          }
          left={(
            <>
              <ComposerIconButton
                active={runtimeMode !== 'fullAccess'}
                icon="lock"
                label="Agent access"
                onPress={() => setOpenSheet('access')}
              />
            </>
          )}
          placeholder={`Work on ${daemon.activeProfile?.name ?? 'your daemon'}`}
          right={(
            <SendButton
              busy={submitting}
              disabled={startDisabled}
              label="Start task"
              onPress={() => void start()}
            />
          )}
          value={prompt}
          onBlur={() => {
            if (skillBlurTimer.current) clearTimeout(skillBlurTimer.current);
            skillBlurTimer.current = setTimeout(() => setInputFocused(false), 120);
          }}
          onChangeText={(text) => {
            setPrompt(text);
            setCursor((current) => Math.min(current, text.length));
            setDismissedSkillAutocomplete(null);
          }}
          onFocus={() => {
            if (skillBlurTimer.current) clearTimeout(skillBlurTimer.current);
            setInputFocused(true);
          }}
          onKeyPress={handleComposerKeyPress}
          onSelectionChange={(event) => {
            setCursor(event.nativeEvent.selection.end);
          }}
        />
      </View>

      <Sheet onDismiss={() => setOpenSheet(null)} title="Daemon" visible={openSheet === 'daemon'}>
        {daemon.profiles.map((profile) => (
          <SheetRow
            description={profile.address}
            key={profile.id}
            label={profile.name}
            onPress={pick(() => void daemon.selectProfile(profile.id))}
            selected={profile.id === daemon.activeProfile?.id}
          />
        ))}
        <SheetRow
          label="Manage daemons…"
          leading={(
            <PaduIcon
              name="settings"
              size={16}
              tintColor={theme.textSecondary}
            />
          )}
          onPress={() => {
            setOpenSheet(null);
            router.push('/daemons');
          }}
        />
      </Sheet>

      <Sheet onDismiss={() => setOpenSheet(null)} title="Project" visible={openSheet === 'project'}>
        {projects.map((project) => (
          <SheetRow
            description={project.path}
            key={project.id}
            label={project.name}
            onPress={pick(() => setProjectId(project.id))}
            selected={project.id === projectId}
          />
        ))}
        <SheetRow
          description="Pick any folder on the daemon host, or create an empty workspace"
          label="Browse daemon host…"
          leading={(
            <PaduIcon
              name="server"
              size={16}
              tintColor={theme.textSecondary}
            />
          )}
          onPress={() => {
            setOpenSheet(null);
            setProjectPickerOpen(true);
          }}
        />
      </Sheet>

      <ModelPickerSheet
        model={model}
        onApply={(selection) => {
          setProvider(selection.provider);
          setModel(selection.model);
          setReasoningEffort(selection.reasoningEffort);
        }}
        onDismiss={() => setOpenSheet(null)}
        provider={provider}
        providers={installedProviders}
        visible={openSheet === 'model'}
      />

      <Sheet onDismiss={() => setOpenSheet(null)} title="Workspace" visible={openSheet === 'workspace'}>
        <SheetRow
          description="Run in the project checkout"
          label="Work locally"
          onPress={pick(() => setIsolated(false))}
          selected={!isolated}
        />
        <SheetRow
          description="A separate branch and folder that never touches the checkout"
          disabled={projectless}
          label="Isolated worktree"
          onPress={pick(() => setIsolated(true))}
          selected={isolated}
        />
      </Sheet>

      <Sheet onDismiss={() => setOpenSheet(null)} title="Base branch" visible={openSheet === 'branch'}>
        {branches.isPending ? (
          <View style={styles.sheetLoading}>
            <ActivityIndicator color={theme.textTertiary} />
          </View>
        ) : branches.error ? (
          <Text style={[styles.sheetNote, { color: theme.danger }]}>
            {branches.error instanceof Error ? branches.error.message : String(branches.error)}
          </Text>
        ) : !branches.data ? (
          <Text style={[styles.sheetNote, { color: theme.textTertiary }]}>
            This project isn’t a Git repository.
          </Text>
        ) : (
          branches.data.branches.map((branch) => (
            <SheetRow
              description={branch.name === branches.data?.current ? 'Current branch' : undefined}
              key={branch.name}
              label={branch.name}
              onPress={pick(() => setBaseBranch(branch.name))}
              selected={branch.name === (baseBranch ?? branches.data?.default_branch ?? branches.data?.current)}
            />
          ))
        )}
      </Sheet>

      <AccessSheet
        mode={runtimeMode}
        onApply={setRuntimeMode}
        onDismiss={() => setOpenSheet(null)}
        visible={openSheet === 'access'}
      />
      <RemoteProjectPicker
        visible={projectPickerOpen}
        onDismiss={() => setProjectPickerOpen(false)}
        onSelect={(project) => setProjectId(project.id)}
      />
    </KeyboardAvoidingView>
  );
}

function SelectorRow({
  icon,
  label,
  value,
  onPress,
  loading = false,
}: {
  icon: PaduIconName;
  label: string;
  value: string;
  onPress: () => void;
  loading?: boolean;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityLabel={`${label}: ${value}`}
      accessibilityRole="button"
      disabled={loading}
      onPress={onPress}
      style={({ pressed }) => [
        styles.row,
        { backgroundColor: pressed ? theme.overlay : 'transparent' },
      ]}>
      <PaduIcon name={icon} size={19} tintColor={theme.textSecondary} />
      {loading ? (
        <ActivityIndicator color={theme.textTertiary} size="small" />
      ) : (
        <Text numberOfLines={1} style={[styles.rowValue, { color: theme.text }]}>
          {value}
        </Text>
      )}
      <PaduIcon
        name="chevronsUpDown"
        size={13}
        tintColor={theme.textTertiary}
      />
    </Pressable>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  spacer: { flex: 1 },
  rows: { gap: 2, paddingBottom: 8, paddingHorizontal: Spacing.three },
  row: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    flexDirection: 'row',
    gap: 14,
    minHeight: 52,
    paddingHorizontal: 6,
  },
  rowValue: { flexShrink: 1, fontSize: 16.5, fontWeight: '500' },
  composerShell: { paddingHorizontal: 12 },
  feedbackScroll: { flexGrow: 0, maxHeight: Spacing.six * 3 },
  feedbackContent: { paddingBottom: Spacing.one },
  error: { borderRadius: Radius.medium, marginBottom: 8, padding: 11 },
  errorText: { fontSize: 12.5, fontWeight: '600', lineHeight: 17 },
  retryAction: {
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: TOUCH_TARGET,
    minWidth: TOUCH_TARGET,
  },
  warning: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    flexDirection: 'row',
    gap: Spacing.two,
    marginBottom: Spacing.two,
    padding: Spacing.two + Spacing.one,
  },
  warningDismiss: {
    alignItems: 'center',
    height: TOUCH_TARGET,
    justifyContent: 'center',
    width: TOUCH_TARGET,
  },
  warningText: { flex: 1, fontSize: 12.5, fontWeight: '600', lineHeight: 17 },
  sheetLoading: { alignItems: 'center', paddingVertical: 14 },
  sheetSection: {
    fontSize: 11,
    fontWeight: '700',
    letterSpacing: 0.5,
    marginBottom: 4,
    marginHorizontal: 12,
    marginTop: 12,
  },
  sheetNote: { fontSize: 13, lineHeight: 18, paddingHorizontal: 12, paddingVertical: 10 },
});
