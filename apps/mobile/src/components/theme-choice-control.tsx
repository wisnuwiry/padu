import { Pressable, StyleSheet, Text, View } from 'react-native';

import { Radius, Spacing } from '@/constants/theme';
import { useAppearance } from '@/lib/appearance-context';
import {
  THEME_CHOICES,
  THEME_LABELS,
  type ThemeChoice,
} from '@/lib/appearance';
import { useTheme } from '@/hooks/use-theme';

/**
 * System / Light / Dark picker, mirroring the web client's three-way control:
 * the same three values and the same labels. Used where there is room to show
 * every choice at once, rather than the compact cycling button.
 */
export function ThemeChoiceControl() {
  const theme = useTheme();
  const { preference, setPreference } = useAppearance();

  return (
    <View style={styles.field}>
      <Text style={[styles.label, { color: theme.textTertiary }]}>Theme</Text>
      <View
        style={[
          styles.group,
          { backgroundColor: theme.surface, borderColor: theme.border },
        ]}>
        {THEME_CHOICES.map((choice: ThemeChoice) => {
          const selected = choice === preference;
          return (
            <Pressable
              key={choice}
              accessibilityLabel={THEME_LABELS[choice]}
              accessibilityRole="button"
              accessibilityState={{ selected }}
              onPress={() => setPreference(choice)}
              style={({ pressed }) => [
                styles.segment,
                selected && {
                  backgroundColor: theme.accentSoft,
                  borderColor: theme.accent,
                },
                { opacity: pressed && !selected ? 0.6 : 1 },
              ]}>
              <Text
                style={[
                  styles.segmentLabel,
                  { color: selected ? theme.text : theme.textSecondary },
                ]}>
                {THEME_LABELS[choice]}
              </Text>
            </Pressable>
          );
        })}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  field: { gap: Spacing.two },
  label: { fontSize: 12.5, fontWeight: '600' },
  group: {
    borderRadius: Radius.medium,
    borderWidth: 1,
    flexDirection: 'row',
    gap: Spacing.half,
    padding: Spacing.half,
  },
  segment: {
    alignItems: 'center',
    borderRadius: Radius.small,
    borderWidth: 1,
    borderColor: 'transparent',
    flex: 1,
    justifyContent: 'center',
    minHeight: 44,
  },
  segmentLabel: { fontSize: 14, fontWeight: '600' },
});
