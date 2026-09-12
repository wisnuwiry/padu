import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from 'react'
import type { AgentSession, Project, ReviewDiffSource } from '@padu/client'
import { ControlMenu } from '@/components/control-menu'
import { PanelResizeHandle } from '@/components/panel-resize-handle'
import { Button } from '@/components/ui/button'
import { Tooltip } from '@/components/ui/tooltip'
import { PaduIcon } from '@/components/padu-icon'
import { useI18n } from '@/lib/i18n'
import { usePrimaryShortcut } from '@/lib/platform'
import { isProjectlessProject } from '@/lib/project-presentation'
import { sessionCwd } from '@/lib/daemon-api'
import { fullscreenCycleNext, fullscreenTabOrder, isFullscreenExpandableSurface, openFileInPanel, tabNavigationIndex, type TabNavigationKey } from '@/lib/right-panel-state'
import type { BackgroundWorkKey } from '@/lib/runtime-context'
import { cn } from '@/lib/utils'
import type {
  PanelSurface,
  PanelTab,
  RightPanelHandle,
  RightPanelProps,
} from './types'
export type { PanelSurface, PanelTab, RightPanelHandle, RightPanelProps } from './types'
import { PanelChooser, PanelTabStrip } from './panel-tab-strip'
import { ChangesPanel } from './changes-panel'
import { FilesPanel, type FileBuffer } from './files-panel'
import { BackgroundWorkPanel, sameBackgroundWorkKey } from './background-work-panel'
import { TerminalPanel } from './terminal-panel'
import { clamp, panelContentId, panelTabId, useViewportWidth } from './shared'
export { PanelTabButton, PanelTabStrip } from './panel-tab-strip'
export { panelContentId, panelTabId } from './shared'
export type { PanelTabStripProps } from './panel-tab-strip'



interface PanelState {
  tabs: PanelTab[]
  activeId: string | null
}

