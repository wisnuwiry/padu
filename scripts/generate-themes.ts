#!/usr/bin/env bun

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";

const root = resolve(import.meta.dir, "..");
const themesJsonPath = join(root, "themes.json");

interface AccentTokens {
  accent: string;
  accentSoft: string;
  ring: string;
  codeText: string;
}

interface SchemeDefinition {
  name: string;
  tint?: { hue: number; chroma: number };
  light: AccentTokens;
  dark: AccentTokens;
}

interface BaseTokens {
  text: string;
  textSecondary: string;
  textTertiary: string;
  textGhost: string;
  canvas: string;
  surface: string;
  surfaceMuted: string;
  raised: string;
  inset: string;
  composer: string;
  terminal: string;
  sidebar: string;
  backgroundElement: string;
  backgroundSelected: string;
  separator: string;
  border: string;
  borderStrong: string;
  sidebarBorder: string;
  codeWash: string;
  inverse: string;
  onInverse: string;
  resizeHandle: string;
  gauge: string;
  warning: string;
  warningSoft: string;
  success: string;
  successSoft: string;
  favorite: string;
  danger: string;
  dangerSoft: string;
  shadow: string;
}

interface ThemesConfig {
  $schema?: string;
  defaultScheme: string;
  schemes: Record<string, SchemeDefinition>;
  baseTokens: {
    light: BaseTokens;
    dark: BaseTokens;
  };
}

function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const clean = hex.replace("#", "");
  const num = parseInt(clean, 16);
  return {
    r: (num >> 16) & 255,
    g: (num >> 8) & 255,
    b: num & 255,
  };
}

function hexToRustRgb(hex: string): string {
  const clean = hex.replace("#", "").toUpperCase();
  return `0x${clean}`;
}

function pascalCase(str: string): string {
  return str
    .split(/[-_]/)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join("");
}

function generateTsClientTheme(config: ThemesConfig): string {
  const schemeIds = Object.keys(config.schemes);
  const schemeUnion = schemeIds.map((id) => JSON.stringify(id)).join(" | ");

  return `// Generated from themes.json by scripts/generate-themes.ts. Do not edit directly.

export type ColorSchemeId = ${schemeUnion};
export type ThemeMode = "light" | "dark";

export interface AccentTokens {
  accent: string;
  accentSoft: string;
  ring: string;
  codeText: string;
}

export interface SchemeDefinition {
  name: string;
  tint?: { hue: number; chroma: number };
  light: AccentTokens;
  dark: AccentTokens;
}

export interface BaseTokens {
  text: string;
  textSecondary: string;
  textTertiary: string;
  textGhost: string;
  canvas: string;
  surface: string;
  surfaceMuted: string;
  raised: string;
  inset: string;
  composer: string;
  terminal: string;
  sidebar: string;
  backgroundElement: string;
  backgroundSelected: string;
  separator: string;
  border: string;
  borderStrong: string;
  sidebarBorder: string;
  codeWash: string;
  inverse: string;
  onInverse: string;
  resizeHandle: string;
  gauge: string;
  warning: string;
  warningSoft: string;
  success: string;
  successSoft: string;
  favorite: string;
  danger: string;
  dangerSoft: string;
  shadow: string;
}

export interface ThemeTokens extends BaseTokens, AccentTokens {}

export const DEFAULT_COLOR_SCHEME: ColorSchemeId = ${JSON.stringify(config.defaultScheme)};

export const COLOR_SCHEMES: Record<ColorSchemeId, SchemeDefinition> = ${JSON.stringify(config.schemes, null, 2)} as const;

export const BASE_TOKENS: { light: BaseTokens; dark: BaseTokens } = ${JSON.stringify(config.baseTokens, null, 2)} as const;

export function resolveThemeColors(
  mode: ThemeMode = "dark",
  schemeId: ColorSchemeId = DEFAULT_COLOR_SCHEME
): ThemeTokens {
  const scheme = COLOR_SCHEMES[schemeId] ?? COLOR_SCHEMES[DEFAULT_COLOR_SCHEME];
  const base = BASE_TOKENS[mode];
  const accents = scheme[mode];
  return {
    ...base,
    ...accents,
  };
}

export const getThemeColors = resolveThemeColors;
`;
}

