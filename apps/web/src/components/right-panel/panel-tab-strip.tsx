import type { ReactNode } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { FileTypeIcon, PaduIcon } from '@/components/padu-icon'
import { useI18n } from '@/lib/i18n'
import { usePrimaryShortcut } from '@/lib/platform'
import { cn } from '@/lib/utils'
import { Kbd } from '../ui/kbd'
import { backgroundWorkKindIcon, backgroundWorkKindLabel } from './background-work-panel'
import { panelContentId, panelTabId } from './shared'
import type { PanelSurface, PanelTab } from './types'
import type { TabNavigationKey } from '@/lib/right-panel-state'

export interface PanelTabStripProps {
  tabs: PanelTab[]
  activeId?: string | null
  showingConversation?: boolean
  fullscreen?: boolean
  onSelectConversation?: () => void
  onActivateTab: (tabId: string) => void
  onCloseTab: (tabId: string) => void
  onNavigateStrip?: (index: number, key: TabNavigationKey) => void
}

export function PanelTabStrip({
  tabs,
  activeId,
  showingConversation = false,
  fullscreen = false,
  onSelectConversation,
  onActivateTab,
  onCloseTab,
  onNavigateStrip,
}: PanelTabStripProps) {
  const { t } = useI18n()
  const tabStrip = useRef<HTMLDivElement>(null)
  const [tabOverflow, setTabOverflow] = useState({ start: false, end: false })

  const updateTabOverflow = useCallback(() => {
    const strip = tabStrip.current
    if (!strip) return
    const next = {
      start: strip.scrollLeft > 1,
      end: strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 1,
    }
    setTabOverflow((current) => current.start === next.start && current.end === next.end
      ? current
      : next)
  }, [])

  useEffect(() => {
    const strip = tabStrip.current
    if (!strip) return
    updateTabOverflow()
    const resize = new ResizeObserver(updateTabOverflow)
    resize.observe(strip)
    return () => resize.disconnect()
  }, [tabs, updateTabOverflow])

  useEffect(() => {
    if (!activeId) return
    const frame = window.requestAnimationFrame(() => {
      const strip = tabStrip.current
      const tab = document.getElementById(panelTabId(activeId))
      if (!strip || !tab) return
      const stripBounds = strip.getBoundingClientRect()
      const tabBounds = tab.getBoundingClientRect()
      const inset = 16
      if (tabBounds.left < stripBounds.left + inset) {
        strip.scrollLeft -= stripBounds.left + inset - tabBounds.left
      } else if (tabBounds.right > stripBounds.right - inset) {
        strip.scrollLeft += tabBounds.right - (stripBounds.right - inset)
      }
      updateTabOverflow()
    })
    return () => window.cancelAnimationFrame(frame)
  }, [activeId, updateTabOverflow])

  return (
    <div className={cn(
      'relative min-w-0 flex-1 self-stretch overflow-hidden',
      fullscreen && 'flex justify-center',
    )}>
      <div
        aria-label={t('right_panel.tabs')}
        className={cn(
          'flex h-full min-w-0 items-center gap-1 overflow-x-auto overscroll-x-contain',
          fullscreen && 'justify-center',
        )}
        ref={tabStrip}
        role="tablist"
        onScroll={updateTabOverflow}
      >
        {fullscreen && (
          <div
            aria-selected={showingConversation}
            className={cn(
              'flex h-7 min-w-[100px] max-w-44 shrink-0 items-center gap-1.5 rounded-md px-2 text-left text-[12px] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring cursor-pointer',
              showingConversation ? 'bg-accent text-foreground' : 'text-[var(--text-secondary)]',
            )}
            id="right-panel-tab-conversation"
            role="tab"
            tabIndex={showingConversation ? 0 : -1}
            onClick={onSelectConversation}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) return
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                onSelectConversation?.()
              } else if (!event.metaKey && !event.ctrlKey && !event.altKey && ['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
                event.preventDefault()
                onNavigateStrip?.(0, event.key as TabNavigationKey)
              }
            }}
          >
            <PaduIcon className="size-[13px] text-[var(--text-secondary)]" name="compose" />
            <span className="min-w-0 flex-1 truncate">{t('right_panel.conversation')}</span>
          </div>
        )}
        {tabs.map((tab, index) => (
          <PanelTabButton
            active={!showingConversation && tab.id === activeId}
            key={tab.id}
            tab={tab}
            onActivate={() => onActivateTab(tab.id)}
            onClose={() => onCloseTab(tab.id)}
            onNavigate={(key) => onNavigateStrip?.(index + 1, key)}
          />
        ))}
        <span aria-hidden="true" className="h-px w-4 shrink-0" />
      </div>
      <span
        aria-hidden="true"
        className={cn(
          'pointer-events-none absolute inset-y-0 left-0 w-4 bg-gradient-to-r from-background to-transparent transition-opacity motion-reduce:transition-none',
          tabOverflow.start ? 'opacity-100' : 'opacity-0',
        )}
      />
      <span
        aria-hidden="true"
        className={cn(
          'pointer-events-none absolute inset-y-0 right-0 w-4 bg-gradient-to-l from-background to-transparent transition-opacity motion-reduce:transition-none',
          tabOverflow.end ? 'opacity-100' : 'opacity-0',
        )}
      />
    </div>
  )
}

