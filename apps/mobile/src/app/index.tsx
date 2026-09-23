import type { AgentSession } from '@padu/client';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { useMemo, useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  Image,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  RefreshControl,
  SectionList,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { AppSymbol } from '@/components/app-symbol';
import { ConnectionErrorCard } from '@/components/connection-error-card';
import { GlassSurface } from '@/components/glass-surface';
import { ProviderIcon, providerBrandColor } from '@/components/provider-icon';
import { ConnectionStatus } from '@/components/connection-status';
import { RenameDialog } from '@/components/rename-dialog';
import { Sheet, SheetRow } from '@/components/sheet';
import { NativeTint, Radius, Spacing } from '@/constants/theme';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import { sessionBusy } from '@/lib/mobile-runtime';
import { useRuntime } from '@/lib/runtime-context';
import {
  displaySessionTitle,
  groupSessions,
  providerLabel,
  relativeSessionTime,
  type SessionListItem,
} from '@/lib/session-presentation';

const DaemonPickerTop = 8;
const DaemonPickerHeight = 38;
const DaemonPickerGap = 12;

export default function TasksScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const taskState = useTaskState();
  const [search, setSearch] = useState('');
  const [actionTarget, setActionTarget] = useState<AgentSession | null>(null);
  const [renameTarget, setRenameTarget] = useState<AgentSession | null>(null);
  const visibleSessions = useMemo(() => {
    if (!taskState.data) return [];
    const query = search.trim().toLocaleLowerCase();
    if (!query) return taskState.data.sessions;
    const projects = new Map(taskState.data.projects.map((project) => [project.id, project]));
    return taskState.data.sessions.filter((session) => {
      const project = projects.get(session.project_id);
      return [
        displaySessionTitle(session),
        project?.name,
        project?.path,
        providerLabel(session.provider),
        session.model,
      ].some((value) => value?.toLocaleLowerCase().includes(query));
    });
  }, [search, taskState.data]);
  const sections = useMemo(
    () => taskState.data ? groupSessions(taskState.data.projects, visibleSessions) : [],
    [taskState.data, visibleSessions],
  );

  function confirmDelete(session: AgentSession) {
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
              .then(() => Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success))
              .catch((cause) => {
                Alert.alert(
                  'Couldn’t delete task',
                  cause instanceof Error ? cause.message : String(cause),
                );
              });
          },
        },
      ],
    );
  }

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      style={[styles.screen, { backgroundColor: theme.background }]}>
      <View
        pointerEvents="box-none"
        style={[styles.floatingActions, { top: insets.top + DaemonPickerTop }]}>
        <GlassSurface interactive style={styles.daemonButton}>
          <Pressable
            accessibilityHint="Opens the daemon switcher"
            accessibilityLabel={daemon.activeProfile
              ? `Connected daemon: ${daemon.activeProfile.name}`
              : 'Add a daemon'}
            accessibilityRole="button"
            hitSlop={8}
            onPress={() => router.push('/daemons')}
            style={({ pressed }) => [styles.daemonButtonInner, { opacity: pressed ? 0.62 : 1 }]}>
            {daemon.activeProfile ? <ConnectionStatus compact phase={daemon.phase} /> : (
              <AppSymbol
                name={{ ios: 'plus', android: 'add', web: 'add' }}
                size={14}
                tintColor={theme.text}
              />
            )}
            <Text numberOfLines={1} style={[styles.daemonButtonText, { color: theme.text }]}>
              {daemon.activeProfile?.name ?? 'Add daemon'}
            </Text>
            <AppSymbol
              name={{ ios: 'chevron.down', android: 'keyboard_arrow_down', web: 'keyboard_arrow_down' }}
              size={12}
              tintColor={theme.textTertiary}
            />
          </Pressable>
        </GlassSurface>
      </View>

      {!daemon.profiles.length && daemon.phase !== 'booting' ? (
        <Onboarding />
      ) : (
        <SectionList
          sections={sections}
          keyExtractor={(item) => item.session.id}
          contentContainerStyle={[
            styles.listContent,
            {
              paddingTop:
                insets.top + DaemonPickerTop + DaemonPickerHeight + DaemonPickerGap,
            },
            sections.length === 0 && styles.listContentEmpty,
          ]}
          refreshControl={(
            <RefreshControl
              refreshing={taskState.isRefetching}
              tintColor={theme.textTertiary}
              onRefresh={() => {
                if (daemon.phase === 'connected') void taskState.refetch();
                else void daemon.reconnect();
              }}
            />
          )}
          renderSectionHeader={({ section }) => (
            <Text style={[styles.sectionTitle, { color: theme.textTertiary }]}>
              {section.title.toUpperCase()}
            </Text>
          )}
          renderItem={({ item }) => (
            <SessionRow
              item={item}
              onLongPress={() => {
                void Haptics.selectionAsync();
                setActionTarget(item.session);
              }}
            />
          )}
          ListHeaderComponent={daemon.error ? <ConnectionErrorCard /> : undefined}
          ListEmptyComponent={(
            <TaskListEmpty
              connecting={daemon.phase === 'booting' || daemon.phase === 'connecting'}
              error={taskState.error}
              searching={Boolean(search.trim())}
            />
          )}
          showsVerticalScrollIndicator={false}
          stickySectionHeadersEnabled={false}
        />
      )}

      {(daemon.profiles.length > 0 || daemon.phase === 'booting') && (
        <View pointerEvents="box-none" style={[styles.searchDock, { bottom: insets.bottom + 14 }]}>
          <GlassSurface interactive style={styles.searchCapsule}>
            <View style={styles.searchCapsuleInner}>
              <AppSymbol
                name={{ ios: 'magnifyingglass', android: 'search', web: 'search' }}
                size={17}
                tintColor={theme.textSecondary}
              />
              <TextInput
                accessibilityLabel="Search tasks"
                autoCapitalize="none"
                autoCorrect={false}
                placeholder="Search"
                placeholderTextColor={theme.textTertiary}
                selectionColor={NativeTint}
                style={[styles.searchInput, { color: theme.text }]}
                value={search}
                onChangeText={setSearch}
              />
              {search.length > 0 && (
                <Pressable
                  accessibilityLabel="Clear search"
                  accessibilityRole="button"
                  hitSlop={8}
                  onPress={() => setSearch('')}
                  style={({ pressed }) => ({ opacity: pressed ? 0.5 : 1 })}>
                  <AppSymbol
                    name={{ ios: 'xmark.circle.fill', android: 'cancel', web: 'cancel' }}
                    size={16}
                    tintColor={theme.textTertiary}
                  />
                </Pressable>
              )}
            </View>
          </GlassSurface>
          {daemon.phase === 'connected' && (
            <GlassSurface interactive style={styles.composeButton}>
              <Pressable
                accessibilityLabel="New task"
                accessibilityRole="button"
                hitSlop={6}
                onPress={() => router.push('/new-task')}
                style={({ pressed }) => [styles.roundInner, { opacity: pressed ? 0.5 : 1 }]}>
                <AppSymbol
                  name={{ ios: 'square.and.pencil', android: 'edit_square', web: 'edit' }}
                  size={20}
                  tintColor={theme.text}
                />
              </Pressable>
            </GlassSurface>
          )}
        </View>
      )}

      <Sheet onDismiss={() => setActionTarget(null)} visible={actionTarget !== null}>
        {actionTarget && (
          <>
            <Text numberOfLines={1} style={[styles.actionSheetTitle, { color: theme.text }]}>
              {displaySessionTitle(actionTarget)}
            </Text>
            <SheetRow
              label="Rename task"
              leading={<AppSymbol name={{ ios: 'pencil', android: 'edit', web: 'edit' }} size={16} tintColor={theme.textSecondary} />}
              onPress={() => {
                const target = actionTarget;
                setActionTarget(null);
                setRenameTarget(target);
              }}
            />
            <SheetRow
              destructive
              label="Delete task"
              leading={<AppSymbol name={{ ios: 'trash', android: 'delete', web: 'delete' }} size={16} tintColor={theme.danger} />}
              onPress={() => {
                const target = actionTarget;
                setActionTarget(null);
                if (target) confirmDelete(target);
              }}
            />
          </>
        )}
      </Sheet>
      {renameTarget && (
        <RenameDialog
          initialValue={displaySessionTitle(renameTarget)}
          onDismiss={() => setRenameTarget(null)}
          onSubmit={(title) => runtime.renameSession(renameTarget.id, title)}
          visible
        />
      )}
    </KeyboardAvoidingView>
  );
}