function generateMobileTheme(config: ThemesConfig): string {
  const defaultScheme = config.schemes[config.defaultScheme] ?? Object.values(config.schemes)[0];

  const lightTokens = {
    text: config.baseTokens.light.text,
    textSecondary: config.baseTokens.light.textSecondary,
    textTertiary: config.baseTokens.light.textTertiary,
    textGhost: config.baseTokens.light.textGhost,
    background: config.baseTokens.light.canvas,
    surface: config.baseTokens.light.surface,
    surfaceMuted: config.baseTokens.light.surfaceMuted,
    raised: config.baseTokens.light.raised,
    inset: config.baseTokens.light.inset,
    composer: config.baseTokens.light.composer,
    backgroundElement: config.baseTokens.light.backgroundElement,
    backgroundSelected: config.baseTokens.light.backgroundSelected,
    overlay: "hsla(220, 10%, 12%, 0.05)",
    overlayStrong: "hsla(220, 10%, 12%, 0.09)",
    separator: config.baseTokens.light.separator,
    border: config.baseTokens.light.border,
    borderStrong: config.baseTokens.light.borderStrong,
    accent: defaultScheme.light.accent,
    accentSoft: defaultScheme.light.accentSoft,
    codeText: defaultScheme.light.codeText,
    codeWash: config.baseTokens.light.codeWash,
    inverse: config.baseTokens.light.inverse,
    onInverse: config.baseTokens.light.onInverse,
    success: config.baseTokens.light.success,
    successSoft: config.baseTokens.light.successSoft,
    warning: config.baseTokens.light.warning,
    warningSoft: config.baseTokens.light.warningSoft,
    danger: config.baseTokens.light.danger,
    dangerSoft: config.baseTokens.light.dangerSoft,
    shadow: config.baseTokens.light.shadow,
  };

  const darkTokens = {
    text: config.baseTokens.dark.text,
    textSecondary: config.baseTokens.dark.textSecondary,
    textTertiary: config.baseTokens.dark.textTertiary,
    textGhost: config.baseTokens.dark.textGhost,
    background: config.baseTokens.dark.canvas,
    surface: config.baseTokens.dark.raised,
    surfaceMuted: config.baseTokens.dark.surfaceMuted,
    raised: config.baseTokens.dark.raised,
    inset: config.baseTokens.dark.inset,
    composer: config.baseTokens.dark.composer,
    backgroundElement: config.baseTokens.dark.backgroundElement,
    backgroundSelected: config.baseTokens.dark.backgroundSelected,
    overlay: "hsla(220, 10%, 90%, 0.05)",
    overlayStrong: "hsla(220, 10%, 90%, 0.09)",
    separator: config.baseTokens.dark.separator,
    border: config.baseTokens.dark.border,
    borderStrong: config.baseTokens.dark.borderStrong,
    accent: defaultScheme.dark.accent,
    accentSoft: defaultScheme.dark.accentSoft,
    codeText: defaultScheme.dark.codeText,
    codeWash: config.baseTokens.dark.codeWash,
    inverse: config.baseTokens.dark.inverse,
    onInverse: config.baseTokens.dark.onInverse,
    success: config.baseTokens.dark.success,
    successSoft: config.baseTokens.dark.successSoft,
    warning: config.baseTokens.dark.warning,
    warningSoft: config.baseTokens.dark.warningSoft,
    danger: config.baseTokens.dark.danger,
    dangerSoft: config.baseTokens.dark.dangerSoft,
    shadow: config.baseTokens.dark.shadow,
  };

  return `// Generated from themes.json by scripts/generate-themes.ts. Do not edit directly.

import { Platform, PlatformColor, type ColorValue } from 'react-native';
import {
  COLOR_SCHEMES,
  DEFAULT_COLOR_SCHEME,
  resolveThemeColors,
  type ColorSchemeId,
  type ThemeMode,
} from '@padu/client';

export { COLOR_SCHEMES, DEFAULT_COLOR_SCHEME, type ColorSchemeId, type ThemeMode };

/**
 * Padu's palette, mirrored from themes.json so every client reads as one product.
 */
export const Colors = {
  light: ${JSON.stringify(lightTokens, null, 4)},
  dark: ${JSON.stringify(darkTokens, null, 4)},
} as const;

export type ThemeColor = keyof typeof Colors.light & keyof typeof Colors.dark;

export function getMobileThemeColors(
  mode: ThemeMode = 'dark',
  schemeId: ColorSchemeId = DEFAULT_COLOR_SCHEME
) {
  const resolved = resolveThemeColors(mode, schemeId);
  return {
    ...Colors[mode],
    accent: resolved.accent,
    accentSoft: resolved.accentSoft,
    codeText: resolved.codeText,
  };
}

/** System tint for interactive affordances */
export const NativeTint: ColorValue =
  Platform.OS === 'ios' ? PlatformColor('systemBlue') : '#3b82f6';

export const Fonts = Platform.select({
  ios: {
    sans: 'system-ui',
    serif: 'ui-serif',
    rounded: 'ui-rounded',
    mono: 'ui-monospace',
  },
  default: {
    sans: 'normal',
    serif: 'serif',
    rounded: 'normal',
    mono: 'monospace',
  },
  web: {
    sans: 'var(--font-display)',
    serif: 'var(--font-serif)',
    rounded: 'var(--font-rounded)',
    mono: 'var(--font-mono)',
  },
});

export const MonoFont = Platform.select({
  ios: 'Menlo',
  android: 'monospace',
  default: 'monospace',
});

export const Spacing = {
  half: 2,
  one: 4,
  two: 8,
  three: 16,
  four: 24,
  five: 32,
  six: 64,
} as const;

export const Radius = {
  tiny: 6,
  small: 8,
  medium: 12,
  large: 18,
  pill: 999,
} as const;

export const BottomTabInset = Platform.select({ ios: 50, android: 80 }) ?? 0;
export const MaxContentWidth = 800;
`;
}

