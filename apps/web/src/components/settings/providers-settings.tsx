import { useQuery, useQueryClient } from '@tanstack/react-query'
import type { AgentProfile, DaemonSettings, PlanUsage, ProviderKind, UpdateAgentProfile } from '@padu/client'
import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { ProviderIcon, PROVIDERS, PaduIcon } from '@/components/padu-icon'
import { Input } from '@/components/ui/input'
import { useAgentProfiles, useDaemonSettings, useProviderProbes } from '@/hooks/use-daemon-data'
import {
  authenticateAgy,
  cancelAgyInstall,
  checkAgyAuth,
  daemonKeys,
  fetchAgyAccount,
  fetchPlanUsage,
  installAgyAcp,
  logoutAgy,
  removeAgyAcp,
  updateAgentProfile,
  updateDaemonSettings,
} from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { cn } from '@/lib/utils'
import { Toggle, abbreviateHomePath, errorMessage, type Translator } from './shared'

export function ProvidersSettings() {
  const { t } = useI18n()
  const { client, config } = useDaemon()
  const queryClient = useQueryClient()
  const settings = useDaemonSettings()
  const profiles = useAgentProfiles()
  const probes = useProviderProbes()
  const [expanded, setExpanded] = useState<ProviderKind | null>(null)
  const [paths, setPaths] = useState<Partial<Record<ProviderKind, string>>>({})
  const [installingAgy, setInstallingAgy] = useState(false)
  const [agyInstallPercent, setAgyInstallPercent] = useState(0)
  const [agyAuthenticated, setAgyAuthenticated] = useState(false)
  const [agyAccount, setAgyAccount] = useState<string | null>(null)
  const [draggedId, setDraggedId] = useState<ProviderKind | null>(null)
  const [dragOverId, setDragOverId] = useState<ProviderKind | null>(null)
  const agyCancelRequested = useRef(false)
  useEffect(() => {
    if (!client) return
    void checkAgyAuth(client).then(setAgyAuthenticated).catch(() => setAgyAuthenticated(false))
    void fetchAgyAccount(client).then(setAgyAccount).catch(() => setAgyAccount(null))
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

  const profileById = new Map((profiles.data ?? []).map((profile) => [profile.agentId, profile]))
  const ordered =
    profiles.data && profiles.data.length > 0
      ? [...profiles.data]
          .sort((a, b) => a.priority - b.priority)
          .map((profile) => PROVIDERS.find((candidate) => candidate.id === profile.agentId))
          .filter((provider): provider is (typeof PROVIDERS)[number] => Boolean(provider))
      : PROVIDERS

  async function refreshProfiles() {
    if (!config) return
    await queryClient.invalidateQueries({ queryKey: daemonKeys.agentProfiles(config.address) })
    // The daemon derives legacy disabled_providers from the registry, so the
    // settings snapshot must be re-read after every profile write.
    await queryClient.invalidateQueries({ queryKey: daemonKeys.settings(config.address) })
  }

  async function sendProfileUpdate(update: UpdateAgentProfile) {
    if (!client || !config) return
    try {
      const profile = await updateAgentProfile(client, update)
      queryClient.setQueryData<AgentProfile[]>(
        daemonKeys.agentProfiles(config.address),
        (current) =>
          current?.map((candidate) =>
            candidate.agentId === profile.agentId ? profile : candidate,
          ),
      )
      await queryClient.invalidateQueries({ queryKey: daemonKeys.settings(config.address) })
    } catch (error) {
      if (errorMessage(error).includes('version conflict')) {
        await refreshProfiles()
        toast.error(t('providers.profiles_conflict'))
      } else {
        toast.error(errorMessage(error))
      }
    }
  }

  function toggleProfile(providerId: ProviderKind, enabled: boolean) {
    const profile = profileById.get(providerId)
    if (!client || !profile) {
      if (!settings.data) return
      const disabledProviders = enabled
        ? settings.data.disabled_providers.filter((kind) => kind !== providerId)
        : [...new Set([...settings.data.disabled_providers, providerId])]
      void apply({ ...settings.data, disabled_providers: disabledProviders })
      return
    }
    void sendProfileUpdate({
      agentId: profile.agentId,
      roleTags: profile.roleTags,
      costTier: profile.costTier,
      priority: profile.priority,
      maxRetryBeforeEscalate: profile.maxRetryBeforeEscalate,
      enabled,
      expectedVersion: profile.version,
    })
  }

  function reorderProfile(moverId: ProviderKind, targetId: ProviderKind) {
    if (moverId === targetId) return
    const fromIndex = ordered.findIndex((p) => p.id === moverId)
    const toIndex = ordered.findIndex((p) => p.id === targetId)
    if (fromIndex === -1 || toIndex === -1 || fromIndex === toIndex) return
    if (!client || !config) return

    const newOrdered = [...ordered]
    const [moved] = newOrdered.splice(fromIndex, 1)
    if (!moved) return
    newOrdered.splice(toIndex, 0, moved)

    const currentPriorities = ordered.map(
      (p) => profileById.get(p.id)?.priority ?? Number.MAX_SAFE_INTEGER,
    )

    const updates: UpdateAgentProfile[] = []
    for (let newIdx = 0; newIdx < newOrdered.length; newIdx++) {
      const provider = newOrdered[newIdx]!
      const profile = profileById.get(provider.id)
      if (!profile) continue
      const targetPriority =
        newIdx < currentPriorities.length && currentPriorities[newIdx] !== Number.MAX_SAFE_INTEGER
          ? currentPriorities[newIdx]!
          : newIdx
      if (profile.priority !== targetPriority) {
        updates.push({
          agentId: profile.agentId,
          roleTags: profile.roleTags,
          costTier: profile.costTier,
          priority: targetPriority,
          maxRetryBeforeEscalate: profile.maxRetryBeforeEscalate,
          enabled: profile.enabled,
          expectedVersion: profile.version,
        })
      }
    }

    if (updates.length === 0) return

    queryClient.setQueryData<AgentProfile[]>(
      daemonKeys.agentProfiles(config.address),
      (current) => {
        if (!current) return current
        const map = new Map(updates.map((u) => [u.agentId, u]))
        return current.map((p) => {
          const u = map.get(p.agentId)
          return u ? { ...p, priority: u.priority, version: p.version + 1 } : p
        })
      },
    )

    void (async () => {
      try {
        for (const update of updates) {
          await updateAgentProfile(client, update)
        }
        await refreshProfiles()
      } catch (error) {
        if (errorMessage(error).includes('version conflict')) {
          await refreshProfiles()
          toast.error(t('providers.profiles_conflict'))
        } else {
          toast.error(errorMessage(error))
        }
      }
    })()
  }

  return (
    <div className="mt-[15px] overflow-hidden rounded-[13px] bg-[var(--raised)] px-5 py-[14px]">
      <div className="flex items-start gap-5">
        <div className="min-w-0 flex-1">
          <div className="text-[13.5px] font-medium">{t('providers.coding_agents')}</div>
            <p className="mt-[5px] text-[12px] leading-[18px] text-[var(--text-secondary)]">
              {t('providers.web_description')}
            </p>
            <p className="mt-[5px] text-[12px] leading-[18px] text-[var(--text-secondary)]">
              {t('providers.priority_description')}
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
        {ordered.map((provider, index) => {
          const probe = probes.data[provider.id]
          const probeState = probes.states[provider.id]
          const installed = probe?.installed ?? false
          const profile = profileById.get(provider.id)
          // The registry is the source of truth once loaded; the legacy
          // settings list covers the pre-load window.
          const disabled = profile ? !profile.enabled : (settings.data?.disabled_providers.includes(provider.id) ?? false)
          const open = expanded === provider.id
          const detail = providerProbeDetail(provider.command, probe, probeState, disabled, t)
          const detailParts = [detail]
          if (profile) {
            detailParts.unshift(
              t('providers.priority_position', { position: index + 1, total: ordered.length }),
            )
          }
          if (NO_REWIND_SUPPORT.includes(provider.id)) {
            detailParts.push(t('providers.no_rewind_support'))
          }
          const detailText = detailParts.join('  ·  ')
          const dotColor = probeState.error
            ? 'bg-[var(--warning)]'
            : !installed
              ? 'bg-[var(--text-ghost)]'
              : disabled
                ? 'bg-[var(--warning)]'
                : 'bg-[var(--success)]'
          const isDragging = draggedId === provider.id
          const isDragOver = dragOverId === provider.id && draggedId !== provider.id
          return (
            <div
              className={cn(
                'border-b last:border-0 transition-colors',
                isDragOver && 'rounded-[7px] bg-accent/60 ring-1 ring-inset ring-ring',
                isDragging && 'opacity-40',
              )}
              key={provider.id}
              onDragLeave={(e) => {
                if (e.currentTarget.contains(e.relatedTarget as Node)) return
                if (dragOverId === provider.id) {
                  setDragOverId(null)
                }
              }}
              onDragOver={(e) => {
                if (draggedId && draggedId !== provider.id) {
                  e.preventDefault()
                  e.dataTransfer.dropEffect = 'move'
                  if (dragOverId !== provider.id) {
                    setDragOverId(provider.id)
                  }
                }
              }}
              onDrop={(e) => {
                e.preventDefault()
                setDragOverId(null)
                const moverId = (e.dataTransfer.getData('text/plain') as ProviderKind) || draggedId
                if (moverId && moverId !== provider.id) {
                  reorderProfile(moverId, provider.id)
                }
                setDraggedId(null)
              }}
            >
              <div className="flex items-center gap-2.5 py-[11px]">
                {profile ? (
                  <span
                    aria-label={t('providers.drag_to_reorder', { provider: provider.name })}
                    className="grid size-6 shrink-0 cursor-grab place-items-center rounded-[7px] text-[var(--text-tertiary)] outline-none hover:bg-accent hover:text-[var(--text-secondary)] active:cursor-grabbing"
                    draggable
                    title={t('providers.drag_to_reorder', { provider: provider.name })}
                    onDragEnd={() => {
                      setDraggedId(null)
                      setDragOverId(null)
                    }}
                    onDragStart={(e) => {
                      e.dataTransfer.setData('text/plain', provider.id)
                      e.dataTransfer.effectAllowed = 'move'
                      setDraggedId(provider.id)
                    }}
                  >
                    <PaduIcon className="size-3" name="gripVertical" />
                  </span>
                ) : (
                  <span className="size-6 shrink-0" />
                )}
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
                  <span className="mt-[3px] block truncate text-[10.5px] text-[var(--text-tertiary)]" title={detailText}>{detailText}</span>
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
                  <span title={t('providers.toggle_tooltip', { provider: provider.name })}>
                    <Toggle
                      checked={!disabled}
                      label={t(disabled ? 'providers.enable' : 'providers.disable', { provider: provider.name })}
                      onChange={(enabled) => toggleProfile(provider.id, enabled)}
                    />
                  </span>
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
                        type="button"
                        onClick={async () => {
                          if (!client || !config) return
                          if (installingAgy) {
                            agyCancelRequested.current = true
                            await cancelAgyInstall(client).catch(() => undefined)
                            return
                          }
                          agyCancelRequested.current = false
                          setInstallingAgy(true)
                          setAgyInstallPercent(0)
                          try {
                            await installAgyAcp(client)
                            await queryClient.invalidateQueries({ queryKey: daemonKeys.settings(config.address) })
                            await queryClient.invalidateQueries({ queryKey: daemonKeys.providers(config.address) })
                            await fetchAgyAccount(client)
                              .then(setAgyAccount)
                              .catch(() => setAgyAccount(null))
                          } catch (error) {
                            if (agyCancelRequested.current) return
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
                        {installingAgy && <PaduIcon className="size-3" name="x" />}
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
                                setAgyAccount(null)
                                toast.success(t('providers.agy_signed_out'))
                              } else {
                                await authenticateAgy(client)
                                setAgyAuthenticated(true)
                                toast.success(t('providers.agy_signed_in'))
                                await fetchAgyAccount(client)
                                  .then(setAgyAccount)
                                  .catch(() => setAgyAccount(null))
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
                          type="button"
                          onClick={async () => {
                            if (!client || !config) return
                            if (installingAgy) {
                              agyCancelRequested.current = true
                              await cancelAgyInstall(client).catch(() => undefined)
                              return
                            }
                            agyCancelRequested.current = false
                            setInstallingAgy(true)
                            setAgyInstallPercent(0)
                            try {
                              await installAgyAcp(client)
                              await queryClient.invalidateQueries({ queryKey: daemonKeys.settings(config.address) })
                              await queryClient.invalidateQueries({ queryKey: daemonKeys.providers(config.address) })
                              await fetchAgyAccount(client)
                                .then(setAgyAccount)
                                .catch(() => setAgyAccount(null))
                            } catch (error) {
                              if (agyCancelRequested.current) return
                              toast(errorMessage(error))
                            } finally {
                              setInstallingAgy(false)
                            }
                          }}
                        >
                          {installingAgy
                            ? t('providers.agy_downloading', { percent: agyInstallPercent })
                            : t('providers.agy_reinstall')}
                          {installingAgy && <PaduIcon className="size-3" name="x" />}
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
                              setAgyAccount(null)
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
                  {provider.id === 'agy' && agyAccount && (
                    <p className="truncate text-[10px] text-[var(--text-secondary)]" title={agyAccount}>
                      {agyAccount}
                    </p>
                  )}
                  <p className="truncate text-[10px] text-[var(--text-ghost)]" title={providerProbeCaption(provider.command, probe, Boolean(settings.data.provider_binary_overrides?.[provider.id]), t)}>
                    {providerProbeCaption(provider.command, probe, Boolean(settings.data.provider_binary_overrides?.[provider.id]), t)}
                  </p>
                  <ProviderAccountLine
                    provider={provider.id}
                    settings={settings.data}
                    version={probe?.version ?? null}
                  />
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

/// Providers whose daemon `fetchPlanUsage` returns an account snapshot.
/// Everything else renders no account line — unknown means hidden, never a
/// placeholder. Mirrors the desktop's `PLAN_USAGE_PROVIDERS` refresh on row
/// expand.
const PLAN_USAGE_PROVIDERS: ProviderKind[] = ['claude', 'codex', 'openCode', 'grok']

/// Providers without conversation rollback or fork. Mirrors
/// `ProviderKind::supports_conversation_rollback` /
/// `supports_conversation_fork` in `crates/padu-protocol/src/model.rs`;
/// rows for these carry the no-rewind capability note.
const NO_REWIND_SUPPORT: ProviderKind[] = ['kimi', 'fx']

function ProviderAccountLine({
  provider,
  settings,
  version,
}: {
  provider: ProviderKind
  settings: DaemonSettings
  version: string | null
}) {
  const { t } = useI18n()
  const { client, config } = useDaemon()
  const plan = useQuery<PlanUsage | null>({
    queryKey: daemonKeys.planUsage(config?.address ?? 'disconnected', provider),
    queryFn: () => fetchPlanUsage(client!, provider, settings, version),
    enabled: PLAN_USAGE_PROVIDERS.includes(provider) && Boolean(client && config),
    staleTime: 5 * 60_000,
    retry: false,
  })
  const usage = plan.data
  const label =
    usage?.accountLabel && usage?.planLabel
      ? t('providers.account_plan', { user: usage.accountLabel, plan: usage.planLabel })
      : (usage?.accountLabel ?? usage?.planLabel ?? null)
  if (!label) return null
  return (
    <p className="truncate text-[10px] text-[var(--text-secondary)]" title={label}>
      {label}
    </p>
  )
}
