import { useState } from 'react'
import { PaduIcon } from '@/components/padu-icon'
import { useI18n } from '@/lib/i18n'
import { useMacLikePlatform } from '@/lib/platform'
import { Kbd } from './ui/kbd'

export interface KeybindingItem {
  titleKey: string
  descKey: string
  mac: string
  other: string
}

export interface KeybindingSection {
  sectionKey: string
  items: KeybindingItem[]
}

export const KEYBINDING_SECTIONS: KeybindingSection[] = [
  {
    sectionKey: 'keybindings.section_general',
    items: [
      {
        titleKey: 'keybindings.new_task',
        descKey: 'keybindings.new_task_desc',
        mac: '⌘N',
        other: 'Ctrl+N',
      },
      {
        titleKey: 'keybindings.open_project',
        descKey: 'keybindings.open_project_desc',
        mac: '⌘O',
        other: 'Ctrl+O',
      },
      {
        titleKey: 'keybindings.command_palette',
        descKey: 'keybindings.command_palette_desc',
        mac: '⌘K',
        other: 'Ctrl+K',
      },
      {
        titleKey: 'keybindings.open_settings',
        descKey: 'keybindings.open_settings_desc',
        mac: '⌘,',
        other: 'Ctrl+,',
      },
      {
        titleKey: 'keybindings.close_window',
        descKey: 'keybindings.close_window_desc',
        mac: '⌘W',
        other: 'Ctrl+W',
      },
      {
        titleKey: 'keybindings.quit',
        descKey: 'keybindings.quit_desc',
        mac: '⌘Q',
        other: 'Ctrl+Q',
      },
    ],
  },
  {
    sectionKey: 'keybindings.section_navigation',
    items: [
      {
        titleKey: 'keybindings.toggle_sidebar',
        descKey: 'keybindings.toggle_sidebar_desc',
        mac: '⌘B',
        other: 'Ctrl+B',
      },
      {
        titleKey: 'keybindings.toggle_right_panel',
        descKey: 'keybindings.toggle_right_panel_desc',
        mac: '⇧⌘B',
        other: 'Ctrl+Shift+B',
      },
      {
        titleKey: 'keybindings.open_browser',
        descKey: 'keybindings.open_browser_desc',
        mac: '⌥⌘B',
        other: 'Ctrl+Alt+B',
      },
      {
        titleKey: 'keybindings.open_terminal',
        descKey: 'keybindings.open_terminal_desc',
        mac: '⌘T',
        other: 'Ctrl+T',
      },
      {
        titleKey: 'keybindings.open_files',
        descKey: 'keybindings.open_files_desc',
        mac: '⇧⌘E',
        other: 'Ctrl+Shift+E',
      },
      {
        titleKey: 'keybindings.open_review',
        descKey: 'keybindings.open_review_desc',
        mac: '⌘D',
        other: 'Ctrl+D',
      },
      {
        titleKey: 'keybindings.toggle_usage_panel',
        descKey: 'keybindings.toggle_usage_panel_desc',
        mac: '⌘U',
        other: 'Ctrl+U',
      },
      {
        titleKey: 'keybindings.toggle_fps',
        descKey: 'keybindings.toggle_fps_desc',
        mac: '⌥⇧⌘F',
        other: 'Ctrl+Alt+Shift+F',
      },
      {
        titleKey: 'keybindings.navigate_back',
        descKey: 'keybindings.navigate_back_desc',
        mac: '⌘[',
        other: 'Ctrl+[',
      },
      {
        titleKey: 'keybindings.navigate_forward',
        descKey: 'keybindings.navigate_forward_desc',
        mac: '⌘]',
        other: 'Ctrl+]',
      },
      {
        titleKey: 'keybindings.previous_session',
        descKey: 'keybindings.previous_session_desc',
        mac: '⌥⌘↑',
        other: 'Ctrl+Alt+Up',
      },
      {
        titleKey: 'keybindings.next_session',
        descKey: 'keybindings.next_session_desc',
        mac: '⌥⌘↓',
        other: 'Ctrl+Alt+Down',
      },
      {
        titleKey: 'keybindings.next_task',
        descKey: 'keybindings.next_task_desc',
        mac: 'Ctrl+Tab',
        other: 'Ctrl+Tab',
      },
      {
        titleKey: 'keybindings.prev_task',
        descKey: 'keybindings.prev_task_desc',
        mac: 'Ctrl+Shift+Tab',
        other: 'Ctrl+Shift+Tab',
      },
    ],
  },
  {
    sectionKey: 'keybindings.section_chat',
    items: [
      {
        titleKey: 'keybindings.focus_composer',
        descKey: 'keybindings.focus_composer_desc',
        mac: '⌘L',
        other: 'Ctrl+L',
      },
      {
        titleKey: 'keybindings.select_model',
        descKey: 'keybindings.select_model_desc',
        mac: '⌘/',
        other: 'Ctrl+/',
      },
      {
        titleKey: 'keybindings.cancel_turn',
        descKey: 'keybindings.cancel_turn_desc',
        mac: 'Esc',
        other: 'Esc',
      },
    ],
  },
  {
    sectionKey: 'keybindings.section_editor',
    items: [
      {
        titleKey: 'keybindings.save_file',
        descKey: 'keybindings.save_file_desc',
        mac: '⌘S',
        other: 'Ctrl+S',
      },
      {
        titleKey: 'keybindings.find',
        descKey: 'keybindings.find_desc',
        mac: '⌘F',
        other: 'Ctrl+F',
      },
      {
        titleKey: 'keybindings.replace',
        descKey: 'keybindings.replace_desc',
        mac: '⌥⌘F',
        other: 'Ctrl+Alt+F',
      },
      {
        titleKey: 'keybindings.find_next',
        descKey: 'keybindings.find_next_desc',
        mac: '⌘G',
        other: 'Ctrl+G',
      },
      {
        titleKey: 'keybindings.find_prev',
        descKey: 'keybindings.find_prev_desc',
        mac: '⇧⌘G',
        other: 'Ctrl+Shift+G',
      },
    ],
  },
  {
    sectionKey: 'keybindings.section_browser',
    items: [
      {
        titleKey: 'keybindings.browser_focus',
        descKey: 'keybindings.browser_focus_desc',
        mac: '⌘L',
        other: 'Ctrl+L',
      },
      {
        titleKey: 'keybindings.browser_reload',
        descKey: 'keybindings.browser_reload_desc',
        mac: '⌘R',
        other: 'Ctrl+R',
      },
      {
        titleKey: 'keybindings.browser_hard_reload',
        descKey: 'keybindings.browser_hard_reload_desc',
        mac: '⇧⌘R',
        other: 'Ctrl+Shift+R',
      },
      {
        titleKey: 'keybindings.browser_back',
        descKey: 'keybindings.browser_back_desc',
        mac: '⌘[',
        other: 'Ctrl+[',
      },
      {
        titleKey: 'keybindings.browser_forward',
        descKey: 'keybindings.browser_forward_desc',
        mac: '⌘]',
        other: 'Ctrl+]',
      },
    ],
  },
  {
    sectionKey: 'keybindings.section_files',
    items: [
      {
        titleKey: 'keybindings.files_find',
        descKey: 'keybindings.files_find_desc',
        mac: '⌘P',
        other: 'Ctrl+P',
      },
      {
        titleKey: 'keybindings.files_new_file',
        descKey: 'keybindings.files_new_file_desc',
        mac: 'N',
        other: 'N',
      },
      {
        titleKey: 'keybindings.files_new_folder',
        descKey: 'keybindings.files_new_folder_desc',
        mac: '⇧N',
        other: 'Shift+N',
      },
      {
        titleKey: 'keybindings.files_refresh',
        descKey: 'keybindings.files_refresh_desc',
        mac: 'R',
        other: 'R',
      },
      {
        titleKey: 'keybindings.files_hidden',
        descKey: 'keybindings.files_hidden_desc',
        mac: 'H',
        other: 'H',
      },
    ],
  },
  {
    sectionKey: 'keybindings.section_review',
    items: [
      {
        titleKey: 'keybindings.refresh_review',
        descKey: 'keybindings.refresh_review_desc',
        mac: '⌘R',
        other: 'Ctrl+R',
      },
      {
        titleKey: 'keybindings.toggle_review_layout',
        descKey: 'keybindings.toggle_review_layout_desc',
        mac: '⇧⌘T',
        other: 'Ctrl+Shift+T',
      },
      {
        titleKey: 'keybindings.toggle_review_files',
        descKey: 'keybindings.toggle_review_files_desc',
        mac: '⌘\\',
        other: 'Ctrl+\\',
      },
      {
        titleKey: 'keybindings.toggle_review_collapse',
        descKey: 'keybindings.toggle_review_collapse_desc',
        mac: '⇧⌘C',
        other: 'Ctrl+Shift+C',
      },
      {
        titleKey: 'keybindings.toggle_review_file',
        descKey: 'keybindings.toggle_review_file_desc',
        mac: '⇧⌘K',
        other: 'Ctrl+Shift+K',
      },
      {
        titleKey: 'keybindings.toggle_review_tree',
        descKey: 'keybindings.toggle_review_tree_desc',
        mac: '⇧⌘O',
        other: 'Ctrl+Shift+O',
      },
    ],
  },
]

