import { ActivityIndicator, StyleSheet, Text, View } from 'react-native';

import { AppSymbol } from '@/components/app-symbol';
import { Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';

/**
 * Everything the task list shows when it has no rows: connecting, no search
 * results, a load failure, or a genuinely empty daemon.
 */
export function TaskListEmpty({
  connecting,
  error,
  searching,
}: {
  connecting: boolean;
  error: unknown;
  searching: boolean;
}) {
  const theme = useTheme();
  const { error: daemonError } = useDaemon();
  // A connection error already has its own card above the list.
  if (daemonError) return null;
  if (connecting) {
    return (
      <View style={styles.state}>
        <ActivityIndicator color={theme.textTertiary} />
        <Text style={[styles.title, { color: theme.textSecondary }]}>
          Connecting to daemon…
        </Text>
      </View>
    );
  }
  if (searching) {
    return (
      <View style={styles.state}>
        <Text style={[styles.title, { color: theme.text }]}>No matching tasks</Text>
        <Text style={[styles.body, { color: theme.textSecondary }]}>
          Try another title, project, or agent.
        </Text>
      </View>
    );
  }
  if (error) {
    return (
      <View style={styles.state}>
        <Text style={[styles.title, { color: theme.text }]}>Couldn’t load tasks</Text>
        <Text style={[styles.body, { color: theme.textSecondary }]}>
          {error instanceof Error ? error.message : String(error)}
        </Text>
      </View>
    );
  }
  return (
    <View style={styles.state}>
      <View style={[styles.icon, { backgroundColor: theme.overlayStrong }]}>
        <AppSymbol
          name={{ ios: 'text.bubble', android: 'chat_bubble', web: 'chat' }}
          size={25}
          tintColor={theme.textTertiary}
        />
      </View>
      <Text style={[styles.title, { color: theme.text }]}>No tasks yet</Text>
      <Text style={[styles.body, { color: theme.textSecondary }]}>
        Start an agent on anything — a bug, a feature, a question about the code.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  state: {
    alignItems: 'center',
    flex: 1,
    justifyContent: 'center',
    minHeight: 360,
    paddingHorizontal: 40,
  },
  icon: {
    alignItems: 'center',
    borderRadius: 20,
    height: 64,
    justifyContent: 'center',
    marginBottom: 18,
    width: 64,
  },
  title: { fontSize: 17, fontWeight: '700', textAlign: 'center' },
  body: { fontSize: 14, lineHeight: 20, marginTop: 7, maxWidth: 320, textAlign: 'center' },
});
