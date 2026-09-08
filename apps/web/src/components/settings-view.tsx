import { useQueryClient } from '@tanstack/react-query'
import type {
  DaemonSettings,
  HostProfile,
  Project,
  ProviderKind,
} from '@padu/client'
import { useEffect, useState, type ReactNode } from 'react'
import { toast } from 'sonner'
import { ControlMenu } from '@/components/control-menu'
import { HostDialog } from '@/components/host-dialog'
import { KeybindingsSettings } from '@/components/keybindings-settings'
import { NotificationsSettings } from '@/components/notifications-settings'
import { SkillsSettings } from '@/components/skills-settings'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { UsageSettings } from '@/components/usage-settings'
import { ProviderIcon, PROVIDERS, PaduIcon, type PaduIconName } from '@/components/padu-icon'
import {
  useDaemonSettings,
  useProviderProbes,
  useTaskState,
} from '@/hooks/use-daemon-data'
import { useCopyFeedback } from '@/hooks/use-copy-feedback'
import {
  authenticateAgy,
  checkAgyAuth,
  daemonKeys,
  displayTitle,
  installAgyAcp,
  logoutAgy,
  removeAgyAcp,
  setSessionArchived,
  updateDaemonSettings,
} from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
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

export type SettingsPageId =
  | 'general'
  | 'appearance'
  | 'keybindings'
  | 'notifications'
  | 'providers'
  | 'skills'
  | 'archived'
  | 'usage'
  | 'daemon'
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
  { id: 'archived', label: 'Archived', labelKey: 'settings.archived', icon: 'package', keywords: 'archived archive hidden conversations tasks restore', keywordsKey: 'settings.archived_keywords' },
  { id: 'usage', label: 'Usage', labelKey: 'settings.usage', icon: 'chartColumn', keywords: 'usage tokens cost spend cache daily monthly project model history', keywordsKey: 'settings.usage_keywords' },
  { id: 'daemon', label: 'Hosts & Daemon', labelKey: 'settings.daemon', icon: 'server', keywords: 'hosts host remote server devbox cloud daemon web network connection url token websocket ssh lan', keywordsKey: 'settings.daemon_keywords' },
  { id: 'about', label: 'About', labelKey: 'settings.about', icon: 'info', keywords: 'about info version update host connected github sponsor repo contribute', keywordsKey: 'settings.about_keywords' },
]

export function isSettingsPageId(value: string): value is SettingsPageId {
  return SETTINGS_PAGES.some((page) => page.id === value)
}

export function SettingsView({
  page,
  projects,
  onBack,
  onPageChange,
  onOpenOnboarding,
}: {
  page: SettingsPageId
  projects: Project[]
  onBack: () => void
  onPageChange: (page: SettingsPageId) => void
  onOpenOnboarding?: () => void
}) {
  const { t } = useI18n()
  const [query, setQuery] = useState('')
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

  return (
    <div className="flex h-dvh min-w-0 flex-1 bg-background">
      <aside className="flex h-full w-[252px] shrink-0 flex-col bg-sidebar pt-3">
        <div className="px-3">
          <button
            className="flex h-[34px] w-full items-center gap-[9px] rounded-lg px-[9px] text-[13px] text-[var(--text-secondary)] outline-none hover:bg-sidebar-accent active:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
            type="button"
            onClick={onBack}
          >
            <PaduIcon className="size-[15px] text-[var(--text-tertiary)]" name="arrowLeft" />
            {t('settings.back')}
          </button>
        </div>
        <div className="px-3 pt-2">
          <label className="flex h-8 items-center gap-2 rounded-lg border bg-[var(--inset)] px-2.5 focus-within:border-ring">
            <PaduIcon className="size-[13px] text-[var(--text-tertiary)]" name="search" />
            <input
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
        page === 'skills' ? 'overflow-hidden' : 'overflow-y-auto px-8 pb-12 pt-5',
      )}>
        {page === 'skills' ? (
          <SkillsSettings projects={projects} />
        ) : (
          <div className={cn('mx-auto w-full', page === 'usage' ? 'max-w-[1024px]' : 'max-w-[760px]')}>
            <h1 className="text-[18px] font-medium">{activePage?.localizedLabel}</h1>
            {page === 'general' && <GeneralSettings onOpenOnboarding={onOpenOnboarding} />}
            {page === 'appearance' && <AppearanceSettings />}
            {page === 'keybindings' && <KeybindingsSettings />}
            {page === 'notifications' && <NotificationsSettings />}
            {page === 'providers' && <ProvidersSettings />}
            {page === 'archived' && <ArchivedSettings />}
            {page === 'usage' && <UsageSettings projects={projects} />}
            {page === 'daemon' && <DaemonSettings />}
            {page === 'about' && <AboutSettings onPageChange={onPageChange} />}
          </div>
        )}
      </main>
    </div>
  )
}