function Onboarding() {
  const theme = useTheme();
  return (
    <View style={styles.onboarding}>
      <Image
        accessibilityLabel="Padu"
        source={require('@/assets/images/icon.png')}
        style={styles.appIcon}
      />
      <Text style={[styles.onboardingTitle, { color: theme.text }]}>Your agents, everywhere.</Text>
      <Text style={[styles.onboardingBody, { color: theme.textSecondary }]}>
        Connect to Padu running on your Mac, workstation, or private server. Add more than one and
        switch whenever you need.
      </Text>
      <Pressable
        accessibilityRole="button"
        onPress={() => router.push('/daemon-editor')}
        style={({ pressed }) => [
          styles.primaryButton,
          { backgroundColor: theme.inverse, opacity: pressed ? 0.78 : 1 },
        ]}>
        <AppSymbol
          name={{ ios: 'plus', android: 'add', web: 'add' }}
          size={17}
          tintColor={theme.onInverse}
        />
        <Text style={[styles.primaryButtonText, { color: theme.onInverse }]}>Add a daemon</Text>
      </Pressable>
      <View style={styles.securityNote}>
        <AppSymbol
          name={{ ios: 'lock.shield', android: 'shield_lock', web: 'lock' }}
          size={15}
          tintColor={theme.textTertiary}
        />
        <Text style={[styles.securityText, { color: theme.textTertiary }]}>
          Tokens stay on this device and go directly to the host you choose.
        </Text>
      </View>
    </View>
  );
}

