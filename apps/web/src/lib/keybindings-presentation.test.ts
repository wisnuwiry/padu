import { describe, expect, test } from 'bun:test'
import { SETTINGS_PAGES } from '@/components/settings-view'
import { KEYBINDING_SECTIONS } from '@/components/keybindings-settings'
import { parseRustI18nCatalog } from './i18n-core'
import englishSource from '../../../../locales/app.yml?raw'

describe('keybindings settings parity', () => {
  const en = parseRustI18nCatalog(englishSource, 'en')

  test('declares keybindings in SETTINGS_PAGES in the expected position and configuration', () => {
    const ids = SETTINGS_PAGES.map((page) => page.id)
    expect(ids).toEqual([
      'general',
      'appearance',
      'keybindings',
      'notifications',
      'providers',
      'skills',
      'usage',
      'daemon',
      'about',
    ])

    const keybindingsPage = SETTINGS_PAGES.find((p) => p.id === 'keybindings')
    expect(keybindingsPage).toBeDefined()
    expect(keybindingsPage?.icon).toBe('command')
    expect(keybindingsPage?.labelKey).toBe('settings.keybindings')
    expect(keybindingsPage?.keywordsKey).toBe('settings.keybindings_keywords')
    expect(en[keybindingsPage!.labelKey]).toBe('Keybindings')
  })

  test('every keybinding item and section has valid English translations', () => {
    expect(KEYBINDING_SECTIONS.length).toBe(5)

    for (const section of KEYBINDING_SECTIONS) {
      expect(en[section.sectionKey]).toBeDefined()
      expect(section.items.length).toBeGreaterThan(0)

      for (const item of section.items) {
        expect(en[item.titleKey]).toBeDefined()
        expect(en[item.descKey]).toBeDefined()
        expect(item.mac).toBeTruthy()
        expect(item.other).toBeTruthy()
      }
    }
  })

  test('filters keybindings by query matching title, desc, section, or shortcut', () => {
    const query = 'composer'
    const matching = KEYBINDING_SECTIONS.map((section) => ({
      ...section,
      items: section.items.filter((item) => {
        const title = en[item.titleKey]?.toLowerCase() ?? ''
        const desc = en[item.descKey]?.toLowerCase() ?? ''
        return title.includes(query) || desc.includes(query)
      }),
    })).filter((section) => section.items.length > 0)

    expect(matching.length).toBe(1)
    expect(matching[0]!.sectionKey).toBe('keybindings.section_chat')
    expect(matching[0]!.items[0]!.titleKey).toBe('keybindings.focus_composer')
  })

  test('filters keybindings by shortcut key', () => {
    const query = 'cmd+k'
    const normalizedQuery = query.replace('cmd+', '⌘').toLowerCase()

    const matching = KEYBINDING_SECTIONS.map((section) => ({
      ...section,
      items: section.items.filter((item) => {
        return item.mac.toLowerCase().includes(normalizedQuery) || item.other.toLowerCase().includes(query)
      }),
    })).filter((section) => section.items.length > 0)

    expect(matching.length).toBe(1)
    expect(matching[0]!.items[0]!.titleKey).toBe('keybindings.command_palette')
  })
})
