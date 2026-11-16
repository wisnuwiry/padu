import { DEFAULT_COLOR_SCHEME, type ColorSchemeId } from '@padu/client'

export type ThemeChoice = 'system' | 'light' | 'dark'

/**
 * Key for persisting the active accent color scheme.
 */
export const COLOR_SCHEME_KEY = 'padu.color-scheme'

export const COLOR_SCHEME_IDS: ColorSchemeId[] = [
  'purple',
  'blue',
  'green',
  'amber',
  'coral',
  'rose',
  'neutral',
]

/**
 * Whether code surfaces wrap long lines. Shared by the Settings → Code toggle
 * and every surface that renders code, so they stay in step.
 */
export const CODE_WORD_WRAP_KEY = 'padu.code-word-wrap'

/** `overflow` value for a `@pierre/diffs` surface under the word-wrap setting. */
export function codeOverflow(wordWrap: boolean): 'wrap' | 'scroll' {
  return wordWrap ? 'wrap' : 'scroll'
}

export function readThemeChoice(storage: Pick<Storage, 'getItem'> | null): ThemeChoice {
  const stored = storage?.getItem('padu.theme')
  return stored === 'light' || stored === 'dark' ? stored : 'system'
}

export function readColorSchemeChoice(storage: Pick<Storage, 'getItem'> | null): ColorSchemeId {
  const stored = storage?.getItem(COLOR_SCHEME_KEY) as ColorSchemeId | null
  return stored && COLOR_SCHEME_IDS.includes(stored) ? stored : DEFAULT_COLOR_SCHEME
}

export function resolvedTheme(
  choice: ThemeChoice,
  systemPrefersDark: boolean,
): Exclude<ThemeChoice, 'system'> {
  if (choice === 'system') return systemPrefersDark ? 'dark' : 'light'
  return choice
}

export function applyThemeChoice(
  root: Pick<HTMLElement, 'classList'>,
  choice: ThemeChoice,
  systemPrefersDark: boolean,
) {
  const resolved = resolvedTheme(choice, systemPrefersDark)
  root.classList.toggle('dark', resolved === 'dark')
  root.classList.toggle('light', resolved === 'light')
}

export function applyColorSchemeChoice(
  root: { setAttribute(name: string, value: string): void },
  scheme: ColorSchemeId,
) {
  root.setAttribute('data-color-scheme', scheme)
}