function TaskListEmpty({
  connecting,
  error,
  searching,
}: {
  connecting: boolean;
  error: unknown;
  searching: boolean;
}) {
  const theme = useTheme();
  const { error: daemonError, phase } = useDaemon();
  if (daemonError) return null;
  if (connecting) {
    return (
      <View style={styles.emptyState}>
        <ActivityIndicator color={theme.textTertiary} />
        <Text style={[styles.emptyTitle, { color: theme.textSecondary }]}>Connecting to daemon…</Text>
      </View>
    );
  }
  if (searching) {
    return (
      <View style={styles.emptyState}>
        <Text style={[styles.emptyTitle, { color: theme.text }]}>No matching tasks</Text>
        <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>Try another title, project, or agent.</Text>
      </View>
    );
  }
  if (error) {
    return (
      <View style={styles.emptyState}>
        <Text style={[styles.emptyTitle, { color: theme.text }]}>Couldn’t load tasks</Text>
        <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
          {error instanceof Error ? error.message : String(error)}
        </Text>
      </View>
    );
  }
  return (
    <View style={styles.emptyState}>
      <View style={[styles.emptyIcon, { backgroundColor: theme.overlayStrong }]}>
        <AppSymbol
          name={{ ios: 'text.bubble', android: 'chat_bubble', web: 'chat' }}
          size={25}
          tintColor={theme.textTertiary}
        />
      </View>
      <Text style={[styles.emptyTitle, { color: theme.text }]}>No tasks yet</Text>
      <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
        Start an agent on anything — a bug, a feature, a question about the code.
      </Text>
      {phase === 'connected' && (
        <Pressable
          accessibilityRole="button"
          onPress={() => router.push('/new-task')}
          style={({ pressed }) => [
            styles.emptyAction,
            { backgroundColor: theme.inverse, opacity: pressed ? 0.7 : 1 },
          ]}>
          <Text style={[styles.emptyActionText, { color: theme.onInverse }]}>New task</Text>
        </Pressable>
      )}
    </View>
  );
}

