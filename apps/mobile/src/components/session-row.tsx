import type { AgentSession } from '@padu/client';
import { router } from 'expo-router';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';

import { ProviderIcon, providerBrandColor } from '@/components/provider-icon';
import { Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { sessionBusy } from '@/lib/mobile-runtime';
import {
  displaySessionTitle,
  providerLabel,
  relativeSessionTime,
  type SessionListItem,
} from '@/lib/session-presentation';

/** One task in the list: title, project, agent, and its live status. */
export function SessionRow({ item, onLongPress }: { item: SessionListItem; onLongPress: () => void }) {
  const theme = useTheme();
  const session = item.session;
  const status = statusPresentation(session, theme);
  return (
    <Pressable
      accessibilityHint="Long press for actions"
      accessibilityLabel={`${displaySessionTitle(session)}, ${item.projectName}${status ? `, ${status.label}` : ''}`}
      accessibilityRole="button"
      delayLongPress={350}
      onLongPress={onLongPress}
      onPress={() => router.push({ pathname: '/session/[id]', params: { id: session.id } })}
      style={({ pressed }) => [
        styles.row,
        {
          backgroundColor: pressed ? theme.backgroundSelected : theme.surface,
          borderColor: theme.border,
        },
      ]}>
      <View style={styles.copy}>
        <Text numberOfLines={2} style={[styles.title, { color: theme.text }]}>
          {displaySessionTitle(session)}
        </Text>
        <View style={styles.metadata}>
          <Text numberOfLines={1} style={[styles.metaText, { color: theme.textSecondary }]}>
            {item.projectName}
          </Text>
          <Text style={[styles.bullet, { color: theme.textGhost }]}>·</Text>
          <View style={styles.providerBadge}>
            <ProviderIcon
              color={providerBrandColor(session.provider) ?? theme.textTertiary}
              provider={session.provider}
              size={13}
            />
            <Text style={[styles.metaText, { color: theme.textTertiary }]}>
              {providerLabel(session.provider)}
            </Text>
          </View>
        </View>
      </View>
      <View style={styles.trailing}>
        <Text style={[styles.time, { color: theme.textTertiary }]}>
          {relativeSessionTime(item.timestamp)}
        </Text>
        {status && (
          <View style={styles.statusLine}>
            {status.spinner
              ? <ActivityIndicator color={status.color} size="small" style={styles.spinner} />
              : <View style={[styles.dot, { backgroundColor: status.color }]} />}
            <Text style={[styles.statusLabel, { color: status.color }]}>{status.label}</Text>
          </View>
        )}
      </View>
    </Pressable>
  );
}

function statusPresentation(
  session: AgentSession,
  theme: ReturnType<typeof useTheme>,
): { label: string; color: string; spinner: boolean } | null {
  if (session.status === 'waiting') {
    return { label: 'Needs input', color: theme.warning, spinner: false };
  }
  if (sessionBusy(session)) {
    return { label: 'Working', color: theme.success, spinner: true };
  }
  if (session.status === 'failed') {
    return { label: 'Failed', color: theme.danger, spinner: false };
  }
  return null;
}

const styles = StyleSheet.create({
  row: {
    borderRadius: Radius.medium,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: 12,
    marginBottom: 8,
    marginHorizontal: Spacing.three,
    minHeight: 74,
    paddingHorizontal: 14,
    paddingVertical: 12,
  },
  copy: { flex: 1, justifyContent: 'center' },
  title: { fontSize: 15.5, fontWeight: '600', letterSpacing: -0.15, lineHeight: 20 },
  metadata: { alignItems: 'center', flexDirection: 'row', marginTop: 6, minWidth: 0 },
  metaText: { flexShrink: 1, fontSize: 12.5 },
  bullet: { fontSize: 12, marginHorizontal: 6 },
  providerBadge: { alignItems: 'center', flexDirection: 'row', gap: 5 },
  trailing: { alignItems: 'flex-end', gap: 6, justifyContent: 'center', minWidth: 64 },
  time: { fontSize: 11.5 },
  statusLine: { alignItems: 'center', flexDirection: 'row', gap: 5 },
  dot: { borderRadius: Radius.pill, height: 7, width: 7 },
  spinner: { height: 12, transform: [{ scale: 0.6 }], width: 12 },
  statusLabel: { fontSize: 11.5, fontWeight: '600' },
});
