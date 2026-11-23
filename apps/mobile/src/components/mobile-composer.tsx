import type {
  AgentSession,
  ContextWindowSource,
  EmbeddedNote,
  MessageAttachment,
  PendingPermission,
  PendingUserInput,
  UserInputAnswer,
} from '@padu/client';
import {
  parseCompactionSubmission,
  supportsCompaction,
} from '@padu/client';
import {
  detectSkillTrigger,
  filterSkillCommands,
  replaceSkillTrigger,
  SKILL_AUTOCOMPLETE_CAP,
  type SkillCommand,
} from '@padu/client/composer-skills';
import * as Haptics from 'expo-haptics';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import {
  ActivityIndicator,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
  type ScrollViewInstance,
  type TextInputKeyPressEvent,
  type TextInputProps,
} from 'react-native';
import { useQuery } from '@tanstack/react-query';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { PaduIcon, type PaduIconName } from './padu-icon';
import { ContextGauge } from './context-gauge';
import { AccessSheet, ModelSheet } from './session-option-sheets';
import { MonoFont, NativeTint, Radius, Spacing } from '@/constants/theme';
import { useComposerCommands, useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import {
  applyComposerDraftChanges,
  daemonKeys,
  inspectBranches,
  loadComposerDrafts,
} from '@/lib/daemon-api';
import { useDaemon } from '@/lib/daemon-context';
import {
  createDraftFlushScheduler,
} from '@/lib/draft-flush';
import {
  composerDraftSnapshot,
  composeMobileProviderPrompt,
  queuedMessageMetadata,
  queuedVisiblePrompt,
  sessionBusy,
  sessionCwd,
} from '@/lib/mobile-runtime';
import { reconcileSkillWarningSession } from '@/lib/skill-warning-state';
import {
  mobilePromptSelectionContext,
  renderSkillSubmissionPrompt,
  useMobilePromptResolutionGuard,
} from '@/lib/skill-submission';
import { useRuntime } from '@/lib/runtime-context';

const COMPOSER_HIT_TARGET = 44;
const AUTOCOMPLETE_BLUR_DELAY_MS = 300;
const EMPTY_SKILL_WARNINGS: readonly string[] = [];

/**
 * The composer surface shared by the session screen and the new-task screen:
 * a tall rounded card holding the input with an icon toolbar underneath —
 * option toggles on the left, meters and the send button on the right.
 */
export function ComposerCard({
  left,
  right,
  ...inputProps
}: TextInputProps & {
  left?: ReactNode;
  right?: ReactNode;
}) {
  const theme = useTheme();
  return (
    <View style={[styles.card, { backgroundColor: theme.composer, borderColor: theme.border }]}>
      <TextInput
        multiline
        placeholderTextColor={theme.textTertiary}
        selectionColor={NativeTint}
        style={[styles.input, { color: theme.text }]}
        {...inputProps}
      />
      <View style={styles.toolbar}>
        <View style={styles.cluster}>{left}</View>
        <View style={styles.toolbarSpacer} />
        <View style={styles.cluster}>{right}</View>
      </View>
    </View>
  );
}

export function ComposerIconButton({
  icon,
  label,
  onPress,
  active = false,
  disabled = false,
}: {
  icon: PaduIconName;
  label: string;
  onPress: () => void;
  active?: boolean;
  disabled?: boolean;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityLabel={label}
      accessibilityRole="button"
      accessibilityState={{ disabled, selected: active }}
      disabled={disabled}
      hitSlop={4}
      onPress={onPress}
      style={({ pressed }) => [
        styles.iconButton,
        {
          backgroundColor: active ? theme.overlayStrong : 'transparent',
          opacity: disabled ? 0.35 : pressed ? 0.55 : 1,
        },
      ]}>
      <PaduIcon name={icon} size={19} tintColor={active ? theme.accent : theme.textSecondary} />
    </Pressable>
  );
}

export function SendButton({
  onPress,
  disabled,
  busy = false,
  steering = false,
  queueing = false,
  label,
}: {
  onPress: () => void;
  disabled: boolean;
  busy?: boolean;
  steering?: boolean;
  queueing?: boolean;
  label: string;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityLabel={label}
      accessibilityRole="button"
      disabled={disabled}
      hitSlop={4}
      onPress={onPress}
      style={({ pressed }) => [
        styles.sendButton,
        {
          backgroundColor: disabled
            ? theme.surfaceMuted
            : steering ? NativeTint : theme.inverse,
          opacity: pressed || busy ? 0.6 : 1,
        },
      ]}>
      {busy ? (
        <ActivityIndicator color={disabled ? theme.textTertiary : theme.onInverse} size="small" />
      ) : (
        <PaduIcon
          name="arrowUp"
          size={17}
          tintColor={disabled ? theme.textTertiary : steering ? '#ffffff' : theme.onInverse}
        />
      )}
    </Pressable>
  );
}

type QueuedMessage = NonNullable<AgentSession['queued_messages']>[number];

/**
 * Status card above the composer input: queued follow-ups (each with steer
 * and delete actions) and the worktree/branch line. The location line hides
 * when the project has no git — a local checkout without a resolvable
 * branch renders nothing instead of a bare "Local" label. The whole card
 * renders nothing while there is nothing to show. The context-window meter
 * lives below the input instead, in ComposerContextRow.
 */
function ComposerStatusCard({
  session,
  queued,
  canSteer,
  onEditMessage,
  onSteerMessage,
  onRemoveMessage,
}: {
  session: AgentSession;
  queued: QueuedMessage[];
  canSteer: boolean;
  onEditMessage: (message: QueuedMessage) => void;
  onSteerMessage: (message: QueuedMessage) => void;
  onRemoveMessage: (messageId: string) => void;
}) {
  const theme = useTheme();
  const daemon = useDaemon();
  const workspace = session.workspace;
  const isWorktree = workspace?.kind === 'worktree';
  const projectPath = useTaskState().data?.projects
    .find((item) => item.id === session.project_id)?.path ?? null;
  const workspaceBranch = workspace?.kind === 'worktree'
    ? workspace.branch
    : workspace?.kind === 'newWorktree'
      ? workspace.baseBranch ?? null
      : null;
  // A local checkout carries no branch on the session itself, so resolve it
  // from the daemon's branch snapshot for the session's project — the same
  // source the new-task screen uses. Only queried while the workspace
  // branch is unknown.
  const branchQuery = useQuery({
    queryKey: daemonKeys.branches(
      daemon.activeProfile?.id ?? 'disconnected',
      projectPath ?? 'missing',
    ),
    queryFn: () => inspectBranches(daemon.client!, projectPath!),
    enabled: daemon.phase === 'connected'
      && Boolean(daemon.client && projectPath)
      && !workspaceBranch,
    staleTime: 30_000,
  });
  const branch = workspaceBranch
    ?? branchQuery.data?.current
    ?? branchQuery.data?.detached_head
    ?? null;
  // No git, no line: keep the row while the snapshot is still resolving,
  // hide it once it resolves without a branch on a local checkout.
  const showLocation = isWorktree || Boolean(branch) || branchQuery.isLoading;

  if (!queued.length && !showLocation) return null;

  return (
    <View style={[styles.statusCard, { backgroundColor: theme.composer, borderColor: theme.border }]}>
      {queued.map((message) => (
        <View key={message.id}>
          <View style={styles.queuedItem}>
            <Pressable
              accessibilityHint="Restores this queued message to the composer"
              accessibilityLabel={`Edit queued message: ${queuedVisiblePrompt(message)}`}
              accessibilityRole="button"
              onPress={() => onEditMessage(message)}
              style={({ pressed }) => [
                styles.queuedEdit,
                { opacity: pressed ? 0.55 : 1 },
              ]}>
              <PaduIcon
                name="clock"
                size={12}
                tintColor={theme.textTertiary}
              />
              <Text numberOfLines={1} style={[styles.queuedText, { color: theme.textSecondary }]}>
                {queuedVisiblePrompt(message)}
              </Text>
            </Pressable>
            <View style={styles.queueActions}>
              {canSteer && (
                <Pressable
                  accessibilityHint="Sends this queued message to the working agent now"
                  accessibilityLabel="Steer working agent"
                  accessibilityRole="button"
                  hitSlop={4}
                  onPress={() => onSteerMessage(message)}
                  style={({ pressed }) => [styles.queueActionButton, { opacity: pressed ? 0.55 : 1 }]}>
                  <PaduIcon
                    name="cornerDownRight"
                    size={13}
                    tintColor={theme.textTertiary}
                  />
                  <Text style={[{ fontSize: 12, fontWeight: '700' }, { color: theme.textTertiary }]}>Steer</Text>
                </Pressable>
              )}
              <Pressable
                accessibilityLabel="Remove queued message"
                accessibilityRole="button"
                hitSlop={4}
                onPress={() => onRemoveMessage(message.id)}
                style={({ pressed }) => [styles.queueActionButton, { opacity: pressed ? 0.55 : 1 }]}>
                <PaduIcon
                  name="trash"
                  size={13}
                  tintColor={theme.textTertiary}
                />
              </Pressable>
            </View>
          </View>
          <View style={[styles.queueSeparator, { backgroundColor: theme.border }]} />
        </View>
      ))}
      {showLocation && (
        <View
          accessibilityLabel={[isWorktree ? 'worktree' : 'local', branch].filter(Boolean).join(', ')}
          accessibilityRole="text"
          style={styles.statusLine}>
          {isWorktree ? (
            <View style={styles.statusItem}>
              <PaduIcon name="fork" size={12} tintColor={theme.textTertiary} />
              <Text style={[styles.statusText, { color: theme.textSecondary }]}>Worktree</Text>
            </View>
          ) : (
            <View style={styles.statusItem}>
              <PaduIcon name="laptop" size={12} tintColor={theme.textTertiary} />
              <Text style={[styles.statusText, { color: theme.textSecondary }]}>Local</Text>
            </View>
          )}
          {branch && (
            <View style={styles.statusItem}>
              <PaduIcon name="gitBranch" size={12} tintColor={theme.textTertiary} />
              <Text numberOfLines={1} style={[styles.statusText, { color: theme.textSecondary }]}>
                {branch}
              </Text>
            </View>
          )}
        </View>
      )}
    </View>
  );
}

export function ComposerSkillAutocomplete({
  commands,
  highlight,
  loading,
  onAccept,
  onHighlight,
}: {
  commands: SkillCommand[];
  highlight: number;
  loading: boolean;
  onAccept: (index: number) => void;
  onHighlight: (index: number) => void;
}) {
  const theme = useTheme();
  const listRef = useRef<ScrollViewInstance>(null);
  useEffect(() => {
    listRef.current?.scrollTo({ y: highlight * COMPOSER_HIT_TARGET, animated: false });
  }, [highlight]);
  return (
    <View
      accessibilityLabel="Skill suggestions"
      accessibilityLiveRegion="polite"
      style={[
        styles.skillAutocomplete,
        {
          backgroundColor: theme.surface,
          borderColor: theme.border,
        },
      ]}>
      {loading && !commands.length ? (
        <View style={styles.skillLoading}>
          <PaduIcon name="clock" size={14} tintColor={theme.textTertiary} />
          <Text style={[styles.skillLoadingText, { color: theme.textTertiary }]}>
            Loading skills…
          </Text>
        </View>
      ) : (
        <ScrollView
          keyboardShouldPersistTaps="always"
          ref={listRef}
          style={styles.skillList}>
          {commands.map((command, index) => (
            <Pressable
              accessibilityHint={command.description || undefined}
              accessibilityLabel={`Use skill ${command.name}`}
              accessibilityRole="button"
              accessibilityState={{ selected: highlight === index }}
              key={command.name}
              onPress={() => onAccept(index)}
              onPressIn={() => onHighlight(index)}
              style={({ pressed }) => [
                styles.skillRow,
                {
                  backgroundColor: highlight === index
                    ? theme.backgroundSelected
                    : pressed
                      ? theme.overlayStrong
                      : 'transparent',
                  borderColor: theme.separator,
                },
              ]}>
              <PaduIcon name="sparkle" size={14} tintColor={theme.textTertiary} />
              <View style={styles.skillRowCopy}>
                <Text numberOfLines={1} style={[styles.skillRowName, { color: theme.text }]}>
                  ${command.name}
                </Text>
                {command.description ? (
                  <Text
                    numberOfLines={2}
                    style={[styles.skillRowDescription, { color: theme.textSecondary }]}>
                    {command.description}
                  </Text>
                ) : null}
              </View>
              <Text style={[styles.skillRowScope, { color: theme.textTertiary }]}>Skill</Text>
            </Pressable>
          ))}
        </ScrollView>
      )}
    </View>
  );
}

/**
 * Read-only context-window meter below the composer card, shrunk to its
 * content and hugging the right edge. Renders nothing until the daemon
 * reports usage or a selected window.
 */
function ComposerContextRow({
  session,
  onPress,
}: {
  session: AgentSession;
  onPress?: () => void;
}) {
  const theme = useTheme();
  const usage = session.context_usage;
  const legacyContextWindow = session.provider === 'claude' ? null : session.context_window;
  const percent = usage?.window ? Math.round(usage.tokens * 100 / usage.window) : null;
  if (!usage && !legacyContextWindow) return null;
  const canCompact = supportsCompaction(session.provider);
  const source = contextSourceLabel(usage?.source);
  const overBudget = usage?.window && usage.tokens > usage.window
    ? usage.tokens - usage.window
    : 0;

  const summary = percent != null
    ? `Effective context ${percent}%${overBudget > 0 ? `, ${formatTokens(overBudget)} over budget` : ''}${source ? `, ${source}` : ''}`
    : usage
      ? `Effective context ${formatTokens(usage.tokens)} tokens${source ? `, ${source}` : ''}`
      : legacyContextWindow;
  const content = (
    <View
      accessible={!canCompact}
      accessibilityHint={!canCompact ? undefined : 'Opens context compaction options'}
      accessibilityLabel={summary || 'Session context'}
      accessibilityRole={!canCompact ? 'text' : undefined}
      style={styles.contextRow}>
      <Text style={[styles.statusText, { color: theme.textTertiary }]}>Effective context</Text>
      {percent != null && usage?.window ? (
        <>
          <Text style={[styles.statusText, { color: theme.textTertiary }]}>
            {formatTokens(usage.tokens)} / {formatTokens(usage.window)}
            {overBudget > 0 ? ` · ${formatTokens(overBudget)} over` : ''}
          </Text>
          <ContextGauge percent={percent} />
        </>
      ) : (
        <>
          <PaduIcon name="gauge" size={12} tintColor={theme.textTertiary} />
          <Text style={[styles.statusText, { color: theme.textTertiary }]}>
            {usage ? formatTokens(usage.tokens) : legacyContextWindow}
          </Text>
        </>
      )}
      {canCompact && (
        <PaduIcon
          name={session.compacting ? 'loaderCircle' : 'sparkle'}
          size={12}
          tintColor={session.compacting ? theme.accent : theme.textTertiary}
        />
      )}
    </View>
  );
  if (!canCompact || !onPress) return content;
  return (
    <Pressable
      accessibilityHint="Opens context compaction options"
      accessibilityLabel={summary || 'Session context'}
      accessibilityRole="button"
      accessibilityState={{ disabled: session.compacting }}
      hitSlop={12}
      onPress={onPress}
      style={({ pressed }) => ({ opacity: pressed ? 0.6 : 1 })}>
      {content}
    </Pressable>
  );
}

function contextSourceLabel(source: ContextWindowSource | null | undefined) {
  switch (source) {
    case 'harnessEnvironment': return 'CLAUDE_CODE_AUTO_COMPACT_WINDOW';
    case 'claudeModelOneMillion': return 'model name ([1m])';
    case 'claudeModelDefault': return 'default for non-[1m] model';
    case 'provider': return 'provider-reported window';
    case 'unknown':
    default: return 'budget unknown';
  }
}

function formatTokens(tokens: number) {
  if (tokens >= 999_500) return `${(tokens / 1_000_000).toFixed(1)}M`;
  return tokens >= 1_000 ? `${(tokens / 1_000).toFixed(1)}k` : `${tokens}`;
}

export function MobileComposer({
  session,
  initialSkillWarnings = EMPTY_SKILL_WARNINGS,
}: {
  session: AgentSession;
  initialSkillWarnings?: readonly string[];
}) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const [draft, setDraft] = useState('');
  const [preservedAttachments, setPreservedAttachments] = useState<MessageAttachment[]>([]);
  const [preservedEmbeddedNotes, setPreservedEmbeddedNotes] = useState<EmbeddedNote[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [compactPanelOpen, setCompactPanelOpen] = useState(false);
  const [compactFocus, setCompactFocus] = useState('');
  const supportsCompactionFocus = session.provider === 'claude' || session.provider === 'agy';
  const [skillWarnings, setSkillWarnings] = useState<readonly string[]>(
    initialSkillWarnings,
  );
  const [modelSheetOpen, setModelSheetOpen] = useState(false);
  const [accessSheetOpen, setAccessSheetOpen] = useState(false);
  const [cursor, setCursor] = useState(0);
  const [inputFocused, setInputFocused] = useState(false);
  const [skillSelection, setSkillSelection] = useState({ key: '', index: 0 });
  const [dismissedSkillAutocomplete, setDismissedSkillAutocomplete] = useState<string | null>(null);
  const busy = sessionBusy(session);
  const liveRuntime = runtime.runtimes[session.id];
  const canSteer = busy && Boolean(liveRuntime?.supportsSteer) && session.status !== 'connecting';
  const permission = runtime.permissions[session.id];
  const userInput = runtime.userInputs[session.id];
  const runtimeError = runtime.errors[session.id];
  const queued = session.queued_messages ?? [];
  const project = useTaskState().data?.projects.find(
    (candidate) => candidate.id === session.project_id,
  );
  const composerCwd = project ? sessionCwd(session, project) : null;
  const composerCommands = useComposerCommands(session.provider, composerCwd);
  const skillCatalog = composerCommands.data;
  const skillCommands = skillCatalog?.commands ?? [];
  const promptResolutionContext = {
    hostId: daemon.activeProfile?.id ?? null,
    hostAddress: daemon.activeProfile?.address ?? null,
    sessionId: session.id,
    projectId: session.project_id,
    projectRoot: composerCwd ?? '',
    provider: session.provider,
    binaryOverride: composerCommands.binaryOverride,
    binaryVersion: composerCommands.binaryVersion,
    binaryIdentity: composerCommands.binaryIdentity,
    selection: mobilePromptSelectionContext(session.workspace, {
      model: session.model ?? null,
      reasoningEffort: session.reasoning_effort ?? null,
      runtimeMode: session.runtime_mode,
      interactionMode: session.interaction_mode,
      agentPreset: session.agent_preset ?? null,
      serviceTier: session.service_tier ?? null,
      isolated: session.workspace?.kind === 'newWorktree',
      baseBranch: session.workspace?.kind === 'newWorktree'
        ? session.workspace.baseBranch ?? null
        : null,
    }),
  };
  const resolvePromptInStableContext = useMobilePromptResolutionGuard(
    promptResolutionContext,
  );
  const skillTrigger = inputFocused
    ? detectSkillTrigger(draft, cursor)
    : null;
  const skillAutocompleteKey = skillTrigger
    ? `${skillTrigger.start}:${skillTrigger.end}:${skillTrigger.query}`
    : null;
  const skillRows = skillTrigger
    ? filterSkillCommands(
        skillCommands,
        skillTrigger.query,
        SKILL_AUTOCOMPLETE_CAP,
      )
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

  const warningSession = useRef({
    sessionId: session.id,
    warnings: initialSkillWarnings,
  });
  useEffect(() => {
    setLocalError(null);
    const next = reconcileSkillWarningSession(
      warningSession.current,
      session.id,
      initialSkillWarnings,
    );
    if (next !== warningSession.current) {
      warningSession.current = next;
      setSkillWarnings(next.warnings);
    }
    setDismissedSkillAutocomplete(null);
  }, [initialSkillWarnings, session.id]);
  useEffect(() => {
    setDismissedSkillAutocomplete(null);
    setSkillSelection({ key: '', index: 0 });
  }, [composerCwd, session.provider]);
  useEffect(() => {
    setPreservedAttachments([]);
    setPreservedEmbeddedNotes([]);
  }, [session.id]);

  // Cross-device draft, persisted on the daemon like the desktop composer:
  // prefill once per session, save edits debounced, clear on send.
  const autocompleteBlurTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const draftLoadedFor = useRef<string | null>(null);
  const draftLoadingFor = useRef<string | null>(null);
  const [draftHydrationRevision, setDraftHydrationRevision] = useState(0);
  const draftFlush = useRef(createDraftFlushScheduler()).current;
  const draftRef = useRef(draft);
  const attachmentsRef = useRef(preservedAttachments);
  const embeddedNotesRef = useRef(preservedEmbeddedNotes);
  draftRef.current = draft;
  attachmentsRef.current = preservedAttachments;
  embeddedNotesRef.current = preservedEmbeddedNotes;
  useEffect(() => () => {
    if (autocompleteBlurTimer.current) clearTimeout(autocompleteBlurTimer.current);
  }, []);
  useEffect(() => {
    const client = daemon.client;
    if (!client || daemon.phase !== 'connected' || draftLoadedFor.current === session.id) return;
    if (draftLoadingFor.current === session.id) return;
    draftLoadingFor.current = session.id;
    void loadComposerDrafts(client).then((drafts) => {
      const saved = drafts.sessions?.[session.id];
      const text = saved?.text;
      if (text && !draftRef.current.trim()) {
        setDraft(text);
        setCursor(text.length);
      }
      if (saved?.attachments?.length && !attachmentsRef.current.length) {
        setPreservedAttachments(saved.attachments);
      }
      if (saved?.embedded_notes?.length && !embeddedNotesRef.current.length) {
        setPreservedEmbeddedNotes(saved.embedded_notes);
      }
    }).catch(() => {}).finally(() => {
      if (draftLoadingFor.current !== session.id) return;
      draftLoadingFor.current = null;
      draftLoadedFor.current = session.id;
      setDraftHydrationRevision((value) => value + 1);
    });
  }, [daemon.client, daemon.phase, session.id]);
  useEffect(() => {
    const client = daemon.client;
    if (!client || draftLoadedFor.current !== session.id) return;
    draftFlush.schedule(`session:${session.id}`, () => applyComposerDraftChanges(client, [{
        target: { type: 'session', sessionId: session.id },
        draft: composerDraftSnapshot(
          draft,
          preservedAttachments,
          preservedEmbeddedNotes,
        ),
      }]));
  }, [
    daemon.client,
    draft,
    draftFlush,
    preservedAttachments,
    preservedEmbeddedNotes,
    draftHydrationRevision,
    session.id,
  ]);
  const requestSignature = permission?.requestId ?? userInput?.requestId;
  const lastRequest = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (requestSignature && requestSignature !== lastRequest.current) {
      void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Warning);
    }
    lastRequest.current = requestSignature;
  }, [requestSignature]);

  function acceptSkill(index = skillHighlight) {
    if (!skillTrigger) return;
    const command = skillRows[index];
    if (!command) return;
    const replacement = replaceSkillTrigger(
      draft,
      skillTrigger,
      command,
    );
    setDraft(replacement.text);
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

  async function renderProviderPrompt(
    visiblePrompt: string,
    attachments: readonly MessageAttachment[] = [],
    embeddedNotes: readonly EmbeddedNote[] = [],
  ) {
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
      return {
        providerText: composeMobileProviderPrompt(
          rendered.providerText,
          attachments,
          embeddedNotes,
        ),
        warnings: rendered.warnings,
      };
    });
  }

  async function runContextCompaction(focus: string | null, clearComposer = false) {
    if (submitting || session.compacting || sessionBusy(session)) return;
    setSubmitting(true);
    setLocalError(null);
    try {
      await runtime.compactContext(session, supportsCompactionFocus ? focus : null);
      if (clearComposer) {
        setDraft('');
        setPreservedAttachments([]);
        setPreservedEmbeddedNotes([]);
        setCursor(0);
        await draftFlush.flush();
        if (daemon.client) {
          await applyComposerDraftChanges(daemon.client, [{
            target: { type: 'session', sessionId: session.id },
            draft: null,
          }]).catch(() => {});
        }
      }
      setCompactFocus('');
      setCompactPanelOpen(false);
      await Haptics.selectionAsync();
    } catch (cause) {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    } finally {
      setSubmitting(false);
    }
  }

  async function submit() {
    const prompt = draft.trim();
    if (!prompt || submitting) return;
    if (/^\/compact(?:\s|$)/u.test(prompt) && !supportsCompaction(session.provider)) {
      setLocalError(
        `Context compaction is not available for ${session.provider}.`,
      );
      return;
    }
    const compaction = preservedAttachments.length === 0 && preservedEmbeddedNotes.length === 0
      ? parseCompactionSubmission(session.provider, prompt)
      : null;
    if (compaction) {
      await runContextCompaction(compaction.focus, true);
      return;
    }
    const submittedSession = session;
    setSubmitting(true);
    setLocalError(null);
    setSkillWarnings([]);
    try {
      if (daemon.client) {
        await applyComposerDraftChanges(daemon.client, [{
          target: { type: 'session', sessionId: submittedSession.id },
          draft: composerDraftSnapshot(
            draft,
            preservedAttachments,
            preservedEmbeddedNotes,
          ),
        }]).catch(() => {});
      }
      // While the agent is busy, always queue — never steer directly from
      // the composer. Steering is reserved for the steer action on existing
      // queued messages. sendPrompt detects sessionBusy internally and calls
      // queueSubmission, so the message lands at the bottom of the queue list.
      const resolution = await renderProviderPrompt(
        prompt,
        preservedAttachments,
        preservedEmbeddedNotes,
      );
      resolution.assertCurrent();
      setSkillWarnings(resolution.result.warnings);
      resolution.assertCurrent();
      await runtime.sendPrompt(
        submittedSession,
        prompt,
        resolution.result.providerText,
        preservedAttachments,
        preservedEmbeddedNotes,
      );
      setDraft('');
      setPreservedAttachments([]);
      setPreservedEmbeddedNotes([]);
      setCursor(0);
      setDismissedSkillAutocomplete(null);
      setSkillSelection({ key: '', index: 0 });
      await draftFlush.flush();
      if (daemon.client) {
        await applyComposerDraftChanges(daemon.client, [{
          target: { type: 'session', sessionId: session.id },
          draft: null,
        }]).catch(() => {});
      }
      await Haptics.selectionAsync();
    } catch (cause) {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    } finally {
      setSubmitting(false);
    }
  }

  async function stop() {
    setLocalError(null);
    try {
      await runtime.cancel(session.id);
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Warning);
    } catch (cause) {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function steerQueued(message: QueuedMessage) {
    setLocalError(null);
    try {
      const metadata = queuedMessageMetadata(message);
      await runtime.steerPrompt(
        session,
        metadata.visiblePrompt,
        metadata.providerPrompt,
        metadata.attachments,
        metadata.embeddedNotes,
      );
      await runtime.removeQueuedMessage(session.id, message.id);
      await Haptics.selectionAsync();
    } catch (cause) {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    }
  }

  function removeQueued(messageId: string) {
    void Haptics.selectionAsync();
    void runtime.removeQueuedMessage(session.id, messageId).catch((cause) => {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
    });
  }

  function editQueued(message: QueuedMessage) {
    const metadata = queuedMessageMetadata(message);
    void Haptics.selectionAsync();
    setDraft(metadata.visiblePrompt);
    setCursor(metadata.visiblePrompt.length);
    setPreservedAttachments(metadata.attachments);
    setPreservedEmbeddedNotes(metadata.embeddedNotes);
    setSkillWarnings([]);
    setLocalError(null);
    removeQueued(message.id);
  }

  function applyOptions(changes: Parameters<typeof runtime.updateSessionOptions>[1]) {
    runtime.updateSessionOptions(session.id, changes).catch((cause) => {
      setLocalError(cause instanceof Error ? cause.message : String(cause));
    });
  }

  const disconnected = daemon.phase !== 'connected';
  const placeholder = disconnected
    ? 'Reconnect to message this agent'
    : busy
      ? 'Queue a follow-up…'
      : 'Message agent';

  return (
    <View style={[styles.shell, { paddingBottom: Math.max(insets.bottom, 10) }]}>
      {permission && !userInput && (
        <PermissionPanel
          permission={permission}
          onRespond={(optionId) => runtime.respond(session.id, permission.requestId, optionId)}
        />
      )}
      {userInput && (
        <UserInputPanel
          input={userInput}
          onSubmit={(answers) => runtime.respondUserInput(session.id, userInput.requestId, answers)}
        />
      )}
      {(localError || runtimeError) && (
        <View
          accessibilityLiveRegion="polite"
          style={[styles.errorBanner, { backgroundColor: theme.dangerSoft }]}>
          <Text style={[styles.errorText, { color: theme.danger }]}>
            {localError || runtimeError}
          </Text>
          <Pressable
            accessibilityLabel="Dismiss error"
            accessibilityRole="button"
            hitSlop={8}
            onPress={() => {
              setLocalError(null);
              runtime.dismissError(session.id);
            }}
            style={({ pressed }) => ({ opacity: pressed ? 0.5 : 1 })}>
            <PaduIcon
              name="x"
              size={12}
              tintColor={theme.danger}
            />
          </Pressable>
        </View>
      )}
      {skillWarnings.length > 0 && (
        <View
          accessibilityLiveRegion="polite"
          style={[styles.warningBanner, { backgroundColor: theme.warningSoft }]}>
          <PaduIcon name="bell" size={13} tintColor={theme.warning} />
          <Text style={[styles.errorText, { color: theme.warning }]}>
            {skillWarnings.join(' ')}
          </Text>
          <Pressable
            accessibilityLabel="Dismiss skill warning"
            accessibilityRole="button"
            onPress={() => setSkillWarnings([])}
            style={({ pressed }) => [
              styles.warningDismiss,
              { opacity: pressed ? 0.5 : 1 },
            ]}>
            <PaduIcon name="x" size={12} tintColor={theme.warning} />
          </Pressable>
        </View>
      )}
      <ComposerStatusCard
        canSteer={canSteer}
        onEditMessage={editQueued}
        onRemoveMessage={removeQueued}
        onSteerMessage={(message) => void steerQueued(message)}
        queued={queued}
        session={session}
      />
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
      {(preservedAttachments.length > 0 || preservedEmbeddedNotes.length > 0) && (
        <View
          accessibilityLiveRegion="polite"
          style={[styles.preservedMetadata, {
            backgroundColor: theme.surfaceMuted,
            borderColor: theme.border,
          }]}>
          <PaduIcon name="paperclip" size={13} tintColor={theme.textTertiary} />
          <Text style={[styles.preservedMetadataText, { color: theme.textSecondary }]}>
            {preservedAttachments.length + preservedEmbeddedNotes.length}
            {' '}
            queued item
            {preservedAttachments.length + preservedEmbeddedNotes.length === 1 ? '' : 's'}
            {' '}
            will be kept with this message.
          </Text>
        </View>
      )}
      <ComposerCard
        accessibilityLabel="Message agent"
        editable={!disconnected && !submitting}
        left={(
          <>
            <ComposerIconButton
              active={session.runtime_mode !== 'fullAccess'}
              icon="lock"
              label="Agent access"
              onPress={() => setAccessSheetOpen(true)}
            />
          </>
        )}
        placeholder={placeholder}
        right={(
          <>
            <ComposerIconButton
              icon="gauge"
              label="Model"
              onPress={() => setModelSheetOpen(true)}
            />
            {busy && draft.trim() ? (
              <SendButton
                busy={submitting}
                disabled={submitting || disconnected}
                label="Queue message"
                queueing
                onPress={() => void submit()}
              />
            ) : busy ? (
              <Pressable
                accessibilityLabel="Stop agent"
                accessibilityRole="button"
                hitSlop={4}
                onPress={() => void stop()}
                style={({ pressed }) => [
                  styles.sendButton,
                  { backgroundColor: theme.dangerSoft, opacity: pressed ? 0.55 : 1 },
                ]}>
                <PaduIcon
                  name="stopFilled"
                  size={14}
                  tintColor={theme.danger}
                />
              </Pressable>
            ) : (
              <SendButton
                busy={submitting}
                disabled={!draft.trim() || submitting || disconnected}
                label="Send message"
                onPress={() => void submit()}
              />
            )}
          </>
        )}
        value={draft}
        onBlur={() => {
          if (autocompleteBlurTimer.current) clearTimeout(autocompleteBlurTimer.current);
          autocompleteBlurTimer.current = setTimeout(() => {
            setInputFocused(false);
          }, AUTOCOMPLETE_BLUR_DELAY_MS);
        }}
        onChangeText={(value) => {
          setDraft(value);
          if (cursor > value.length) setCursor(value.length);
        }}
        onFocus={() => {
          if (autocompleteBlurTimer.current) clearTimeout(autocompleteBlurTimer.current);
          setInputFocused(true);
          if (cursor === 0 && draft.length > 0) setCursor(draft.length);
        }}
        onKeyPress={handleComposerKeyPress}
        onSelectionChange={(event) => {
          setCursor(event.nativeEvent.selection.end);
        }}
      />
      <ComposerContextRow
        session={session}
        onPress={supportsCompaction(session.provider)
          ? () => setCompactPanelOpen((open) => !open)
          : undefined}
      />
      {compactPanelOpen && supportsCompaction(session.provider) && (
        <View
          style={[
            styles.compactPanel,
            {
              backgroundColor: theme.surface,
              borderColor: theme.border,
            },
          ]}>
          <Text style={[styles.compactPanelTitle, { color: theme.text }]}>
            {session.compacting ? 'Compacting context…' : 'Compact context'}
          </Text>
          <Text style={[styles.compactPanelBody, { color: theme.textSecondary }]}>
            Ask the harness to summarize this conversation. Earlier turns and Git checkpoints stay available.
          </Text>
          {supportsCompactionFocus && (
            <TextInput
              accessibilityLabel="Optional compaction focus"
              editable={!session.compacting && !sessionBusy(session)}
              placeholder="Optional focus, such as architecture decisions"
              placeholderTextColor={theme.textGhost}
              style={[
                styles.compactFocusInput,
                {
                  backgroundColor: theme.overlay,
                  borderColor: theme.border,
                  color: theme.text,
                },
              ]}
              value={compactFocus}
              onChangeText={setCompactFocus}
              onSubmitEditing={() => void runContextCompaction(compactFocus.trim() || null)}
            />
          )}
          <Pressable
            accessibilityLabel={session.compacting ? 'Compacting context' : 'Compact context'}
            accessibilityRole="button"
            disabled={session.compacting || submitting || sessionBusy(session)}
            onPress={() => void runContextCompaction(
              supportsCompactionFocus ? compactFocus.trim() || null : null,
            )}
            style={({ pressed }) => [
              styles.compactAction,
              {
                backgroundColor: theme.accent,
                opacity: session.compacting || submitting || sessionBusy(session)
                  ? 0.45
                  : pressed
                    ? 0.7
                    : 1,
              },
            ]}>
            <Text style={[styles.compactActionText, { color: theme.background }]}>
              {session.compacting ? 'Compacting…' : 'Compact context'}
            </Text>
          </Pressable>
        </View>
      )}

      <ModelSheet
        model={session.model ?? null}
        onApply={(selection) => applyOptions(selection)}
        onDismiss={() => setModelSheetOpen(false)}
        provider={session.provider}
        reasoningEffort={session.reasoning_effort ?? null}
        visible={modelSheetOpen}
      />
      <AccessSheet
        mode={session.runtime_mode}
        onApply={(mode) => applyOptions({ runtimeMode: mode })}
        onDismiss={() => setAccessSheetOpen(false)}
        visible={accessSheetOpen}
      />
    </View>
  );
}

