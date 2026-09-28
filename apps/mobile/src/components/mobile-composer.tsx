import type {
  AgentSession,
  PendingPermission,
  PendingUserInput,
  UserInputAnswer,
} from '@padu/client';
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
  type TextInputProps,
} from 'react-native';
import { useQuery } from '@tanstack/react-query';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { PaduIcon, type PaduIconName } from './padu-icon';
import { ContextGauge } from './context-gauge';
import { AccessSheet, ModelSheet } from './session-option-sheets';
import { MonoFont, NativeTint, Radius } from '@/constants/theme';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import {
  applyComposerDraftChanges,
  daemonKeys,
  inspectBranches,
  loadComposerDrafts,
} from '@/lib/daemon-api';
import { useDaemon } from '@/lib/daemon-context';
import { sessionBusy } from '@/lib/mobile-runtime';
import { useRuntime } from '@/lib/runtime-context';

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
  onSteerMessage,
  onRemoveMessage,
}: {
  session: AgentSession;
  queued: QueuedMessage[];
  canSteer: boolean;
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
      {queued.map((message, index) => (
        <View key={message.id}>
          <View style={styles.queuedItem}>
            <PaduIcon
              name="clock"
              size={12}
              tintColor={theme.textTertiary}
            />
            <Text numberOfLines={1} style={[styles.queuedText, { color: theme.textSecondary }]}>
              {message.display_content ?? message.content}
            </Text>
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

/**
 * Read-only context-window meter below the composer card, shrunk to its
 * content and hugging the right edge. Renders nothing until the daemon
 * reports usage or a selected window.
 */
function ComposerContextRow({ session }: { session: AgentSession }) {
  const theme = useTheme();
  const usage = session.context_usage;
  const percent = usage?.window ? Math.round(usage.tokens * 100 / usage.window) : null;
  if (!usage && !session.context_window) return null;

  const summary = percent != null
    ? `context ${percent}%`
    : usage
      ? `${formatTokens(usage.tokens)} tokens`
      : session.context_window;
  return (
    <View
      accessibilityLabel={summary || 'Session context'}
      accessibilityRole="text"
      style={styles.contextRow}>
      {percent != null && usage?.window ? (
        <>
          <Text style={[styles.statusText, { color: theme.textTertiary }]}>
            {formatTokens(usage.tokens)} / {formatTokens(usage.window)}
          </Text>
          <ContextGauge percent={percent} />
        </>
      ) : (
        <>
          <PaduIcon name="gauge" size={12} tintColor={theme.textTertiary} />
          <Text style={[styles.statusText, { color: theme.textTertiary }]}>
            {usage ? formatTokens(usage.tokens) : session.context_window}
          </Text>
        </>
      )}
    </View>
  );
}

function formatTokens(tokens: number) {
  if (tokens >= 999_500) return `${(tokens / 1_000_000).toFixed(1)}M`;
  return tokens >= 1_000 ? `${(tokens / 1_000).toFixed(1)}k` : `${tokens}`;
}

export function MobileComposer({ session }: { session: AgentSession }) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const [draft, setDraft] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [modelSheetOpen, setModelSheetOpen] = useState(false);
  const [accessSheetOpen, setAccessSheetOpen] = useState(false);
  const busy = sessionBusy(session);
  const liveRuntime = runtime.runtimes[session.id];
  const canSteer = busy && Boolean(liveRuntime?.supportsSteer) && session.status !== 'connecting';
  const permission = runtime.permissions[session.id];
  const userInput = runtime.userInputs[session.id];
  const runtimeError = runtime.errors[session.id];
  const queued = session.queued_messages ?? [];

  useEffect(() => setLocalError(null), [session.id]);

  // Cross-device draft, persisted on the daemon like the desktop composer:
  // prefill once per session, save edits debounced, clear on send.
  const draftTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const draftLoadedFor = useRef<string | null>(null);
  const draftRef = useRef(draft);
  draftRef.current = draft;
  useEffect(() => {
    const client = daemon.client;
    if (!client || daemon.phase !== 'connected' || draftLoadedFor.current === session.id) return;
    draftLoadedFor.current = session.id;
    void loadComposerDrafts(client).then((drafts) => {
      const text = drafts.sessions?.[session.id]?.text;
      if (text && !draftRef.current.trim()) setDraft(text);
    }).catch(() => {});
  }, [daemon.client, daemon.phase, session.id]);
  useEffect(() => {
    const client = daemon.client;
    if (!client || draftLoadedFor.current !== session.id) return;
    if (draftTimer.current) clearTimeout(draftTimer.current);
    draftTimer.current = setTimeout(() => {
      void applyComposerDraftChanges(client, [{
        target: { type: 'session', sessionId: session.id },
        draft: draft.trim() ? { text: draft } : null,
      }]).catch(() => {});
    }, 800);
    return () => {
      if (draftTimer.current) clearTimeout(draftTimer.current);
    };
  }, [daemon.client, draft, session.id]);

  const requestSignature = permission?.requestId ?? userInput?.requestId;
  const lastRequest = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (requestSignature && requestSignature !== lastRequest.current) {
      void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Warning);
    }
    lastRequest.current = requestSignature;
  }, [requestSignature]);

  async function submit() {
    const prompt = draft.trim();
    if (!prompt || submitting) return;
    setSubmitting(true);
    setLocalError(null);
    try {
      // While the agent is busy, always queue — never steer directly from
      // the composer. Steering is reserved for the steer action on existing
      // queued messages. sendPrompt detects sessionBusy internally and calls
      // queueSubmission, so the message lands at the bottom of the queue list.
      await runtime.sendPrompt(session, prompt);
      setDraft('');
      if (draftTimer.current) clearTimeout(draftTimer.current);
      if (daemon.client) {
        void applyComposerDraftChanges(daemon.client, [{
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
      await runtime.steerPrompt(session, message.display_content ?? message.content);
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
      <ComposerStatusCard
        canSteer={canSteer}
        onRemoveMessage={removeQueued}
        onSteerMessage={(message) => void steerQueued(message)}
        queued={queued}
        session={session}
      />
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
        onChangeText={setDraft}
      />
      <ComposerContextRow session={session} />

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
    gap: 9,
    marginBottom: 7,
    paddingHorizontal: 10,
    paddingVertical: 7,
  },
  errorText: { flex: 1, fontSize: 12, fontWeight: '600', lineHeight: 17 },
  queuedItem: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 8,
    minHeight: 20,
    paddingVertical: 2,
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
    minHeight: 28,
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
