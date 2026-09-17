import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { toast } from 'sonner'
import { ControlMenu } from '@/components/control-menu'
import { MarkdownView } from '@/components/markdown-view'
import { PaduIcon } from '@/components/padu-icon'
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
  UnsupportedConversationBackgroundError,
  OversizedConversationBackgroundError,
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
  const opacityPercent = Math.round(background.opacity * 100)
  const heightPercent = Math.round(background.heightPercent)
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
    <div className="mt-3 flex w-full items-start gap-4">
      <div className="flex w-[190px] shrink-0 flex-col gap-3">
        <input
          ref={fileInput}
          accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml,image/bmp,image/tiff,image/x-icon,image/x-portable-anymap"
          className="hidden"
          type="file"
          onChange={(event) => {
            const file = event.target.files?.[0]
            event.currentTarget.value = ''
            if (file) void chooseConversationBackground(file).catch((error: unknown) => {
              toast.error(t(error instanceof UnsupportedConversationBackgroundError
                ? 'settings.background_unsupported'
                : error instanceof OversizedConversationBackgroundError
                ? 'settings.background_too_large'
                : 'settings.background_unavailable'))
            })
          }}
        />
        <button className="flex justify-start items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring" type="button" onClick={() => fileInput.current?.click()}>
          <PaduIcon className="size-3.5" name="folderOpen" />
          {t('settings.background_choose')}
        </button>
        {background.imageUrl && <button className="flex items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-xs text-[var(--text-secondary)] hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring" type="button" onClick={() => void clearConversationBackground().catch(() => toast.error(t('settings.background_unavailable')))}>
          <PaduIcon className="size-3.5 text-destructive" name="trash" />
          {t('settings.background_remove')}
        </button>}
        {background.fileName && <span className="truncate text-xs text-[var(--text-secondary)]" title={background.fileName}>{background.fileName}</span>}
        <div className="my-0.5 h-px w-full bg-[var(--border)]" />
        <label className="space-y-1 text-xs">
          <span className="flex items-center justify-between gap-2">
            <span className="text-[var(--text-secondary)]">{t('settings.background_opacity')}</span>
            <span className="text-foreground">{Math.round(background.opacity * 100)}%</span>
          </span>
          <input aria-label={t('settings.background_opacity')} className="padu-slider w-full cursor-pointer" max="100" min="0" step="1" style={{ '--slider-progress': `${opacityPercent}%` } as CSSProperties} type="range" value={opacityPercent} onChange={(event) => updateConversationBackground({ opacity: Number(event.target.value) / 100 })} />
        </label>
        <label className="space-y-1 text-xs">
          <span className="flex items-center justify-between gap-2">
            <span className="text-[var(--text-secondary)]">{t('settings.background_height')}</span>
            <span className="text-foreground">{Math.round(background.heightPercent)}%</span>
          </span>
          <input aria-label={t('settings.background_height')} className="padu-slider w-full cursor-pointer" max="100" min="20" step="1" style={{ '--slider-progress': `${((heightPercent - 20) / 80) * 100}%` } as CSSProperties} type="range" value={heightPercent} onChange={(event) => updateConversationBackground({ heightPercent: Number(event.target.value) })} />
        </label>
      </div>
      <div className="@container relative aspect-[188/142] min-h-[240px] min-w-0 flex-1 overflow-hidden rounded-lg border bg-[var(--inset)]">
        {background.imageUrl && <img alt="" aria-hidden="true" className="absolute inset-x-0 top-0 h-auto w-full object-cover object-top" src={background.imageUrl} style={{ height: `${heightPercent}%`, opacity: background.opacity }} />}
        <div
          className="absolute inset-x-0 top-0 z-[1] bg-gradient-to-b from-transparent to-background/95"
          style={{ height: `${heightPercent}%` }}
        />
        <div className="relative z-[2] flex h-full flex-col justify-end gap-[clamp(4px,1cqw,12px)] p-[clamp(10px,4cqw,24px)] text-[clamp(9px,2cqw,14px)] leading-[clamp(13px,3cqw,20px)]">
          <div className="flex w-full justify-end">
            <div className="max-w-[82%] min-w-0 rounded-xl bg-[var(--raised)] px-[clamp(8px,2cqw,12px)] py-[clamp(6px,1.5cqw,8px)]">
              <MarkdownView compact text={t('settings.background_preview_user')} />
            </div>
          </div>
          <div className="max-w-[86%] min-w-0 py-[clamp(2px,0.75cqw,4px)]">
            <MarkdownView compact text={t('settings.background_preview_assistant')} />
          </div>
        </div>
      </div>
    </div>
  </section>
    </>
  )
}
