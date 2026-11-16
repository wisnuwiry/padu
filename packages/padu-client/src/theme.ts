// Generated from themes.json by scripts/generate-themes.ts. Do not edit directly.

export type ColorSchemeId = "purple" | "blue" | "green" | "amber" | "coral" | "rose" | "neutral";
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

export const DEFAULT_COLOR_SCHEME: ColorSchemeId = "purple";

export const COLOR_SCHEMES: Record<ColorSchemeId, SchemeDefinition> = {
  "purple": {
    "name": "Purple",
    "tint": {
      "hue": 276.935,
      "chroma": 0.182
    },
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
  "blue": {
    "name": "Blue",
    "tint": {
      "hue": 250,
      "chroma": 0.18
    },
    "light": {
      "accent": "#2563eb",
      "accentSoft": "rgba(37, 99, 235, 0.12)",
      "ring": "#2563eb",
      "codeText": "#1d4ed8"
    },
    "dark": {
      "accent": "#3b82f6",
      "accentSoft": "rgba(59, 130, 246, 0.15)",
      "ring": "#3b82f6",
      "codeText": "#93c5fd"
    }
  },
  "green": {
    "name": "Green",
    "tint": {
      "hue": 155,
      "chroma": 0.16
    },
    "light": {
      "accent": "#059669",
      "accentSoft": "rgba(5, 150, 105, 0.12)",
      "ring": "#059669",
      "codeText": "#047857"
    },
    "dark": {
      "accent": "#10b981",
      "accentSoft": "rgba(16, 185, 129, 0.15)",
      "ring": "#10b981",
      "codeText": "#6ee7b7"
    }
  },
  "amber": {
    "name": "Amber",
    "tint": {
      "hue": 85,
      "chroma": 0.16
    },
    "light": {
      "accent": "#d97706",
      "accentSoft": "rgba(217, 119, 6, 0.12)",
      "ring": "#d97706",
      "codeText": "#b45309"
    },
    "dark": {
      "accent": "#f59e0b",
      "accentSoft": "rgba(245, 158, 11, 0.15)",
      "ring": "#f59e0b",
      "codeText": "#fcd34d"
    }
  },
  "coral": {
    "name": "Coral",
    "tint": {
      "hue": 35,
      "chroma": 0.17
    },
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
  },
  "rose": {
    "name": "Rose",
    "tint": {
      "hue": 15,
      "chroma": 0.19
    },
    "light": {
      "accent": "#e11d48",
      "accentSoft": "rgba(225, 29, 72, 0.12)",
      "ring": "#e11d48",
      "codeText": "#be123c"
    },
    "dark": {
      "accent": "#f43f5e",
      "accentSoft": "rgba(244, 63, 94, 0.15)",
      "ring": "#f43f5e",
      "codeText": "#fda4af"
    }
  },
  "neutral": {
    "name": "Neutral",
    "tint": {
      "hue": 0,
      "chroma": 0
    },
    "light": {
      "accent": "#27272a",
      "accentSoft": "rgba(39, 39, 42, 0.12)",
      "ring": "#27272a",
      "codeText": "#18181b"
    },
    "dark": {
      "accent": "#e4e4e7",
      "accentSoft": "rgba(228, 228, 231, 0.15)",
      "ring": "#e4e4e7",
      "codeText": "#f4f4f5"
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