function PermissionPanel({
  permission,
  onRespond,
}: {
  permission: PendingPermission;
  onRespond: (optionId: string) => Promise<void>;
}) {
  const theme = useTheme();
  const [responding, setResponding] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  return (
    <RequestPanel borderColor={theme.warning}>
      <View style={styles.requestHeading}>
        <PaduIcon
          name="bell"
          size={16}
          tintColor={theme.warning}
        />
        <Text style={[styles.requestTitle, { color: theme.text }]}>{permission.title}</Text>
      </View>
      {permission.detail ? (
        <ScrollView
          nestedScrollEnabled
          style={[styles.detailScroll, { backgroundColor: theme.inset, borderColor: theme.border }]}>
          <Text selectable style={[styles.requestDetail, { color: theme.textSecondary }]}>
            {permission.detail}
          </Text>
        </ScrollView>
      ) : null}
      {error && <Text style={[styles.panelError, { color: theme.danger }]}>{error}</Text>}
      <View style={styles.optionActions}>
        {permission.options.map((option) => (
          <Pressable
            accessibilityRole="button"
            disabled={Boolean(responding)}
            key={option.id}
            onPress={() => {
              setResponding(option.id);
              setError(null);
              void Haptics.selectionAsync();
              void onRespond(option.id).catch((cause) => {
                setError(cause instanceof Error ? cause.message : String(cause));
                setResponding(null);
              });
            }}
            style={({ pressed }) => [
              styles.optionButton,
              {
                backgroundColor: option.allow ? theme.inverse : theme.surfaceMuted,
                opacity: pressed || (responding && responding !== option.id) ? 0.55 : 1,
              },
            ]}>
            {responding === option.id && (
              <ActivityIndicator
                color={option.allow ? theme.onInverse : theme.text}
                size="small"
              />
            )}
            <Text style={[
              styles.optionButtonText,
              { color: option.allow ? theme.onInverse : theme.text },
            ]}>{option.label}</Text>
          </Pressable>
        ))}
      </View>
    </RequestPanel>
  );
}