function SessionRow({ item, onLongPress }: { item: SessionListItem; onLongPress: () => void }) {
  const theme = useTheme();
  const session = item.session;
  const status = statusPresentation(session, theme);
  return (
    <Pressable
      accessibilityHint="Long press for actions"
      accessibilityLabel={`${displaySessionTitle(session)}, ${item.projectName}${status ? `, ${status.label}` : ''}`}
      accessibilityRole="button"
      delayLongPress={350}
      onLongPress={onLongPress}
      onPress={() => router.push({ pathname: '/session/[id]', params: { id: session.id } })}
      style={({ pressed }) => [
        styles.sessionRow,
        {
          backgroundColor: pressed ? theme.backgroundSelected : theme.surface,
          borderColor: theme.border,
        },
      ]}>
      <View style={styles.sessionCopy}>
        <Text numberOfLines={2} style={[styles.sessionTitle, { color: theme.text }]}>
          {displaySessionTitle(session)}
        </Text>
        <View style={styles.sessionMetadata}>
          <Text numberOfLines={1} style={[styles.sessionMetaText, { color: theme.textSecondary }]}>
            {item.projectName}
          </Text>
          <Text style={[styles.metadataBullet, { color: theme.textGhost }]}>·</Text>
          <View style={styles.providerBadge}>
            <ProviderIcon
              color={providerBrandColor(session.provider) ?? theme.textTertiary}
              provider={session.provider}
              size={13}
            />
            <Text style={[styles.sessionMetaText, { color: theme.textTertiary }]}>
              {providerLabel(session.provider)}
            </Text>
          </View>
        </View>
      </View>
      <View style={styles.sessionTrailing}>
        <Text style={[styles.sessionTime, { color: theme.textTertiary }]}>
          {relativeSessionTime(item.timestamp)}
        </Text>
        {status && (
          <View style={styles.statusLine}>
            {status.spinner
              ? <ActivityIndicator color={status.color} size="small" style={styles.statusSpinner} />
              : <View style={[styles.statusDot, { backgroundColor: status.color }]} />}
            <Text style={[styles.statusLabel, { color: status.color }]}>{status.label}</Text>
          </View>
        )}
      </View>
    </Pressable>
  );
}

