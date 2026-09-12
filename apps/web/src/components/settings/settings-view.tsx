import type { Project } from '@padu/client'
import { useEffect, useRef, useState } from 'react'
import { KeybindingsSettings } from '@/components/keybindings-settings'
import { NotificationsSettings } from '@/components/notifications-settings'
import { PaduIcon } from '@/components/padu-icon'
import { SkillsSettings } from '@/components/skills-settings'
import { Kbd } from '@/components/ui/kbd'
import { UsageSettings } from '@/components/usage-settings'
import { useI18n } from '@/lib/i18n'
import { cn } from '@/lib/utils'
import { AboutSettings } from './about-settings'
import { AppearanceSettings } from './appearance-settings'
import { ArchivedSettings } from './archived-settings'
import { DaemonSettings } from './daemon-settings'
import { GeneralSettings } from './general-settings'
import { ProvidersSettings } from './providers-settings'
import { SETTINGS_PAGES, type SettingsPageId } from './types'

export function SettingsView({
  page,
  projects,
  onBack,
  onPageChange,
  onOpenOnboarding,
  onRestoreSession,
}: {
  page: SettingsPageId
  projects: Project[]
  onBack: () => void
  onPageChange: (page: SettingsPageId) => void
  onOpenOnboarding?: () => void
  onRestoreSession?: (sessionId: string) => void
}) {
  const { t } = useI18n()
  const [query, setQuery] = useState('')
  const searchRef = useRef<HTMLInputElement>(null)
  const localizedPages = SETTINGS_PAGES.map((candidate) => ({
    ...candidate,
    localizedLabel: t(candidate.labelKey),
    localizedKeywords: `${candidate.keywords} ${t(candidate.keywordsKey)}`.toLowerCase(),
  }))
  const pages = localizedPages.filter((candidate) =>
    !query.trim() || candidate.localizedKeywords.includes(query.trim().toLowerCase()),
  )
  const activePage = localizedPages.find((candidate) => candidate.id === page)

  useEffect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && query) setQuery('')
    }
    window.addEventListener('keydown', escape)
    return () => window.removeEventListener('keydown', escape)
  }, [query])

  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if (event.key !== '/' || event.metaKey || event.ctrlKey || event.altKey) return
      const target = event.target as HTMLElement | null
      if (
        target instanceof HTMLInputElement
        || target instanceof HTMLTextAreaElement
        || target instanceof HTMLSelectElement
        || target?.isContentEditable
      ) return
      event.preventDefault()
      searchRef.current?.focus()
    }
    window.addEventListener('keydown', focusSearch)
    return () => window.removeEventListener('keydown', focusSearch)
  }, [])

  return (
    <div className="flex h-dvh min-w-0 flex-1 bg-background">
      <aside className="flex h-full w-[252px] shrink-0 flex-col bg-sidebar">
        <div className="flex h-12 flex-none items-center px-3">
          <button
            className="flex h-[26px] flex-none items-center gap-1.5 rounded-md px-1.5 text-[13px] text-[var(--text-secondary)] outline-none hover:bg-sidebar-accent active:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
            type="button"
            onClick={onBack}
          >
            <PaduIcon className="size-[14px] text-[var(--text-tertiary)]" name="arrowLeft" />
            {t('settings.back')}
          </button>
          <div className="min-w-0 flex-1" />
        </div>
        <div className="px-3 pt-2">
          <label className="group flex h-8 items-center gap-2 rounded-lg border bg-[var(--inset)] px-2.5 focus-within:border-ring">
            <PaduIcon className="size-[13px] text-[var(--text-tertiary)]" name="search" />
            <input
              ref={searchRef}
              aria-label={t('settings.search')}
              className="min-w-0 flex-1 bg-transparent text-[12px] outline-none placeholder:text-[var(--text-ghost)]"
              placeholder={t('settings.search')}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (!pages.length || !['ArrowDown', 'ArrowUp'].includes(event.key)) return
                event.preventDefault()
                const current = pages.findIndex((candidate) => candidate.id === page)
                const delta = event.key === 'ArrowDown' ? 1 : -1
                onPageChange(pages[(current + delta + pages.length) % pages.length]!.id)
              }}
            />
            <Kbd className="group-focus-within:hidden" size="xs">/</Kbd>
          </label>
        </div>
        <nav aria-label={t('common.settings')} className="mt-[18px] flex flex-col gap-[3px] px-3">
          {pages.map((candidate) => (
            <button
              aria-current={page === candidate.id ? 'page' : undefined}
              className={cn(
                'flex h-9 items-center gap-2.5 rounded-lg px-[11px] text-[13px] text-[var(--text-secondary)] outline-none hover:bg-sidebar-accent focus-visible:ring-1 focus-visible:ring-ring',
                page === candidate.id && 'bg-sidebar-accent text-foreground',
              )}
              key={candidate.id}
              type="button"
              onClick={() => onPageChange(candidate.id)}
            >
              <PaduIcon className="size-[15px] text-[var(--text-tertiary)]" name={candidate.icon} />
              {candidate.localizedLabel}
            </button>
          ))}
        </nav>
      </aside>
      <main className={cn(
        'min-w-0 flex-1 border-l bg-background',
        page === 'skills' ? 'overflow-hidden' : 'overflow-y-auto pb-12',
      )}>
        {page === 'skills' ? (
          <SkillsSettings projects={projects} />
        ) : (
          <>
            <div className="sticky top-0 z-10 flex h-12 items-center bg-background px-6 text-sm">
              <span className="text-[var(--text-tertiary)]">{t('common.settings')}</span>
              <span className="mx-1.5 text-[var(--text-ghost)]">/</span>
              <span className="text-[var(--text-secondary)]">{activePage?.localizedLabel}</span>
            </div>
            <div className="px-8 pt-5">
              <div className={cn('mx-auto w-full', page === 'usage' ? 'max-w-[1024px]' : 'max-w-[760px]')}>
                <h1 className="text-[18px] font-medium">{activePage?.localizedLabel}</h1>
                {page === 'general' && <GeneralSettings onOpenOnboarding={onOpenOnboarding} />}
                {page === 'appearance' && <AppearanceSettings />}
                {page === 'keybindings' && <KeybindingsSettings />}
                {page === 'notifications' && <NotificationsSettings />}
                {page === 'providers' && <ProvidersSettings />}
                {page === 'archived' && <ArchivedSettings projects={projects} onRestoreSession={onRestoreSession} />}
                {page === 'usage' && <UsageSettings projects={projects} />}
                {page === 'daemon' && <DaemonSettings />}
                {page === 'about' && <AboutSettings onPageChange={onPageChange} />}
              </div>
            </div>
          </>
        )}
      </main>
    </div>
  )
}