function ArchivedSettings() {
  const { t } = useI18n()
  const { client, config } = useDaemon()
  const queryClient = useQueryClient()
  const taskState = useTaskState()
  const archived = (taskState.data?.sessions.filter((session) => Boolean(session.archived_at)) ?? [])
    .slice()
    .sort((left, right) => (right.archived_at ?? 0) - (left.archived_at ?? 0))

  async function restore(sessionId: string) {
    if (!client || !config) return
    await setSessionArchived(client, sessionId, false)
    queryClient.setQueryData(daemonKeys.taskState(config.address), (current: typeof taskState.data) => current && ({
      ...current,
      sessions: current.sessions.map((session) => session.id === sessionId
        ? { ...session, archived_at: null }
        : session),
    }))
  }

  return (
    <div className="mt-[15px] overflow-hidden rounded-[13px] bg-[var(--raised)]">
      {archived.length === 0 ? (
        <p className="px-5 py-5 text-[13px] text-[var(--text-tertiary)]">{t('settings.archived_empty')}</p>
      ) : archived.map((session, index) => (
        <div className={cn('flex min-h-[58px] items-center gap-3 px-5 py-3', index > 0 && 'border-t')} key={session.id}>
          <PaduIcon className="size-4 shrink-0 text-[var(--text-tertiary)]" name="package" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-medium">{displayTitle(session)}</div>
            <div className="truncate text-[11.5px] text-[var(--text-tertiary)]">{session.id}</div>
          </div>
          <Button size="sm" variant="outline" onClick={() => void restore(session.id)}>{t('settings.restore')}</Button>
        </div>
      ))}
    </div>
  )
}

function GeneralSettings({ onOpenOnboarding }: { onOpenOnboarding?: () => void }) {
  const { t } = useI18n()
  const [analytics, setAnalytics] = useStoredBoolean('padu.analytics-enabled', true)
  return (
    <div>
      <SettingsCard>
        <SettingText
          title={t('settings.local_by_default')}
          description={t('settings.local_by_default_web_description')}
        />
      </SettingsCard>
      <SettingsCard row>
        <SettingText
          title={t('settings.share_anonymous_usage_data')}
          description={t('settings.share_anonymous_usage_data_description')}
        />
        <Toggle checked={analytics} label={t('settings.share_anonymous_usage_data')} onChange={setAnalytics} />
      </SettingsCard>
      {onOpenOnboarding && (
        <SettingsCard row>
          <SettingText
            title={t('onboarding.command_title')}
            description={t('onboarding.welcome_subtitle')}
          />
          <button
            className="flex h-8 shrink-0 items-center justify-center rounded-lg bg-accent px-3.5 text-[12.5px] font-medium text-foreground outline-none hover:bg-accent/80 active:opacity-80 focus-visible:ring-1 focus-visible:ring-ring"
            type="button"
            onClick={onOpenOnboarding}
          >
            {t('onboarding.replay_button')}
          </button>
        </SettingsCard>
      )}
    </div>
  )
}

