import { StyleSheet, Text, View } from 'react-native';

import { AppSymbol } from '@/components/app-symbol';
import { Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import type { ConnectionTone } from '@/lib/daemon-profile';

/** `neutral` is for guidance rather than a connection state. */
type NoteTone = ConnectionTone | 'neutral';

const TONE_ICONS: Record<NoteTone, Parameters<typeof AppSymbol>[0]['name']> = {
  secure: { ios: 'lock.fill', android: 'lock', web: 'lock' },
  warning: { ios: 'wifi', android: 'wifi', web: 'wifi' },
  danger: {
    ios: 'exclamationmark.shield.fill',
    android: 'gpp_bad',
    web: 'warning',
  },
  neutral: { ios: 'info.circle', android: 'info', web: 'info' },
};

/**
 * One line stating how a daemon is reached: a tone-coloured icon and its
 * sentence. Shared by the daemon editor and the connect-link preview so the
 * same host is described identically in both.
 */
export function ConnectionNote({
  tone,
  text,
}: {
  tone: NoteTone;
  text: string;
}) {
  const theme = useTheme();
  const color =
    tone === 'danger'
      ? theme.danger
      : tone === 'warning'
        ? theme.warning
        : tone === 'secure'
          ? theme.success
          : theme.textSecondary;

  return (
    <View style={styles.row}>
      <AppSymbol name={TONE_ICONS[tone]} size={14} tintColor={color} />
      <Text style={[styles.text, { color }]}>{text}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  row: { alignItems: 'flex-start', flexDirection: 'row', gap: Spacing.two },
  text: { flex: 1, fontSize: 12.5, lineHeight: 17 },
});
