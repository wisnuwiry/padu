import { IconButton } from '@/components/button';
import type { PaduIconName } from '@/components/padu-icon';
import { useAppearance } from '@/lib/appearance-context';
import { THEME_LABELS, type ThemeChoice } from '@/lib/appearance';

/** System → Light → Dark → System. */
const NEXT_CHOICE: Record<ThemeChoice, ThemeChoice> = {
  system: 'light',
  light: 'dark',
  dark: 'system',
};

const CHOICE_ICONS: Record<ThemeChoice, PaduIconName> = {
  system: 'appearance',
  light: 'sun',
  dark: 'moon',
};

/**
 * Compact theme switch for the header chrome: one tap steps through System,
 * Light, and Dark. The icon names the current choice and the accessibility
 * label says it outright, so the state is never carried by appearance alone.
 * It is a plain header control, not a filled chip.
 */
export function ThemeToggleButton() {
  const { preference, setPreference } = useAppearance();

  return (
    <IconButton
      accessibilityHint="Switches between system, light, and dark"
      glyphSize={18}
      icon={CHOICE_ICONS[preference]}
      label={`Theme: ${THEME_LABELS[preference]}`}
      onPress={() => setPreference(NEXT_CHOICE[preference])}
    />
  );
}