export function KeybindingsSettings() {
  const { t } = useI18n()
  const isMac = useMacLikePlatform()
  const [query, setQuery] = useState('')

  const normalizedQuery = query.trim().toLowerCase()

  const matchingSections = KEYBINDING_SECTIONS.map((section) => {
    const sectionTitle = t(section.sectionKey)
    const sectionMatches = !normalizedQuery || sectionTitle.toLowerCase().includes(normalizedQuery)

    const matchingItems = section.items.filter((item) => {
      if (sectionMatches) return true
      const title = t(item.titleKey).toLowerCase()
      const desc = t(item.descKey).toLowerCase()
      const shortcut = (isMac ? item.mac : item.other).toLowerCase()
      const altShortcut = (isMac ? item.other : item.mac).toLowerCase()
      return (
        title.includes(normalizedQuery) ||
        desc.includes(normalizedQuery) ||
        shortcut.includes(normalizedQuery) ||
        altShortcut.includes(normalizedQuery)
      )
    })

    return {
      sectionKey: section.sectionKey,
      items: matchingItems,
    }
  }).filter((section) => section.items.length > 0)

  return (
    <div>
      <div className="mt-[15px] flex h-8 items-center gap-2 rounded-lg border bg-[var(--inset)] px-2.5 focus-within:border-ring">
        <PaduIcon className="size-[13px] text-[var(--text-tertiary)]" name="search" />
        <input
          aria-label={t('keybindings.search')}
          className="min-w-0 flex-1 bg-transparent text-[12px] outline-none placeholder:text-[var(--text-ghost)]"
          placeholder={t('keybindings.search')}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape' && query) {
              event.preventDefault()
              setQuery('')
            }
          }}
        />
        {query && (
          <button
            aria-label={t('common.clear')}
            className="grid size-4 place-items-center rounded text-[var(--text-tertiary)] hover:text-foreground"
            type="button"
            onClick={() => setQuery('')}
          >
            <PaduIcon className="size-3" name="x" />
          </button>
        )}
      </div>

      <div className="mt-5 flex flex-col gap-5">
        {matchingSections.map((section) => (
          <div className="flex flex-col gap-2" key={section.sectionKey}>
            <h2 className="px-1 text-[13px] font-semibold text-[var(--text-secondary)]">
              {t(section.sectionKey)}
            </h2>
            <div className="overflow-hidden rounded-[13px] bg-[var(--raised)]">
              {section.items.map((item, index) => (
                <div key={item.titleKey}>
                  {index > 0 && <div className="mx-5 border-t" />}
                  <div className="flex min-h-[54px] items-center justify-between gap-6 px-5 py-2.5">
                    <div className="min-w-0 flex-1">
                      <div className="text-[13.5px] font-medium text-foreground">
                        {t(item.titleKey)}
                      </div>
                      <div className="mt-0.5 text-[12px] leading-4 text-[var(--text-secondary)]">
                        {t(item.descKey)}
                      </div>
                    </div>
                    <Kbd size="md">
                      {isMac ? item.mac : item.other}
                    </Kbd>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}

        {matchingSections.length === 0 && (
          <div className="flex min-h-[160px] items-center justify-center text-[13px] text-[var(--text-tertiary)]">
            {t('keybindings.no_results')}
          </div>
        )}
      </div>
    </div>
  )
}
