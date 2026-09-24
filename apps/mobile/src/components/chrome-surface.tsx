import type { ReactNode } from 'react';
import { StyleSheet, View, type ViewProps } from 'react-native';

import { useTheme } from '@/hooks/use-theme';

/**
 * Floating chrome surface: an opaque, bordered panel. The same on every
 * platform.
 *
 * This replaces the Liquid Glass material the app used on iOS 26. That material
 * exists on one OS version only, so iOS and Android rendered visibly different
 * chrome — and the translucent fallback left a floating control hard to read
 * once content scrolled beneath it. A solid surface with a hairline border
 * reads the same everywhere and costs no blur pass in any frame.
 */
export function ChromeSurface({
  children,
  style,
}: {
  children: ReactNode;
  style?: ViewProps['style'];
}) {
  const theme = useTheme();
  return (
    <View
      style={[
        styles.surface,
        { backgroundColor: theme.surface, borderColor: theme.borderStrong },
        style,
      ]}>
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  surface: {
    borderWidth: StyleSheet.hairlineWidth,
    // Clips the pressed fill to the surface's own radius.
    overflow: 'hidden',
  },
});
