import { useEffect, useRef, useState } from 'react'
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
import {
  chooseConversationBackground,
  clearConversationBackground,
  updateConversationBackground,
  useConversationBackground,
} from '@/lib/conversation-background'
import { cn } from '@/lib/utils'
import { SettingText } from './shared'

export function AppearanceSettings() {
  const { language, locale, setLanguage, t } = useI18n()
  const [theme, setTheme] = useState<ThemeChoice>(() => typeof window === 'undefined'
    ? 'system'
    : readThemeChoice(window.localStorage))
  const background = useConversationBackground()
  const fileInput = useRef<HTMLInputElement>(null)
  useEffect(() => {
    const systemAppearance = window.matchMedia('(prefers-color-scheme: dark)')
    const apply = () => applyThemeChoice(document.documentElement, theme, systemAppearance.matches)
    apply()
    window.localStorage.setItem('padu.theme', theme)
    systemAppearance.addEventListener('change', apply)
    return () => systemAppearance.removeEventListener('change', apply)
  }, [theme])
  return (
    <>
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
  <section className="mt-[15px] w-full rounded-[13px] bg-[var(--raised)] px-5 py-4">
    <SettingText title={t('settings.background')} description={t('settings.background_description')} />
    <div className="mt-4 flex items-start gap-4 overflow-hidden rounded-lg border bg-background p-4">
      <div className="flex w-44 shrink-0 flex-col gap-3">
        <input
          ref={fileInput}
          accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml,image/bmp,image/tiff,image/x-icon,image/x-portable-anymap"
          className="hidden"
          type="file"
          onChange={(event) => {
            const file = event.target.files?.[0]
            event.currentTarget.value = ''
            if (file) void chooseConversationBackground(file).catch(() => undefined)
          }}
        />
        <button className="rounded-md border px-3 py-2 text-xs outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring" type="button" onClick={() => fileInput.current?.click()}>
          {t('settings.background_choose')}
        </button>
        {background.imageUrl && <button className="rounded-md px-2 py-1.5 text-left text-xs text-[var(--text-secondary)] hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring" type="button" onClick={() => void clearConversationBackground()}>{t('settings.background_remove')}</button>}
        {background.fileName && <span className="truncate text-xs text-[var(--text-secondary)]" title={background.fileName}>{background.fileName}</span>}
        <label className="space-y-1 text-xs">
          <span className="block text-[var(--text-secondary)]">{t('settings.background_opacity')} · {Math.round(background.opacity * 100)}%</span>
          <input aria-label={t('settings.background_opacity')} className="w-full accent-[var(--accent)]" max="100" min="0" step="1" type="range" value={Math.round(background.opacity * 100)} onChange={(event) => updateConversationBackground({ opacity: Number(event.target.value) / 100 })} />
        </label>
        <label className="space-y-1 text-xs">
          <span className="block text-[var(--text-secondary)]">{t('settings.background_height')} · {Math.round(background.heightPercent)}%</span>
          <input aria-label={t('settings.background_height')} className="w-full accent-[var(--accent)]" max="100" min="20" step="1" type="range" value={background.heightPercent} onChange={(event) => updateConversationBackground({ heightPercent: Number(event.target.value) })} />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          <span className="text-[var(--text-secondary)]">{t('settings.background_fit')}</span>
          <select className="rounded-md border bg-background px-2 py-1 outline-none focus-visible:ring-1 focus-visible:ring-ring" value={background.fit} onChange={(event) => updateConversationBackground({ fit: event.target.value as 'cover' | 'contain' })}>
            <option value="cover">{t('settings.background_cover')}</option>
            <option value="contain">{t('settings.background_contain')}</option>
          </select>
        </label>
      </div>
      <div className="relative aspect-[188/142] min-h-[260px] min-w-0 flex-1 overflow-hidden rounded-lg border bg-[var(--inset)]">
        {background.imageUrl && <img alt="" aria-hidden="true" className="absolute inset-x-0 top-0 w-full rounded-lg object-cover object-top" src={background.imageUrl} style={{ height: `${background.heightPercent}%`, opacity: background.opacity, objectFit: background.fit }} />}
        <div className="absolute inset-x-0 bottom-0 z-[1] h-[58%] bg-gradient-to-b from-transparent to-background/95" />
        <div className="relative z-[2] flex h-full flex-col justify-end gap-3 p-6 text-xs">
          <div className="max-w-[72%] rounded-2xl rounded-tl-md bg-background/95 px-4 py-3 shadow-sm">
            <div className="mb-1 text-[10px] font-medium text-[var(--text-tertiary)]">You</div>
            {t('settings.background_preview_user')}
          </div>
          <div className="max-w-[78%] self-end rounded-2xl rounded-tr-md bg-card/95 px-4 py-3 shadow-sm">
            <div className="mb-1 text-[10px] font-medium text-[var(--text-tertiary)]">Padu</div>
            {t('settings.background_preview_assistant')}
          </div>
          <div className="max-w-[64%] rounded-2xl rounded-tl-md bg-background/95 px-4 py-3 shadow-sm">{t('settings.background_preview_user')}</div>
        </div>
      </div>
    </div>
  </section>
    </>
  )
}
