import * as Haptics from 'expo-haptics';
import { router, Stack } from 'expo-router';
import { navigateBack } from '@/components/screen-header';
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
import { ConnectionStatus } from '@/components/connection-status';
import { DaemonAvatar } from '@/components/daemon-avatar';
import { Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import {
  displayHost,
  type DaemonProfile,
  type DaemonTransport,
} from '@/lib/daemon-profile';

function transportLabel(kind: DaemonTransport): string {
  switch (kind) {
    case 'cloudflare':
      return 'Cloudflare';
    case 'tailscale':
      return 'Tailscale';
    case 'ssh_relay':
      return 'SSH';
    default:
      return '';
  }
}

export default function DaemonsScreen() {
  const theme = useTheme();
  const daemon = useDaemon();
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
      <Stack.Screen
        options={{
          headerRight: () => (
            <View style={styles.headerActions}>
              <Pressable
                accessibilityHint="Import a daemon from the desktop's QR code"
                accessibilityLabel="Import from link"
                accessibilityRole="button"
                hitSlop={10}
                onPress={() => router.push('/daemon-import')}>
                <AppSymbol
                  name={{
                    ios: 'qrcode.viewfinder',
                    android: 'qr_code_scanner',
                    web: 'qr_code_scanner',
                  }}
                  size={21}
                  tintColor={theme.accent}
                />
              </Pressable>
              <Pressable
                accessibilityLabel="Add daemon"
                accessibilityRole="button"
                hitSlop={10}
                onPress={() => router.push('/daemon-editor')}>
                <AppSymbol
                  name={{ ios: 'plus', android: 'add', web: 'add' }}
                  size={21}
                  tintColor={theme.accent}
                />
              </Pressable>
            </View>
          ),
        }}
      />
      <FlatList
        data={daemon.profiles}
        keyExtractor={(item) => item.id}
        contentInsetAdjustmentBehavior="automatic"
        contentContainerStyle={styles.listContent}
        ListHeaderComponent={(
          <>
            <Text style={[styles.intro, { color: theme.textSecondary }]}>
              Switch hosts without re-entering credentials. Only the selected daemon stays connected.
            </Text>
            {daemon.error ? <ConnectionErrorCard /> : null}
          </>
        )}
        ListEmptyComponent={(
          <View style={styles.empty}>
            <Text style={[styles.emptyTitle, { color: theme.text }]}>No saved daemons</Text>
            <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
              Scan the QR code in Padu Desktop’s host dialog with your camera,
              or add the address and token by hand.
            </Text>
            <Pressable
              accessibilityLabel="Import from link"
              accessibilityRole="button"
              onPress={() => router.push('/daemon-import')}
              style={({ pressed }) => [
                styles.emptyAction,
                { backgroundColor: theme.inverse, opacity: pressed ? 0.8 : 1 },
              ]}>
              <Text style={[styles.emptyActionLabel, { color: theme.onInverse }]}>
                Import from Link
              </Text>
            </Pressable>
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
              Tokens never pass through a Padu service. Native apps protect them with the device keychain.
            </Text>
          </View>
        ) : null}
        renderItem={({ item }) => {
          const active = item.id === daemon.activeProfile?.id;
          return (
            <Pressable
              accessibilityLabel={`${item.name}, ${active ? 'selected' : 'saved daemon'}`}
              accessibilityRole="button"
              onPress={() => void select(item)}
              style={({ pressed }) => [
                styles.row,
                {
                  backgroundColor: pressed ? theme.backgroundSelected : theme.surface,
                  borderColor: active ? theme.accent : 'transparent',
                },
              ]}>
              <DaemonAvatar name={item.name} />
              <View style={styles.copy}>
                <View style={styles.nameLine}>
                  <Text numberOfLines={1} style={[styles.name, { color: theme.text }]}>{item.name}</Text>
                  {item.kind !== 'direct' ? (
                    <View style={[styles.transportBadge, { backgroundColor: theme.backgroundElement }]}>
                      <Text style={[styles.transportBadgeLabel, { color: theme.textSecondary }]}>
                        {transportLabel(item.kind)}
                      </Text>
                    </View>
                  ) : null}
                  {active && <ConnectionStatus phase={daemon.phase} />}
                </View>
                <Text numberOfLines={1} style={[styles.host, { color: theme.textSecondary }]}>
                  {displayHost(item.address)}
                </Text>
              </View>
              {selectingId === item.id ? (
                <ActivityIndicator color={theme.accent} />
              ) : active ? (
                <AppSymbol
                  name={{ ios: 'checkmark.circle.fill', android: 'check_circle', web: 'check_circle' }}
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
                  router.push({ pathname: '/daemon-editor', params: { id: item.id } });
                }}
                style={({ pressed }) => [styles.editButton, { opacity: pressed ? 0.45 : 1 }]}>
                <AppSymbol
                  name={{ ios: 'ellipsis.circle', android: 'more_horiz', web: 'more_horiz' }}
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
  headerActions: { alignItems: 'center', flexDirection: 'row', gap: 18 },
  listContent: { paddingBottom: 36 },
  intro: { fontSize: 13.5, lineHeight: 19, margin: Spacing.three, marginBottom: 12 },
  row: {
    alignItems: 'center',
    borderRadius: Radius.large,
    borderWidth: 1.5,
    flexDirection: 'row',
    gap: 12,
    marginBottom: 10,
    marginHorizontal: Spacing.three,
    minHeight: 76,
    padding: 12,
  },
  copy: { flex: 1, minWidth: 0 },
  nameLine: { alignItems: 'center', flexDirection: 'row', gap: 9 },
  name: { flexShrink: 1, fontSize: 16, fontWeight: '700' },
  transportBadge: {
    borderRadius: 5,
    paddingHorizontal: 6,
    paddingVertical: 1.5,
  },
  transportBadgeLabel: { fontSize: 11, fontWeight: '600' },
  host: { fontSize: 12.5, marginTop: 5 },
  editButton: { alignItems: 'center', height: 42, justifyContent: 'center', width: 34 },
  footer: {
    alignItems: 'flex-start',
    flexDirection: 'row',
    gap: 8,
    marginHorizontal: 24,
    marginTop: 14,
  },
  footerText: { flex: 1, fontSize: 12, lineHeight: 17 },
  empty: { alignItems: 'center', paddingHorizontal: 40, paddingTop: 100 },
  emptyTitle: { fontSize: 18, fontWeight: '700' },
  emptyBody: { fontSize: 14, lineHeight: 20, marginTop: 8, maxWidth: 320, textAlign: 'center' },
  emptyAction: {
    borderRadius: Radius.large,
    marginTop: 20,
    paddingHorizontal: 20,
    paddingVertical: 12,
  },
  emptyActionLabel: { fontSize: 14, fontWeight: '700' },
});
