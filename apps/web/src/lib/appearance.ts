export type ThemeChoice = 'system' | 'light' | 'dark'

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
