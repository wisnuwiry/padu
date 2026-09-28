import * as Haptics from 'expo-haptics';
import { router, Stack } from 'expo-router';
import { memo, useCallback, useState, type ComponentProps } from 'react';
import {
  ActivityIndicator,
  FlatList,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';

import { PaduIcon } from '@/components/padu-icon';
import { Button, IconButton } from '@/components/button';
import { ConnectionErrorCard } from '@/components/connection-error-card';
import { ConnectionNote } from '@/components/connection-note';
import { ConnectionStatus } from '@/components/connection-status';
import { DaemonAvatar } from '@/components/daemon-avatar';
import { navigateBack, ScreenHeader } from '@/components/screen-header';
import { TransportBadge } from '@/components/transport-badge';
import { MaxContentWidth, Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import {
  classifyConnection,
  describeConnection,
  displayHost,
  formatLastUsed,
  type DaemonProfile,
} from '@/lib/daemon-profile';

/** Stable identity so the list doesn't see a new key fn every render. */
function daemonKeyExtractor(item: DaemonProfile): string {
  return item.id;
}

export default function DaemonsScreen() {
  const theme = useTheme();
  const daemon = useDaemon();
  const [selectingId, setSelectingId] = useState<string | null>(null);

  const select = useCallback(async (profile: DaemonProfile) => {
    if (selectingId) return;
    if (profile.id === daemon.activeProfile?.id) {
      navigateBack();
      return;
    }
    setSelectingId(profile.id);
    try {
      await Haptics.selectionAsync();
      const connected = await daemon.selectProfile(profile.id);
      if (connected) navigateBack();
      else await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    } finally {
      setSelectingId(null);
    }
  }, [daemon, selectingId]);

  const edit = useCallback((id: string) => {
    router.push({ pathname: '/daemon-editor', params: { id } });
  }, []);

  const renderDaemon = useCallback(({ item }: { item: DaemonProfile }) => (
    <DaemonRow
      active={item.id === daemon.activeProfile?.id}
      onEdit={edit}
      onSelect={select}
      phase={daemon.phase}
      profile={item}
      selecting={item.id === selectingId}
    />
  ), [daemon.activeProfile?.id, daemon.phase, edit, select, selectingId]);

  const empty = daemon.profiles.length === 0;

  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ headerShown: false }} />
      <ScreenHeader
        right={(
          <>
            <IconButton
              accessibilityHint="Opens the camera to scan the desktop’s QR code"
              glyphSize={18}
              icon="scan"
              label="Import from link"
              onPress={() => router.push('/daemon-import')}
            />
            <IconButton
              accessibilityHint="Opens the daemon editor"
              glyphSize={18}
              icon="plus"
              label="Add daemon"
              onPress={() => router.push('/daemon-editor')}
              variant="filled"
            />
          </>
        )}
        title="Daemons"
      />
      <FlatList
        data={daemon.profiles}
        keyExtractor={daemonKeyExtractor}
        contentInsetAdjustmentBehavior="never"
        contentContainerStyle={[styles.listContent, empty && styles.listContentEmpty]}
        style={styles.list}
        ListHeaderComponent={(
          <>
            <Text style={[styles.intro, { color: theme.textSecondary }]}>
              Switch hosts without re-entering credentials. Only the selected
              daemon stays connected.
            </Text>
            {daemon.error ? <ConnectionErrorCard /> : null}
          </>
        )}
        ListEmptyComponent={(
          <View style={styles.empty}>
            <View style={[styles.emptyIcon, { backgroundColor: theme.overlayStrong }]}>
              <PaduIcon
                name="server"
                size={26}
                tintColor={theme.textTertiary}
              />
            </View>
            <Text style={[styles.emptyTitle, { color: theme.text }]}>
              No saved daemons
            </Text>
            <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
              Scan the QR code in Padu Desktop’s host dialog with your camera, or
              add the address and token by hand.
            </Text>
            <View style={styles.emptyActions}>
              <Button
                accessibilityHint="Opens the import screen"
                icon="scan"
                label="Import from Link"
                onPress={() => router.push('/daemon-import')}
              />
              <Button
                accessibilityHint="Opens the daemon editor"
                icon="plus"
                label="Add by Hand"
                onPress={() => router.push('/daemon-editor')}
                variant="secondary"
              />
            </View>
          </View>
        )}
        ListFooterComponent={daemon.profiles.length ? (
          <View style={styles.footer}>
            <PaduIcon
              name="lock"
              size={14}
              tintColor={theme.textTertiary}
            />
            <Text style={[styles.footerText, { color: theme.textTertiary }]}>
              Tokens never pass through a Padu service. Native apps protect them
              with the device keychain.
            </Text>
          </View>
        ) : undefined}
        renderItem={renderDaemon}
        showsVerticalScrollIndicator={false}
      />
    </View>
  );
}

/**
 * One saved host. Memoized so connecting to one daemon (the `selecting`
 * spinner) doesn't re-render every other row. Reads the same card treatment
 * as the task list: hairline border, accent when selected.
 */