export const RightPanel = forwardRef<RightPanelHandle, RightPanelProps>(function RightPanel({
  active,
  open,
  panelWidth,
  session,
  project,
  requestedSurface,
  requestedDiffSource,
  requestedBackgroundWorkKey,
  requestedFile,
  requestSignal,
  sidebarWidth,
  sidebarVisible = true,
  onToggleSidebar,
  onOpenChange,
  onPanelWidthChange,
  expanded = false,
  showConversation = false,
  onExpandedChange,
  onShowConversationChange,
  onExpandableChange,
  onTabsReport,
  onAddToChat,
  onFindFile,
}, ref) {
  const { t } = useI18n()
  const [{ tabs, activeId }, setPanelState] = useState<PanelState>({
    tabs: [],
    activeId: null,
  })
  const [fileBuffers, setFileBuffers] = useState<Record<string, FileBuffer>>({})
  const [diffSource, setDiffSource] = useState<ReviewDiffSource>('uncommitted')

  const viewportWidth = useViewportWidth()
  const maxPanelWidth = Math.max(280, Math.min(1_000, viewportWidth - sidebarWidth - 360))
  const fittedPanelWidth = clamp(panelWidth, 280, maxPanelWidth)
  const bufferRoot = session && project ? sessionCwd(session, project) : undefined
  const hasProject = Boolean(project && !isProjectlessProject(project))

  useEffect(() => {
    setFileBuffers({})
  }, [bufferRoot])

  const openSurface = useCallback((
    surface: PanelSurface,
    source: ReviewDiffSource = 'uncommitted',
    backgroundWorkKey?: BackgroundWorkKey | null,
    file?: string | null,
  ) => {
    if (surface === 'changes') setDiffSource(source)
    if (surface === 'changes') onPanelWidthChange((current) => Math.max(current, 820))
    if (surface === 'files' && file) {
      onPanelWidthChange((current) => Math.max(current, 684))
      setPanelState((current) => openFileInPanel(
        current,
        file,
        undefined,
        () => ({
          id: crypto.randomUUID(),
          surface: 'file',
          selectedFile: file,
        }),
      ))
      return
    }
    setPanelState((current) => {
      const reusable = surface === 'terminal'
        ? undefined
        : current.tabs.find((tab) => tab.surface === surface && (
            surface === 'file'
              ? tab.selectedFile === file
              : surface !== 'backgroundWork'
                || (backgroundWorkKey && tab.backgroundWorkKey
                  && sameBackgroundWorkKey(backgroundWorkKey, tab.backgroundWorkKey))
          ))
      if (reusable) {
        return {
          ...current,
          activeId: reusable.id,
          tabs: (surface === 'files' || surface === 'file') && file
            ? current.tabs.map((tab) => tab.id === reusable.id
              ? { ...tab, selectedFile: file }
              : tab)
            : current.tabs,
        }
      }
      if (surface === 'backgroundWork' && !backgroundWorkKey) return current
      const id = crypto.randomUUID()
      const tab: PanelTab = surface === 'terminal'
        ? { id, surface, terminalId: crypto.randomUUID() }
        : {
            id,
            surface,
            selectedFile: surface === 'files' || surface === 'file' ? file ?? null : undefined,
            backgroundWorkKey: surface === 'backgroundWork'
              ? backgroundWorkKey ?? undefined
              : undefined,
          }
      return { tabs: [...current.tabs, tab], activeId: id }
    })
  }, [onPanelWidthChange, onShowConversationChange])

  useEffect(() => {
    if (requestSignal > 0) {
      openSurface(requestedSurface, requestedDiffSource, requestedBackgroundWorkKey, requestedFile)
    }
  }, [openSurface, requestedBackgroundWorkKey, requestedDiffSource, requestedFile, requestedSurface, requestSignal])

  const activeTab = tabs.find((tab) => tab.id === activeId) ?? tabs.at(-1)

  const canExpand = tabs.some((tab) => isFullscreenExpandableSurface(tab.surface))
  const fullscreen = expanded && open && canExpand
  const showingConversation = fullscreen && showConversation

  useEffect(() => {
    onExpandableChange?.(canExpand)
  }, [canExpand, onExpandableChange])

  useEffect(() => {
    if (!canExpand && expanded) onExpandedChange?.(false)
    if (!open && expanded) onExpandedChange?.(false)
  }, [canExpand, expanded, open, onExpandedChange])

  const toggleExpanded = useCallback((next: boolean) => {
    if (next && !canExpand) return
    onShowConversationChange?.(false)
    onExpandedChange?.(next)
  }, [canExpand, onExpandedChange, onShowConversationChange])

  function focusConversationTab() {
    window.requestAnimationFrame(() => {
      document.getElementById('right-panel-tab-conversation')?.focus()
    })
  }

  const cycleTabs = useCallback((direction: 1 | -1) => {
    if (tabs.length === 0) return
    if (!fullscreen) {
      if (tabs.length <= 1) return
      const activeIndex = Math.max(0, tabs.findIndex((tab) => tab.id === activeTab?.id))
      const nextIndex = (activeIndex + direction + tabs.length) % tabs.length
      const tab = tabs[nextIndex]
      if (!tab) return
      setPanelState((currentState) => ({ ...currentState, activeId: tab.id }))
      window.requestAnimationFrame(() => {
        document.getElementById(panelTabId(tab.id))?.focus()
      })
      return
    }
    const activeIndex = Math.max(0, tabs.findIndex((tab) => tab.id === activeTab?.id))
    const current = showingConversation ? 0 : activeIndex + 1
    const next = fullscreenCycleNext(current, tabs.length, direction)
    const order = fullscreenTabOrder(tabs.length)
    const target = order[next]
    if (target === undefined) return
    if (target === -1) {
      onShowConversationChange?.(true)
      focusConversationTab()
    } else {
      const tab = tabs[target]
      if (!tab) return
      onShowConversationChange?.(false)
      setPanelState((currentState) => ({ ...currentState, activeId: tab.id }))
      window.requestAnimationFrame(() => {
        document.getElementById(panelTabId(tab.id))?.focus()
      })
    }
  }, [fullscreen, tabs, activeTab?.id, showingConversation, onShowConversationChange])

  useEffect(() => {
    if (!active || !open) return
    function onKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null
      const inDialogOrMenu = Boolean(target?.closest('[role="dialog"], [role="menu"]'))
      if (inDialogOrMenu) return

      const mod = event.metaKey || event.ctrlKey
      if (mod && !event.shiftKey && event.key.toLowerCase() === 'j') {
        event.preventDefault()
        toggleExpanded(!expanded)
        return
      }
      if (mod && event.altKey && (event.key === 'ArrowRight' || event.key === 'ArrowLeft')) {
        event.preventDefault()
        cycleTabs(event.key === 'ArrowRight' ? 1 : -1)
        return
      }
      if (event.key === 'Escape' && expanded) {
        if (target?.closest('input, textarea, select, [contenteditable="true"]')) return
        event.preventDefault()
        toggleExpanded(false)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [active, open, expanded, toggleExpanded, cycleTabs])

  const closeTab = useCallback((tabId: string) => {
    setPanelState((current) => {
      const index = current.tabs.findIndex((tab) => tab.id === tabId)
      if (index < 0) return current
      const remaining = current.tabs.filter((tab) => tab.id !== tabId)
      const nextActiveId = current.activeId === tabId
        ? remaining[Math.min(index, remaining.length - 1)]?.id ?? null
        : current.activeId
      if (remaining.length === 0) {
        onShowConversationChange?.(false)
        onExpandedChange?.(false)
        onOpenChange(false)
      }
      return { tabs: remaining, activeId: nextActiveId }
    })
  }, [onExpandedChange, onOpenChange, onShowConversationChange])

  function focusTab(index: number) {
    const tab = tabs[index]
    if (!tab) return
    onShowConversationChange?.(false)
    setPanelState((current) => ({ ...current, activeId: tab.id }))
    window.requestAnimationFrame(() => {
      document.getElementById(panelTabId(tab.id))?.focus()
    })
  }

  const activateTab = useCallback((tabId: string) => {
    onShowConversationChange?.(false)
    setPanelState((current) => ({ ...current, activeId: tabId }))
  }, [onShowConversationChange])

  function navigateStrip(stripIndex: number, key: TabNavigationKey) {
    const next = tabNavigationIndex(tabs.length + 1, stripIndex, key)
    if (next === 0) {
      onShowConversationChange?.(true)
      focusConversationTab()
    } else {
      focusTab(next - 1)
    }
  }

  useImperativeHandle(ref, () => ({
    activateTab,
    closeTab,
    openSurface: (surface, diff, bgKey, file) => {
      openSurface(surface, diff, bgKey, file)
    },
    cycleTabs,
  }), [activateTab, closeTab, openSurface, cycleTabs])

  useEffect(() => {
    onTabsReport?.(tabs, activeId)
  }, [tabs, activeId, onTabsReport])

  const fullscreenShortcut = usePrimaryShortcut('⌘J', 'Ctrl+J')
  const toggleSidebarShortcut = usePrimaryShortcut('⌘B', 'Ctrl+B')
  const togglePanelShortcut = usePrimaryShortcut('⇧⌘B', 'Ctrl+Shift+B')

  const updateTab = useCallback((tabId: string, update: Partial<PanelTab>) => {
    setPanelState((current) => ({
      ...current,
      tabs: current.tabs.map((tab) => tab.id === tabId ? { ...tab, ...update } : tab),
    }))
  }, [])

  const openFile = useCallback((tabId: string, selectedFile: string, treeWidth: number) => {
    onPanelWidthChange((current) => Math.max(current, treeWidth + 500))
    setPanelState((current) => openFileInPanel(
      current,
      selectedFile,
      tabId,
      () => ({
        id: crypto.randomUUID(),
        surface: 'file',
        selectedFile,
      }),
    ))
  }, [onPanelWidthChange])

  const setTabDirty = useCallback((tabId: string, dirty: boolean) => {
    setPanelState((current) => {
      const tab = current.tabs.find((candidate) => candidate.id === tabId)
      if (!tab || Boolean(tab.dirty) === dirty) return current
      return {
        ...current,
        tabs: current.tabs.map((candidate) => candidate.id === tabId
          ? { ...candidate, dirty }
          : candidate),
      }
    })
  }, [])

  return (
    <aside
      className={cn(
        'flex shrink-0 flex-col border-l bg-background',
        (!open || (fullscreen && showingConversation)) && 'hidden',
        fullscreen && !showingConversation && 'relative z-30 min-w-0 flex-1 shadow-none',
        !fullscreen
          && 'absolute inset-y-0 right-0 z-30 shadow-2xl xl:relative xl:z-auto xl:shadow-none',
      )}
      style={fullscreen ? undefined : { width: `min(${fittedPanelWidth}px, 92vw)` }}
    >
      {!fullscreen && (
        <PanelResizeHandle
          edge="left"
          label={t('right_panel.resize')}
          max={maxPanelWidth}
          min={280}
          value={fittedPanelWidth}
          onChange={onPanelWidthChange}
        />
      )}
      <header className="flex h-12 shrink-0 items-center gap-1.5 px-2.5 pr-3.5">
        {fullscreen && !sidebarVisible && onToggleSidebar && (
          <Tooltip content={t('menu.toggle_sidebar')} shortcut={toggleSidebarShortcut}>
            <Button
              aria-label={t('menu.toggle_sidebar')}
              size="icon-sm"
              variant="ghost"
              onClick={onToggleSidebar}
            >
              <PaduIcon name="panelLeft" />
            </Button>
          </Tooltip>
        )}
        <PanelTabStrip
          activeId={activeTab?.id}
          fullscreen={fullscreen}
          showingConversation={showingConversation}
          tabs={tabs}
          onActivateTab={activateTab}
          onCloseTab={closeTab}
          onNavigateStrip={fullscreen
            ? navigateStrip
            : (index, key) => focusTab(tabNavigationIndex(tabs.length, index - 1, key))}
          onSelectConversation={() => onShowConversationChange?.(true)}
        />
        {tabs.length > 0 && (
          <ControlMenu
            align="right"
            caret={false}
            highlightTriggerWhenOpen={false}
            label={t('right_panel.add_tab')}
            placement="below"
            selectionMode="status"
            triggerClassName="size-7 justify-center px-0"
            items={[
              {
                id: 'terminal',
                label: t('right_panel.terminal'),
                icon: 'terminal',
                onSelect: () => openSurface('terminal'),
              },
              ...(hasProject
                ? [
                    {
                      id: 'files',
                      label: t('right_panel.files'),
                      icon: 'folder' as const,
                      selected: tabs.some((tab) => tab.surface === 'files'),
                      onSelect: () => openSurface('files'),
                    },
                    {
                      id: 'changes',
                      label: t('right_panel.diff'),
                      icon: 'fileDiff' as const,
                      selected: tabs.some((tab) => tab.surface === 'changes'),
                      onSelect: () => openSurface('changes'),
                    },
                  ]
                : []),
            ]}
          >
            <PaduIcon className="size-3.5" name="plus" />
          </ControlMenu>
        )}
        {canExpand && (
          <Tooltip
            content={t(fullscreen ? 'right_panel.collapse' : 'right_panel.expand')}
            shortcut={fullscreenShortcut}
          >
            <Button
              className={"size-7"}
              aria-label={t(fullscreen ? 'right_panel.collapse' : 'right_panel.expand')}
              aria-pressed={fullscreen}
              size="icon-sm" variant="ghost"
              onClick={() => toggleExpanded(!fullscreen)}
            >
              <PaduIcon className="size-3.5! text-[var(--text-tertiary)]" name={fullscreen ? 'minimize' : 'maximize'} />
            </Button>
          </Tooltip>
        )}
        {!fullscreen && (
          <Tooltip content={t('right_panel.toggle')} shortcut={togglePanelShortcut}>
            <Button aria-label={t('right_panel.hide')} className={"size-7"} size="icon-sm" variant="ghost" onClick={() => onOpenChange(false)}>
              <PaduIcon name="panelRight" />
            </Button>
          </Tooltip>
        )}
      </header>

      {!activeTab && !showingConversation && (
        <PanelChooser hasProject={hasProject} onSelect={openSurface} />
      )}
      {!showingConversation && tabs.map((tab) => (
        <div
          aria-labelledby={panelTabId(tab.id)}
          className={cn('min-h-0 flex-1', tab.id === activeTab?.id ? 'flex' : 'hidden')}
          id={panelContentId(tab.id)}
          key={tab.id}
          role="tabpanel"
        >
          {(tab.surface === 'files' || tab.surface === 'file') && (
            <FilesPanel
              active={active && tab.id === activeTab?.id}
              buffers={fileBuffers}
              panelWidth={fittedPanelWidth}
              project={project}
              requestedFile={tab.selectedFile ?? null}
              session={session}
              setBuffers={setFileBuffers}
              tabId={tab.id}
              onDirtyChange={setTabDirty}
              onOpenFile={openFile}
              onAddToChat={onAddToChat}
              onFindFile={onFindFile}
            />
          )}
          {tab.surface === 'changes' && (
            <ChangesPanel
              diffSource={diffSource}
              panelWidth={fittedPanelWidth}
              project={project}
              session={session}
              onDiffSourceChange={setDiffSource}
            />
          )}
          {tab.surface === 'terminal' && tab.terminalId && (
            <TerminalPanel
              project={project}
              session={session}
              terminalId={tab.terminalId}
              onTitle={(title) => updateTab(tab.id, { title })}
            />
          )}
          {tab.surface === 'backgroundWork' && tab.backgroundWorkKey && (
            <BackgroundWorkPanel
              session={session}
              workKey={tab.backgroundWorkKey}
              onTitle={(title) => updateTab(tab.id, { title })}
            />
          )}
        </div>
      ))}
    </aside>
  )
})
