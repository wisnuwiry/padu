import * as Haptics from 'expo-haptics';
import { router, Stack } from 'expo-router';
import {
  HeaderAction,
  HeaderActionGroup,
  navigateBack,
  ScreenHeader,
  useScreenHeaderInset,
} from '@/components/screen-header';
import { useState } from 'react';
import {
  ActivityIndicator,
  FlatList,
  Pressable,
  StyleSheet,
  Text,
  View,
  type GestureResponderEvent,
} from 'react-native';

import { AppSymbol } from '@/components/app-symbol';
import { ConnectionErrorCard } from '@/components/connection-error-card';
import { ConnectionNote } from '@/components/connection-note';
import { ConnectionStatus } from '@/components/connection-status';
import { DaemonAvatar } from '@/components/daemon-avatar';
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

export default function DaemonsScreen() {
  const theme = useTheme();
  const daemon = useDaemon();
  const headerInset = useScreenHeaderInset();
  const [selectingId, setSelectingId] = useState<string | null>(null);

  async function select(profile: DaemonProfile) {
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
  }

  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ headerShown: false }} />
      <ScreenHeader
        title="Daemons"
        right={
          <HeaderActionGroup>
            <HeaderAction
              icon={{
                ios: 'qrcode.viewfinder',
                android: 'qr_code_scanner',
                web: 'qr_code_scanner',
              }}
              label="Import from link"
              onPress={() => router.push('/daemon-import')}
            />
            <HeaderAction
              icon={{ ios: 'plus', android: 'add', web: 'add' }}
              label="Add daemon"
              onPress={() => router.push('/daemon-editor')}
            />
          </HeaderActionGroup>
        }
      />
      <FlatList
        data={daemon.profiles}
        keyExtractor={(item) => item.id}
        contentInsetAdjustmentBehavior="automatic"
        contentContainerStyle={[styles.listContent, { paddingTop: headerInset }]}
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
            <AppSymbol
              name={{ ios: 'server.rack', android: 'dns', web: 'dns' }}
              size={26}
              tintColor={theme.textTertiary}
            />
            <Text style={[styles.emptyTitle, { color: theme.text }]}>
              No saved daemons
            </Text>
            <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
              Scan the QR code in Padu Desktop’s host dialog with your camera, or
              add the address and token by hand.
            </Text>
            <View style={styles.emptyActions}>
              <Pressable
                accessibilityLabel="Import from link"
                accessibilityRole="button"
                onPress={() => router.push('/daemon-import')}
                style={({ pressed }) => [
                  styles.emptyPrimary,
                  { backgroundColor: theme.inverse, opacity: pressed ? 0.8 : 1 },
                ]}>
                <Text
                  style={[styles.emptyPrimaryLabel, { color: theme.onInverse }]}>
                  Import from Link
                </Text>
              </Pressable>
              <Pressable
                accessibilityLabel="Add daemon by hand"
                accessibilityRole="button"
                onPress={() => router.push('/daemon-editor')}
                style={({ pressed }) => [
                  styles.emptySecondary,
                  { borderColor: theme.borderStrong, opacity: pressed ? 0.7 : 1 },
                ]}>
                <Text
                  style={[styles.emptySecondaryLabel, { color: theme.text }]}>
                  Add by Hand
                </Text>
              </Pressable>
            </View>
          </View>
        )}
        ListFooterComponent={daemon.profiles.length ? (
          <View style={styles.footer}>
            <AppSymbol
              name={{ ios: 'key.horizontal', android: 'key', web: 'key' }}
              size={14}
              tintColor={theme.textTertiary}
            />
            <Text style={[styles.footerText, { color: theme.textTertiary }]}>
              Tokens never pass through a Padu service. Native apps protect them
              with the device keychain.
            </Text>
          </View>
        ) : undefined}
        renderItem={({ item }) => {
          const active = item.id === daemon.activeProfile?.id;
          // Only a host that needs attention spells it out; repeating
          // "Encrypted connection." on every row would be noise.
          const description = describeConnection(
            classifyConnection(item.address, item.kind),
          );
          const lastUsed =
            item.lastConnectedAt === null
              ? null
              : formatLastUsed(item.lastConnectedAt);
          const warn = description.tone !== 'secure';
          return (
            <Pressable
              accessibilityLabel={[
                item.name,
                active ? 'selected' : 'saved daemon',
                lastUsed ? `last used ${lastUsed}` : null,
                warn ? description.text : null,
              ]
                .filter(Boolean)
                .join(', ')}
              accessibilityRole="button"
              onPress={() => void select(item)}
              style={({ pressed }) => [
                styles.row,
                {
                  backgroundColor: pressed
                    ? theme.backgroundSelected
                    : theme.surface,
                  borderColor: active ? theme.accent : 'transparent',
                },
              ]}>
              <DaemonAvatar name={item.name} />
              <View style={styles.copy}>
                <View style={styles.nameLine}>
                  <Text
                    numberOfLines={1}
                    style={[styles.name, { color: theme.text }]}>
                    {item.name}
                  </Text>
                  <TransportBadge kind={item.kind} />
                  {active && <ConnectionStatus phase={daemon.phase} />}
                </View>
                <View style={styles.metaLine}>
                  <Text
                    numberOfLines={1}
                    style={[styles.host, { color: theme.textSecondary }]}>
                    {displayHost(item.address)}
                  </Text>
                  {lastUsed ? (
                    <View style={styles.lastUsed}>
                      <AppSymbol
                        name={{ ios: 'clock', android: 'schedule', web: 'schedule' }}
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
              {selectingId === item.id ? (
                <ActivityIndicator color={theme.accent} />
              ) : active ? (
                <AppSymbol
                  name={{
                    ios: 'checkmark.circle.fill',
                    android: 'check_circle',
                    web: 'check_circle',
                  }}
                  size={22}
                  tintColor={theme.accent}
                />
              ) : null}
              <Pressable
                accessibilityLabel={`Edit ${item.name}`}
                accessibilityRole="button"
                hitSlop={10}
                onPress={(event: GestureResponderEvent) => {
                  event.stopPropagation();
                  router.push({
                    pathname: '/daemon-editor',
                    params: { id: item.id },
                  });
                }}
                style={({ pressed }) => [
                  styles.editButton,
                  { opacity: pressed ? 0.45 : 1 },
                ]}>
                <AppSymbol
                  name={{
                    ios: 'ellipsis.circle',
                    android: 'more_horiz',
                    web: 'more_horiz',
                  }}
                  size={22}
                  tintColor={theme.textTertiary}
                />
              </Pressable>
            </Pressable>
          );
        }}
        showsVerticalScrollIndicator={false}
      />
    </View>
  );
}

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
  intro: {
    fontSize: 13,
    lineHeight: 18,
    marginBottom: Spacing.three,
    marginHorizontal: Spacing.three,
    marginTop: Spacing.two,
  },
  row: {
    alignItems: 'center',
    borderRadius: Radius.large,
    borderWidth: 1.5,
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
  editButton: {
    alignItems: 'center',
    height: 44,
    justifyContent: 'center',
    width: 40,
  },
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
    gap: Spacing.two,
    paddingHorizontal: Spacing.five,
    paddingTop: Spacing.six,
  },
  emptyTitle: { fontSize: 22, fontWeight: '700' },
  emptyBody: { fontSize: 14, lineHeight: 20, maxWidth: 320, textAlign: 'center' },
  emptyActions: { alignItems: 'center', gap: Spacing.two, marginTop: Spacing.three },
  emptyPrimary: {
    alignItems: 'center',
    borderRadius: Radius.large,
    justifyContent: 'center',
    minHeight: 50,
    paddingHorizontal: Spacing.five,
  },
  emptyPrimaryLabel: { fontSize: 15, fontWeight: '700' },
  emptySecondary: {
    alignItems: 'center',
    borderRadius: Radius.large,
    borderWidth: 1.5,
    justifyContent: 'center',
    minHeight: 50,
    paddingHorizontal: Spacing.five,
  },
  emptySecondaryLabel: { fontSize: 15, fontWeight: '600' },
});
