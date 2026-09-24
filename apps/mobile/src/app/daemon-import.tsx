import * as Clipboard from 'expo-clipboard';
import * as Haptics from 'expo-haptics';
import { router, Stack } from 'expo-router';
import { useMemo, useState } from 'react';
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
import { ConnectionNote } from '@/components/connection-note';
import { navigateBack, ScreenHeader } from '@/components/screen-header';
import { TransportBadge } from '@/components/transport-badge';
import { MaxContentWidth, Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import {
  connectLinkFallbackName,
  parseConnectUrl,
} from '@/lib/daemon-connect-link';
import { useDaemon } from '@/lib/daemon-context';
import {
  classifyConnection,
  describeConnection,
  displayHost,
} from '@/lib/daemon-profile';

/**
 * Import a daemon from the connect link the desktop renders as a QR code.
 *
 * The link is a `padu://connect?...` URL, so scanning the desktop's QR with the
 * system camera opens this app directly and imports without this screen. This
 * screen is the manual path: paste the link text, or use it when the camera
 * handed the URL to a different device. It previews what the link carries
 * before anything is saved.
 */
export default function DaemonImportScreen() {
  const theme = useTheme();
  const daemon = useDaemon();
  const [link, setLink] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const parsedLink = useMemo(() => parseConnectUrl(link), [link]);
  const connectionClass = parsedLink
    ? classifyConnection(parsedLink.address, parsedLink.kind)
    : null;
  const description =
    connectionClass && connectionClass !== 'invalid'
      ? describeConnection(connectionClass)
      : null;
  const ready = Boolean(parsedLink) && description !== null;
  const canImport = ready && !busy;

  async function pasteLink() {
    const text = (await Clipboard.getStringAsync()).trim();
    if (!text) {
      setError('There is nothing on the clipboard to paste.');
      return;
    }
    setLink(text);
    setError(null);
  }

  async function importLink() {
    if (busy || !canImport) return;
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

  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ headerShown: false }} />
      <ScreenHeader title="Import Link" />
      <ScrollView
        automaticallyAdjustKeyboardInsets
        contentContainerStyle={styles.content}
        contentInsetAdjustmentBehavior="automatic"
        keyboardDismissMode="interactive"
        keyboardShouldPersistTaps="handled"
        showsVerticalScrollIndicator={false}>
        <View style={styles.column}>
          <Text style={[styles.intro, { color: theme.textSecondary }]}>
            Copy the connect link or QR code from Padu Desktop → Settings →
            Daemon. The token stays in this device’s keychain.
          </Text>

          <Pressable
            accessibilityHint="Opens the camera to scan the desktop’s QR code"
            accessibilityLabel="Scan QR code"
            accessibilityRole="button"
            onPress={() => router.push('/scan')}
            style={({ pressed }) => [
              styles.scan,
              {
                backgroundColor: theme.surface,
                borderColor: theme.borderStrong,
                opacity: pressed ? 0.7 : 1,
              },
            ]}>
            <AppSymbol
              name={{
                ios: 'qrcode.viewfinder',
                android: 'qr_code_scanner',
                web: 'qr_code_scanner',
              }}
              size={17}
              tintColor={theme.text}
            />
            <Text style={[styles.scanLabel, { color: theme.text }]}>
              Scan QR Code
            </Text>
            <AppSymbol
              name={{
                ios: 'chevron.forward',
                android: 'chevron_right',
                web: 'chevron_right',
              }}
              size={13}
              tintColor={theme.textTertiary}
            />
          </Pressable>

          <View style={styles.divider}>
            <View
              style={[styles.dividerLine, { backgroundColor: theme.separator }]}
            />
            <Text style={[styles.dividerLabel, { color: theme.textTertiary }]}>
              or paste the link
            </Text>
            <View
              style={[styles.dividerLine, { backgroundColor: theme.separator }]}
            />
          </View>

          <View style={styles.field}>
            <View style={styles.labelRow}>
              <Text style={[styles.label, { color: theme.textSecondary }]}>
                Link
              </Text>
              <View style={styles.labelActions}>
                {link ? (
                  <Pressable
                    accessibilityLabel="Clear link"
                    accessibilityRole="button"
                    hitSlop={10}
                    onPress={() => {
                      setLink('');
                      setError(null);
                    }}
                    style={({ pressed }) => [
                      styles.textAction,
                      { opacity: pressed ? 0.5 : 1 },
                    ]}>
                    <Text
                      style={[
                        styles.textActionLabel,
                        { color: theme.textSecondary },
                      ]}>
                      Clear
                    </Text>
                  </Pressable>
                ) : null}
                <Pressable
                  accessibilityHint="Reads the Padu connect link from the clipboard"
                  accessibilityLabel="Paste link"
                  accessibilityRole="button"
                  hitSlop={10}
                  onPress={() => void pasteLink()}
                  style={({ pressed }) => [
                    styles.textAction,
                    { opacity: pressed ? 0.5 : 1 },
                  ]}>
                  <Text
                    style={[styles.textActionLabel, { color: theme.accent }]}>
                    Paste
                  </Text>
                </Pressable>
              </View>
            </View>

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
                textAlignVertical="top"
                value={link}
              />
            </View>
          </View>

          {error ? (
            <View accessibilityLiveRegion="polite" style={styles.noteRow}>
              <AppSymbol
                name={{
                  ios: 'exclamationmark.triangle',
                  android: 'warning',
                  web: 'warning',
                }}
                size={14}
                tintColor={theme.danger}
              />
              <Text style={[styles.noteText, { color: theme.danger }]}>
                {error}
              </Text>
            </View>
          ) : link.trim() && !ready ? (
            <View accessibilityLiveRegion="polite">
              <ConnectionNote
                tone="neutral"
                text="This doesn’t look like a Padu connect link yet."
              />
            </View>
          ) : null}

          {ready && parsedLink && description ? (
            <View
              accessibilityLiveRegion="polite"
              style={[
                styles.preview,
                { backgroundColor: theme.surface, borderColor: theme.border },
              ]}>
              <View style={styles.previewHeader}>
                <Text
                  numberOfLines={1}
                  style={[styles.previewName, { color: theme.text }]}>
                  {connectLinkFallbackName(parsedLink)}
                </Text>
                <TransportBadge kind={parsedLink.kind} />
              </View>
              <Text
                numberOfLines={1}
                style={[styles.previewHost, { color: theme.textSecondary }]}>
                {displayHost(parsedLink.address)}
              </Text>
              <ConnectionNote
                tone={description.tone}
                text={description.text}
              />
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
        </View>
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  content: {
    paddingBottom: Spacing.six,
    paddingHorizontal: Spacing.three,
    paddingTop: Spacing.three,
  },
  column: {
    alignSelf: 'center',
    gap: Spacing.three,
    maxWidth: MaxContentWidth,
    width: '100%',
  },
  intro: { fontSize: 13, lineHeight: 18 },
  scan: {
    alignItems: 'center',
    borderRadius: Radius.large,
    borderWidth: 1.5,
    flexDirection: 'row',
    gap: Spacing.two,
    minHeight: 50,
    paddingHorizontal: Spacing.three,
  },
  scanLabel: { flex: 1, fontSize: 15, fontWeight: '600' },
  divider: { alignItems: 'center', flexDirection: 'row', gap: Spacing.two },
  dividerLine: { flex: 1, height: StyleSheet.hairlineWidth },
  dividerLabel: { fontSize: 12.5, fontWeight: '600' },
  field: { gap: Spacing.two },
  labelRow: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
  },
  label: { fontSize: 12.5, fontWeight: '600' },
  labelActions: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: Spacing.three,
  },
  textAction: { justifyContent: 'center', minHeight: 32 },
  textActionLabel: { fontSize: 13, fontWeight: '600' },
  inputShell: {
    borderRadius: Radius.large,
    borderWidth: 1.5,
    minHeight: 92,
    paddingHorizontal: Spacing.three,
    paddingVertical: Spacing.two,
  },
  input: { fontSize: 14, lineHeight: 19, minHeight: 72 },
  noteRow: {
    alignItems: 'flex-start',
    flexDirection: 'row',
    gap: Spacing.two,
  },
  noteText: { flex: 1, fontSize: 12.5, lineHeight: 17 },
  preview: {
    borderRadius: Radius.large,
    borderWidth: StyleSheet.hairlineWidth,
    gap: Spacing.two,
    padding: Spacing.three,
  },
  previewHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: Spacing.two,
  },
  previewName: { flex: 1, fontSize: 15.5, fontWeight: '600' },
  previewHost: { fontSize: 13 },
  primary: {
    alignItems: 'center',
    borderRadius: Radius.large,
    justifyContent: 'center',
    minHeight: 50,
  },
  primaryLabel: { fontSize: 15, fontWeight: '700' },
});
