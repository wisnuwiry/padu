import { CameraView, useCameraPermissions, type BarcodeScanningResult } from 'expo-camera';
import * as Haptics from 'expo-haptics';
import { Stack } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import {
  ActivityIndicator,
  AppState,
  Linking,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  View,
  useWindowDimensions,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { AppSymbol } from '@/components/app-symbol';
import { navigateBack } from '@/components/screen-header';
import { Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';

/**
 * Camera scanner for the `padu://connect?...` QR code Padu Desktop renders.
 * A successful scan imports the daemon in place; the paste screen stays one
 * tap away for devices without a usable camera (simulators, desktop web).
 */
export default function ScanScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { width } = useWindowDimensions();
  const daemon = useDaemon();
  const [permission, requestPermission, getPermission] = useCameraPermissions();
  const [scanning, setScanning] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [mountFailed, setMountFailed] = useState(false);
  const unlockTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (unlockTimer.current) clearTimeout(unlockTimer.current);
  }, []);

  // A grant made in Settings must take effect when the user comes back.
  useEffect(() => {
    const subscription = AppState.addEventListener('change', (state) => {
      if (state === 'active') void getPermission();
    });
    return () => subscription.remove();
  }, [getPermission]);

  function releaseSoon() {
    unlockTimer.current = setTimeout(() => setScanning(true), 1600);
  }

  async function handleScan({ data }: BarcodeScanningResult) {
    if (!scanning) return;
    setScanning(false);
    setMessage(null);
    setBusy(true);
    try {
      const result = await daemon.importConnectLink(data);
      if (!result) {
        setMessage('That code isn’t a Padu connect QR.');
        void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
        releaseSoon();
        return;
      }
      await Haptics.notificationAsync(
        result.connected
          ? Haptics.NotificationFeedbackType.Success
          : Haptics.NotificationFeedbackType.Warning,
      );
      navigateBack();
    } catch (cause) {
      setMessage(cause instanceof Error ? cause.message : String(cause));
      void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
      releaseSoon();
    } finally {
      setBusy(false);
    }
  }

  const frame = Math.min(260, width - 96);
  const live = (permission?.granted ?? false) && !mountFailed;
  // Denied for good: the OS will not show the prompt again, so the only way
  // back is the system settings app.
  const blocked = permission !== null && !permission.granted && !permission.canAskAgain;

  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ headerShown: false }} />
      {live && (
        <CameraView
          barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
          facing="back"
          onBarcodeScanned={scanning ? handleScan : undefined}
          onMountError={({ message: cause }) => {
            setMountFailed(true);
            setMessage(cause);
          }}
          style={StyleSheet.absoluteFill}
        />
      )}

      <View
        pointerEvents="box-none"
        style={[
          styles.overlay,
          {
            paddingBottom: insets.bottom + Spacing.four,
            paddingTop: insets.top + Spacing.two,
          },
        ]}>
        <View
          style={[styles.chrome, { backgroundColor: theme.surface, borderColor: theme.border }]}>
          <AppSymbol
            name={{ ios: 'qrcode.viewfinder', android: 'qr_code_scanner', web: 'qr_code_scanner' }}
            size={17}
            tintColor={theme.textSecondary}
          />
          <Text numberOfLines={1} style={[styles.chromeTitle, { color: theme.text }]}>
            Scan QR code
          </Text>
          <Pressable
            accessibilityLabel="Close scanner"
            accessibilityRole="button"
            hitSlop={6}
            onPress={navigateBack}
            style={({ pressed }) => [styles.closeButton, { opacity: pressed ? 0.5 : 1 }]}>
            <AppSymbol
              name={{ ios: 'xmark', android: 'close', web: 'close' }}
              size={15}
              tintColor={theme.textSecondary}
            />
          </Pressable>
        </View>

        <View style={styles.body}>
          {permission === null ? (
            <ActivityIndicator color={theme.textSecondary} />
          ) : live ? (
            <>
              <View style={[styles.frame, { borderColor: theme.accent, height: frame, width: frame }]} />
              <View
                accessibilityLiveRegion="polite"
                style={[styles.hint, { backgroundColor: theme.surface, borderColor: theme.border }]}>
                <Text style={[styles.hintText, { color: message ? theme.danger : theme.text }]}>
                  {message
                    ?? (busy
                      ? 'Adding daemon…'
                      : 'Point at the QR code in Padu Desktop’s host dialog.')}
                </Text>
              </View>
            </>
          ) : (
            <View
              style={[styles.prompt, { backgroundColor: theme.surface, borderColor: theme.border }]}>
              <AppSymbol
                name={{ ios: 'camera.badge.ellipsis', android: 'no_photography', web: 'no_photography' }}
                size={22}
                tintColor={theme.textTertiary}
              />
              <Text style={[styles.promptTitle, { color: theme.text }]}>
                {mountFailed ? 'Camera unavailable' : 'Camera access is off'}
              </Text>
              <Text style={[styles.promptBody, { color: theme.textSecondary }]}>
                {message
                  ?? (blocked
                    ? 'Turn on camera access for Padu in Settings, or enter the link by hand.'
                    : 'Padu needs the camera to scan the QR code. You can enter the link by hand instead.')}
              </Text>
              {permission?.canAskAgain ? (
                <Pressable
                  accessibilityLabel="Allow camera access"
                  accessibilityRole="button"
                  onPress={() => void requestPermission()}
                  style={({ pressed }) => [
                    styles.promptAction,
                    { backgroundColor: theme.inverse, opacity: pressed ? 0.78 : 1 },
                  ]}>
                  <Text style={[styles.promptActionText, { color: theme.onInverse }]}>
                    Allow camera access
                  </Text>
                </Pressable>
              ) : blocked && Platform.OS !== 'web' ? (
                <Pressable
                  accessibilityLabel="Open settings"
                  accessibilityRole="button"
                  onPress={() => void Linking.openSettings()}
                  style={({ pressed }) => [
                    styles.promptAction,
                    { backgroundColor: theme.inverse, opacity: pressed ? 0.78 : 1 },
                  ]}>
                  <Text style={[styles.promptActionText, { color: theme.onInverse }]}>
                    Open Settings
                  </Text>
                </Pressable>
              ) : null}
            </View>
          )}
        </View>

        <Pressable
          accessibilityLabel="Enter link manually"
          accessibilityRole="button"
          onPress={navigateBack}
          style={({ pressed }) => [
            styles.manualButton,
            { backgroundColor: theme.surface, borderColor: theme.borderStrong, opacity: pressed ? 0.7 : 1 },
          ]}>
          <AppSymbol
            name={{ ios: 'keyboard', android: 'keyboard', web: 'keyboard' }}
            size={16}
            tintColor={theme.textSecondary}
          />
          <Text style={[styles.manualButtonText, { color: theme.text }]}>Enter link manually</Text>
        </Pressable>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  overlay: {
    flex: 1,
    justifyContent: 'space-between',
    paddingHorizontal: Spacing.three,
  },
  chrome: {
    alignItems: 'center',
    alignSelf: 'center',
    borderRadius: Radius.pill,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: Spacing.two,
    maxWidth: 420,
    minHeight: 46,
    paddingLeft: Spacing.three,
    paddingRight: Spacing.one,
    width: '100%',
  },
  chromeTitle: { flex: 1, fontSize: 15, fontWeight: '600' },
  closeButton: {
    alignItems: 'center',
    height: 44,
    justifyContent: 'center',
    width: 44,
  },
  body: { alignItems: 'center', gap: Spacing.four, justifyContent: 'center' },
  frame: {
    borderRadius: Radius.large,
    borderWidth: 3,
  },
  hint: {
    borderRadius: Radius.large,
    borderWidth: StyleSheet.hairlineWidth,
    maxWidth: 340,
    paddingHorizontal: Spacing.three,
    paddingVertical: Spacing.two,
  },
  hintText: { fontSize: 13.5, lineHeight: 18, textAlign: 'center' },
  prompt: {
    alignItems: 'center',
    borderRadius: Radius.large,
    borderWidth: StyleSheet.hairlineWidth,
    gap: Spacing.two,
    maxWidth: 360,
    padding: Spacing.four,
  },
  promptTitle: { fontSize: 16, fontWeight: '700' },
  promptBody: { fontSize: 13.5, lineHeight: 18, textAlign: 'center' },
  promptAction: {
    alignItems: 'center',
    borderRadius: Radius.large,
    justifyContent: 'center',
    marginTop: Spacing.two,
    minHeight: 46,
    paddingHorizontal: Spacing.four,
  },
  promptActionText: { fontSize: 15, fontWeight: '700' },
  manualButton: {
    alignItems: 'center',
    alignSelf: 'center',
    borderRadius: Radius.large,
    borderWidth: 1.5,
    flexDirection: 'row',
    gap: Spacing.two,
    justifyContent: 'center',
    maxWidth: 420,
    minHeight: 50,
    width: '100%',
  },
  manualButtonText: { fontSize: 15, fontWeight: '600' },
});
