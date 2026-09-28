import type { AgentSession, SidebarGrouping, SidebarOrdering } from '@padu/client';
import AsyncStorage from '@react-native-async-storage/async-storage';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Alert,
  FlatList,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  RefreshControl,
  ScrollView,
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
import { MaxContentWidth, Radius, Spacing } from '@/constants/theme';
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
  relativeSessionTime,
  sessionTimestamp,
  type SessionGroup,
  type SessionListItem,
} from '@/lib/session-presentation';

/** Matches the daemon's `SIDEBAR_PROJECT_REVEAL_BATCH`: one tap reveals more. */
const SHOW_MORE_BATCH = 30;

/** Stable identity so the list doesn't see a new key fn every render. */
function sessionKeyExtractor(item: SessionListItem): string {
  return item.session.id;
}

/** Shared empty array so the tab list keeps a stable `data` prop with no tab. */
const EMPTY_TAB_DATA: SessionListItem[] = [];

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
  // presentation state that travels back with the request. Project grouping
  // renders as horizontally scrollable project tabs; updated grouping keeps
  // the stacked date sections.
  const [grouping, setGrouping] = useState<SidebarGrouping>('project');
  const [ordering, setOrdering] = useState<SidebarOrdering>('newest');
  const [collapsedGroups, setCollapsedGroups] = useState<ReadonlySet<string>>(new Set());
  const [revealedOlderCounts, setRevealedOlderCounts] = useState<Record<string, number>>({});
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
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
  const projectNamesById = useMemo(() => (
    new Map((taskState.data?.projects ?? []).map((project) => [project.id, project.name]))
  ), [taskState.data]);
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
  // Project tabs: pinned tasks get their own leading tab, then one tab per
  // project / projectless group — horizontally scrollable.
  const tabs = useMemo(
    () => sections.filter((section) => (
      (section.kind === 'pinned' ||
        section.kind === 'project' ||
        section.kind === 'projectless') &&
      section.data.length > 0
    )),
    [sections],
  );
  const activeTab = useMemo(() => (
    tabs.find((tab) => tab.id === activeTabId) ?? tabs[0] ?? null
  ), [activeTabId, tabs]);
  useEffect(() => {
    if (grouping !== 'project') return;
    if (!activeTab && tabs.length > 0) setActiveTabId(tabs[0]!.id);
    else if (activeTabId && !tabs.some((tab) => tab.id === activeTabId)) {
      setActiveTabId(tabs[0]?.id ?? null);
    }
  }, [activeTab, activeTabId, grouping, tabs]);
  // Membership against every session (not the filtered view), so typing a
  // filter never yanks the open session — only a real disappearance refalls.
  const allSessionIds = useMemo(
    () => new Set((taskState.data?.sessions ?? []).map((session) => session.id)),
    [taskState.data],
  );
  const fallbackSessionId = grouping === 'project'
    ? (activeTab?.data[0]?.session.id
      ?? sections[0]?.data[0]?.session.id
      ?? taskState.data?.sessions[0]?.id
      ?? null)
    : (sections[0]?.data[0]?.session.id
      ?? taskState.data?.sessions[0]?.id
      ?? null);

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

  function togglePin(session: AgentSession) {
    const pinned = session.pinned_at != null;
    void Haptics.selectionAsync();
    void runtime.setSessionPinned(session.id, !pinned)
      .then(() => Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success))
      .catch((cause) => {
        Alert.alert(
          pinned ? 'Couldn’t unpin task' : 'Couldn’t pin task',
          cause instanceof Error ? cause.message : String(cause),
        );
      });
  }

  function toggleArchive(session: AgentSession) {
    const archived = session.archived_at != null;
    void Haptics.selectionAsync();
    void runtime.setSessionArchived(session.id, !archived)
      .then(() => {
        void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
        // Archiving hides the task from the list; fall forward when the open
        // wide-pane task was just archived, mirroring web.
        if (!archived && session.id === selectedSessionId) setSelectedSessionId(null);
      })
      .catch((cause) => {
        Alert.alert(
          archived ? 'Couldn’t unarchive task' : 'Couldn’t archive task',
          cause instanceof Error ? cause.message : String(cause),
        );
      });
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
  const selectProjectTab = useCallback((sectionId: string) => {
    void Haptics.selectionAsync();
    setActiveTabId(sectionId);
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
    // Breathing room between the tab bar / search and the first row, plus
    // clearance for the floating New Task button above the home indicator.
    {
      paddingTop: Spacing.two,
      paddingBottom: insets.bottom + Spacing.six + Spacing.four,
    },
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
  const projectTabBar = useMemo(() => {
    if (grouping !== 'project' || tabs.length === 0) return null;
    return (
      <View style={styles.tabBarOuter}>
        <ScrollView
          horizontal
          accessibilityRole="tablist"
          contentContainerStyle={styles.tabBarContent}
          showsHorizontalScrollIndicator={false}>
          {tabs.map((tab) => {
            const selected = tab.id === activeTab?.id;
            const count = tab.data.length;
            const isPinnedTab = tab.kind === 'pinned';
            return (
              <Pressable
                key={tab.id}
                accessibilityLabel={`${tab.title}, ${count}${tab.hasMore ? ' or more' : ''} tasks`}
                accessibilityRole="tab"
                accessibilityState={{ selected }}
                hitSlop={{ top: 6, bottom: 6, left: 2, right: 2 }}
                onPress={() => selectProjectTab(tab.id)}
                style={[
                  styles.tab,
                  {
                    backgroundColor: selected ? theme.accentSoft : theme.surface,
                    borderColor: selected ? theme.accent : theme.border,
                  },
                ]}>
                <PaduIcon
                  name={isPinnedTab ? 'pin' : selected ? 'folderOpen' : 'folder'}
                  size={13}
                  tintColor={selected ? theme.accent : theme.textTertiary}
                />
                <Text
                  numberOfLines={1}
                  style={[
                    styles.tabLabel,
                    { color: selected ? theme.text : theme.textSecondary },
                  ]}>
                  {tab.title}
                </Text>
                <View
                  style={[
                    styles.tabCount,
                    { backgroundColor: selected ? theme.accentSoft : theme.overlay },
                  ]}>
                  <Text
                    style={[
                      styles.tabCountLabel,
                      { color: selected ? theme.accent : theme.textTertiary },
                    ]}>
                    {tab.hasMore ? `${count}+` : `${count}`}
                  </Text>
                </View>
              </Pressable>
            );
          })}
        </ScrollView>
      </View>
    );
  }, [activeTab?.id, grouping, tabs, selectProjectTab, theme]);
  const projectTabFooter = useMemo(() => {
    if (!activeTab?.hasMore) return null;
    return (
      <Pressable
        accessibilityLabel={`Show more tasks in ${activeTab.title}`}
        accessibilityRole="button"
        onPress={() => revealMoreSessions(activeTab.id)}
        style={styles.showMore}>
        <Text style={[styles.showMoreLabel, { color: theme.textTertiary }]}>
          Show more
        </Text>
      </Pressable>
    );
  }, [activeTab, revealMoreSessions, theme.textTertiary]);

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

  const projectTabList = (
    <FlatList
      data={activeTab?.data ?? EMPTY_TAB_DATA}
      keyExtractor={sessionKeyExtractor}
      contentContainerStyle={listContentStyle}
      style={styles.list}
      refreshControl={(
        <RefreshControl
          refreshing={taskState.isRefetching}
          tintColor={theme.textTertiary}
          onRefresh={handleRefresh}
        />
      )}
      ListHeaderComponent={listHeaderComponent ?? undefined}
      ListFooterComponent={projectTabFooter ?? undefined}
      ListEmptyComponent={listEmptyComponent}
      renderItem={renderSessionItem}
      showsVerticalScrollIndicator={false}
    />
  );

  const taskList = grouping === 'project' ? (
    <>
      {projectTabBar}
      {projectTabList}
    </>
  ) : (
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

  const actionTargetProject = actionTarget
    ? (projectNamesById.get(actionTarget.project_id) ?? 'No project')
    : null;
  const actionTargetPinned = (actionTarget?.pinned_at ?? null) != null;
  const actionTargetArchived = (actionTarget?.archived_at ?? null) != null;
  const actionTargetTime = actionTarget
    ? relativeSessionTime(sessionTimestamp(actionTarget))
    : null;

  const sheets = (
    <>
      <Sheet onDismiss={() => setActionTarget(null)} visible={actionTarget !== null}>
        {actionTarget && (
          <>
            <View style={styles.actionSheetHeader}>
              <Text numberOfLines={2} style={[styles.actionSheetTitle, { color: theme.text }]}>
                {displaySessionTitle(actionTarget)}
              </Text>
              <View style={styles.actionSheetMeta}>
                <PaduIcon name="folder" size={13} tintColor={theme.textTertiary} />
                <Text
                  numberOfLines={1}
                  style={[styles.actionSheetMetaText, { color: theme.textSecondary }]}>
                  {actionTargetProject}
                </Text>
                <Text style={[styles.actionSheetMetaText, { color: theme.textGhost }]}>·</Text>
                <PaduIcon name="clock" size={13} tintColor={theme.textTertiary} />
                <Text style={[styles.actionSheetMetaText, { color: theme.textSecondary }]}>
                  {actionTargetTime}
                </Text>
                {actionTargetPinned && (
                  <View
                    accessibilityLabel="Pinned"
                    accessibilityRole="image"
                    style={[styles.actionSheetBadge, { backgroundColor: theme.accentSoft }]}>
                    <PaduIcon name="pin" size={11} tintColor={theme.accent} />
                    <Text style={[styles.actionSheetBadgeLabel, { color: theme.accent }]}>
                      Pinned
                    </Text>
                  </View>
                )}
                {actionTargetArchived && (
                  <View
                    accessibilityLabel="Archived"
                    accessibilityRole="image"
                    style={[styles.actionSheetBadge, { backgroundColor: theme.overlay }]}>
                    <PaduIcon name="archive" size={11} tintColor={theme.textSecondary} />
                    <Text style={[styles.actionSheetBadgeLabel, { color: theme.textSecondary }]}>
                      Archived
                    </Text>
                  </View>
                )}
              </View>
            </View>
            <View style={[styles.actionSheetDivider, { backgroundColor: theme.separator }]} />
            <SheetRow
              label={actionTargetPinned ? 'Unpin task' : 'Pin task'}
              description={actionTargetPinned ? 'Remove from pinned' : 'Keep at the top of the list'}
              leading={<PaduIcon name="pin" size={16} tintColor={theme.textSecondary} />}
              onPress={() => {
                const target = actionTarget;
                setActionTarget(null);
                if (target) togglePin(target);
              }}
            />
            <SheetRow
              label={actionTargetArchived ? 'Unarchive task' : 'Archive task'}
              description={actionTargetArchived ? 'Restore to the task list' : 'Hide from the task list'}
              leading={<PaduIcon name="archive" size={16} tintColor={theme.textSecondary} />}
              onPress={() => {
                const target = actionTarget;
                setActionTarget(null);
                if (target) toggleArchive(target);
              }}
            />
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
  tabBarOuter: {
    alignSelf: 'center',
    maxWidth: MaxContentWidth,
    width: '100%',
  },
  tabBarContent: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: Spacing.one,
    paddingHorizontal: Spacing.three,
    paddingVertical: Spacing.one,
  },
  // Compact trigger: the visual pill is 32pt tall; `hitSlop` on the Pressable
  // keeps the effective touch target at ≥ 44pt.
  tab: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: 5,
    maxWidth: 200,
    minHeight: 32,
    paddingHorizontal: 10,
    paddingVertical: 6,
  },
  tabLabel: {
    flexShrink: 1,
    fontSize: 13,
    fontWeight: '600',
  },
  tabCount: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    justifyContent: 'center',
    minHeight: 18,
    minWidth: 22,
    paddingHorizontal: 6,
  },
  tabCountLabel: { fontSize: 11, fontWeight: '700' },
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
  actionSheetHeader: {
    gap: Spacing.one,
    marginHorizontal: 12,
    marginTop: 6,
  },
  actionSheetTitle: {
    fontSize: 17,
    fontWeight: '700',
    letterSpacing: -0.2,
    lineHeight: 22,
  },
  actionSheetMeta: {
    alignItems: 'center',
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 6,
  },
  actionSheetMetaText: {
    flexShrink: 1,
    fontSize: 12.5,
    fontWeight: '500',
  },
  actionSheetBadge: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    flexDirection: 'row',
    gap: 4,
    paddingHorizontal: 8,
    paddingVertical: 3,
  },
  actionSheetBadgeLabel: { fontSize: 11.5, fontWeight: '700' },
  actionSheetDivider: {
    height: StyleSheet.hairlineWidth,
    marginHorizontal: 12,
    marginVertical: Spacing.two,
  },
});