const DaemonRow = memo(function DaemonRow({
  active,
  onEdit,
  onSelect,
  phase,
  profile,
  selecting,
}: {
  active: boolean;
  onEdit: (id: string) => void;
  onSelect: (profile: DaemonProfile) => void;
  phase: ComponentProps<typeof ConnectionStatus>['phase'];
  profile: DaemonProfile;
  selecting: boolean;
}) {
  const theme = useTheme();
  // Only a host that needs attention spells it out; repeating
  // "Encrypted connection." on every row would be noise.
  const description = describeConnection(
    classifyConnection(profile.address, profile.kind),
  );
  const lastUsed =
    profile.lastConnectedAt === null
      ? null
      : formatLastUsed(profile.lastConnectedAt);
  const warn = description.tone !== 'secure';
  return (
    <Pressable
      accessibilityLabel={[
        profile.name,
        active ? 'selected' : 'saved daemon',
        lastUsed ? `last used ${lastUsed}` : null,
        warn ? description.text : null,
      ]
        .filter(Boolean)
        .join(', ')}
      accessibilityRole="button"
      accessibilityState={{ selected: active }}
      onPress={() => void onSelect(profile)}
      style={({ pressed }) => [
        styles.row,
        {
          backgroundColor: pressed
            ? theme.backgroundSelected
            : theme.surface,
          borderColor: active ? theme.accent : theme.border,
        },
      ]}>
      <DaemonAvatar name={profile.name} />
      <View style={styles.copy}>
        <View style={styles.nameLine}>
          <Text
            numberOfLines={1}
            style={[styles.name, { color: theme.text }]}>
            {profile.name}
          </Text>
          <TransportBadge kind={profile.kind} />
          {active && <ConnectionStatus phase={phase} />}
        </View>
        <View style={styles.metaLine}>
          <Text
            numberOfLines={1}
            style={[styles.host, { color: theme.textSecondary }]}>
            {displayHost(profile.address)}
          </Text>
          {lastUsed ? (
            <View style={styles.lastUsed}>
              <PaduIcon
                name="clock"
                size={11}
                tintColor={theme.textTertiary}
              />
              <Text
                style={[styles.lastUsedText, { color: theme.textTertiary }]}>
                {lastUsed}
              </Text>
            </View>
          ) : null}
        </View>
        {warn ? (
          <View style={styles.warningSlot}>
            <ConnectionNote
              tone={description.tone}
              text={description.text}
            />
          </View>
        ) : null}
      </View>
      <View style={styles.trailing}>
        {selecting ? (
          <ActivityIndicator color={theme.accent} />
        ) : active ? (
          <PaduIcon
            name="check"
            size={20}
            tintColor={theme.accent}
          />
        ) : null}
        <IconButton
          glyphSize={20}
          icon="ellipsis"
          label={`Edit ${profile.name}`}
          onPress={(event) => {
            event.stopPropagation();
            onEdit(profile.id);
          }}
          tintColor={theme.textTertiary}
        />
      </View>
    </Pressable>
  );
});

const styles = StyleSheet.create({
  screen: { flex: 1 },
  // A tablet is the same column with more margin, not a stretched list.
  list: {
    alignSelf: 'center',
    flex: 1,
    maxWidth: MaxContentWidth,
    width: '100%',
  },
  listContent: { paddingBottom: Spacing.five },
  listContentEmpty: { flexGrow: 1 },
  intro: {
    fontSize: 13,
    lineHeight: 18,
    marginBottom: Spacing.three,
    marginHorizontal: Spacing.three,
    marginTop: Spacing.three,
  },
  row: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: Spacing.two,
    marginBottom: Spacing.two,
    marginHorizontal: Spacing.three,
    minHeight: 76,
    padding: Spacing.three,
  },
  copy: { flex: 1, minWidth: 0 },
  nameLine: { alignItems: 'center', flexDirection: 'row', gap: Spacing.two },
  name: { flexShrink: 1, fontSize: 16, fontWeight: '700' },
  metaLine: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: Spacing.two,
    marginTop: Spacing.one,
  },
  host: { flex: 1, fontSize: 12.5 },
  lastUsed: { alignItems: 'center', flexDirection: 'row', gap: Spacing.one },
  lastUsedText: { fontSize: 12 },
  warningSlot: { marginTop: Spacing.two },
  trailing: { alignItems: 'center', flexDirection: 'row', gap: Spacing.one },
  footer: {
    alignItems: 'flex-start',
    flexDirection: 'row',
    gap: Spacing.two,
    marginHorizontal: Spacing.three,
    marginTop: Spacing.three,
  },
  footerText: { flex: 1, fontSize: 12, lineHeight: 17 },
  empty: {
    alignItems: 'center',
    flex: 1,
    gap: Spacing.two,
    justifyContent: 'center',
    paddingHorizontal: Spacing.four,
  },
  emptyIcon: {
    alignItems: 'center',
    borderRadius: 20,
    height: 64,
    justifyContent: 'center',
    marginBottom: Spacing.two,
    width: 64,
  },
  emptyTitle: { fontSize: 17, fontWeight: '700', textAlign: 'center' },
  emptyBody: { fontSize: 14, lineHeight: 20, maxWidth: 320, textAlign: 'center' },
  emptyActions: {
    alignItems: 'stretch',
    gap: Spacing.two,
    marginTop: Spacing.three,
    maxWidth: 320,
    width: '100%',
  },
});