function generateWebThemesCss(config: ThemesConfig): string {
  const defaultSchemeKey = config.defaultScheme;
  const defaultScheme = config.schemes[defaultSchemeKey] ?? Object.values(config.schemes)[0];

  let css = `/* Generated from themes.json by scripts/generate-themes.ts. Do not edit directly. */

:root {
  --color-scheme-id: "${defaultSchemeKey}";
  --ring: ${defaultScheme.light.ring};
  --code-text: ${defaultScheme.light.codeText};
}

.dark {
  --color-scheme-id: "${defaultSchemeKey}";
  --ring: ${defaultScheme.dark.ring};
  --code-text: ${defaultScheme.dark.codeText};
}

@media (prefers-color-scheme: dark) {
  :root:not(.light) {
    --color-scheme-id: "${defaultSchemeKey}";
    --ring: ${defaultScheme.dark.ring};
    --code-text: ${defaultScheme.dark.codeText};
  }
}
`;

  // Add data-color-scheme overrides
  for (const [id, scheme] of Object.entries(config.schemes)) {
    css += `
/* Color Scheme: ${scheme.name} */
[data-color-scheme="${id}"] {
  --color-scheme-id: "${id}";
  --ring: ${scheme.light.ring};
  --code-text: ${scheme.light.codeText};
}

.dark [data-color-scheme="${id}"],
[data-color-scheme="${id}"].dark,
html.dark[data-color-scheme="${id}"] {
  --color-scheme-id: "${id}";
  --ring: ${scheme.dark.ring};
  --code-text: ${scheme.dark.codeText};
}
`;
  }

  return css;
}

function generateLandingThemesCss(config: ThemesConfig): string {
  const defaultSchemeKey = config.defaultScheme;
  const defaultScheme = config.schemes[defaultSchemeKey] ?? Object.values(config.schemes)[0];

  return `/* Generated from themes.json by scripts/generate-themes.ts. Do not edit directly. */

@theme {
  --color-primary: ${defaultScheme.dark.accent};
  --color-primary-foreground: #ffffff;
  --color-mock-accent: ${defaultScheme.dark.accent};
  --color-mock-accent-bright: #a78bfa;
}
`;
}

