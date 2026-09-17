import type { AgentSession, ProviderKind, ProviderModel, ProviderProbe } from '@padu/client'
import { Popover } from '@base-ui/react/popover'
import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from 'react'
import { ProviderIcon, PROVIDERS, providerMeta, PaduIcon } from '@/components/padu-icon'
import { useDaemonSettings, useProviderProbes } from '@/hooks/use-daemon-data'
import { useI18n } from '@/lib/i18n'
import {
  modelPickerSubtitle,
  nextModelPickerHighlight,
  selectedModelPickerIndex,
} from '@/lib/model-picker-presentation'
import { cn } from '@/lib/utils'

type PickerTab = 'favorites' | ProviderKind

export function ModelPicker({
  session,
  currentProbe,
  openSignal,
  onOpenSignalHandled,
  onChange,
  returnFocus,
}: {
  session: AgentSession
  currentProbe?: ProviderProbe
  openSignal?: number
  onOpenSignalHandled?: () => void
  onChange: (provider: ProviderKind, model: ProviderModel) => void
  returnFocus?: RefObject<HTMLElement | null>
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [tab, setTab] = useState<PickerTab>(session.provider)
  const [highlight, setHighlight] = useState<number | null>(null)
  const [favorites, setFavorites] = useState<string[]>(() => {
    if (typeof window === 'undefined') return []
    try { return JSON.parse(window.localStorage.getItem('padu.favorite-models') ?? '[]') as string[] }
    catch { return [] }
  })
  const search = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLDivElement>(null)
  const settings = useDaemonSettings()
  const probes = useProviderProbes()
  const lockedProvider = session.messages.length ? session.provider : null
  const currentModel = currentProbe?.models.find((model) => model.id === session.model)
    ?? currentProbe?.models.find((model) => model.is_default)
    ?? currentProbe?.models[0]
  const selectedModelId = session.model ?? currentModel?.id
  const selectedName = currentModel?.name ?? session.model ?? providerMeta(session.provider).shortName

  const probeMap = useMemo(() => {
    const base = { ...(probes.data ?? {}) } as Partial<Record<ProviderKind, ProviderProbe>>
    if (currentProbe) {
      base[session.provider] = currentProbe
    }
    return base
  }, [probes.data, currentProbe, session.provider])

  const disabledProviders = useMemo(
    () => settings.data?.disabled_providers ?? [],
    [settings.data?.disabled_providers],
  )
  const detectionSettled = !probes.isPending && Boolean(settings.data)

  const shownProviders = useMemo(() => {
    return PROVIDERS.filter(({ id }) => {
      const isCurrent = id === session.provider
      const isInstalled = probeMap[id]?.installed ?? false
      const hasModels = (probeMap[id]?.models?.length ?? 0) > 0
      if (!isInstalled && !isCurrent && (!hasModels || detectionSettled)) return false
      const disabled = disabledProviders.includes(id)
      if (disabled && lockedProvider !== id) return false
      return true
    })
  }, [disabledProviders, lockedProvider, probeMap, session.provider, detectionSettled])

  const usableProviders = useMemo(() => {
    return shownProviders.filter(({ id }) => {
      if (lockedProvider && id !== lockedProvider) return false
      return true
    })
  }, [lockedProvider, shownProviders])

  const hasAnyModels = useMemo(() => {
    return Object.values(probeMap).some((probe) => (probe?.models?.length ?? 0) > 0)
  }, [probeMap])
  const noProviders = detectionSettled && shownProviders.length === 0 && !hasAnyModels

  const resetPickerState = useCallback(() => {
    setQuery('')
    setHighlight(null)
    const providerEnabled = usableProviders.some(({ id }) => id === session.provider)
    const fallback = providerEnabled ? session.provider : (usableProviders[0]?.id ?? 'favorites')
    setTab(fallback)
  }, [session.provider, usableProviders])

  const handleOpenChange = (nextOpen: boolean) => {
    if (nextOpen) {
      resetPickerState()
    }
    setOpen(nextOpen)
  }

  useEffect(() => {
    if (!openSignal) return
    resetPickerState()
    setOpen(true)
    onOpenSignalHandled?.()
  }, [onOpenSignalHandled, openSignal, resetPickerState])

  useEffect(() => {
    if (!open) {
      setTab(session.provider)
    }
  }, [open, session.provider])

  useEffect(() => {
    if (!open || tab === 'favorites') return
    if (shownProviders.some(({ id }) => id === tab)) return
    const fallback = usableProviders[0]?.id ?? 'favorites'
    setTab(fallback)
  }, [open, tab, shownProviders, usableProviders])

  const rows = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    const queryTokens = normalized ? normalized.split(/\s+/) : []
    const searching = queryTokens.length > 0
    const providers = searching
      ? usableProviders
      : usableProviders.filter(({ id }) => tab === 'favorites' || id === tab)

    const result = providers.flatMap(({ id }) => {
      const probe = probeMap[id]
      const models = probe?.models ?? []
      return models
        .filter((model) => {
          const key = `${id}:${model.id}`
          if (!searching && tab === 'favorites') {
            return favorites.includes(key)
          }
          if (searching) {
            const searchable = `${model.name} ${model.id} ${model.sub_provider ?? ''} ${providerMeta(id).name} ${providerMeta(id).shortName}`.toLowerCase()
            return queryTokens.every((token) => searchable.includes(token))
          }
          return true
        })
        .map((model) => ({ provider: id, model }))
    })

    if (!searching && tab === 'favorites') {
      result.sort((a, b) => {
        const indexA = favorites.indexOf(`${a.provider}:${a.model.id}`)
        const indexB = favorites.indexOf(`${b.provider}:${b.model.id}`)
        return (indexA === -1 ? 9999 : indexA) - (indexB === -1 ? 9999 : indexB)
      })
    }

    return result
  }, [favorites, probeMap, query, tab, usableProviders])

  const selectedIndex = selectedModelPickerIndex(
    rows,
    session.provider,
    selectedModelId,
  )

  useEffect(() => {
    setHighlight((current) => {
      if (current === null || rows.length === 0) return null
      return Math.min(current, rows.length - 1)
    })
  }, [rows.length])

  useEffect(() => {
    if (highlight === null) return
    list.current
      ?.querySelector<HTMLElement>(`[data-model-index="${highlight}"]`)
      ?.scrollIntoView({ block: 'nearest' })
  }, [highlight])

  useEffect(() => {
    if (!open || query.trim() || highlight !== null) return
    const frame = requestAnimationFrame(() => {
      if (selectedIndex < 0) {
        list.current?.scrollTo({ top: 0 })
        return
      }
      list.current
        ?.querySelector<HTMLElement>(`[data-model-index="${selectedIndex}"]`)
        ?.scrollIntoView({ block: 'nearest' })
    })
    return () => cancelAnimationFrame(frame)
  }, [open, query, selectedIndex, tab, highlight])

  function choose(index: number) {
    const row = rows[index]
    if (!row) return
    onChange(row.provider, row.model)
    setOpen(false)
  }

  function toggleFavorite(provider: ProviderKind, model: string) {
    const key = `${provider}:${model}`
    setFavorites((current) => {
      const next = current.includes(key) ? current.filter((item) => item !== key) : [...current, key]
      window.localStorage.setItem('padu.favorite-models', JSON.stringify(next))
      return next
    })
  }

  return (
    <Popover.Root modal={false} open={open} onOpenChange={handleOpenChange}>
      <Popover.Trigger
        aria-label={t('models.choose')}
        className={cn(
          'flex h-6 max-w-[224px] items-center gap-1.5 rounded-[6px] px-[7px] text-[11.5px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-50',
          noProviders && 'text-amber-500',
          open && 'bg-accent text-foreground',
        )}
        disabled={session.status !== 'idle'}
      >
        {noProviders ? (
          <>
            <PaduIcon className="size-[11px] text-amber-500" name="alert" />
            <span className="truncate">{t('models.no_providers')}</span>
          </>
        ) : (
          <>
            <ProviderIcon className="size-[10.5px]" provider={session.provider} />
            <span className="truncate">{selectedName}</span>
          </>
        )}
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Positioner
          align="start"
          className="z-[100] outline-none"
          collisionPadding={8}
          side="top"
          sideOffset={4}
        >
          <Popover.Popup
            aria-label={t('models.choose')}
            className={cn(
              'padu-popover-surface flex max-w-[calc(100vw-32px)] overflow-hidden rounded-[10px] outline-none',
              noProviders ? 'h-auto w-[320px]' : 'h-[332px] w-[400px]',
            )}
            finalFocus={returnFocus
              ? (closeType) => closeType === 'keyboard' ? true : returnFocus.current
              : undefined}
            initialFocus={noProviders ? undefined : search}
            role="dialog"
          >
            {noProviders ? (
              <div className="flex w-full flex-col items-center gap-2 p-6 text-center">
                <div className="flex size-10 items-center justify-center rounded-[10px] bg-accent text-[var(--text-tertiary)]">
                  <PaduIcon className="size-5" name="bot" />
                </div>
                <div className="text-[12.5px] font-medium text-foreground">
                  {t('models.no_providers_title')}
                </div>
                <div className="text-[12px] leading-relaxed text-[var(--text-secondary)]">
                  {t('models.no_providers_description')}
                </div>
                <button
                  className="mt-1 flex h-7 items-center gap-1.5 rounded-[7px] border border-border px-3 text-[12px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
                  type="button"
                  onClick={() => {
                    setOpen(false)
                    window.location.hash = '#/settings/providers'
                  }}
                >
                  <PaduIcon className="size-3 text-[var(--text-tertiary)]" name="settings" />
                  <span>{t('models.open_provider_settings')}</span>
                </button>
              </div>
            ) : (
              <>
                <div className="flex h-full w-[40px] shrink-0 flex-col items-center gap-0.5 overflow-y-auto overflow-x-hidden border-r bg-background p-1">
                  <ModelTab
                    active={tab === 'favorites' && !query.trim()}
                    label={t('models.favorites')}
                    onClick={() => {
                      setTab('favorites')
                      setQuery('')
                      setHighlight(null)
                    }}
                  >
                    <PaduIcon className="size-[13.5px]" name="star" />
                  </ModelTab>
                  <div className="my-[2px] h-px w-[26px] shrink-0 bg-border" />
                  {shownProviders.map((provider) => {
                    const isUsable = usableProviders.some((candidate) => candidate.id === provider.id)
                    return (
                      <ModelTab
                        active={tab === provider.id && !query.trim()}
                        disabled={!isUsable}
                        key={provider.id}
                        label={provider.name}
                        onClick={() => {
                          if (!isUsable) return
                          setTab(provider.id)
                          setQuery('')
                          setHighlight(null)
                        }}
                      >
                        <ProviderIcon className="size-[14px]" provider={provider.id} />
                      </ModelTab>
                    )
                  })}
                </div>
                <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-card">
                  <div className="h-[42px] shrink-0 px-2 pb-1.5 pt-1.5">
                    <label className="flex h-[28px] items-center gap-1.5 rounded-[7px] bg-[var(--raised)] px-2">
                      <PaduIcon className="size-[12.5px] text-[var(--text-secondary)]" name="search" />
                      <input
                        aria-activedescendant={highlight !== null && rows[highlight]
                          ? `model-${rows[highlight]!.provider}-${rows[highlight]!.model.id}`
                          : undefined}
                        className="min-w-0 flex-1 bg-transparent text-[12px] outline-none placeholder:text-[var(--text-ghost)]"
                        placeholder={t('input.search_models')}
                        ref={search}
                        value={query}
                        onChange={(event) => {
                          const next = event.target.value
                          setQuery(next)
                          setHighlight(next.trim() ? 0 : null)
                        }}
                        onKeyDown={(event) => {
                          if (event.key === 'ArrowDown') {
                            event.preventDefault()
                            setHighlight((current) => nextModelPickerHighlight(current, rows.length, 'next'))
                          } else if (event.key === 'ArrowUp') {
                            event.preventDefault()
                            setHighlight((current) => nextModelPickerHighlight(current, rows.length, 'previous'))
                          } else if (event.key === 'Enter') {
                            event.preventDefault()
                            choose(highlight ?? (selectedIndex >= 0 ? selectedIndex : 0))
                          } else if (event.key === 'Tab' && !query.trim()) {
                            event.preventDefault()
                            const tabs: PickerTab[] = ['favorites', ...usableProviders.map(({ id }) => id)]
                            if (tabs.length > 0) {
                              const current = tabs.indexOf(tab)
                              const delta = event.shiftKey ? -1 : 1
                              const nextIndex = current === -1 ? 0 : (current + delta + tabs.length) % tabs.length
                              setTab(tabs[nextIndex]!)
                              setHighlight(null)
                            }
                          }
                        }}
                      />
                    </label>
                  </div>
                  <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto overflow-x-hidden p-[5px]" ref={list}>
                    {!rows.length && (
                      <div className="grid h-full place-items-center p-4 text-center text-[11.5px] text-[var(--text-ghost)]">
                        {t(query.trim()
                          ? 'models.none_found'
                          : tab === 'favorites'
                            ? 'models.favorite_hint'
                            : (probes.isFetching && (!probeMap[tab]?.models || probeMap[tab]?.models?.length === 0))
                              ? 'models.loading'
                              : 'models.none_reported')}
                      </div>
                    )}
                    {rows.map((row, index) => {
                      const selected = row.provider === session.provider && row.model.id === selectedModelId
                      const favorite = favorites.includes(`${row.provider}:${row.model.id}`)
                      return (
                        <div
                          aria-selected={selected}
                          className={cn(
                            'flex h-[36px] min-h-[36px] shrink-0 w-full min-w-0 items-center gap-2 overflow-hidden rounded-[6px] border border-transparent px-[7px] text-left outline-none hover:bg-accent',
                            selected && 'border-border bg-accent',
                            index === highlight && 'border-ring/50 bg-accent',
                          )}
                          id={`model-${row.provider}-${row.model.id}`}
                          key={`${row.provider}-${row.model.id}`}
                          data-model-index={index}
                          role="option"
                          tabIndex={0}
                          onClick={() => choose(index)}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter' || event.key === ' ') {
                              event.preventDefault()
                              choose(index)
                            }
                          }}
                          onMouseEnter={() => setHighlight(index)}
                        >
                          <span className="grid size-[22px] shrink-0 place-items-center rounded-[5px] bg-accent data-[selected=true]:bg-accent-foreground/10" data-selected={selected}>
                            <ProviderIcon className="size-[11px]" provider={row.provider} />
                          </span>
                          <span className="min-w-0 flex-1">
                            <span className={cn('block truncate text-[12px] font-medium leading-[16px]', selected ? 'text-foreground' : 'text-[var(--text-secondary)]')}>{row.model.name}</span>
                            <span className="mt-px block truncate text-[10.5px] leading-[14px] text-[var(--text-tertiary)]">
                              {modelPickerSubtitle(providerMeta(row.provider).shortName, row.model.sub_provider)}
                            </span>
                          </span>
                          {selected && <PaduIcon className="size-[11px] shrink-0 text-[var(--text-tertiary)]" name="check" />}
                          <span
                            aria-label={t(favorite ? 'models.remove_favorite' : 'models.add_favorite')}
                            className="grid size-5 shrink-0 place-items-center rounded-[4px] hover:bg-[color:var(--foreground)]/[0.08]"
                            role="button"
                            tabIndex={0}
                            onClick={(event) => { event.stopPropagation(); toggleFavorite(row.provider, row.model.id) }}
                            onKeyDown={(event) => {
                              if (event.key === 'Enter' || event.key === ' ') {
                                event.preventDefault()
                                event.stopPropagation()
                                toggleFavorite(row.provider, row.model.id)
                              }
                            }}
                          >
                            <PaduIcon className={cn('size-2.5 text-[var(--text-ghost)] opacity-70', favorite && 'text-amber-500 opacity-100')} name={favorite ? 'starFilled' : 'star'} />
                          </span>
                        </div>
                      )
                    })}
                  </div>
                </div>
              </>
            )}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  )
}

function ModelTab({ children, label, active, disabled = false, onClick }: { children: React.ReactNode; label: string; active: boolean; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      aria-label={label}
      className={cn('grid size-[30px] shrink-0 place-items-center rounded-[6px] text-[var(--text-tertiary)] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-35', active && 'bg-accent text-foreground')}
      disabled={disabled}
      type="button"
      onClick={onClick}
    >
      {children}
    </button>
  )
}