export function PanelTabButton({
  tab,
  active,
  onActivate,
  onClose,
  onNavigate,
}: {
  tab: PanelTab
  active: boolean
  onActivate: () => void
  onClose: () => void
  onNavigate: (key: TabNavigationKey) => void
}) {
  const { t } = useI18n()
  const saveShortcut = usePrimaryShortcut('⌘S', 'Ctrl+S')
  const title = tab.surface === 'files' || tab.surface === 'file'
    ? tab.selectedFile?.split('/').at(-1) ?? t('right_panel.files')
    : tab.surface === 'changes'
      ? t('right_panel.diff')
      : tab.surface === 'backgroundWork'
        ? tab.title?.trim() || backgroundWorkKindLabel(tab.backgroundWorkKey?.kind, t)
        : tab.title?.trim() || t('right_panel.terminal')
  const icon = tab.surface === 'files' || tab.surface === 'file'
    ? 'folder'
    : tab.surface === 'changes'
      ? 'fileDiff'
      : tab.surface === 'backgroundWork'
        ? backgroundWorkKindIcon(tab.backgroundWorkKey?.kind)
        : 'terminal'
  return (
    <div
      aria-controls={panelContentId(tab.id)}
      aria-selected={active}
      className={cn(
        'flex h-7 min-w-[100px] max-w-44 shrink-0 items-center gap-1.5 rounded-md px-2 text-left text-[12px] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring',
        active ? 'bg-accent text-foreground' : 'text-[var(--text-secondary)]',
      )}
      id={panelTabId(tab.id)}
      role="tab"
      tabIndex={active ? 0 : -1}
      onClick={onActivate}
      onMouseDown={(event) => {
        if (event.button === 1) {
          event.preventDefault()
          onClose()
        }
      }}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          onActivate()
        } else if (!event.metaKey && !event.ctrlKey && !event.altKey && ['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
          event.preventDefault()
          onNavigate(event.key as TabNavigationKey)
        }
      }}
    >
      {(tab.surface === 'files' || tab.surface === 'file') && tab.selectedFile
        ? <FileTypeIcon className="size-[13px]" path={tab.selectedFile} />
        : <PaduIcon className="size-[13px] text-[var(--text-secondary)]" name={icon} />}
      <span className="min-w-0 flex-1 truncate">{title}</span>
      {tab.dirty && (
        <span
          aria-label={t('files.unsaved_changes', { shortcut: saveShortcut })}
          className="size-[7px] shrink-0 rounded-full bg-[var(--warning)]"
          role="status"
          title={t('files.unsaved_changes', { shortcut: saveShortcut })}
        />
      )}
      <button
        aria-label={t('right_panel.close_tab', { title })}
        className="grid size-4 shrink-0 place-items-center rounded hover:bg-accent"
        tabIndex={active ? 0 : -1}
        type="button"
        onClick={(event) => {
          event.stopPropagation()
          onClose()
        }}
      >
        <PaduIcon className="size-2.5 text-[var(--text-tertiary)]" name="x" />
      </button>
    </div>
  )
}

export function PanelChooser({
  onSelect,
  hasProject = false,
}: {
  onSelect: (surface: PanelSurface) => void
  hasProject?: boolean
}) {
  const { t } = useI18n()
  const terminalShortcut = usePrimaryShortcut('⌘T', 'Ctrl+T')
  const filesShortcut = usePrimaryShortcut('⇧⌘E', 'Ctrl+Shift+E')
  const diffShortcut = usePrimaryShortcut('⌘D', 'Ctrl+D')
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center px-5 pb-8">
      <div className="w-full max-w-[420px] text-center">
        <h3 className="text-[13px] font-medium">{t('right_panel.open_surface')}</h3>
        <p className="mt-[5px] text-[11px] text-[var(--text-tertiary)]">{t('right_panel.choose_surface')}</p>
        <div className={cn('mt-5 grid gap-2 text-left', hasProject ? 'grid-cols-2' : 'grid-cols-1')}>
          <PanelCard
            icon={<PaduIcon className="size-[18px]" name="terminal" />}
            label={t('right_panel.terminal')}
            description={t('right_panel.terminal_description')}
            shortcut={terminalShortcut}
            onClick={() => onSelect('terminal')}
          />
          {hasProject && (
            <>
              <PanelCard
                icon={<PaduIcon className="size-[18px]" name="folder" />}
                label={t('right_panel.files')}
                description={t('right_panel.files_description')}
                shortcut={filesShortcut}
                onClick={() => onSelect('files')}
              />
              <PanelCard
                icon={<PaduIcon className="size-[18px]" name="fileDiff" />}
                label={t('right_panel.diff')}
                description={t('right_panel.diff_description')}
                shortcut={diffShortcut}
                onClick={() => onSelect('changes')}
              />
            </>
          )}
        </div>
      </div>
    </div>
  )
}

function PanelCard({
  icon,
  label,
  description,
  shortcut,
  onClick,
}: {
  icon: ReactNode
  label: string
  description: string
  shortcut?: string
  onClick?: () => void
}) {
  return (
    <button
      className="flex h-32 min-w-0 flex-col rounded-lg border border-[var(--input)] bg-card p-3.5 text-left outline-none hover:border-[var(--text-ghost)] hover:bg-[var(--raised)] active:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
      type="button"
      onClick={onClick}
    >
      <div className="flex w-full items-center justify-between">
        <span className="text-[var(--text-tertiary)]">{icon}</span>
        {shortcut && (
          <Kbd size="sm">
            {shortcut}
          </Kbd>
        )}
      </div>
      <span className="mt-3 text-[12.5px] font-medium">{label}</span>
      <span className="mt-1 text-[10.5px] leading-4 text-[var(--text-tertiary)]">{description}</span>
    </button>
  )
}
