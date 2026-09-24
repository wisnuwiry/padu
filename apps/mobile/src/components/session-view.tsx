import type { AgentSession, Checkpoint, Message } from '@padu/client';
import {
  formatMessageTime,
  formatWorkingElapsed,
} from '@padu/client/transcript-presentation';
import * as Clipboard from 'expo-clipboard';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  FlatList,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  RefreshControl,
  StyleSheet,
  Text,
  View,
  type NativeScrollEvent,
  type NativeSyntheticEvent,
} from 'react-native';

import { ActivityGroup } from '@/components/activity-group';
import { PaduIcon } from '@/components/padu-icon';
import { IconButton } from '@/components/button';
import { ChromeSurface } from '@/components/chrome-surface';
import { MarkdownMessage } from '@/components/markdown-message';
import { MobileComposer } from '@/components/mobile-composer';
import { RenameDialog } from '@/components/rename-dialog';
import { navigateBack, ScreenHeader } from '@/components/screen-header';
import { Sheet, SheetRow } from '@/components/sheet';
import { MonoFont, Radius, Spacing } from '@/constants/theme';
import { useSession, useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import { sessionBusy } from '@/lib/mobile-runtime';
import { useRuntime } from '@/lib/runtime-context';
import {
  buildTranscriptRows,
  displaySessionTitle,
  type TranscriptRow,
} from '@/lib/session-presentation';

export function SessionView({
  sessionId,
  showBack = true,
}: {
  sessionId: string | undefined;
  /** False when embedded in the wide two-pane, which has no back stack. */
  showBack?: boolean;
}) {
  const theme = useTheme();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const query = useSession(sessionId);
  const session = query.data;
  const [expandedFolds, setExpandedFolds] = useState<ReadonlySet<string>>(new Set());
  const [menuOpen, setMenuOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [pinnedToBottom, setPinnedToBottom] = useState(true);
  const transcriptRows = useMemo(
    () => session ? buildTranscriptRows(session, expandedFolds) : [],
    [expandedFolds, session],
  );
  const running = Boolean(session && sessionBusy(session));
  const listRef = useRef<FlatList<TranscriptRow>>(null);
  const nearBottom = useRef(true);
  const laidOut = useRef(false);

  useEffect(() => {
    if (!session || daemon.phase !== 'connected') return;
    void runtime.attachSession(session).catch(() => {});
    // Re-runs when the session starts working (another client may have
    // started the runtime after this screen mounted).
  }, [daemon.phase, runtime.attachSession, session?.id, running]);

  function trackScroll(event: NativeSyntheticEvent<NativeScrollEvent>) {
    const { contentOffset, contentSize, layoutMeasurement } = event.nativeEvent;
    const pinned = contentSize.height - layoutMeasurement.height - contentOffset.y < 120;
    nearBottom.current = pinned;
    setPinnedToBottom((current) => current === pinned ? current : pinned);
  }

  function toggleFold(turnId: string) {
    setExpandedFolds((current) => {
      const next = new Set(current);
      if (next.has(turnId)) next.delete(turnId);
      else next.add(turnId);
      return next;
    });
  }

  async function copyLastResponse() {
    const lastAssistant = [...(session?.messages ?? [])]
      .reverse()
      .find((message) => message.role === 'assistant' && message.content.trim());
    if (!lastAssistant) return;
    await Clipboard.setStringAsync(lastAssistant.content);
    await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
  }

  function confirmDelete() {
    if (!session) return;
    Alert.alert(
      `Delete “${displaySessionTitle(session)}”?`,
      'This removes the task and its transcript from the daemon for every device.',
      [
        { text: 'Cancel', style: 'cancel' },
        {
          text: 'Delete',
          style: 'destructive',
          onPress: () => {
            void runtime.deleteSession(session.id)
              .then(() => {
                void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
                navigateBack();
              })
              .catch((cause) => {
                Alert.alert('Couldn’t delete task', cause instanceof Error ? cause.message : String(cause));
              });
          },
        },
      ],
    );
  }

  const projectName = useTaskState().data?.projects
    .find((project) => project.id === session?.project_id)?.name;
  const subtitleParts = [projectName, daemon.activeProfile?.name].filter(Boolean);

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      style={[styles.screen, { backgroundColor: theme.background }]}>
      <ScreenHeader
        back={showBack}
        right={session ? (
          <>
            <IconButton
              accessibilityHint="Starts a new task"
              glyphSize={18}
              icon="compose"
              label="New task"
              onPress={() => router.push('/new-task')}
            />
            <IconButton
              accessibilityHint="Opens the task menu"
              glyphSize={18}
              icon="ellipsis"
              label="Task options"
              onPress={() => setMenuOpen(true)}
            />
          </>
        ) : undefined}
        subtitle={subtitleParts.length ? subtitleParts.join(' · ') : null}
        title={session ? displaySessionTitle(session) : 'Task'}
      />
      <View style={styles.listFrame}>
        <FlatList
          ref={listRef}
          data={transcriptRows}
          keyExtractor={(item) => item.key}
          contentContainerStyle={[
            styles.content,
            !transcriptRows.length && styles.emptyContent,
          ]}
          refreshControl={(
            <RefreshControl
              refreshing={query.isRefetching}
              tintColor={theme.textTertiary}
              onRefresh={() => void query.refetch()}
            />
          )}
          ListHeaderComponent={daemon.phase === 'error' ? <OfflineBanner /> : undefined}
          ListEmptyComponent={(
            <SessionEmpty loading={query.isPending} error={query.error} missing={query.data === null} />
          )}
          ListFooterComponent={running && session ? <WorkingFooter session={session} /> : undefined}
          renderItem={({ item }) => <TranscriptRowView row={item} onToggleFold={toggleFold} />}
          onContentSizeChange={() => {
            if (!laidOut.current || nearBottom.current) {
              listRef.current?.scrollToEnd({ animated: false });
              laidOut.current = true;
            }
          }}
          onScroll={trackScroll}
          scrollEventThrottle={100}
          showsVerticalScrollIndicator={false}
        />
        {!pinnedToBottom && transcriptRows.length > 0 && (
          <ChromeSurface style={styles.jumpButton}>
            <Pressable
              accessibilityLabel="Scroll to latest"
              accessibilityRole="button"
              onPress={() => listRef.current?.scrollToEnd({ animated: true })}
              style={({ pressed }) => [styles.jumpButtonInner, { opacity: pressed ? 0.6 : 1 }]}>
              <PaduIcon
                name="arrowDown"
                size={15}
                tintColor={theme.textSecondary}
              />
            </Pressable>
          </ChromeSurface>
        )}
      </View>
      {session && <MobileComposer session={session} />}

      <Sheet onDismiss={() => setMenuOpen(false)} visible={menuOpen}>
        <SheetRow
          label="Rename task"
          leading={<PaduIcon name="pencil" size={16} tintColor={theme.textSecondary} />}
          onPress={() => {
            setMenuOpen(false);
            setRenaming(true);
          }}
        />
        <SheetRow
          label="Copy last response"
          leading={<PaduIcon name="copy" size={16} tintColor={theme.textSecondary} />}
          onPress={() => {
            setMenuOpen(false);
            void copyLastResponse();
          }}
        />
        <SheetRow
          destructive
          label="Delete task"
          leading={<PaduIcon name="trash" size={16} tintColor={theme.danger} />}
          onPress={() => {
            setMenuOpen(false);
            confirmDelete();
          }}
        />
      </Sheet>
      {session && (
        <RenameDialog
          initialValue={displaySessionTitle(session)}
          onDismiss={() => setRenaming(false)}
          onSubmit={(title) => runtime.renameSession(session.id, title)}
          visible={renaming}
        />
      )}
    </KeyboardAvoidingView>
  );
}

function OfflineBanner() {
  const theme = useTheme();
  return (
    <View style={[styles.offlineBanner, { backgroundColor: theme.dangerSoft }]}>
      <PaduIcon
        name="wifiOff"
        size={14}
        tintColor={theme.danger}
      />
      <Text style={[styles.offlineText, { color: theme.danger }]}>
        Reconnecting — showing cached messages
      </Text>
    </View>
  );
}

function TranscriptRowView({
  row,
  onToggleFold,
}: {
  row: TranscriptRow;
  onToggleFold: (turnId: string) => void;
}) {
  switch (row.kind) {
    case 'message':
      return <MessageRow footerTimestamp={row.footerTimestamp} message={row.message} />;
    case 'activities':
      return <ActivityGroup block={row.block} live={row.live} />;
    case 'fold':
      return (
        <FoldRow
          expanded={row.expanded}
          label={row.label}
          onPress={() => onToggleFold(row.turn.id)}
        />
      );
    case 'changed':
      return <ChangedFilesCard checkpoint={row.checkpoint} />;
  }
}

/** Desktop's turn fold: a hairline divider carrying "Worked for X ›". */
function FoldRow({
  label,
  expanded,
  onPress,
}: {
  label: string;
  expanded: boolean;
  onPress: () => void;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityHint={expanded ? 'Collapses the agent’s work' : 'Shows the agent’s work'}
      accessibilityLabel={label}
      accessibilityRole="button"
      accessibilityState={{ expanded }}
      onPress={onPress}
      style={({ pressed }) => [styles.foldRow, { opacity: pressed ? 0.6 : 1 }]}>
      <View style={[styles.foldLine, { backgroundColor: theme.border }]} />
      <Text numberOfLines={1} style={[styles.foldLabel, { color: theme.textTertiary }]}>
        {label}
      </Text>
      <PaduIcon
        name={expanded ? 'chevronDown' : 'chevronRight'}
        size={10}
        tintColor={theme.textGhost}
      />
      <View style={[styles.foldLine, { backgroundColor: theme.border }]} />
    </Pressable>
  );
}

function MessageRow({
  message,
  footerTimestamp,
}: {
  message: Message;
  footerTimestamp: number | null;
}) {
  const theme = useTheme();
  const content = message.display_content ?? message.content;

  async function copy() {
    await Clipboard.setStringAsync(message.content);
    await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
  }

  if (message.role === 'system') {
    return (
      <View style={styles.systemFrame}>
        <Text style={[styles.systemMessage, { backgroundColor: theme.overlay, color: theme.textTertiary }]}>
          {content}
        </Text>
      </View>
    );
  }
  if (message.role === 'user') {
    return (
      <View style={[styles.messageFrame, styles.userFrame]}>
        <Pressable
          accessibilityHint="Long press to copy"
          delayLongPress={350}
          onLongPress={() => void copy()}
          style={[styles.userBubble, { backgroundColor: theme.raised }]}>
          <Text selectable style={[styles.userText, { color: theme.text }]}>{content}</Text>
          {message.attachments?.length ? (
            <View style={styles.attachments}>
              {message.attachments.map((attachment) => (
                <View
                  key={`${attachment.path}:${attachment.name}`}
                  style={[styles.attachment, { backgroundColor: theme.overlayStrong }]}>
                  <PaduIcon
                    name="file"
                    size={12}
                    tintColor={theme.textSecondary}
                  />
                  <Text numberOfLines={1} style={[styles.attachmentText, { color: theme.textSecondary }]}>
                    {attachment.name}
                  </Text>
                </View>
              ))}
            </View>
          ) : null}
        </Pressable>
      </View>
    );
  }
  return (
    <View style={styles.messageFrame}>
      <Pressable delayLongPress={350} onLongPress={() => void copy()}>
        <MarkdownMessage value={content} />
      </Pressable>
      {footerTimestamp != null && !message.streaming && (
        <Text style={[styles.messageFooter, { color: theme.textGhost }]}>
          {formatMessageTime(footerTimestamp)}
        </Text>
      )}
    </View>
  );
}

function ChangedFilesCard({ checkpoint }: { checkpoint: Checkpoint }) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  return (
    <View style={[styles.changedCard, { backgroundColor: theme.surface, borderColor: theme.border }]}>
      <Pressable
        accessibilityRole="button"
        onPress={() => setOpen((value) => !value)}
        style={({ pressed }) => [styles.changedHeader, { opacity: pressed ? 0.6 : 1 }]}>
          <PaduIcon
            name="fileDiff"
            size={13}
            tintColor={theme.textSecondary}
          />
        <Text style={[styles.changedTitle, { color: theme.textSecondary }]}>
          {checkpoint.files.length} file{checkpoint.files.length === 1 ? '' : 's'} changed
        </Text>
        <Text style={styles.changedStats}>
          <Text style={{ color: theme.success }}>+{checkpoint.additions}</Text>
          <Text style={{ color: theme.textGhost }}> </Text>
          <Text style={{ color: theme.danger }}>−{checkpoint.deletions}</Text>
        </Text>
        <PaduIcon
          name={open ? 'chevronUp' : 'chevronDown'}
          size={11}
          tintColor={theme.textGhost}
        />
      </Pressable>
      {open && (
        <View style={[styles.changedFiles, { borderTopColor: theme.border }]}>
          {checkpoint.files.map((file) => (
            <View key={file.path} style={styles.changedFileRow}>
              <Text numberOfLines={1} style={[styles.changedFilePath, { color: theme.text }]}>
                {file.path}
              </Text>
              <Text style={styles.changedStats}>
                <Text style={{ color: theme.success }}>+{file.additions}</Text>
                <Text style={{ color: theme.textGhost }}> </Text>
                <Text style={{ color: theme.danger }}>−{file.deletions}</Text>
              </Text>
            </View>
          ))}
        </View>
      )}
    </View>
  );
}

