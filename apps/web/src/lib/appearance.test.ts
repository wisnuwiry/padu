import { describe, expect, test } from 'bun:test'
import {
  applyColorSchemeChoice,
  readColorSchemeChoice,
  readThemeChoice,
  resolvedTheme,
} from './appearance'

describe('appearance preferences', () => {
  test('system follows the current operating-system appearance', () => {
    expect(resolvedTheme('system', true)).toBe('dark')
    expect(resolvedTheme('system', false)).toBe('light')
  })

  test('an explicit appearance overrides the operating system', () => {
    expect(resolvedTheme('light', true)).toBe('light')
    expect(resolvedTheme('dark', false)).toBe('dark')
  })

  test('unknown and missing stored values fall back to system', () => {
    expect(readThemeChoice(null)).toBe('system')
    expect(readThemeChoice({ getItem: () => 'sepia' })).toBe('system')
    expect(readThemeChoice({ getItem: () => 'dark' })).toBe('dark')
  })

  test('color scheme fallback to default when missing or invalid', () => {
    expect(readColorSchemeChoice(null)).toBe('purple')
    expect(readColorSchemeChoice({ getItem: () => 'invalid' })).toBe('purple')
    expect(readColorSchemeChoice({ getItem: () => 'blue' })).toBe('blue')
    expect(readColorSchemeChoice({ getItem: () => 'coral' })).toBe('coral')
  })

  test('applyColorSchemeChoice sets data-color-scheme attribute', () => {
    const el = {
      attr: {} as Record<string, string>,
      setAttribute(name: string, value: string) {
        this.attr[name] = value
      },
    }
    applyColorSchemeChoice(el, 'emerald' as any)
    expect(el.attr['data-color-scheme']).toBe('emerald')
  })
})
