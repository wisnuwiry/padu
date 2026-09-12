import type { PaduIconName } from '@/components/padu-icon'

export type SettingsPageId =
  | 'general'
  | 'appearance'
  | 'keybindings'
  | 'notifications'
  | 'providers'
  | 'skills'
  | 'usage'
  | 'daemon'
  | 'archived'
  | 'about'

export const SETTINGS_PAGES: Array<{
  id: SettingsPageId
  label: string
  labelKey: string
  icon: PaduIconName
  keywords: string
  keywordsKey: string
}> = [
  { id: 'general', label: 'General', labelKey: 'settings.general', icon: 'settings', keywords: 'general local projects conversations privacy analytics telemetry anonymous sharing', keywordsKey: 'settings.general_keywords' },
  { id: 'appearance', label: 'Appearance', labelKey: 'settings.appearance', icon: 'appearance', keywords: 'appearance theme system light dark language', keywordsKey: 'settings.appearance_keywords' },
  { id: 'keybindings', label: 'Keybindings', labelKey: 'settings.keybindings', icon: 'command', keywords: 'keybindings keyboard shortcuts hotkeys bindings shortcuts keys commands', keywordsKey: 'settings.keybindings_keywords' },
  { id: 'notifications', label: 'Notifications', labelKey: 'settings.notifications', icon: 'bell', keywords: 'notifications sound alerts audio banner permission prompt notify test chime task complete', keywordsKey: 'settings.notifications_keywords' },
  { id: 'providers', label: 'Providers', labelKey: 'settings.providers', icon: 'bot', keywords: 'providers agents models cli version install detect claude codex cursor opencode amp grok pi omp oh my pi kimi', keywordsKey: 'settings.providers_keywords' },
  { id: 'skills', label: 'Skills', labelKey: 'settings.skills', icon: 'package', keywords: 'skills library agent disable enable delete shared', keywordsKey: 'settings.skills_keywords' },
  { id: 'usage', label: 'Usage', labelKey: 'settings.usage', icon: 'chartColumn', keywords: 'usage tokens cost spend cache daily monthly project model history', keywordsKey: 'settings.usage_keywords' },
  { id: 'daemon', label: 'Hosts & Daemon', labelKey: 'settings.daemon', icon: 'server', keywords: 'hosts host remote server devbox cloud daemon web network connection url token websocket ssh lan', keywordsKey: 'settings.daemon_keywords' },
  { id: 'archived', label: 'Archived', labelKey: 'settings.archived', icon: 'archive', keywords: 'archived archive hidden conversations tasks restore', keywordsKey: 'settings.archived_keywords' },
  { id: 'about', label: 'About', labelKey: 'settings.about', icon: 'info', keywords: 'about info version update host connected github sponsor repo contribute', keywordsKey: 'settings.about_keywords' },
]

export function isSettingsPageId(value: string): value is SettingsPageId {
  return SETTINGS_PAGES.some((page) => page.id === value)
}
