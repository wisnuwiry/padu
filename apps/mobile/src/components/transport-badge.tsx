import { StyleSheet, Text, View } from 'react-native';

import { Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { transportLabel, type DaemonTransport } from '@/lib/daemon-profile';

/**
 * Short badge naming a non-direct transport, shared by the host list and both
 * daemon preview surfaces so a transport is labelled identically everywhere.
 *
 * Renders nothing for `direct`: it is the default and needs no label.
 */
export function TransportBadge({ kind }: { kind: DaemonTransport }) {
  const theme = useTheme();
  const label = transportLabel(kind);
  if (!label) return null;

  return (
    <View style={[styles.badge, { backgroundColor: theme.backgroundElement }]}>
      <Text style={[styles.label, { color: theme.textSecondary }]}>
        {label}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  badge: {
    borderRadius: Radius.tiny,
    // Optical inset for an 11pt label; the scale has no 6px step.
    paddingHorizontal: 6,
    paddingVertical: Spacing.half,
  },
  label: { fontSize: 11, fontWeight: '600' },
});
