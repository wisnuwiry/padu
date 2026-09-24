import { Pressable, StyleSheet } from 'react-native';

import { AppSymbol } from '@/components/app-symbol';
import { GlassSurface } from '@/components/glass-surface';
import { Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useAppearance } from '@/lib/appearance-context';
import { THEME_LABELS, type ThemeChoice } from '@/lib/appearance';

/** System → Light → Dark → System. */
const NEXT_CHOICE: Record<ThemeChoice, ThemeChoice> = {
  system: 'light',
  light: 'dark',
  dark: 'system',
};

const CHOICE_ICONS: Record<
  ThemeChoice,
  Parameters<typeof AppSymbol>[0]['name']
> = {
  system: {
    ios: 'circle.lefthalf.filled',
    android: 'brightness_auto',
    web: 'brightness_auto',
  },
  light: { ios: 'sun.max', android: 'light_mode', web: 'light_mode' },
  dark: { ios: 'moon', android: 'dark_mode', web: 'dark_mode' },
};

/**
 * Compact theme switch for the floating header chrome: one tap steps through
 * System, Light, and Dark. The icon names the current choice and the
 * accessibility label says it outright, so the state is never carried by
 * appearance alone.
 */
export function ThemeToggleButton() {
  const theme = useTheme();
  const { preference, setPreference } = useAppearance();

  return (
    <GlassSurface interactive style={styles.surface}>
      <Pressable
        accessibilityHint="Switches between system, light, and dark"
        accessibilityLabel={`Theme: ${THEME_LABELS[preference]}`}
        accessibilityRole="button"
        hitSlop={8}
        onPress={() => setPreference(NEXT_CHOICE[preference])}
        style={({ pressed }) => [
          styles.inner,
          { opacity: pressed ? 0.62 : 1 },
        ]}>
        <AppSymbol
          name={CHOICE_ICONS[preference]}
          size={16}
          tintColor={theme.text}
        />
      </Pressable>
    </GlassSurface>
  );
}

const styles = StyleSheet.create({
  surface: { borderRadius: Radius.pill, height: 38, width: 38 },
  inner: { alignItems: 'center', flex: 1, justifyContent: 'center' },
});
