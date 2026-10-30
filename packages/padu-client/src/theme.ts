// Generated from themes.json by scripts/generate-themes.ts. Do not edit directly.

export type ColorSchemeId = "purple" | "coral";
export type ThemeMode = "light" | "dark";

export interface AccentTokens {
  accent: string;
  accentSoft: string;
  ring: string;
  codeText: string;
}

export interface SchemeDefinition {
  name: string;
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

export const DEFAULT_COLOR_SCHEME: ColorSchemeId = "purple";

export const COLOR_SCHEMES: Record<ColorSchemeId, SchemeDefinition> = {
  "purple": {
    "name": "Purple",
    "light": {
      "accent": "#7c3aed",
      "accentSoft": "rgba(124, 58, 237, 0.12)",
      "ring": "#7c3aed",
      "codeText": "#6d28d9"
    },
    "dark": {
      "accent": "#8b5cf6",
      "accentSoft": "rgba(139, 92, 246, 0.15)",
      "ring": "#8b5cf6",
      "codeText": "#c4b5fd"
    }
  },
  "coral": {
    "name": "Coral",
    "light": {
      "accent": "#c85f44",
      "accentSoft": "rgba(200, 95, 68, 0.12)",
      "ring": "#c85f44",
      "codeText": "#9a5528"
    },
    "dark": {
      "accent": "#e2795b",
      "accentSoft": "rgba(226, 121, 91, 0.15)",
      "ring": "#e2795b",
      "codeText": "#e0a882"
    }
  }
} as const;

export const BASE_TOKENS: { light: BaseTokens; dark: BaseTokens } = {
  "light": {
    "text": "#242424",
    "textSecondary": "#666666",
    "textTertiary": "#858585",
    "textGhost": "#a4a4a4",
    "canvas": "#f6f5f6",
    "surface": "#ffffff",
    "surfaceMuted": "#ececec",
    "raised": "#ececec",
    "inset": "#e6e6e6",
    "composer": "#ffffff",
    "terminal": "#ffffff",
    "sidebar": "#f3f3f3",
    "backgroundElement": "#ececec",
    "backgroundSelected": "#e2e1e2",
    "separator": "rgba(28, 31, 37, 0.10)",
    "border": "hsla(220, 10%, 12%, 0.08)",
    "borderStrong": "hsla(220, 10%, 12%, 0.15)",
    "sidebarBorder": "hsla(0, 0%, 8%, 0.12)",
    "codeWash": "hsla(220, 10%, 12%, 0.07)",
    "inverse": "#202227",
    "onInverse": "#f8f8f9",
    "resizeHandle": "#2563eb",
    "gauge": "#2563eb",
    "warning": "#a66b20",
    "warningSoft": "rgba(166, 107, 32, 0.12)",
    "success": "#2f8f52",
    "successSoft": "rgba(47, 143, 82, 0.12)",
    "favorite": "#ca8a04",
    "danger": "#c64a42",
    "dangerSoft": "hsla(4, 55%, 52%, 0.10)",
    "shadow": "rgba(0, 0, 0, 0.12)"
  },
  "dark": {
    "text": "#e2e2e2",
    "textSecondary": "#a3a3a3",
    "textTertiary": "#7d7d7d",
    "textGhost": "#575757",
    "canvas": "#1a1a1a",
    "surface": "#1a1a1a",
    "surfaceMuted": "#2a2a2a",
    "raised": "#232323",
    "inset": "#151515",
    "composer": "#212121",
    "terminal": "#1a1a1a",
    "sidebar": "#181818",
    "backgroundElement": "#232323",
    "backgroundSelected": "#303030",
    "separator": "rgba(230, 230, 230, 0.09)",
    "border": "hsla(220, 10%, 90%, 0.07)",
    "borderStrong": "hsla(220, 10%, 90%, 0.14)",
    "sidebarBorder": "hsla(126.93, 0%, 16%, 1.0)",
    "codeWash": "hsla(220, 10%, 90%, 0.08)",
    "inverse": "#e7e9ec",
    "onInverse": "#17181c",
    "resizeHandle": "#3b82f6",
    "gauge": "#3b82f6",
    "warning": "#e0b36a",
    "warningSoft": "rgba(224, 179, 106, 0.14)",
    "success": "#62c987",
    "successSoft": "rgba(98, 201, 135, 0.14)",
    "favorite": "#eab308",
    "danger": "#e2726a",
    "dangerSoft": "hsla(4, 55%, 63%, 0.10)",
    "shadow": "rgba(0, 0, 0, 0.45)"
  }
} as const;

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
