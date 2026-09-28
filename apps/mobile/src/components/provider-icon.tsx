import type { ProviderKind } from '@padu/client';

import { PaduIcon } from './padu-icon';
import { PROVIDER_ICON_NAME } from './padu-icons.generated';
import { useTheme } from '@/hooks/use-theme';

/** Marks with a real brand color; monochrome brands (OpenAI, Cursor, Grok,
 * OpenCode, Fx, Pi, Agy, Qoder, Command Code, Kimi) tint with the theme text
 * color, and Oh My Pi's SVG carries its own gradient. */
const PROVIDER_BRAND_COLORS: Partial<Record<ProviderKind, string>> = {
  agy: '#4388f0',
  claude: '#d97757',
  deepSeek: '#4d6bfe',
  amp: '#f34e3f',
  ohMyPi: '#d05cdd',
};

/** Brand color for a provider mark, when the brand has one. */
export function providerBrandColor(provider: ProviderKind): string | undefined {
  return PROVIDER_BRAND_COLORS[provider];
}

/** Brand mark for an agent provider. */
export function ProviderIcon({
  provider,
  size = 18,
  color,
}: {
  provider: ProviderKind;
  size?: number;
  color?: string;
}) {
  const theme = useTheme();
  return (
    <PaduIcon
      name={PROVIDER_ICON_NAME[provider]}
      size={size}
      tintColor={color ?? PROVIDER_BRAND_COLORS[provider] ?? theme.text}
    />
  );
}
