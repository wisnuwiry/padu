import { Pressable, StyleSheet, Text, View } from 'react-native';

import { PaduIcon } from '@/components/padu-icon';
import { NativeTint, Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';

export function ConnectionErrorCard() {
  const theme = useTheme();
  const { activeProfile, error, phase, reconnect } = useDaemon();
  const retrying = phase === 'connecting';

  return (
    <View
      accessibilityLiveRegion="polite"
      style={[styles.card, { backgroundColor: theme.dangerSoft }]}>
      <View style={styles.icon}>
        <PaduIcon
          name="alert"
          size={18}
          tintColor={theme.danger}
        />
      </View>
      <View style={styles.copy}>
        <Text style={[styles.title, { color: theme.text }]}>Can’t reach {activeProfile?.name}</Text>
        <Text style={[styles.body, { color: theme.textSecondary }]}>{error}</Text>
      </View>
      <Pressable
        accessibilityLabel="Retry daemon connection"
        accessibilityRole="button"
        disabled={retrying}
        hitSlop={8}
        onPress={() => void reconnect()}
        style={({ pressed }) => [
          styles.retryButton,
          { opacity: pressed || retrying ? 0.5 : 1 },
        ]}>
        <Text style={[styles.retryText, { color: NativeTint }]}>Retry</Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    alignItems: 'flex-start',
    borderRadius: Radius.large,
    flexDirection: 'row',
    gap: 10,
    marginBottom: 10,
    marginHorizontal: Spacing.three,
    padding: 14,
  },
  icon: { paddingTop: 1 },
  copy: { flex: 1 },
  title: { fontSize: 14, fontWeight: '700' },
  body: { fontSize: 12, lineHeight: 17, marginTop: 3 },
  retryButton: { justifyContent: 'center', minHeight: 32, paddingHorizontal: 4 },
  retryText: { fontSize: 13, fontWeight: '700' },
});