function AppearanceSettings() {
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

function ProvidersSettings() {
  const { t } = useI18n()
  const { client, config } = useDaemon()
  const queryClient = useQueryClient()
  const settings = useDaemonSettings()
  const probes = useProviderProbes()
  const [expanded, setExpanded] = useState<ProviderKind | null>(null)
  const [paths, setPaths] = useState<Partial<Record<ProviderKind, string>>>({})
  const [installingAgy, setInstallingAgy] = useState(false)
  const [agyInstallPercent, setAgyInstallPercent] = useState(0)
  const [agyAuthenticated, setAgyAuthenticated] = useState(false)
  useEffect(() => {
    if (!client) return
    void checkAgyAuth(client).then(setAgyAuthenticated).catch(() => setAgyAuthenticated(false))
    return client.subscribeProviderInstallProgress((progress) => {
      if (progress.provider === 'agy') setAgyInstallPercent(progress.percent)
    })
  }, [client])
  const checkedAt = Math.max(
    0,
    ...Object.values(probes.states).map((state) => state.dataUpdatedAt),
  )

  async function apply(next: DaemonSettings) {
    if (!client || !config) return
    try {
      await updateDaemonSettings(client, next)
      queryClient.setQueryData(daemonKeys.settings(config.address), next)
      await queryClient.invalidateQueries({ queryKey: daemonKeys.providers(config.address) })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  function applyProviderPath(provider: ProviderKind, value: string) {
    if (!settings.data) return
    const overrides = { ...(settings.data.provider_binary_overrides ?? {}) }
    const trimmed = value.trim()
    if (trimmed) overrides[provider] = trimmed
    else delete overrides[provider]
    setPaths((current) => ({ ...current, [provider]: trimmed }))
    void apply({ ...settings.data, provider_binary_overrides: overrides })
  }

  function toggleExpandedProvider(provider: ProviderKind) {
    if (expanded) {
      const pending = paths[expanded]
      const applied = settings.data?.provider_binary_overrides?.[expanded] ?? ''
      if (pending !== undefined && pending.trim() !== applied) {
        applyProviderPath(expanded, pending)
      }
    }
    if (expanded !== provider) {
      setPaths((current) => ({
        ...current,
        [provider]: settings.data?.provider_binary_overrides?.[provider] ?? '',
      }))
    }
    setExpanded(expanded === provider ? null : provider)
  }

  return (
    <div className="mt-[15px] overflow-hidden rounded-[13px] bg-[var(--raised)] px-5 py-[14px]">
      <div className="flex items-start gap-5">
        <div className="min-w-0 flex-1">
          <div className="text-[13.5px] font-medium">{t('providers.coding_agents')}</div>
          <p className="mt-[5px] text-[12px] leading-[18px] text-[var(--text-secondary)]">
            {t('providers.web_description')}
          </p>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1.5">
          <button
            className="flex h-7 items-center gap-1.5 rounded-[7px] border border-input px-[11px] text-[10.5px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-60"
            disabled={probes.isFetching}
            type="button"
            onClick={() => {
              if (!config) return
              void queryClient.invalidateQueries({ queryKey: daemonKeys.providers(config.address) })
            }}
          >
            <PaduIcon className={cn('size-[11px] text-[var(--text-tertiary)]', probes.isFetching && 'motion-safe:animate-spin')} name="rotateCw" />
            {probes.isFetching ? t('common.checking') : t('common.refresh')}
          </button>
          {!probes.isFetching && checkedAt > 0 && (
            <span className="text-[9.5px] text-[var(--text-ghost)]">{providerCheckedLabel(checkedAt, t)}</span>
          )}
        </div>
      </div>
      <div className="mt-1 flex flex-col">
        {PROVIDERS.map((provider) => {
          const probe = probes.data[provider.id]
          const probeState = probes.states[provider.id]
          const installed = probe?.installed ?? false
          const disabled = settings.data?.disabled_providers.includes(provider.id) ?? false
          const open = expanded === provider.id
          const detail = providerProbeDetail(provider.command, probe, probeState, disabled, t)
          const dotColor = probeState.error
            ? 'bg-[var(--warning)]'
            : !installed
              ? 'bg-[var(--text-ghost)]'
              : disabled
                ? 'bg-[var(--warning)]'
                : 'bg-[var(--success)]'
          return (
            <div className="border-b last:border-0" key={provider.id}>
              <div className="flex items-center gap-3 py-[11px]">
                <span className="relative grid size-[30px] shrink-0 place-items-center rounded-[7px] bg-accent">
                  <ProviderIcon className={cn('size-4', !installed && 'opacity-50')} provider={provider.id} />
                  <span className={cn('absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-[var(--raised)]', dotColor)} />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex items-baseline gap-[7px]">
                    <span className={cn('truncate text-[12.5px] font-medium', !installed && 'text-[var(--text-secondary)]')}>{provider.name}</span>
                    {probe?.version && (
                      <span className="shrink-0 font-mono text-[10px] text-[var(--text-tertiary)]">v{probe.version}</span>
                    )}
                  </span>
                  <span className="mt-[3px] block truncate text-[10.5px] text-[var(--text-tertiary)]" title={detail}>{detail}</span>
                </span>
                <button
                  aria-expanded={open}
                  aria-label={t(open ? 'providers.hide_settings' : 'providers.show_settings', { provider: provider.name })}
                  className="grid size-7 shrink-0 place-items-center rounded-[7px] text-[var(--text-tertiary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
                  type="button"
                  onClick={() => toggleExpandedProvider(provider.id)}
                >
                  <PaduIcon className="size-2.5" name={open ? 'chevronDown' : 'chevronRight'} />
                </button>
                {installed && (
                  <Toggle
                    checked={!disabled}
                    label={t(disabled ? 'providers.enable' : 'providers.disable', { provider: provider.name })}
                    onChange={(enabled) => {
                      if (!settings.data) return
                      const disabledProviders = enabled
                        ? settings.data.disabled_providers.filter((kind) => kind !== provider.id)
                        : [...new Set([...settings.data.disabled_providers, provider.id])]
                      void apply({ ...settings.data, disabled_providers: disabledProviders })
                    }}
                  />
                )}
              </div>
              {open && settings.data && (
                <div className="mb-[11px] ml-[42px] flex flex-col gap-[5px]">
                  <label className="text-[11.5px] font-medium">{t('providers.binary_path')}</label>
                  <p className="text-[10.5px] leading-[15px] text-[var(--text-tertiary)]">
                    {t('providers.binary_path_description', { provider: provider.shortName })}
                  </p>
                  <div className="mt-[3px] flex flex-wrap items-center gap-2">
                    {provider.id === 'agy' && !installed && (
                      <button
                        className="flex h-[29px] shrink-0 items-center gap-1.5 rounded-[7px] border border-input px-2.5 text-[10.5px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-60"
                        disabled={installingAgy}
                        type="button"
                        onClick={async () => {
                          if (!client || !config) return
                          setInstallingAgy(true)
                          setAgyInstallPercent(0)
                          try {
                            await installAgyAcp(client)
                            await queryClient.invalidateQueries({ queryKey: daemonKeys.settings(config.address) })
                            await queryClient.invalidateQueries({ queryKey: daemonKeys.providers(config.address) })
                          } catch (error) {
                            // The button already communicates the active download;
                            // keep failures as a neutral notification rather than an
                            // error alert that looks like an interactive prompt.
                            toast(errorMessage(error))
                          } finally {
                            setInstallingAgy(false)
                          }
                        }}
                      >
                        {installingAgy && (
                          <PaduIcon className="size-3 motion-safe:animate-spin motion-reduce:animate-none" name="loaderCircle" />
                        )}
                        {installingAgy
                          ? t('providers.agy_downloading', { percent: agyInstallPercent })
                          : t('providers.agy_install_acp')}
                      </button>
                    )}
                    {provider.id === 'agy' && installed && (
                      <div className="order-2 flex w-full items-center gap-1.5">
                        <button
                          className="h-[29px] rounded-[7px] border border-input px-2.5 text-[10.5px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-60"
                          disabled={installingAgy}
                          type="button"
                          onClick={async () => {
                            if (!client || !config) return
                            if (agyAuthenticated && !window.confirm(t('providers.agy_sign_out_confirm'))) return
                            setInstallingAgy(true)
                            try {
                              if (agyAuthenticated) {
                                await logoutAgy(client)
                                setAgyAuthenticated(false)
                                toast.success(t('providers.agy_signed_out'))
                              } else {
                                await authenticateAgy(client)
                                setAgyAuthenticated(true)
                                toast.success(t('providers.agy_signed_in'))
                              }
                            } catch (error) {
                              toast(errorMessage(error))
                            } finally {
                              setInstallingAgy(false)
                            }
                          }}
                        >
                          {agyAuthenticated ? t('providers.agy_sign_out') : t('providers.agy_sign_in')}
                        </button>
                        <button
                          className="h-[29px] rounded-[7px] border border-input px-2.5 text-[10.5px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-60"
                          disabled={installingAgy}
                          type="button"
                          onClick={async () => {
                            if (!client || !config) return
                            setInstallingAgy(true)
                            setAgyInstallPercent(0)
                            try {
                              await installAgyAcp(client)
                              await queryClient.invalidateQueries({ queryKey: daemonKeys.settings(config.address) })
                              await queryClient.invalidateQueries({ queryKey: daemonKeys.providers(config.address) })
                            } catch (error) {
                              toast(errorMessage(error))
                            } finally {
                              setInstallingAgy(false)
                            }
                          }}
                        >
                          {installingAgy
                            ? t('providers.agy_downloading', { percent: agyInstallPercent })
                            : t('providers.agy_reinstall')}
                        </button>
                        <button
                          className="h-[29px] rounded-[7px] border border-input px-2.5 text-[10.5px] text-destructive outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-60"
                          disabled={installingAgy}
                          type="button"
                          onClick={async () => {
                            if (!client || !config || !window.confirm(t('providers.agy_remove_confirm'))) return
                            setInstallingAgy(true)
                            try {
                              await removeAgyAcp(client)
                              setAgyAuthenticated(false)
                              toast.success(t('providers.agy_removed'))
                              await queryClient.invalidateQueries({ queryKey: daemonKeys.settings(config.address) })
                              await queryClient.invalidateQueries({ queryKey: daemonKeys.providers(config.address) })
                            } catch (error) {
                              toast(errorMessage(error))
                            } finally {
                              setInstallingAgy(false)
                            }
                          }}
                        >
                          {t('providers.agy_remove_download')}
                        </button>
                      </div>
                    )}
                    <Input
                      autoFocus
                      className="h-[29px] max-w-[430px] flex-1 bg-[var(--inset)] font-mono text-[11px]"
                      placeholder={t('input.detected_automatically')}
                      value={paths[provider.id] ?? settings.data.provider_binary_overrides?.[provider.id] ?? ''}
                      onChange={(event) => setPaths((current) => ({ ...current, [provider.id]: event.target.value }))}
                      onKeyDown={(event) => {
                        if (event.key !== 'Enter') return
                        applyProviderPath(provider.id, event.currentTarget.value)
                      }}
                    />
                    {settings.data.provider_binary_overrides?.[provider.id] && (
                      <button
                        className="h-[29px] shrink-0 rounded-[7px] border border-input px-2.5 text-[10.5px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
                        type="button"
                        onClick={() => applyProviderPath(provider.id, '')}
                      >
                        {t('common.reset')}
                      </button>
                    )}
                  </div>
                  <p className="truncate text-[10px] text-[var(--text-ghost)]" title={providerProbeCaption(provider.command, probe, Boolean(settings.data.provider_binary_overrides?.[provider.id]), t)}>
                    {providerProbeCaption(provider.command, probe, Boolean(settings.data.provider_binary_overrides?.[provider.id]), t)}
                  </p>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function providerProbeDetail(
  command: string,
  probe: ReturnType<typeof useProviderProbes>['data'][ProviderKind],
  state: ReturnType<typeof useProviderProbes>['states'][ProviderKind],
  disabled: boolean,
  t: Translator,
) {
  if (state.isPending) return t('common.checking')
  if (state.error) return t('providers.check_failed', { command, error: errorMessage(state.error) })
  if (!probe?.installed) return t('providers.not_detected_as', { command })
  const parts = []
  if (probe.path) parts.push(abbreviateHomePath(probe.path))
  if (disabled) parts.push(t('providers.disabled_for_new_tasks'))
  else if (probe.models.length) parts.push(t(
    probe.models.length === 1 ? 'providers.model_count_one' : 'providers.model_count_many',
    { count: probe.models.length },
  ))
  return parts.join('  ·  ') || t('providers.detected_as', { command })
}

function providerProbeCaption(
  command: string,
  probe: ReturnType<typeof useProviderProbes>['data'][ProviderKind],
  hasOverride: boolean,
  t: Translator,
) {
  if (hasOverride && probe?.installed && probe.path) return t('providers.using_override', { path: probe.path })
  if (hasOverride) return t('providers.invalid_override')
  if (probe?.installed && probe.path) return t('providers.detected_at', { path: probe.path })
  return t('providers.searches_path', { command })
}

function providerCheckedLabel(updatedAt: number, t: Translator) {
  const seconds = Math.max(0, Math.floor((Date.now() - updatedAt) / 1_000))
  if (seconds < 60) return t('providers.checked_just_now')
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return t('providers.checked_minutes_ago', { count: minutes })
  const hours = Math.floor(minutes / 60)
  return t('providers.checked_hours_ago', { count: hours })
}

type Translator = (key: string, params?: Record<string, string | number>) => string

function abbreviateHomePath(path: string) {
  return path
    .replace(/^\/Users\/[^/]+(?=\/|$)/, '~')
    .replace(/^\/home\/[^/]+(?=\/|$)/, '~')
    .replace(/^\/root(?=\/|$)/, '~')
}

function formatHostLastConnected(
  lastConnectedAt: number | null | undefined,
  t: (key: string, params?: Record<string, string | number>) => string,
) {
  if (!lastConnectedAt) {
    return t('host.never_connected')
  }
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - lastConnectedAt))
  let timeStr: string
  if (seconds < 90) {
    timeStr = t('providers.checked_just_now')
  } else if (seconds < 3600) {
    timeStr = t('providers.checked_minutes_ago', { count: Math.floor(seconds / 60) })
  } else if (seconds < 86400) {
    timeStr = t('providers.checked_hours_ago', { count: Math.floor(seconds / 3600) })
  } else {
    timeStr = `${Math.floor(seconds / 86400)}d ago`
  }
  return t('host.last_connected', { time: timeStr })
}

function RemoteHostsCard() {
  const { t } = useI18n()
  const { hosts, activeHostId, switchHost, addHost, updateHost, removeHost } = useDaemon()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingHost, setEditingHost] = useState<HostProfile | null>(null)

  return (
    <SettingsCard>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="text-[13.5px] font-medium text-foreground">{t('host.remote_hosts')}</div>
          <div className="mt-1 text-[12.5px] leading-[18px] text-[var(--text-secondary)]">
            {t('host.remote_hosts_description')}
          </div>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="shrink-0 gap-1.5"
          onClick={() => {
            setEditingHost(null)
            setDialogOpen(true)
          }}
        >
          <PaduIcon className="size-3 text-[var(--text-secondary)]" name="plus" />
          {t('host.add_host')}
        </Button>
      </div>

      {hosts.length === 0 ? (
        <div className="mt-3.5 flex flex-col items-center justify-center gap-2 rounded-lg border border-border bg-[var(--overlay)]/40 px-4 py-6 text-center">
          <div className="flex size-9 items-center justify-center rounded-full bg-[var(--overlay)]">
            <PaduIcon className="size-4.5 text-[var(--text-tertiary)]" name="server" />
          </div>
          <p className="text-[13px] font-medium text-[var(--text-secondary)]">{t('host.no_remote_hosts')}</p>
        </div>
      ) : (
        <div className="mt-3.5 flex flex-col gap-2.5">
          {hosts.map((host) => {
            const isActive = activeHostId === host.id
            const hasToken = Boolean(host.token && host.token.trim().length > 0)
            const lastConnLabel = formatHostLastConnected(host.lastConnectedAt, t)

            return (
              <div
                key={host.id}
                className={cn(
                  'flex items-center justify-between gap-3 rounded-lg border bg-card p-3.5 transition-colors',
                  isActive ? 'border-ring ring-1 ring-ring/20' : 'border-border hover:border-border-strong',
                )}
              >
                <div className="flex min-w-0 flex-1 items-center gap-3">
                  <div className="relative flex size-9 shrink-0 items-center justify-center rounded-lg bg-[var(--overlay)]">
                    <PaduIcon
                      className={cn('size-4', isActive ? 'text-ring' : 'text-[var(--text-secondary)]')}
                      name="server"
                    />
                    <span
                      className={cn(
                        'absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-card',
                        isActive ? 'bg-[var(--success)]' : 'bg-[var(--text-ghost)]',
                      )}
                    />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[13.5px] font-medium text-foreground">
                        {host.name || host.address}
                      </span>
                      {isActive && (
                        <span className="rounded bg-[var(--success-soft)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--success)]">
                          {t('host.active')}
                        </span>
                      )}
                    </div>
                    <div className="truncate font-mono text-[12px] text-[var(--text-secondary)]">
                      {host.address}
                    </div>
                    <div className="mt-0.5 flex items-center gap-1.5 text-[11.5px] text-[var(--text-tertiary)]">
                      <span className="inline-flex items-center gap-1">
                        <PaduIcon className="size-2.5" name="lock" />
                        {hasToken ? t('host.authenticated') : t('host.no_auth')}
                      </span>
                      <span>·</span>
                      <span>{lastConnLabel}</span>
                    </div>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {!isActive && (
                    <Button
                      size="sm"
                      variant="default"
                      onClick={() => void switchHost(host.id).catch(() => {})}
                    >
                      {t('host.connect')}
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="outline"
                    className="gap-1 text-[var(--text-secondary)]"
                    onClick={() => {
                      setEditingHost(host)
                      setDialogOpen(true)
                    }}
                  >
                    <PaduIcon className="size-3 text-[var(--text-tertiary)]" name="pencil" />
                    {t('common.edit')}
                  </Button>
                </div>
              </div>
            )
          })}
        </div>
      )}

      <HostDialog
        editingHost={editingHost}
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onDelete={async (id) => {
          await removeHost(id)
        }}
        onSave={async (data) => {
          if (editingHost) {
            await updateHost(editingHost.id, data)
            if (activeHostId === editingHost.id) {
              await switchHost(editingHost.id)
            }
          } else {
            const created = await addHost(data)
            await switchHost(created.id)
          }
        }}
      />
    </SettingsCard>
  )
}

function DaemonSettings() {
  const { t } = useI18n()
  const { config, phase, reconnect, disconnect, forget } = useDaemon()
  const [error, setError] = useState<string | null>(null)
  return (
    <div>
      <RemoteHostsCard />
      <SettingsCard>
        <SettingText title={t('daemon.credentials_title')} description={t('daemon.web_connection_description')} />
        <div className="mt-4 divide-y rounded-xl border bg-background px-3">
          <DetailRow label={t('daemon.websocket_url')} value={config?.address ?? t('daemon.not_configured')} copy />
          <DetailRow
            copy={Boolean(config?.token)}
            label={t('daemon.token')}
            secret={Boolean(config?.token)}
            value={config?.token ?? t('daemon.not_configured')}
          />
          <DetailRow label={t('daemon.status')} value={t(`daemon.phase_${phase}`)} />
        </div>
        {error && <p className="mt-3 text-[11.5px] text-destructive">{error}</p>}
        <div className="mt-4 flex flex-wrap justify-end gap-2">
          <Button variant="ghost" onClick={disconnect}>{t('daemon.disconnect')}</Button>
          <Button variant="destructive" onClick={forget}>{t('daemon.forget')}</Button>
          <Button
            disabled={phase === 'connecting'}
            onClick={() => {
              setError(null)
              void reconnect().catch((cause) => setError(errorMessage(cause)))
            }}
          >
            {t('daemon.reconnect')}
          </Button>
        </div>
      </SettingsCard>
    </div>
  )
}

function SettingsCard({ children, row = false }: { children: ReactNode; row?: boolean }) {
  return (
    <section className={cn('mt-[15px] w-full rounded-[13px] bg-[var(--raised)] px-5 py-[14px]', row && 'flex items-center gap-6')}>
      {children}
    </section>
  )
}

function SettingText({ title, description }: { title: string; description: string }) {
  return (
    <div className="min-w-0 flex-1">
      <div className="text-[13.5px] font-medium">{title}</div>
      <p className="mt-[5px] text-[12.5px] leading-[18px] text-[var(--text-secondary)]">{description}</p>
    </div>
  )
}

function Toggle({ checked, label, onChange }: { checked: boolean; label: string; onChange: (checked: boolean) => void }) {
  return (
    <button
      aria-checked={checked}
      aria-label={label}
      className={cn(
        'flex h-5 w-9 shrink-0 items-center rounded-full border p-0.5 outline-none transition-colors focus-visible:ring-1 focus-visible:ring-ring',
        checked ? 'justify-end border-foreground bg-foreground' : 'justify-start border-input bg-[var(--inset)]',
      )}
      role="switch"
      type="button"
      onClick={() => onChange(!checked)}
    >
      <span className={cn('size-3.5 rounded-full', checked ? 'bg-background' : 'bg-[var(--text-tertiary)]')} />
    </button>
  )
}

function DetailRow({
  label,
  value,
  copy = false,
  secret = false,
}: {
  label: string
  value: string
  copy?: boolean
  secret?: boolean
}) {
  const { t } = useI18n()
  const copyFeedback = useCopyFeedback()
  const [revealed, setRevealed] = useState(false)
  return (
    <div className="flex min-h-12 items-center gap-4 text-[11.5px]">
      <span className="w-28 shrink-0 text-[var(--text-tertiary)]">{label}</span>
      <span className="min-w-0 flex-1 truncate font-mono">
        {secret && !revealed ? '••••••••••••••••••••••••' : value}
      </span>
      {secret && (
        <Button
          aria-label={t(revealed ? 'daemon.hide_token' : 'daemon.reveal_token')}
          aria-pressed={revealed}
          size="icon-sm"
          title={t(revealed ? 'daemon.hide_token' : 'daemon.reveal_token')}
          type="button"
          variant="outline"
          onClick={() => setRevealed((current) => !current)}
        >
          <PaduIcon name={revealed ? 'eyeOff' : 'eye'} />
        </Button>
      )}
      {copy && (
        <Button size="sm" variant="outline" onClick={() => void copyFeedback.copyText(value)}>
          <PaduIcon name={copyFeedback.copied ? 'check' : 'copy'} />
          {t(copyFeedback.copied ? 'common.copied' : 'common.copy')}
        </Button>
      )}
    </div>
  )
}

function useStoredBoolean(key: string, fallback: boolean) {
  const [value, setValue] = useState(() => typeof window === 'undefined' ? fallback : window.localStorage.getItem(key) !== 'false')
  const update = (next: boolean) => {
    setValue(next)
    window.localStorage.setItem(key, String(next))
  }
  return [value, update] as const
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

function AboutSettings({
  onPageChange,
}: {
  onPageChange: (page: SettingsPageId) => void
}) {
  const { t } = useI18n()
  const { activeHost, config } = useDaemon()
  const [checkingUpdate, setCheckingUpdate] = useState(false)
  const [updateResult, setUpdateResult] = useState<string | null>(null)

  const currentVersion = '0.1.1'

  const hostName = activeHost
    ? activeHost.name || activeHost.address
    : t('about.local_daemon')
  const hostAddress = activeHost
    ? activeHost.address
    : config?.address || 'ws://127.0.0.1:47319'
  const isRemote = Boolean(activeHost)

  async function checkForUpdates() {
    setCheckingUpdate(true)
    setUpdateResult(null)
    try {
      const response = await fetch(
        'https://api.github.com/repos/wisnuwiry/padu/releases/latest',
        { headers: { Accept: 'application/vnd.github.v3+json' } },
      )
      if (!response.ok) {
        throw new Error('Failed to check for updates')
      }
      const data = (await response.json()) as { tag_name?: string }
      const latestTag = data.tag_name?.replace(/^v/, '') || currentVersion
      if (latestTag > currentVersion) {
        setUpdateResult(`v${latestTag}`)
        toast.info(t('about.update_available'))
      } else {
        setUpdateResult('latest')
        toast.success(t('about.up_to_date'))
      }
    } catch {
      window.open('https://github.com/wisnuwiry/padu/releases', '_blank')
    } finally {
      setCheckingUpdate(false)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {/* Unified About + Version + Updates Card */}
      <div className="mt-[15px] flex w-full items-center justify-between gap-4 rounded-[13px] bg-[var(--raised)] p-5">
        <div className="flex min-w-0 flex-1 items-center gap-4">
          <div className="flex size-[60px] shrink-0 items-center justify-center rounded-[15px] border border-border/40 bg-black text-white">
            <PaduIcon className="size-9" name="logo" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="text-[16px] font-semibold text-foreground">Padu</span>
              <span className="rounded-md border bg-background px-2 py-0.5 font-mono text-[11px] font-medium text-[var(--text-secondary)]">
                v{currentVersion}
              </span>
            </div>
            <div className="mt-1 text-[12px] leading-[17px] text-[var(--text-secondary)]">
              {t('about.tagline')}
            </div>
          </div>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1">
          {updateResult && updateResult !== 'latest' ? (
            <Button
              size="sm"
              variant="default"
              className="gap-1.5 text-[12px]"
              onClick={() => window.open('https://github.com/wisnuwiry/padu/releases', '_blank')}
            >
              <PaduIcon className="size-3.5" name="cloudUpload" />
              {t('about.install_update')}
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              disabled={checkingUpdate}
              className="gap-1.5 text-[12px] text-[var(--text-secondary)]"
              onClick={() => void checkForUpdates()}
            >
              <PaduIcon
                className={cn('size-3.5', checkingUpdate && 'animate-spin')}
                name={checkingUpdate ? 'loaderCircle' : 'rotateCw'}
              />
              {checkingUpdate ? t('about.checking_for_updates') : t('about.check_for_updates')}
            </Button>
          )}
          <span className="text-[11px] text-[var(--text-tertiary)]">
            {checkingUpdate
              ? t('about.checking_for_updates')
              : updateResult === 'latest'
                ? t('about.up_to_date')
                : updateResult
                  ? `${t('about.update_available')} (${updateResult})`
                  : t('about.up_to_date')}
          </span>
        </div>
      </div>

      {/* Connected Host Card */}
      <SettingsCard>
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border bg-background text-[var(--accent)]">
            <PaduIcon className="size-4" name="server" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="truncate text-[13.5px] font-medium text-foreground">
                {hostName}
              </span>
              <span className="rounded bg-[var(--success-soft)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--success)]">
                {isRemote ? t('about.remote_host') : t('about.local_daemon')}
              </span>
            </div>
            <div className="truncate font-mono text-[12px] text-[var(--text-secondary)]">
              {hostAddress}
            </div>
          </div>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)]"
          onClick={() => onPageChange('daemon')}
        >
          {t('about.manage_hosts')}
          <PaduIcon className="size-3 text-[var(--text-tertiary)]" name="arrowRight" />
        </Button>
      </SettingsCard>

      {/* Website, GitHub & Sponsor single row of plain buttons, aligned center */}
      <div className="flex items-center justify-center gap-2 pt-0.5">
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={() => window.open('https://padu.dev', '_blank')}
        >
          <PaduIcon className="size-3.5" name="globe" />
          {t('about.website')}
          <PaduIcon className="size-2.5 text-[var(--text-tertiary)]" name="arrowUpRight" />
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={() => window.open('https://github.com/wisnuwiry/padu', '_blank')}
        >
          <PaduIcon className="size-3.5" name="github" />
          GitHub
          <PaduIcon className="size-2.5 text-[var(--text-tertiary)]" name="arrowUpRight" />
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={() => window.open('https://github.com/sponsors/wisnuwiry', '_blank')}
        >
          <PaduIcon className="size-3.5 text-red-500" name="heart" />
          Sponsor
          <PaduIcon className="size-2.5 text-[var(--text-tertiary)]" name="arrowUpRight" />
        </Button>
      </div>
    </div>
  )
}
