import type { AgentSession } from '@padu/client';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { useMemo, useState } from 'react';
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
import { Sheet, SheetRow } from '@/components/sheet';
import { TaskListEmpty } from '@/components/task-list-empty';
import { ThemeToggleButton } from '@/components/theme-toggle-button';
import { MaxContentWidth, Spacing } from '@/constants/theme';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import { useRuntime } from '@/lib/runtime-context';
import {
  displaySessionTitle,
  groupSessions,
  providerLabel,
} from '@/lib/session-presentation';

/** The task list: the home screen, and the daemon it is talking to. */
export default function TasksScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const taskState = useTaskState();
  const [search, setSearch] = useState('');
  const [searchOpen, setSearchOpen] = useState(false);
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

  const showOnboarding = !daemon.profiles.length && daemon.phase !== 'booting';

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      style={[styles.screen, { backgroundColor: theme.background }]}>
      {showOnboarding ? (
        <Onboarding />
      ) : (
        <>
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
          {searchOpen && (
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
          )}
          <SectionList
            sections={sections}
            keyExtractor={(item) => item.session.id}
            contentContainerStyle={[
              // Clears the floating New Task button, which sits above the
              // home indicator.
              { paddingBottom: insets.bottom + Spacing.six + Spacing.four },
              sections.length === 0 && styles.listContentEmpty,
            ]}
            contentInsetAdjustmentBehavior="never"
            style={styles.list}
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
            ListHeaderComponent={daemon.error ? (
              <View style={styles.alertSlot}>
                <ConnectionErrorCard />
              </View>
            ) : undefined}
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
        </>
      )}

      {!showOnboarding && !searchOpen && (
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

const styles = StyleSheet.create({
  screen: { flex: 1 },
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