function generateRustPalette(config: ThemesConfig): string {

  const schemeEntries = Object.entries(config.schemes);
  const enumVariants = schemeEntries
    .map(([id]) => {
      const variant = pascalCase(id);
      if (id === config.defaultScheme) {
        return `    #[default]\n    ${variant},`;
      }
      return `    ${variant},`;
    })
    .join("\n");

  const allVariants = schemeEntries
    .map(([id]) => `ColorScheme::${pascalCase(id)}`)
    .join(", ");

  const asStrArms = schemeEntries
    .map(([id]) => `            ColorScheme::${pascalCase(id)} => "${id}",`)
    .join("\n");

  const parseArms = schemeEntries
    .map(
      ([id]) =>
        `            "${id}" | "${pascalCase(id)}" => Some(ColorScheme::${pascalCase(id)}),`
    )
    .join("\n");

  const labelArms = schemeEntries
    .map(([id, scheme]) => `            ColorScheme::${pascalCase(id)} => "${scheme.name}",`)
    .join("\n");

  const tintArms = schemeEntries
    .map(([id, scheme]) => {
      if (scheme.tint && scheme.tint.chroma > 0.0) {
        return `            ColorScheme::${pascalCase(id)} => bezel::theme::Tint::new(${scheme.tint.hue.toFixed(3)}, ${scheme.tint.chroma.toFixed(3)}),`;
      }
      return `            ColorScheme::${pascalCase(id)} => bezel::theme::Tint::NONE,`;
    })
    .join("\n");

  const darkTokenArms = schemeEntries
    .map(([id, scheme]) => {
      const accentHex = hexToRustRgb(scheme.dark.accent);
      const codeHex = hexToRustRgb(scheme.dark.codeText);
      return `            ColorScheme::${pascalCase(id)} => (gpui::rgb(${accentHex}).into(), gpui::rgb(${codeHex}).into()),`;
    })
    .join("\n");

  const lightTokenArms = schemeEntries
    .map(([id, scheme]) => {
      const accentHex = hexToRustRgb(scheme.light.accent);
      const codeHex = hexToRustRgb(scheme.light.codeText);
      return `            ColorScheme::${pascalCase(id)} => (gpui::rgb(${accentHex}).into(), gpui::rgb(${codeHex}).into()),`;
    })
    .join("\n");

  return `// Generated from themes.json by scripts/generate-themes.ts. Do not edit directly.

use gpui::Hsla;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorScheme {
${enumVariants}
}

#[allow(dead_code)]
impl ColorScheme {
    pub const ALL: [Self; ${schemeEntries.length}] = [${allVariants}];

    pub const fn as_str(self) -> &'static str {
        match self {
${asStrArms}
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
${labelArms}
        }
    }

    pub const fn tint(self) -> bezel::theme::Tint {
        match self {
${tintArms}
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
${parseArms}
            _ => None,
        }
    }

    /// Returns (accent, code_text) for dark theme.
    pub fn dark_tokens(self) -> (Hsla, Hsla) {
        match self {
${darkTokenArms}
        }
    }

    /// Returns (accent, code_text) for light theme.
    pub fn light_tokens(self) -> (Hsla, Hsla) {
        match self {
${lightTokenArms}
        }
    }
}
`;
}

function main() {
  const isCheck = process.argv.includes("--check");
  const rawConfig = readFileSync(themesJsonPath, "utf-8");
  const config = JSON.parse(rawConfig) as ThemesConfig;

  const targets = [
    {
      path: join(root, "packages/padu-client/src/theme.ts"),
      content: generateTsClientTheme(config),
      name: "TypeScript Client Theme (packages/padu-client/src/theme.ts)",
    },
    {
      path: join(root, "apps/mobile/src/constants/theme.ts"),
      content: generateMobileTheme(config),
      name: "Mobile Theme (apps/mobile/src/constants/theme.ts)",
    },
    {
      path: join(root, "apps/web/src/themes.css"),
      content: generateWebThemesCss(config),
      name: "Web CSS Themes (apps/web/src/themes.css)",
    },
    {
      path: join(root, "apps/landing/src/themes.css"),
      content: generateLandingThemesCss(config),
      name: "Landing CSS Themes (apps/landing/src/themes.css)",
    },
    {
      path: join(root, "apps/desktop/src/theme_palette.rs"),
      content: generateRustPalette(config),
      name: "Rust Desktop Palette (apps/desktop/src/theme_palette.rs)",
    },
  ];

  let hasDiff = false;

  for (const target of targets) {
    const existing = existsSync(target.path) ? readFileSync(target.path, "utf-8") : null;
    if (existing !== target.content) {
      if (isCheck) {
        console.error(`[theme:check] File is out of date: ${target.name}`);
        hasDiff = true;
      } else {
        writeFileSync(target.path, target.content, "utf-8");
        console.log(`[theme:generate] Updated ${target.name}`);
      }
    } else {
      if (!isCheck) {
        console.log(`[theme:generate] Up to date: ${target.name}`);
      }
    }
  }

  if (isCheck) {
    if (hasDiff) {
      console.error("[theme:check] Themes out of sync! Run `bun run theme:generate`.");
      process.exit(1);
    } else {
      console.log("[theme:check] All generated theme files are current.");
    }
  }
}

main();
