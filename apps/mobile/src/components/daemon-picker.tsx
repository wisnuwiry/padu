import { router } from 'expo-router';
import { Pressable, StyleSheet, Text } from 'react-native';

import { PaduIcon } from '@/components/padu-icon';
import { ConnectionStatus } from '@/components/connection-status';
import { Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';

/**
 * The host switcher: which daemon every action on the task list talks to. A
 * plain header control rather than a filled chip, so the bar stays light.
 */
export function DaemonPicker() {
  const theme = useTheme();
  const daemon = useDaemon();
  return (
    <Pressable
      accessibilityHint="Opens the daemon switcher"
      accessibilityLabel={daemon.activeProfile
        ? `Connected daemon: ${daemon.activeProfile.name}`
        : 'Add a daemon'}
      accessibilityRole="button"
      hitSlop={6}
      onPress={() => router.push('/daemons')}
      style={({ pressed }) => [styles.picker, { opacity: pressed ? 0.62 : 1 }]}>
      {daemon.activeProfile ? (
        <ConnectionStatus compact phase={daemon.phase} />
      ) : (
        <PaduIcon
          name="plus"
          size={14}
          tintColor={theme.text}
        />
      )}
      <Text numberOfLines={1} style={[styles.name, { color: theme.text }]}>
        {daemon.activeProfile?.name ?? 'Add daemon'}
      </Text>
      <PaduIcon
        name="chevronDown"
        size={12}
        tintColor={theme.textTertiary}
      />
    </Pressable>
  );
}

const styles = StyleSheet.create({
  picker: {
    alignItems: 'center',
    alignSelf: 'flex-start',
    flexDirection: 'row',
    gap: Spacing.one,
    minHeight: 36,
    maxWidth: 200,
  },
  name: { flexShrink: 1, fontSize: 13, fontWeight: '600' },
});
