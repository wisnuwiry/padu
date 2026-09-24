import { useMemo } from 'react';
import { View, type ViewProps } from 'react-native';
import { SvgXml } from 'react-native-svg';

import { PADU_ICON_SVGS, type PaduIconName } from './padu-icons.generated';
import { useTheme } from '@/hooks/use-theme';

export type { PaduIconName };

/**
 * Design-system mark from `assets/icons/`, mirroring web's `PaduIcon`.
 * The tint defaults to the theme text color; pass `tintColor` for a semantic
 * role (secondary, tertiary, danger, accent). Decorative by default — pass
 * `accessibilityLabel` when the icon alone carries meaning.
 */
export function PaduIcon({
  name,
  size = 18,
  tintColor,
  style,
  accessibilityLabel,
}: {
  name: PaduIconName;
  size?: number;
  tintColor?: string;
  style?: ViewProps['style'];
  accessibilityLabel?: string;
}) {
  const theme = useTheme();
  const tint = tintColor ?? theme.text;
  const entry = PADU_ICON_SVGS[name];
  const xml = useMemo(
    () => `<svg viewBox="${entry.viewBox}">${entry.body.replaceAll('%%TINT%%', tint)}</svg>`,
    [entry, tint],
  );
  return (
    <View
      accessibilityElementsHidden={accessibilityLabel == null}
      accessibilityLabel={accessibilityLabel}
      accessibilityRole={accessibilityLabel != null ? 'image' : undefined}
      style={[{ alignItems: 'center', height: size, justifyContent: 'center', width: size }, style]}>
      <SvgXml height={size} width={size} xml={xml} />
    </View>
  );
}
