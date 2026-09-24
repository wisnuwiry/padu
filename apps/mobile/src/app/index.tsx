import type { AgentSession } from '@padu/client';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Alert,
  KeyboardAvoidingView,
  Platform,
  RefreshControl,
  SectionList,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { AppSymbol } from '@/components/app-symbol';
import { IconButton } from '@/components/button';
import { ConnectionErrorCard } from '@/components/connection-error-card';
import { DaemonPicker } from '@/components/daemon-picker';
import { Onboarding } from '@/components/onboarding';
import { RenameDialog } from '@/components/rename-dialog';
import { ScreenHeader } from '@/components/screen-header';
import { SearchField } from '@/components/search-field';
import { SessionRow } from '@/components/session-row';
import { SessionView } from '@/components/session-view';
import { Sheet, SheetRow } from '@/components/sheet';
import { TaskListEmpty } from '@/components/task-list-empty';
import { ThemeToggleButton } from '@/components/theme-toggle-button';
import { PaneMinima } from '@/constants/layout';
import { MaxContentWidth, Spacing } from '@/constants/theme';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useResponsive } from '@/hooks/use-responsive';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import { useRuntime } from '@/lib/runtime-context';
import {
  displaySessionTitle,
  groupSessions,
  providerLabel,
  type SessionListItem,
} from '@/lib/session-presentation';

/** Stable identity so the list doesn't see a new key fn every render. */
function sessionKeyExtractor(item: SessionListItem): string {
  return item.session.id;
}