function UserInputPanel({
  input,
  onSubmit,
}: {
  input: PendingUserInput;
  onSubmit: (answers: UserInputAnswer[]) => Promise<void>;
}) {
  const theme = useTheme();
  const [index, setIndex] = useState(0);
  const [selections, setSelections] = useState<Record<string, string[]>>({});
  const [customAnswers, setCustomAnswers] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setIndex(0);
    setSelections({});
    setCustomAnswers({});
    setSubmitting(false);
    setError(null);
  }, [input.requestId]);

  const question = input.questions[index];
  if (!question) return null;
  const selected = selections[question.id] ?? [];
  const custom = customAnswers[question.id] ?? '';
  const canContinue = Boolean(custom.trim() || selected.length);
  const last = index === input.questions.length - 1;

  function toggle(label: string) {
    void Haptics.selectionAsync();
    setCustomAnswers((values) => ({ ...values, [question.id]: '' }));
    setSelections((values) => {
      const previous = values[question.id] ?? [];
      return {
        ...values,
        [question.id]: question.multiSelect
          ? previous.includes(label)
            ? previous.filter((value) => value !== label)
            : [...previous, label]
          : [label],
      };
    });
  }

  async function advance() {
    if (!canContinue || submitting) return;
    if (!last) {
      setIndex((value) => value + 1);
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await onSubmit(input.questions.map((item) => {
        const customValue = customAnswers[item.id]?.trim();
        return {
          questionId: item.id,
          answers: customValue ? [customValue] : selections[item.id] ?? [],
        };
      }));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setSubmitting(false);
    }
  }

  return (
    <RequestPanel borderColor={theme.accent}>
      <View style={styles.questionHeader}>
        <Text style={[styles.questionEyebrow, { color: theme.textTertiary }]}>{question.header}</Text>
        {input.questions.length > 1 && (
          <Text style={[styles.progress, { color: theme.textTertiary }]}>
            {index + 1} of {input.questions.length}
          </Text>
        )}
      </View>
      <Text style={[styles.questionText, { color: theme.text }]}>{question.question}</Text>
      <ScrollView keyboardShouldPersistTaps="handled" nestedScrollEnabled style={styles.questionOptions}>
        {question.options.map((option) => {
          const checked = selected.includes(option.label);
          return (
            <Pressable
              accessibilityRole={question.multiSelect ? 'checkbox' : 'radio'}
              accessibilityState={{ checked }}
              key={option.label}
              onPress={() => toggle(option.label)}
              style={({ pressed }) => [
                styles.questionOption,
                {
                  backgroundColor: checked ? theme.overlayStrong : theme.surfaceMuted,
                  borderColor: checked ? NativeTint : 'transparent',
                  opacity: pressed ? 0.65 : 1,
                },
              ]}>
              <View style={styles.questionOptionCopy}>
                <Text style={[styles.questionOptionLabel, { color: theme.text }]}>{option.label}</Text>
                {option.description && option.description !== option.label && (
                  <Text style={[styles.questionOptionDescription, { color: theme.textSecondary }]}>
                    {option.description}
                  </Text>
                )}
              </View>
              {checked && (
                <PaduIcon
                  name="check"
                  size={14}
                  tintColor={theme.accent}
                />
              )}
            </Pressable>
          );
        })}
        <TextInput
          accessibilityLabel="Custom answer"
          multiline
          placeholder="Write another answer…"
          placeholderTextColor={theme.textTertiary}
          selectionColor={NativeTint}
          style={[
            styles.customAnswer,
            {
              backgroundColor: theme.surfaceMuted,
              borderColor: custom.trim() ? NativeTint : 'transparent',
              color: theme.text,
            },
          ]}
          value={custom}
          onChangeText={(value) => {
            setCustomAnswers((values) => ({ ...values, [question.id]: value }));
            if (value.trim()) setSelections((values) => ({ ...values, [question.id]: [] }));
          }}
        />
      </ScrollView>
      {error && <Text style={[styles.panelError, { color: theme.danger }]}>{error}</Text>}
      <View style={styles.questionActions}>
        {index > 0 ? (
          <Pressable
            accessibilityRole="button"
            onPress={() => setIndex((value) => value - 1)}
            style={styles.backButton}>
            <Text style={[styles.backButtonText, { color: theme.textSecondary }]}>Back</Text>
          </Pressable>
        ) : <View />}
        <Pressable
          accessibilityRole="button"
          disabled={!canContinue || submitting}
          onPress={() => void advance()}
          style={({ pressed }) => [
            styles.nextButton,
            { backgroundColor: theme.inverse, opacity: !canContinue || submitting || pressed ? 0.55 : 1 },
          ]}>
          {submitting && <ActivityIndicator color={theme.onInverse} size="small" />}
          <Text style={[styles.nextButtonText, { color: theme.onInverse }]}>
            {last ? 'Submit' : 'Next'}
          </Text>
        </Pressable>
      </View>
    </RequestPanel>
  );
}

