import * as Haptics from 'expo-haptics';
import { Stack } from 'expo-router';
import { navigateBack } from '@/components/screen-header';
import { useState } from 'react';
import {
  ActivityIndicator,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';

import { AppSymbol } from '@/components/app-symbol';
import { Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';

/**
 * Import a daemon from the connect link the desktop renders as a QR code.
 *
 * The link is a `padu://connect?...` URL, so scanning the desktop's QR with the
 * system camera opens this app directly and imports without this screen. This
 * screen is the manual path: paste the link text, or use it when the camera
 * handed the URL to a different device.
 */
export default function DaemonImportScreen() {
  const theme = useTheme();
  const daemon = useDaemon();
  const [link, setLink] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function importLink() {
    if (busy || !link.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const result = await daemon.importConnectLink(link);
      if (!result) {
        setError('That is not a Padu connect link.');
        await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
        return;
      }
      await Haptics.notificationAsync(
        result.connected
          ? Haptics.NotificationFeedbackType.Success
          : Haptics.NotificationFeedbackType.Warning,
      );
      navigateBack();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
    } finally {
      setBusy(false);
    }
  }

  const canImport = Boolean(link.trim()) && !busy;

  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ title: 'Import Link' }} />
      <ScrollView
        contentInsetAdjustmentBehavior="automatic"
        contentContainerStyle={styles.content}
        keyboardShouldPersistTaps="handled">
        <Text style={[styles.intro, { color: theme.textSecondary }]}>
          Point your camera at the QR code in Padu Desktop’s host dialog, or
          paste the link it shows. The link carries the address and token, so
          you never type them on the phone.
        </Text>

        <Text style={[styles.label, { color: theme.textSecondary }]}>Link</Text>
        <View
            style={[
            styles.inputShell,
            {
              backgroundColor: theme.surface,
              borderColor: error ? theme.danger : theme.border,
            },
          ]}>
          <TextInput
            accessibilityLabel="Padu connect link"
            autoCapitalize="none"
            autoCorrect={false}
            multiline
            onChangeText={(value) => {
              setLink(value);
              if (error) setError(null);
            }}
            placeholder="padu://connect?v=1&…"
            placeholderTextColor={theme.textTertiary}
            style={[styles.input, { color: theme.text }]}
            value={link}
          />
        </View>

        {error ? (
          <View style={styles.errorRow}>
            <AppSymbol
              name={{ ios: 'exclamationmark.triangle', android: 'warning', web: 'warning' }}
              size={14}
              tintColor={theme.danger}
            />
            <Text style={[styles.errorText, { color: theme.danger }]}>{error}</Text>
          </View>
        ) : null}

        <Pressable
          accessibilityLabel="Import daemon"
          accessibilityRole="button"
          accessibilityState={{ disabled: !canImport }}
          disabled={!canImport}
          onPress={() => void importLink()}
          style={({ pressed }) => [
            styles.primary,
            {
              backgroundColor: theme.inverse,
              opacity: !canImport ? 0.5 : pressed ? 0.8 : 1,
            },
          ]}>
          {busy ? (
            <ActivityIndicator color={theme.onInverse} />
          ) : (
            <Text style={[styles.primaryLabel, { color: theme.onInverse }]}>
              Import and Connect
            </Text>
          )}
        </Pressable>
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  content: { padding: Spacing.three, paddingBottom: 40 },
  intro: { fontSize: 13.5, lineHeight: 19, marginBottom: 20 },
  label: { fontSize: 12.5, fontWeight: '600', marginBottom: 6 },
  inputShell: {
    borderRadius: Radius.large,
    borderWidth: 1.5,
    minHeight: 96,
    paddingHorizontal: 12,
    paddingVertical: 10,
  },
  input: { fontSize: 14, lineHeight: 19 },
  errorRow: { flexDirection: 'row', gap: 6, marginTop: 10 },
  errorText: { flex: 1, fontSize: 12.5, lineHeight: 17 },
  primary: {
    alignItems: 'center',
    borderRadius: Radius.large,
    justifyContent: 'center',
    marginTop: 24,
    minHeight: 48,
  },
  primaryLabel: { fontSize: 15, fontWeight: '700' },
});
