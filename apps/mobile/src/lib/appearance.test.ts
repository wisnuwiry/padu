import { describe, expect, test } from 'bun:test';

import {
  parseThemeChoice,
  readThemeChoice,
  resolvedTheme,
  THEME_CHOICES,
  THEME_KEY,
  THEME_LABELS,
} from './appearance';

describe('appearance', () => {
  test('accepts the three choices the product defines', () => {
    expect(THEME_CHOICES).toEqual(['system', 'light', 'dark']);
    expect(THEME_LABELS.system).toBe('System');
    expect(THEME_LABELS.light).toBe('Light');
    expect(THEME_LABELS.dark).toBe('Dark');
    expect(parseThemeChoice('system')).toBe('system');
    expect(parseThemeChoice('light')).toBe('light');
    expect(parseThemeChoice('dark')).toBe('dark');
  });

  test('degrades anything unrecognised to system', () => {
    expect(parseThemeChoice(null)).toBe('system');
    expect(parseThemeChoice(undefined)).toBe('system');
    expect(parseThemeChoice('')).toBe('system');
    // A value written by a newer build must not break the launch.
    expect(parseThemeChoice('sepia')).toBe('system');
    expect(parseThemeChoice('SYSTEM')).toBe('system');
  });

  test('reads the choice from storage, defaulting when absent', () => {
    const store = (value: string | null) => ({
      getItem: (key: string) => (key === THEME_KEY ? value : null),
    });
    expect(readThemeChoice(store('dark'))).toBe('dark');
    expect(readThemeChoice(store(null))).toBe('system');
    // No storage (web before hydration) must not throw.
    expect(readThemeChoice(null)).toBe('system');
  });

  test('system follows the OS, an explicit choice overrides it', () => {
    expect(resolvedTheme('system', true)).toBe('dark');
    expect(resolvedTheme('system', false)).toBe('light');
    expect(resolvedTheme('light', true)).toBe('light');
    expect(resolvedTheme('dark', false)).toBe('dark');
  });
});
