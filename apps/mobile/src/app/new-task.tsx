import { useQuery } from '@tanstack/react-query';
import type { ProviderKind, RuntimeMode } from '@padu/client';
import { rememberComposerSession } from '@padu/client/composer-preferences';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { PaduIcon, type PaduIconName } from '@/components/padu-icon';
import {
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
import { useAllProviderModels, useProviderCatalog, useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import {
  applyComposerDraftChanges,
  daemonKeys,
  inspectBranches,
  loadComposerDrafts,
} from '@/lib/daemon-api';
import {
  loadComposerPreferences,
  loadNewTaskExtras,
  saveComposerPreferences,
  saveNewTaskExtras,
} from '@/lib/composer-preferences-store';
import { useDaemon } from '@/lib/daemon-context';
import { useRuntime } from '@/lib/runtime-context';
import { providerLabel } from '@/lib/session-presentation';

type SheetKind = 'daemon' | 'project' | 'model' | 'workspace' | 'branch' | 'access';

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
  const [submitting, setSubmitting] = useState(false);
  const [createdSessionId, setCreatedSessionId] = useState<string | null>(null);
  const [openSheet, setOpenSheet] = useState<SheetKind | null>(null);
  const [projectPickerOpen, setProjectPickerOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const installedProviders = useMemo(
    () => catalog.providers.filter((item) => item.installed).map((item) => item.id),
    [catalog.providers],
  );
  const modelCatalog = useAllProviderModels(installedProviders);
  const projects = taskState.data?.projects ?? [];
  const selectedProject = projects.find((item) => item.id === projectId);
  const projectless = selectedProject?.name === 'No project';
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
  const draftTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const draftLoadedFor = useRef<string | null>(null);
  const promptRef = useRef(prompt);
  promptRef.current = prompt;
  useEffect(() => {
    const client = daemon.client;
    const target = selectedProject?.id;
    if (!client || daemon.phase !== 'connected' || !target) return;
    if (draftLoadedFor.current === target) return;
    draftLoadedFor.current = target;
    void loadComposerDrafts(client).then((drafts) => {
      const text = drafts.new_sessions?.[target]?.text;
      if (text && !promptRef.current.trim()) setPrompt(text);
    }).catch(() => {});
  }, [daemon.client, daemon.phase, selectedProject?.id]);
  useEffect(() => {
    const client = daemon.client;
    const target = selectedProject?.id;
    if (!client || !target || draftLoadedFor.current !== target) return;
    if (draftTimer.current) clearTimeout(draftTimer.current);
    draftTimer.current = setTimeout(() => {
      void applyComposerDraftChanges(client, [{
        target: { type: 'newSession', projectId: target },
        draft: prompt.trim() ? { text: prompt } : null,
      }]).catch(() => {});
    }, 800);
    return () => {
      if (draftTimer.current) clearTimeout(draftTimer.current);
    };
  }, [daemon.client, prompt, selectedProject?.id]);

  function pick(apply: () => void) {
    return () => {
      void Haptics.selectionAsync();
      apply();
      setOpenSheet(null);
    };
  }

  async function start() {
    const value = prompt.trim();
    if (!selectedProject || !provider || !value || submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const session = await runtime.createTask(
        selectedProject.id,
        provider,
        isolated && !projectless,
        value,
        { model, reasoningEffort, runtimeMode, baseBranch },
      );
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
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
      if (draftTimer.current) clearTimeout(draftTimer.current);
      if (daemon.client) {
        void applyComposerDraftChanges(daemon.client, [{
          target: { type: 'newSession', projectId: selectedProject.id },
          draft: null,
        }]).catch(() => {});
      }
      // The page becomes the session in place — no navigation, matching the
      // desktop where the composer stays put and the transcript starts above.
      setCreatedSessionId(session.id);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
      setSubmitting(false);
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
  const startDisabled = !selectedProject || !provider || !prompt.trim() || submitting;

  if (createdSessionId) return <SessionView sessionId={createdSessionId} />;

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
        {error && (
          <View
            accessibilityLiveRegion="polite"
            style={[styles.error, { backgroundColor: theme.dangerSoft }]}>
            <Text style={[styles.errorText, { color: theme.danger }]}>{error}</Text>
          </View>
        )}
        <ComposerCard
          accessibilityLabel="Task prompt"
          autoFocus
          editable={!submitting}
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
          onChangeText={setPrompt}
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
  error: { borderRadius: Radius.medium, marginBottom: 8, padding: 11 },
  errorText: { fontSize: 12.5, fontWeight: '600', lineHeight: 17 },
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