function WorkingFooter({ session }: { session: AgentSession }) {
  const theme = useTheme();
  const startedAt = session.turns.at(-1)?.started_at ?? null;
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1_000));
  useEffect(() => {
    const timer = setInterval(() => setNow(Math.floor(Date.now() / 1_000)), 1_000);
    return () => clearInterval(timer);
  }, []);
  const elapsed = startedAt ? Math.max(0, now - startedAt) : null;
  return (
    <View style={styles.workingFooter}>
      <ActivityIndicator color={theme.textTertiary} size="small" />
      <Text style={[styles.workingText, { color: theme.textTertiary }]}>
        {session.status === 'waiting'
          ? 'Waiting for you'
          : elapsed != null
            ? `Working for ${formatWorkingElapsed(elapsed)}`
            : 'Working'}
      </Text>
    </View>
  );
}

function SessionEmpty({
  loading,
  error,
  missing,
}: {
  loading: boolean;
  error: unknown;
  missing: boolean;
}) {
  const theme = useTheme();
  if (loading) return <ActivityIndicator color={theme.textTertiary} />;
  return (
    <View style={styles.empty}>
      <Text style={[styles.emptyTitle, { color: theme.text }]}>
        {missing ? 'Task not found' : error ? 'Couldn’t load this task' : 'No messages yet'}
      </Text>
      {Boolean(error) && (
        <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
          {error instanceof Error ? error.message : String(error)}
        </Text>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  listFrame: { flex: 1 },
  content: { paddingBottom: 40, paddingHorizontal: Spacing.three, paddingTop: Spacing.two },
  emptyContent: { flexGrow: 1, justifyContent: 'center' },
  offlineBanner: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    flexDirection: 'row',
    gap: 7,
    marginBottom: 12,
    padding: 10,
  },
  offlineText: { fontSize: 12.5, fontWeight: '600' },
  messageFrame: { marginBottom: 14, marginTop: 4 },
  userFrame: { alignItems: 'flex-end', marginTop: 8 },
  userBubble: {
    borderRadius: Radius.large,
    borderBottomRightRadius: 6,
    maxWidth: '88%',
    paddingHorizontal: 14,
    paddingVertical: 10,
  },
  userText: { fontSize: 15.5, lineHeight: 22 },
  messageFooter: { fontSize: 10.5, marginTop: 5 },
  systemFrame: { alignItems: 'center', marginBottom: 16 },
  systemMessage: {
    borderRadius: Radius.pill,
    fontSize: 11.5,
    lineHeight: 16,
    overflow: 'hidden',
    paddingHorizontal: 11,
    paddingVertical: 5,
    textAlign: 'center',
  },
  attachments: { flexDirection: 'row', flexWrap: 'wrap', gap: 6, marginTop: 9 },
  attachment: {
    alignItems: 'center',
    borderRadius: Radius.small,
    flexDirection: 'row',
    gap: 5,
    maxWidth: 220,
    paddingHorizontal: 8,
    paddingVertical: 5,
  },
  attachmentText: { flexShrink: 1, fontSize: 11.5 },
  foldRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 9,
    marginBottom: 6,
    marginTop: 2,
    minHeight: 28,
  },
  foldLine: { flex: 1, height: StyleSheet.hairlineWidth },
  foldLabel: { flexShrink: 1, fontSize: 12, fontWeight: '500' },
  changedCard: {
    borderRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    marginBottom: 18,
    marginTop: 2,
    overflow: 'hidden',
  },
  changedHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 8,
    minHeight: 40,
    paddingHorizontal: 11,
  },
  changedTitle: { flex: 1, fontSize: 12.5, fontWeight: '600' },
  changedStats: { fontSize: 11.5, fontVariant: ['tabular-nums'], fontWeight: '600' },
  changedFiles: {
    borderTopWidth: StyleSheet.hairlineWidth,
    gap: 6,
    paddingHorizontal: 11,
    paddingVertical: 9,
  },
  changedFileRow: { alignItems: 'center', flexDirection: 'row', gap: 8 },
  changedFilePath: { flex: 1, fontFamily: MonoFont, fontSize: 11 },
  jumpButton: {
    borderRadius: Radius.pill,
    bottom: 12,
    height: 40,
    position: 'absolute',
    right: 14,
    width: 40,
  },
  jumpButtonInner: { alignItems: 'center', flex: 1, justifyContent: 'center' },
  workingFooter: { alignItems: 'center', flexDirection: 'row', gap: 8, paddingBottom: 8, paddingLeft: 2, paddingVertical: 10 },
  workingText: { fontSize: 12.5, fontVariant: ['tabular-nums'], fontWeight: '500' },
  empty: { alignItems: 'center', paddingHorizontal: 32 },
  emptyTitle: { fontSize: 17, fontWeight: '700', textAlign: 'center' },
  emptyBody: { fontSize: 13.5, lineHeight: 19, marginTop: 8, textAlign: 'center' },
});
