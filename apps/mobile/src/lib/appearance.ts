/**
 * Which appearance the user picked, and what it resolves to.
 *
 * Mirrors `apps/web/src/lib/appearance.ts` and the shared `ThemePreference` in
 * `crates/padu-protocol/src/theme.rs`, so every surface names this setting the
 * same way. Pure policy only — the storage lives in `appearance-store.ts`, the
 * same split as `daemon-profile.ts` / `daemon-storage.ts`.
 */

export type ThemeChoice = 'system' | 'light' | 'dark';

/** What the choice resolves to once the system preference is applied. */
export type ResolvedTheme = Exclude<ThemeChoice, 'system'>;

/** Order shown in the picker, matching the web client's. */
export const THEME_CHOICES: ThemeChoice[] = ['system', 'light', 'dark'];

/**
 * Mobile's own store key. The web client keeps its copy under `padu.theme` in
 * localStorage — a different store, so only the values have to match.
 */
export const THEME_KEY = 'padu.mobile.theme.v1';

/** English labels mirroring `settings.theme_*` in `locales/app.yml`. */
export const THEME_LABELS: Record<ThemeChoice, string> = {
  system: 'System',
  light: 'Light',
  dark: 'Dark',
};

/**
 * Read a stored choice, degrading to `system` for anything unrecognised —
 * including a value written by a newer build.
 */
export function parseThemeChoice(value: string | null | undefined): ThemeChoice {
  return value === 'light' || value === 'dark' ? value : 'system';
}

export function readThemeChoice(
  store: Pick<Storage, 'getItem'> | null,
): ThemeChoice {
  return parseThemeChoice(store?.getItem(THEME_KEY));
}

/** `system` follows the OS; an explicit choice overrides it. */
export function resolvedTheme(
  choice: ThemeChoice,
  systemPrefersDark: boolean,
): ResolvedTheme {
  if (choice === 'system') return systemPrefersDark ? 'dark' : 'light';
  return choice;
}
