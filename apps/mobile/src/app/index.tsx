import type { AgentSession, SidebarGrouping, SidebarOrdering } from '@padu/client';
import AsyncStorage from '@react-native-async-storage/async-storage';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Alert,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  RefreshControl,
  SectionList,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { PaduIcon } from '@/components/padu-icon';
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
import { useSidebarGroups, useTaskState } from '@/hooks/use-daemon-data';
import { useResponsive } from '@/hooks/use-responsive';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import { useRuntime } from '@/lib/runtime-context';
import {
  daemonGroupsToProjectSections,
  daemonGroupsToSessionSections,
  displaySessionTitle,
  providerLabel,
  type SessionGroup,
  type SessionListItem,
} from '@/lib/session-presentation';

/** Matches the daemon's `SIDEBAR_PROJECT_REVEAL_BATCH`: one tap reveals more. */
const SHOW_MORE_BATCH = 30;

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
  // Sidebar view, mirroring the desktop sidebar: grouping (project/updated)
  // and ordering (newest/oldest) drive the daemon-owned sort & grouping
  // engine; collapsed sections and per-group "show more" counts are local
  // presentation state that travels back with the request.
  const [grouping, setGrouping] = useState<SidebarGrouping>('project');
  const [ordering, setOrdering] = useState<SidebarOrdering>('newest');
  const [collapsedGroups, setCollapsedGroups] = useState<ReadonlySet<string>>(new Set());
  const [revealedOlderCounts, setRevealedOlderCounts] = useState<Record<string, number>>({});
  const [optionsOpen, setOptionsOpen] = useState(false);
  useEffect(() => {
    void AsyncStorage.multiGet(['padu:sidebar_grouping', 'padu:sidebar_ordering'])
      .then((entries) => {
        for (const [key, value] of entries) {
          if (key === 'padu:sidebar_grouping' && (value === 'project' || value === 'updated')) {
            setGrouping(value);
          }
          if (key === 'padu:sidebar_ordering' && (value === 'newest' || value === 'oldest')) {
            setOrdering(value);
          }
        }
      })
      .catch(() => {});
  }, []);
  useEffect(() => {
    void AsyncStorage.setItem('padu:sidebar_grouping', grouping).catch(() => {});
  }, [grouping]);
  useEffect(() => {
    void AsyncStorage.setItem('padu:sidebar_ordering', ordering).catch(() => {});
  }, [ordering]);
  // Daemon-owned ordering and grouping; the day key refetches when the
  // local calendar day rolls over.
  const sidebarGroups = useSidebarGroups(grouping, ordering, new Date().toDateString(), revealedOlderCounts);
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
  const sections = useMemo(() => {
    if (!taskState.data) return [];
    const sessionsById = new Map(taskState.data.sessions.map((item) => [item.id, item]));
    const visibleIds = new Set(visibleSessions.map((item) => item.id));
    const groups = grouping === 'project'
      ? daemonGroupsToProjectSections(
        sidebarGroups.data ?? [],
        sessionsById,
        taskState.data.projects,
        visibleIds,
      )
      : daemonGroupsToSessionSections(
        sidebarGroups.data ?? [],
        sessionsById,
        taskState.data.projects,
        visibleIds,
      );
    return groups.map((section) =>
      collapsedGroups.has(section.id) ? { ...section, data: [] } : section);
  }, [taskState.data, visibleSessions, sidebarGroups.data, grouping, collapsedGroups]);
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
  const toggleSectionCollapsed = useCallback((sectionId: string) => {
    setCollapsedGroups((current) => {
      const next = new Set(current);
      if (next.has(sectionId)) next.delete(sectionId);
      else next.add(sectionId);
      return next;
    });
    // Collapsing resets the section's revealed older sessions, like desktop.
    setRevealedOlderCounts((current) => {
      if (!current[sectionId]) return current;
      const next = { ...current };
      delete next[sectionId];
      return next;
    });
  }, []);
  const revealMoreSessions = useCallback((sectionId: string) => {
    setRevealedOlderCounts((current) => ({
      ...current,
      [sectionId]: (current[sectionId] ?? 0) + SHOW_MORE_BATCH,
    }));
  }, []);
  const renderSectionHeader = useCallback(({ section }: { section: SessionGroup }) => {
    const collapsed = collapsedGroups.has(section.id);
    return (
      <Pressable
        accessibilityLabel={`${section.title}, ${collapsed ? 'collapsed' : 'expanded'}`}
        accessibilityRole="button"
        accessibilityState={{ expanded: !collapsed }}
        onPress={() => toggleSectionCollapsed(section.id)}
        style={styles.sectionHeader}>
        {section.kind === 'pinned' ? (
          <PaduIcon name="pin" size={14} tintColor={theme.textTertiary} />
        ) : section.kind === 'updated' ? null : (
          <PaduIcon
            name={collapsed ? 'folder' : 'folderOpen'}
            size={14}
            tintColor={theme.textTertiary}
          />
        )}
        <Text style={[styles.sectionTitle, { color: theme.textTertiary }]}>
          {section.title.toUpperCase()}
        </Text>
        <View style={styles.sectionChevron}>
          <PaduIcon
            name="chevronDown"
            size={14}
            tintColor={theme.textTertiary}
            style={collapsed ? styles.chevronCollapsed : undefined}
          />
        </View>
      </Pressable>
    );
  }, [collapsedGroups, theme.textTertiary, toggleSectionCollapsed]);
  const renderSectionFooter = useCallback(({ section }: { section: SessionGroup }) => {
    if (!section.hasMore || collapsedGroups.has(section.id)) return null;
    return (
      <Pressable
        accessibilityLabel={`Show more tasks in ${section.title}`}
        accessibilityRole="button"
        onPress={() => revealMoreSessions(section.id)}
        style={styles.showMore}>
        <Text style={[styles.showMoreLabel, { color: theme.textTertiary }]}>
          Show more
        </Text>
      </Pressable>
    );
  }, [collapsedGroups, revealMoreSessions, theme.textTertiary]);
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
            accessibilityHint="Changes how tasks are grouped and ordered"
            glyphSize={18}
            icon="listFilter"
            label="List options"
            onPress={() => setOptionsOpen(true)}
          />
          <IconButton
            accessibilityHint="Searches the task list"
            glyphSize={18}
            icon="search"
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
      renderSectionFooter={renderSectionFooter}
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
        icon="compose"
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
              leading={<PaduIcon name="pencil" size={16} tintColor={theme.textSecondary} />}
              onPress={() => {
                const target = actionTarget;
                setActionTarget(null);
                setRenameTarget(target);
              }}
            />
            <SheetRow
              destructive
              label="Delete task"
              leading={<PaduIcon name="trash" size={16} tintColor={theme.danger} />}
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
      <Sheet title="Task list" onDismiss={() => setOptionsOpen(false)} visible={optionsOpen}>
        <SheetRow
          label="Group by project"
          leading={<PaduIcon name="folder" size={16} tintColor={theme.textSecondary} />}
          onPress={() => {
            setGrouping('project');
            setOptionsOpen(false);
          }}
          selected={grouping === 'project'}
        />
        <SheetRow
          label="Group by updated"
          leading={<PaduIcon name="listFilter" size={16} tintColor={theme.textSecondary} />}
          onPress={() => {
            setGrouping('updated');
            setOptionsOpen(false);
          }}
          selected={grouping === 'updated'}
        />
        <SheetRow
          label="Newest first"
          onPress={() => {
            setOrdering('newest');
            setOptionsOpen(false);
          }}
          selected={ordering === 'newest'}
        />
        <SheetRow
          label="Oldest first"
          onPress={() => {
            setOrdering('oldest');
            setOptionsOpen(false);
          }}
          selected={ordering === 'oldest'}
        />
      </Sheet>
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
                  <PaduIcon
                    name="list"
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
  sectionHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 6,
    marginHorizontal: Spacing.three,
    marginTop: 18,
    marginBottom: 8,
    minHeight: 44,
  },
  sectionTitle: {
    flexShrink: 1,
    fontSize: 11,
    fontWeight: '700',
    letterSpacing: 0.65,
  },
  sectionChevron: { marginLeft: 'auto' },
  chevronCollapsed: { transform: [{ rotate: '-90deg' }] },
  showMore: {
    alignItems: 'flex-start',
    justifyContent: 'center',
    marginHorizontal: Spacing.three,
    minHeight: 44,
    paddingLeft: 20,
  },
  showMoreLabel: { fontSize: 13, fontWeight: '600' },
  actionSheetTitle: {
    fontSize: 14,
    fontWeight: '700',
    marginBottom: 4,
    marginHorizontal: 12,
    marginTop: 6,
  },
});