/** The task list: the home screen, and the daemon it is talking to. */
export default function TasksScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const taskState = useTaskState();
  const { isWide } = useResponsive();
  const [search, setSearch] = useState('');
  const [searchOpen, setSearchOpen] = useState(false);
  const [actionTarget, setActionTarget] = useState<AgentSession | null>(null);
  const [renameTarget, setRenameTarget] = useState<AgentSession | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
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
  // Membership against every session (not the filtered view), so typing a
  // filter never yanks the open session — only a real disappearance refalls.
  const allSessionIds = useMemo(
    () => new Set((taskState.data?.sessions ?? []).map((session) => session.id)),
    [taskState.data],
  );
  const fallbackSessionId = sections[0]?.data[0]?.session.id
    ?? taskState.data?.sessions[0]?.id
    ?? null;

  // The wide two-pane keeps a selection instead of pushing a route. Default
  // to the most recent task so the detail pane is never empty on launch, and
  // fall forward when the selected task disappears (deleted, not filtered).
  useEffect(() => {
    if (!isWide || !taskState.data) return;
    if (selectedSessionId && allSessionIds.has(selectedSessionId)) return;
    if (fallbackSessionId !== selectedSessionId) setSelectedSessionId(fallbackSessionId);
  }, [allSessionIds, fallbackSessionId, isWide, selectedSessionId, taskState.data]);

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

  const showOnboarding = !daemon.profiles.length && daemon.phase !== 'booting';

  // Stable list identities: without these every keystroke and every query
  // refetch hands SectionList new fn/object props and re-renders all rows.
  // Deps stay on stable primitives — the daemon context value and the query
  // result are fresh objects each render, so depend on their fields instead.
  const daemonPhase = daemon.phase;
  const daemonReconnect = daemon.reconnect;
  const taskRefetch = taskState.refetch;
  const handleRefresh = useCallback(() => {
    if (daemonPhase === 'connected') void taskRefetch();
    else void daemonReconnect();
  }, [daemonPhase, daemonReconnect, taskRefetch]);
  const renderSectionHeader = useCallback(({ section }: { section: { title: string } }) => (
    <Text style={[styles.sectionTitle, { color: theme.textTertiary }]}>
      {section.title.toUpperCase()}
    </Text>
  ), [theme.textTertiary]);
  const renderSessionItem = useCallback(({ item }: { item: SessionListItem }) => (
    <SessionRow
      item={item}
      onLongPress={() => {
        void Haptics.selectionAsync();
        setActionTarget(item.session);
      }}
      // Narrow leaves onPress undefined so the row pushes the session route
      // itself; wide selects into the side pane. No per-mode prop spreading.
      onPress={isWide ? () => setSelectedSessionId(item.session.id) : undefined}
      selected={isWide && item.session.id === selectedSessionId}
    />
  ), [isWide, selectedSessionId]);
  const listContentStyle = useMemo(() => [
    // Clears the floating New Task button, which sits above the
    // home indicator.
    { paddingBottom: insets.bottom + Spacing.six + Spacing.four },
    sections.length === 0 && styles.listContentEmpty,
  ], [insets.bottom, sections.length]);
  const listHeaderComponent = useMemo(() => daemon.error ? (
    <View style={styles.alertSlot}>
      <ConnectionErrorCard />
    </View>
  ) : undefined, [daemon.error]);
  const listEmptyComponent = useMemo(() => (
    <TaskListEmpty
      connecting={daemon.phase === 'booting' || daemon.phase === 'connecting'}
      error={taskState.error}
      searching={Boolean(search.trim())}
    />
  ), [daemon.phase, search, taskState.error]);

  const listHeader = (
    <ScreenHeader
      back={false}
      leading={<DaemonPicker />}
      right={(
        <>
          <IconButton
            accessibilityHint="Searches the task list"
            glyphSize={18}
            icon={{ ios: 'magnifyingglass', android: 'search', web: 'search' }}
            label="Search"
            onPress={() => setSearchOpen(true)}
          />
          <ThemeToggleButton />
        </>
      )}
    />
  );

  const searchBar = searchOpen && (
    <>
      <View style={styles.searchBar}>
        <SearchField
          onChange={setSearch}
          onClose={() => {
            setSearch('');
            setSearchOpen(false);
          }}
          value={search}
        />
      </View>
      <View style={[styles.divider, { backgroundColor: theme.separator }]} />
    </>
  );

  const taskList = (
    <SectionList
      sections={sections}
      keyExtractor={sessionKeyExtractor}
      contentContainerStyle={listContentStyle}
      contentInsetAdjustmentBehavior="never"
      style={styles.list}
      refreshControl={(
        <RefreshControl
          refreshing={taskState.isRefetching}
          tintColor={theme.textTertiary}
          onRefresh={handleRefresh}
        />
      )}
      renderSectionHeader={renderSectionHeader}
      renderItem={renderSessionItem}
      ListHeaderComponent={listHeaderComponent}
      ListEmptyComponent={listEmptyComponent}
      showsVerticalScrollIndicator={false}
      stickySectionHeadersEnabled={false}
    />
  );

  const fab = !showOnboarding && !searchOpen && (
    <View
      pointerEvents="box-none"
      style={[styles.fabDock, { bottom: insets.bottom + Spacing.three }]}>
      <IconButton
        accessibilityHint="Starts a new agent task"
        glyphSize={22}
        icon={{ ios: 'square.and.pencil', android: 'edit_square', web: 'edit' }}
        label="New task"
        onPress={() => router.push('/new-task')}
        size={52}
        variant="filled"
      />
    </View>
  );

  const sheets = (
    <>
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
    </>
  );

  // Wide (fold unfolded landscape, iPad landscape, 13" portrait and up):
  // list rail beside the open session, mirroring web's `lg` inline sidebar.
  // Narrow (phones, cover screens, portrait tablets, Split View ⅓): the same
  // single column as before, with push navigation into the session route.
  if (isWide && !showOnboarding) {
    return (
      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
        style={[styles.screen, { backgroundColor: theme.background }]}>
        <View style={styles.wideBody}>
          <View style={[styles.rail, { borderRightColor: theme.separator }]}>
            {listHeader}
            {searchBar}
            {taskList}
            {fab}
          </View>
          <View style={styles.detail}>
            <View style={styles.detailInner}>
              {selectedSessionId ? (
                <SessionView
                  key={selectedSessionId}
                  sessionId={selectedSessionId}
                  showBack={false}
                />
              ) : (
                <View style={styles.detailEmpty}>
                  <AppSymbol
                    name={{ ios: 'bubble.left.and.bubble.right', android: 'forum', web: 'forum' }}
                    size={28}
                    tintColor={theme.textGhost}
                  />
                  <Text style={[styles.detailEmptyTitle, { color: theme.text }]}>
                    Select a task
                  </Text>
                  <Text style={[styles.detailEmptyBody, { color: theme.textSecondary }]}>
                    Choose a task from the list to open it here.
                  </Text>
                </View>
              )}
            </View>
          </View>
        </View>
        {sheets}
      </KeyboardAvoidingView>
    );
  }

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      style={[styles.screen, { backgroundColor: theme.background }]}>
      {showOnboarding ? (
        <Onboarding />
      ) : (
        <>
          {listHeader}
          {searchBar}
          {taskList}
        </>
      )}

      {fab}
      {sheets}
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  // Two-pane shell: the rail keeps desktop's 320–360 width, the detail takes
  // the rest and centers a capped reading column inside it.
  wideBody: { flex: 1, flexDirection: 'row' },
  rail: {
    borderRightWidth: StyleSheet.hairlineWidth,
    flexShrink: 0,
    width: PaneMinima.listRailMax,
  },
  detail: { alignItems: 'center', flex: 1 },
  detailInner: { flex: 1, maxWidth: MaxContentWidth, width: '100%' },
  detailEmpty: {
    alignItems: 'center',
    flex: 1,
    gap: Spacing.two,
    justifyContent: 'center',
    paddingHorizontal: Spacing.four,
  },
  detailEmptyTitle: { fontSize: 17, fontWeight: '700', textAlign: 'center' },
  detailEmptyBody: { fontSize: 13.5, lineHeight: 19, textAlign: 'center' },
  searchBar: {
    alignSelf: 'center',
    maxWidth: MaxContentWidth,
    paddingBottom: Spacing.two,
    paddingHorizontal: Spacing.three,
    paddingTop: Spacing.three,
    width: '100%',
  },
  divider: { height: StyleSheet.hairlineWidth },
  alertSlot: { marginTop: Spacing.three },
  fabDock: { position: 'absolute', right: Spacing.three, zIndex: 20 },
  list: { alignSelf: 'center', flex: 1, maxWidth: MaxContentWidth, width: '100%' },
  listContentEmpty: { flexGrow: 1 },
  sectionTitle: {
    fontSize: 11,
    fontWeight: '700',
    letterSpacing: 0.65,
    marginBottom: 8,
    marginHorizontal: Spacing.three,
    marginTop: 18,
  },
  actionSheetTitle: {
    fontSize: 14,
    fontWeight: '700',
    marginBottom: 4,
    marginHorizontal: 12,
    marginTop: 6,
  },
});