function statusPresentation(
  session: AgentSession,
  theme: ReturnType<typeof useTheme>,
): { label: string; color: string; spinner: boolean } | null {
  if (session.status === 'waiting') {
    return { label: 'Needs input', color: theme.warning, spinner: false };
  }
  if (sessionBusy(session)) {
    return { label: 'Working', color: theme.success, spinner: true };
  }
  if (session.status === 'failed') {
    return { label: 'Failed', color: theme.danger, spinner: false };
  }
  return null;
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  floatingActions: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 8,
    left: Spacing.three,
    position: 'absolute',
    zIndex: 20,
  },
  roundInner: { alignItems: 'center', flex: 1, justifyContent: 'center' },
  searchDock: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 10,
    left: 0,
    paddingHorizontal: Spacing.three,
    position: 'absolute',
    right: 0,
    zIndex: 20,
  },
  searchCapsule: { borderRadius: Radius.pill, flex: 1 },
  searchCapsuleInner: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 9,
    minHeight: 50,
    paddingHorizontal: 16,
  },
  searchInput: { flex: 1, fontSize: 16.5, paddingVertical: 10 },
  composeButton: { borderRadius: Radius.pill, height: 50, width: 50 },
  daemonButton: { borderRadius: Radius.pill, maxWidth: 176 },
  daemonButtonInner: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 6,
    minHeight: DaemonPickerHeight,
    paddingHorizontal: 12,
  },
  daemonButtonText: { flexShrink: 1, fontSize: 13, fontWeight: '600' },
  listContent: { paddingBottom: 96 },
  listContentEmpty: { flexGrow: 1 },
  sectionTitle: {
    fontSize: 11,
    fontWeight: '700',
    letterSpacing: 0.65,
    marginBottom: 8,
    marginHorizontal: Spacing.three,
    marginTop: 18,
  },
  onboarding: {
    alignItems: 'center',
    flex: 1,
    justifyContent: 'center',
    paddingBottom: 48,
    paddingHorizontal: 34,
  },
  appIcon: { borderRadius: 18, height: 72, marginBottom: 24, width: 72 },
  onboardingTitle: { fontSize: 28, fontWeight: '700', letterSpacing: -0.7, textAlign: 'center' },
  onboardingBody: {
    fontSize: 16,
    lineHeight: 23,
    marginTop: 10,
    maxWidth: 440,
    textAlign: 'center',
  },
  primaryButton: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    flexDirection: 'row',
    gap: 8,
    marginTop: 28,
    minHeight: 50,
    paddingHorizontal: 22,
  },
  primaryButtonText: { fontSize: 16, fontWeight: '700' },
  securityNote: {
    alignItems: 'flex-start',
    flexDirection: 'row',
    gap: 8,
    marginTop: 24,
    maxWidth: 330,
  },
  securityText: { flex: 1, fontSize: 12, lineHeight: 17 },
  emptyState: {
    alignItems: 'center',
    flex: 1,
    justifyContent: 'center',
    minHeight: 360,
    paddingHorizontal: 40,
  },
  emptyIcon: {
    alignItems: 'center',
    borderRadius: 20,
    height: 64,
    justifyContent: 'center',
    marginBottom: 18,
    width: 64,
  },
  emptyTitle: { fontSize: 17, fontWeight: '700', textAlign: 'center' },
  emptyBody: { fontSize: 14, lineHeight: 20, marginTop: 7, maxWidth: 320, textAlign: 'center' },
  emptyAction: {
    borderRadius: Radius.pill,
    justifyContent: 'center',
    marginTop: 18,
    minHeight: 42,
    paddingHorizontal: 18,
  },
  emptyActionText: { fontSize: 14, fontWeight: '700' },
  sessionRow: {
    borderRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: 12,
    marginBottom: 8,
    marginHorizontal: Spacing.three,
    minHeight: 74,
    paddingHorizontal: 14,
    paddingVertical: 12,
  },
  sessionCopy: { flex: 1, justifyContent: 'center' },
  sessionTitle: { fontSize: 15.5, fontWeight: '600', letterSpacing: -0.15, lineHeight: 20 },
  sessionMetadata: { alignItems: 'center', flexDirection: 'row', marginTop: 6, minWidth: 0 },
  sessionMetaText: { flexShrink: 1, fontSize: 12.5 },
  metadataBullet: { fontSize: 12, marginHorizontal: 6 },
  providerBadge: { alignItems: 'center', flexDirection: 'row', gap: 5 },
  sessionTrailing: { alignItems: 'flex-end', gap: 6, justifyContent: 'center', minWidth: 64 },
  sessionTime: { fontSize: 11.5 },
  statusLine: { alignItems: 'center', flexDirection: 'row', gap: 5 },
  statusDot: { borderRadius: Radius.pill, height: 7, width: 7 },
  statusSpinner: { height: 12, transform: [{ scale: 0.6 }], width: 12 },
  statusLabel: { fontSize: 11.5, fontWeight: '600' },
  actionSheetTitle: {
    fontSize: 14,
    fontWeight: '700',
    marginBottom: 4,
    marginHorizontal: 12,
    marginTop: 6,
  },
});
