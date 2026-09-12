import { useEffect, useState } from 'react'
import { ControlMenu } from '@/components/control-menu'
import {
  applyThemeChoice,
  readThemeChoice,
  type ThemeChoice,
} from '@/lib/appearance'
import {
  APP_LANGUAGES,
  languageLabel,
  useI18n,
} from '@/lib/i18n'
import { cn } from '@/lib/utils'
import { SettingText } from './shared'

export function AppearanceSettings() {
  const { language, locale, setLanguage, t } = useI18n()
  const [theme, setTheme] = useState<ThemeChoice>(() => typeof window === 'undefined'
    ? 'system'
    : readThemeChoice(window.localStorage))
  useEffect(() => {
    const systemAppearance = window.matchMedia('(prefers-color-scheme: dark)')
    const apply = () => applyThemeChoice(document.documentElement, theme, systemAppearance.matches)
    apply()
    window.localStorage.setItem('padu.theme', theme)
    systemAppearance.addEventListener('change', apply)
    return () => systemAppearance.removeEventListener('change', apply)
  }, [theme])
  return (
    <div className="mt-[15px] w-full overflow-hidden rounded-[13px] bg-[var(--raised)]">
      <div className="flex flex-col gap-3 px-5 py-4">
        <SettingText title={t('settings.theme')} description={t('settings.theme_description')} />
        <div className="flex w-full flex-wrap justify-end gap-2">
          {(['system', 'light', 'dark'] as ThemeChoice[]).map((choice) => {
            const selected = choice === theme
            return (
              <button
                aria-pressed={selected}
                className={cn(
                  'flex w-full flex-1 max-w-[140px] flex-col rounded-[10px] border p-2 text-left outline-none transition-colors focus-visible:ring-1 focus-visible:ring-ring',
                  selected
                    ? 'border-ring bg-ring/8'
                    : 'border-[var(--border)] bg-background hover:bg-accent',
                )}
                key={choice}
                type="button"
                onClick={() => setTheme(choice)}
              >
                <div className="flex w-full aspect-[188/142] items-center justify-center overflow-hidden rounded-[6px] border border-[var(--border)] bg-background">
                  <img
                    alt=""
                    aria-hidden="true"
                    className="h-full w-full rounded-[6px] object-contain"
                    draggable={false}
                    src={`/themes/${choice}.svg`}
                  />
                </div>
                <span className="mt-2 block text-[12px] font-medium text-foreground">
                  {t(`settings.theme_${choice}`)}
                </span>
              </button>
            )
          })}
        </div>
      </div>
      <div className="mx-5 border-t" />
      <div className="flex min-h-[60px] items-center gap-6 px-5 py-3">
        <SettingText title={t('language.title')} description={t('language.description')} />
        <ControlMenu
          align="right"
          items={APP_LANGUAGES.map((choice) => ({
            id: choice,
            label: languageLabel(choice, locale),
            selected: choice === language,
            onSelect: () => setLanguage(choice),
          }))}
          label={languageLabel(language, locale)}
          menuClassName="w-[170px]"
          placement="below"
          triggerClassName="h-8 w-[150px] max-w-none justify-between border bg-background px-3 text-[12px]"
        />
      </div>
    </div>
  )
}
