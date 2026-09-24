import {
  ActivityIndicator,
  Pressable,
  StyleSheet,
  Text,
  type GestureResponderEvent,
} from 'react-native';

import { PaduIcon, type PaduIconName } from '@/components/padu-icon';
import { Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

export type ButtonVariant = 'primary' | 'secondary';
export type IconButtonVariant = 'plain' | 'filled';

/**
 * Padu's button, drawn in JavaScript rather than handed to the platform.
 *
 * A native button is a different control on each platform — an iOS filled
 * capsule against an Android Material button — and the surrounding chrome
 * elsewhere leans on Liquid Glass, which only iOS 26 renders. Owning the
 * control keeps the two platforms identical instead of merely similar.
 */
export function Button({
  accessibilityHint,
  busy = false,
  disabled = false,
  icon,
  label,
  onPress,
  variant = 'primary',
}: {
  accessibilityHint?: string;
  busy?: boolean;
  disabled?: boolean;
  icon?: PaduIconName;
  label: string;
  onPress: (event: GestureResponderEvent) => void;
  variant?: ButtonVariant;
}) {
  const theme = useTheme();
  const inactive = disabled || busy;
  const palette =
    variant === 'secondary'
      ? {
          background: theme.surface,
          border: theme.borderStrong,
          borderWidth: 1.5,
          label: theme.text,
          weight: '600' as const,
        }
      : {
          background: theme.inverse,
          border: 'transparent',
          borderWidth: 0,
          label: theme.onInverse,
          weight: '700' as const,
        };

  return (
    <Pressable
      accessibilityHint={accessibilityHint}
      accessibilityLabel={label}
      accessibilityRole="button"
      accessibilityState={{ busy, disabled: inactive }}
      disabled={inactive}
      onPress={onPress}
      style={({ pressed }) => [
        styles.button,
        {
          backgroundColor: palette.background,
          borderColor: palette.border,
          borderWidth: palette.borderWidth,
          opacity: disabled ? 0.5 : pressed ? 0.78 : 1,
        },
      ]}>
      {busy ? (
        <ActivityIndicator color={palette.label} size="small" />
      ) : (
        <>
          {icon ? (
            <PaduIcon name={icon} size={17} tintColor={palette.label} />
          ) : null}
          <Text
            style={[
              styles.buttonLabel,
              { color: palette.label, fontWeight: palette.weight },
            ]}>
            {label}
          </Text>
        </>
      )}
    </Pressable>
  );
}

/**
 * Square icon-only control for chrome and row affordances: a 44pt target on
 * both platforms (the default `size`), `plain` for transparent chrome,
 * `filled` for the one prominent action. `tintColor` overrides the glyph when
 * a quieter treatment than the variant's is wanted.
 */
export function IconButton({
  accessibilityHint,
  disabled = false,
  glyphSize = 20,
  icon,
  label,
  onPress,
  size = 44,
  tintColor,
  variant = 'plain',
}: {
  accessibilityHint?: string;
  disabled?: boolean;
  glyphSize?: number;
  icon: PaduIconName;
  label: string;
  onPress: (event: GestureResponderEvent) => void;
  /** Box edge; 44 is the accessibility floor. */
  size?: number;
  tintColor?: string;
  variant?: IconButtonVariant;
}) {
  const theme = useTheme();
  const palette =
    variant === 'filled'
      ? { background: theme.inverse, icon: theme.onInverse }
      : { background: 'transparent', icon: theme.text };

  return (
    <Pressable
      accessibilityHint={accessibilityHint}
      accessibilityLabel={label}
      accessibilityRole="button"
      accessibilityState={{ disabled }}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        styles.iconButton,
        {
          backgroundColor: palette.background,
          height: size,
          opacity: disabled ? 0.4 : pressed ? 0.55 : 1,
          width: size,
        },
      ]}>
      <PaduIcon
        name={icon}
        size={glyphSize}
        tintColor={tintColor ?? palette.icon}
      />
    </Pressable>
  );
}

const styles = StyleSheet.create({
  button: {
    alignItems: 'center',
    borderRadius: Radius.large,
    flexDirection: 'row',
    gap: Spacing.two,
    justifyContent: 'center',
    minHeight: 50,
    paddingHorizontal: Spacing.four,
  },
  buttonLabel: { fontSize: 15 },
  iconButton: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    justifyContent: 'center',
  },
});