function RequestPanel({ borderColor, children }: { borderColor: string; children: ReactNode }) {
  const theme = useTheme();
  return (
    <View style={[styles.requestPanel, { backgroundColor: theme.surface, borderColor }]}>
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  shell: { paddingHorizontal: 12, paddingTop: 4 },
  card: {
    borderRadius: 20,
    borderWidth: StyleSheet.hairlineWidth,
    paddingBottom: 8,
    paddingHorizontal: 10,
    paddingTop: 6,
  },
  input: {
    fontSize: 16,
    lineHeight: 21,
    maxHeight: 120,
    minHeight: 42,
    paddingHorizontal: 6,
    paddingVertical: 8,
  },
  toolbar: { alignItems: 'center', flexDirection: 'row', marginTop: 2 },
  toolbarSpacer: { flex: 1 },
  cluster: { alignItems: 'center', flexDirection: 'row', gap: 2 },
  statusCard: {
    alignSelf: 'stretch',
    borderTopStartRadius: Radius.medium,
    borderTopEndRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    gap: 8,
    marginBottom: 0,
    marginHorizontal: 16,
    paddingHorizontal: 10,
    paddingVertical: 4,
  },
  statusLine: {
    alignItems: 'center',
    alignSelf: 'stretch',
    flexDirection: 'row',
    gap: 8,
    minHeight: 20,
  },
  contextRow: {
    alignItems: 'center',
    alignSelf: 'flex-end',
    flexDirection: 'row',
    gap: 6,
    marginTop: 8,
    minHeight: 20,
    paddingHorizontal: 4,
  },
  compactPanel: {
    borderRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    gap: Spacing.two,
    marginTop: Spacing.two,
    padding: Spacing.three,
  },
  compactPanelTitle: { fontSize: 14, fontWeight: '600' },
  compactPanelBody: { fontSize: 12, lineHeight: 16 },
  compactFocusInput: {
    borderRadius: Radius.small,
    borderWidth: StyleSheet.hairlineWidth,
    fontSize: 13,
    minHeight: 44,
    paddingHorizontal: Spacing.two,
    paddingVertical: Spacing.two,
  },
  compactAction: {
    alignItems: 'center',
    borderRadius: Radius.small,
    justifyContent: 'center',
    minHeight: 44,
    paddingHorizontal: Spacing.three,
  },
  compactActionText: { fontSize: 13, fontWeight: '700' },
  statusItem: { alignItems: 'center', flexDirection: 'row', flexShrink: 1, gap: 4 },
  statusText: { flexShrink: 1, fontSize: 11, fontVariant: ['tabular-nums'], fontWeight: '500' },
  iconButton: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    height: 36,
    justifyContent: 'center',
    width: 36,
  },
  sendButton: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    height: 36,
    justifyContent: 'center',
    marginLeft: 4,
    width: 36,
  },
  errorBanner: {
    alignItems: 'center',
    borderRadius: Radius.small,
    flexDirection: 'row',
    gap: Spacing.two,
    marginBottom: Spacing.two,
    paddingHorizontal: Spacing.two,
    paddingVertical: Spacing.two,
  },
  errorText: { flex: 1, fontSize: 12, fontWeight: '600', lineHeight: 17 },
  warningBanner: {
    alignItems: 'center',
    borderRadius: Radius.small,
    flexDirection: 'row',
    gap: Spacing.two,
    marginBottom: Spacing.two,
    paddingHorizontal: Spacing.two,
    paddingVertical: Spacing.two,
  },
  warningDismiss: {
    alignItems: 'center',
    height: COMPOSER_HIT_TARGET,
    justifyContent: 'center',
    width: COMPOSER_HIT_TARGET,
  },
  preservedMetadata: {
    alignItems: 'center',
    borderRadius: Radius.small,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: Spacing.two,
    marginBottom: Spacing.two,
    paddingHorizontal: Spacing.two,
    paddingVertical: Spacing.two,
  },
  preservedMetadataText: {
    flex: 1,
    fontSize: 12,
    fontWeight: '600',
    lineHeight: 17,
  },
  skillAutocomplete: {
    borderRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    marginBottom: Spacing.two,
    marginHorizontal: Spacing.three,
    overflow: 'hidden',
  },
  skillLoading: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: Spacing.two,
    minHeight: COMPOSER_HIT_TARGET,
    paddingHorizontal: Spacing.three,
  },
  skillLoadingText: { fontSize: 12.5, fontWeight: '500' },
  skillList: { maxHeight: Spacing.six * 4 },
  skillRow: {
    alignItems: 'center',
    borderTopWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: Spacing.two,
    minHeight: COMPOSER_HIT_TARGET,
    paddingHorizontal: Spacing.three,
    paddingVertical: Spacing.two,
  },
  skillRowCopy: { flex: 1, minWidth: 0 },
  skillRowName: { fontSize: 14, fontWeight: '600', lineHeight: 18 },
  skillRowDescription: { fontSize: 11.5, lineHeight: 16 },
  skillRowScope: { fontSize: 10.5, fontWeight: '600' },
  queuedItem: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: Spacing.two,
    minHeight: COMPOSER_HIT_TARGET,
    paddingVertical: 2,
  },
  queuedEdit: {
    alignItems: 'center',
    flex: 1,
    flexDirection: 'row',
    gap: Spacing.two,
    minHeight: COMPOSER_HIT_TARGET,
    minWidth: 0,
  },
  queuedText: { flex: 1, fontSize: 12.5 },
  queueSeparator: {
    height: StyleSheet.hairlineWidth,
    marginHorizontal: 0,
  },
  queueActions: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 2,
  },
  queueActionButton: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 3,
    justifyContent: 'center',
    minHeight: COMPOSER_HIT_TARGET,
    paddingHorizontal: 4,
  },
  requestPanel: {
    borderRadius: Radius.large,
    borderWidth: 1,
    marginBottom: 8,
    padding: 12,
  },
  requestHeading: { alignItems: 'center', flexDirection: 'row', gap: 8 },
  requestTitle: { flex: 1, fontSize: 14, fontWeight: '700' },
  detailScroll: {
    borderRadius: Radius.small,
    borderWidth: StyleSheet.hairlineWidth,
    marginTop: 9,
    maxHeight: 110,
    padding: 9,
  },
  requestDetail: { fontFamily: MonoFont, fontSize: 11.5, lineHeight: 17 },
  optionActions: { flexDirection: 'row', flexWrap: 'wrap', gap: 8, justifyContent: 'flex-end', marginTop: 11 },
  optionButton: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    flexDirection: 'row',
    gap: 6,
    minHeight: 38,
    paddingHorizontal: 14,
  },
  optionButtonText: { fontSize: 13, fontWeight: '700' },
  panelError: { fontSize: 12, lineHeight: 17, marginTop: 8 },
  questionHeader: { alignItems: 'center', flexDirection: 'row', justifyContent: 'space-between' },
  questionEyebrow: { fontSize: 11, fontWeight: '700', letterSpacing: 0.55, textTransform: 'uppercase' },
  progress: { fontSize: 11, fontWeight: '600' },
  questionText: { fontSize: 14, fontWeight: '600', lineHeight: 20, marginTop: 7 },
  questionOptions: { marginTop: 10, maxHeight: 260 },
  questionOption: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    borderWidth: 1,
    flexDirection: 'row',
    gap: 8,
    marginBottom: 6,
    minHeight: 44,
    paddingHorizontal: 11,
    paddingVertical: 8,
  },
  questionOptionCopy: { flex: 1 },
  questionOptionLabel: { fontSize: 13, fontWeight: '600' },
  questionOptionDescription: { fontSize: 11.5, lineHeight: 16, marginTop: 2 },
  customAnswer: {
    borderRadius: Radius.medium,
    borderWidth: 1,
    fontSize: 13,
    lineHeight: 18,
    minHeight: 42,
    paddingHorizontal: 11,
    paddingVertical: 9,
  },
  questionActions: { alignItems: 'center', flexDirection: 'row', justifyContent: 'space-between', marginTop: 10 },
  backButton: { justifyContent: 'center', minHeight: 36, paddingHorizontal: 6 },
  backButtonText: { fontSize: 13, fontWeight: '600' },
  nextButton: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    flexDirection: 'row',
    gap: 6,
    justifyContent: 'center',
    minHeight: 36,
    minWidth: 78,
    paddingHorizontal: 14,
  },
  nextButtonText: { fontSize: 13, fontWeight: '700' },
});
