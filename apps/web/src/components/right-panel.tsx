import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query'
import type { Editor } from '@pierre/diffs/edit'
import type { AgentSession, Project, ReviewDiffSource, WorkingTreeEntry } from '@padu/client'
import { GhosttyCore } from '@wterm/ghostty'
import { Terminal, useTerminal } from '@wterm/react'
import {
  forwardRef,
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type Dispatch,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type SetStateAction,
} from 'react'
import { ContextMenu } from '@base-ui/react/context-menu'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import { toast } from 'sonner'
import { ControlMenu } from '@/components/control-menu'
import { PanelResizeHandle } from '@/components/panel-resize-handle'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { Input } from '@/components/ui/input'
import { Tooltip } from '@/components/ui/tooltip'
import { FileTypeIcon, PaduIcon, type PaduIconName } from '@/components/padu-icon'
import type { CodeDiffSurfaceHandle, DiffSurfaceFile } from '@/components/code-surfaces'
import {
  collectWorkspaceDiff,
  createWorkspaceDirectory,
  createWorkspaceFile,
  daemonKeys,
  deleteWorkspacePath,
  listWorkspaceTree,
  renameWorkspacePath,
  readWorkspaceTextFile,
  sessionCwd,
  writeWorkspaceTextFile,
} from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { usePrimaryShortcut } from '@/lib/platform'
import { isProjectlessProject, projectDisplayName } from '@/lib/project-presentation'
import {
  latestReviewTurnSource,
  reviewDiffSourceLabel,
  sameReviewDiffSource,
} from '@/lib/review-diff'
import {
  fullscreenCycleNext,
  fullscreenTabOrder,
  isFullscreenExpandableSurface,
  mergeReviewDiffFiles,
  openFileInPanel,
  tabNavigationIndex,
  treeNavigationAction,
  type TabNavigationKey,
  type TreeNavigationKey,
} from '@/lib/right-panel-state'
import { formatWorkingElapsed, type Translator } from '@/lib/transcript-presentation'
import {
  useRuntime,
  type BackgroundWorkItem,
  type BackgroundWorkKey,
  type BackgroundWorkStatus,
} from '@/lib/runtime-context'
import { cn } from '@/lib/utils'
import { Kbd } from './ui/kbd'

const CodeFileSurface = lazy(() => import('@/components/code-surfaces').then((module) => ({
  default: module.CodeFileSurface,
})))
const CodeDiffSurface = lazy(() => import('@/components/code-surfaces').then((module) => ({
  default: module.CodeDiffSurface,
})))

export type PanelSurface = 'files' | 'file' | 'changes' | 'terminal' | 'backgroundWork'

export interface PanelTab {
  id: string
  surface: PanelSurface
  terminalId?: string
  title?: string
  selectedFile?: string | null
  dirty?: boolean
  backgroundWorkKey?: BackgroundWorkKey
}

export interface RightPanelHandle {
  activateTab: (tabId: string) => void
  closeTab: (tabId: string) => void
  openSurface: (
    surface: PanelSurface,
    diffSource?: ReviewDiffSource,
    backgroundWorkKey?: BackgroundWorkKey | null,
    file?: string | null,
  ) => void
  cycleTabs: (direction: 1 | -1) => void
}

export interface RightPanelProps {
  active: boolean
  open: boolean
  panelWidth: number
  session: AgentSession | null
  project?: Project
  requestedSurface: PanelSurface
  requestedDiffSource: ReviewDiffSource
  requestedBackgroundWorkKey: BackgroundWorkKey | null
  requestedFile: string | null
  requestSignal: number
  sidebarWidth: number
  sidebarVisible?: boolean
  onToggleSidebar?: () => void
  onOpenChange: (open: boolean) => void
  onPanelWidthChange: Dispatch<SetStateAction<number>>
  expanded?: boolean
  showConversation?: boolean
  onExpandedChange?: (expanded: boolean) => void
  onShowConversationChange?: (show: boolean) => void
  onExpandableChange?: (expandable: boolean) => void
  onTabsReport?: (tabs: PanelTab[], activeId: string | null) => void
  onAddToChat?: (name: string, isDir?: boolean) => void
  onFindFile?: () => void
}

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
  const tabStrip = useRef<HTMLDivElement>(null)
  const [tabOverflow, setTabOverflow] = useState({ start: false, end: false })
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
              aria-label={t(fullscreen ? 'right_panel.collapse' : 'right_panel.expand')}
              aria-pressed={fullscreen}
              size="icon-sm"
              variant="ghost"
              onClick={() => toggleExpanded(!fullscreen)}
            >
              <PaduIcon name={fullscreen ? 'minimize' : 'maximize'} />
            </Button>
          </Tooltip>
        )}
        {!fullscreen && (
          <Tooltip content={t('right_panel.toggle')} shortcut={togglePanelShortcut}>
            <Button aria-label={t('right_panel.hide')} size="icon-sm" variant="ghost" onClick={() => onOpenChange(false)}>
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

function PanelChooser({
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

interface FileBuffer {
  content: string
  diskContent: string
  revision: number
  saving: boolean
  editor?: Editor<undefined>
}

function FileBreadcrumbs({
  path,
  dirty,
}: {
  path: string
  dirty?: boolean
}) {
  const { t } = useI18n()
  const [copied, setCopied] = useState(false)
  const segments = path.split('/')
  const fileName = segments.at(-1) ?? path
  const dirSegments = segments.slice(0, -1)

  const handleCopy = () => {
    navigator.clipboard.writeText(path)
    setCopied(true)
    toast.success(t('files.copied_path', { path: fileName }))
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="flex h-[42px] shrink-0 items-center gap-1.5 border-b px-3 text-[12px] text-[var(--text-secondary)]">
      <FileTypeIcon className="size-4 shrink-0" path={path} />
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
        {dirSegments.map((segment, i) => (
          <span key={i} className="flex shrink-0 items-center gap-1 text-[var(--text-tertiary)]">
            <span className="truncate max-w-[120px]">{segment}</span>
            <span className="text-[var(--text-ghost)]">/</span>
          </span>
        ))}
        <span className="truncate font-medium text-foreground">{fileName}</span>
        {dirty && (
          <span
            className="size-1.5 shrink-0 rounded-full bg-[var(--warning)] ml-0.5"
            title={t('files.unsaved_changes', { shortcut: '⌘S' })}
          />
        )}
      </div>
      <Tooltip content={copied ? t('common.copied') : t('files.copy_path')}>
        <button
          type="button"
          aria-label={t('files.copy_path')}
          className="grid size-6 shrink-0 place-items-center rounded hover:bg-accent text-[var(--text-tertiary)] hover:text-foreground cursor-pointer transition-colors"
          onClick={handleCopy}
        >
          <PaduIcon className="size-3.5" name={copied ? 'check' : 'copy'} />
        </button>
      </Tooltip>
    </div>
  )
}

function FilesPanel({
  active,
  buffers,
  tabId,
  session,
  project,
  requestedFile,
  panelWidth,
  setBuffers,
  onDirtyChange,
  onOpenFile,
  onAddToChat,
  onFindFile,
}: {
  active: boolean
  buffers: Record<string, FileBuffer>
  tabId: string
  session: AgentSession | null
  project?: Project
  requestedFile: string | null
  panelWidth: number
  setBuffers: Dispatch<SetStateAction<Record<string, FileBuffer>>>
  onDirtyChange: (tabId: string, dirty: boolean) => void
  onOpenFile: (tabId: string, path: string, treeWidth: number) => void
  onAddToChat?: (name: string, isDir?: boolean) => void
  onFindFile?: () => void
}) {
  const { t } = useI18n()
  const { client, config, phase } = useDaemon()
  const queryClient = useQueryClient()
  const [expanded, setExpanded] = useState<string[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [focusedTreeEntry, setFocusedTreeEntry] = useState<string | null>(null)
  const [showHidden, setShowHidden] = useState(false)
  const treeList = useRef<VirtuosoHandle>(null)
  const buffersRef = useRef(buffers)
  buffersRef.current = buffers
  const [treeWidth, setTreeWidth] = useState(() => readStoredWidth('padu.fileTreeWidth', 184, 140, 360))
  const treeWidthRef = useRef(treeWidth)
  treeWidthRef.current = treeWidth
  const root = session && project ? sessionCwd(session, project) : undefined
  const maxTreeWidth = Math.max(140, Math.min(360, panelWidth - 140))
  const fittedTreeWidth = clamp(treeWidth, 140, maxTreeWidth)
  const previousRoot = useRef(root)

  useEffect(() => {
    if (previousRoot.current === root) return
    previousRoot.current = root
    setExpanded([])
    setSelected(requestedFile)
    setFocusedTreeEntry(null)
  }, [requestedFile, root])

  useEffect(() => {
    if (!requestedFile || requestedFile === selected) return
    setSelected(requestedFile)
    setExpanded((current) => {
      const next = new Set(current)
      for (const path of absoluteParentPaths(root, requestedFile)) next.add(path)
      return [...next]
    })
  }, [requestedFile, root, selected])

  useEffect(() => {
    const timer = window.setTimeout(() => {
      window.localStorage.setItem('padu.fileTreeWidth', String(Math.round(treeWidth)))
    }, 150)
    return () => window.clearTimeout(timer)
  }, [treeWidth])

  const tree = useQuery({
    queryKey: daemonKeys.workspaceTree(config?.address ?? 'disconnected', root ?? 'none', expanded, showHidden),
    queryFn: () => listWorkspaceTree(requireClient(client), root!, expanded, showHidden),
    enabled: phase === 'connected' && Boolean(client && config && root),
    placeholderData: keepPreviousData,
  })
  const file = useQuery({
    queryKey: daemonKeys.workspaceFile(config?.address ?? 'disconnected', root ?? 'none', selected ?? 'none'),
    queryFn: () => readWorkspaceTextFile(requireClient(client), root!, selected!),
    enabled: phase === 'connected' && Boolean(client && config && root && selected),
  })
  const refetchFile = file.refetch

  useEffect(() => {
    if (!selected || file.data === undefined) return
    setBuffers((current) => {
      const buffer = current[selected]
      if (buffer && buffer.content !== buffer.diskContent) return current
      if (buffer?.diskContent === file.data) return current
      return {
        ...current,
        [selected]: {
          content: file.data,
          diskContent: file.data,
          revision: (buffer?.revision ?? -1) + 1,
          saving: false,
          editor: buffer?.editor,
        },
      }
    })
  }, [file.data, selected])

  const selectedBuffer = selected ? buffers[selected] : undefined
  const dirty = Boolean(selectedBuffer && selectedBuffer.content !== selectedBuffer.diskContent)
  useEffect(() => {
    onDirtyChange(tabId, dirty)
  }, [dirty, onDirtyChange, tabId])

  const saveSelected = useCallback(async () => {
    if (!selected || !root || !client || !config) return
    const buffer = buffersRef.current[selected]
    if (!buffer || buffer.saving || buffer.content === buffer.diskContent) return
    const path = selected
    const snapshot = buffer.content
    setBuffers((current) => current[path]
      ? { ...current, [path]: { ...current[path], saving: true } }
      : current)
    try {
      await writeWorkspaceTextFile(client, root, path, snapshot)
      queryClient.setQueryData(
        daemonKeys.workspaceFile(config.address, root, path),
        snapshot,
      )
      setBuffers((current) => current[path]
        ? {
            ...current,
            [path]: {
              ...current[path],
              diskContent: snapshot,
              saving: false,
            },
          }
        : current)
      void queryClient.invalidateQueries({
        queryKey: ['daemon', config.address, 'workspace-diff', root],
      })
    } catch (error) {
      setBuffers((current) => current[path]
        ? { ...current, [path]: { ...current[path], saving: false } }
        : current)
      toast.error(t('files.could_not_save', { path, error: errorMessage(error) }))
    }
  }, [client, config, queryClient, root, selected])

  useEffect(() => {
    if (!active) return
    const save = (event: globalThis.KeyboardEvent) => {
      if (event.key.toLowerCase() !== 's' || (!event.metaKey && !event.ctrlKey)) return
      if (event.altKey) return
      event.preventDefault()
      void saveSelected()
    }
    window.addEventListener('keydown', save, true)
    return () => window.removeEventListener('keydown', save, true)
  }, [active, saveSelected])

  const activateTreeEntry = useCallback((entry: WorkingTreeEntry) => {
    if (entry.isDir) {
      setExpanded((current) => current.includes(entry.absolutePath)
        ? current.filter((path) => path !== entry.absolutePath)
        : [...current, entry.absolutePath])
    } else {
      onOpenFile(tabId, entry.relativePath, treeWidthRef.current)
    }
  }, [onOpenFile, tabId])

  const workingTreeEntries = tree.data ?? []
  const selectedTreeEntry = selected
    ? workingTreeEntries.find((entry) => entry.relativePath === selected)
    : undefined
  const treeTabStop = focusedTreeEntry && workingTreeEntries.some((entry) => entry.absolutePath === focusedTreeEntry)
    ? focusedTreeEntry
    : selectedTreeEntry?.absolutePath ?? workingTreeEntries[0]?.absolutePath

  const focusWorkingTreeIndex = useCallback((index: number) => {
    const entry = workingTreeEntries[index]
    if (!entry) return
    setFocusedTreeEntry(entry.absolutePath)
    focusVirtualTreeRow(treeList, index, workingTreeRowId(entry.absolutePath))
  }, [workingTreeEntries])

  const [fileOperationDialog, setFileOperationDialog] = useState<
    | { kind: 'createFile'; parent: string }
    | { kind: 'createDirectory'; parent: string }
    | { kind: 'rename'; entry: WorkingTreeEntry }
    | null
  >(null)
  const [operationName, setOperationName] = useState('')
  const [deleteTarget, setDeleteTarget] = useState<WorkingTreeEntry | null>(null)

  const createEntry = useCallback((directory: boolean, parent: string) => {
    setFileOperationDialog({ kind: directory ? 'createDirectory' : 'createFile', parent })
    setOperationName('')
  }, [])

  const renameEntry = useCallback((entry: WorkingTreeEntry) => {
    setFileOperationDialog({ kind: 'rename', entry })
    setOperationName(entry.name)
  }, [])

  const deleteEntry = useCallback((entry: WorkingTreeEntry) => {
    setDeleteTarget(entry)
  }, [])

  const handleWorkingTreeKeyDown = useCallback((
    event: ReactKeyboardEvent<HTMLElement>,
    entry: WorkingTreeEntry,
    index: number,
  ) => {
    if (event.key === 'F2') {
      event.preventDefault()
      renameEntry(entry)
      return
    }
    if (event.key === 'Delete' || (event.key === 'Backspace' && (event.metaKey || event.ctrlKey))) {
      event.preventDefault()
      deleteEntry(entry)
      return
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      setFocusedTreeEntry(entry.absolutePath)
      activateTreeEntry(entry)
      return
    }
    if (event.metaKey || event.ctrlKey || event.altKey) return
    if (!isTreeNavigationKey(event.key)) return
    event.preventDefault()
    const action = treeNavigationAction(
      workingTreeEntries.map((candidate) => ({
        depth: candidate.depth,
        directory: candidate.isDir,
        expanded: candidate.isDir && expanded.includes(candidate.absolutePath),
      })),
      index,
      event.key,
    )
    if (action.toggle) {
      setFocusedTreeEntry(entry.absolutePath)
      activateTreeEntry(entry)
    } else {
      focusWorkingTreeIndex(action.index)
    }
  }, [activateTreeEntry, deleteEntry, expanded, focusWorkingTreeIndex, renameEntry, workingTreeEntries])

  const updateSelectedBuffer = useCallback((content: string) => {
    if (!selected) return
    setBuffers((current) => {
      const buffer = current[selected]
      const fallback = file.data === undefined
        ? undefined
        : { content: file.data, diskContent: file.data, revision: 0, saving: false }
      const next = buffer ?? fallback
      if (!next || next.content === content) return current
      return { ...current, [selected]: { ...next, content } }
    })
  }, [file.data, selected])

  const retainSelectedEditor = useCallback((editor: Editor<undefined>) => {
    if (!selected) return
    setBuffers((current) => {
      const buffer = current[selected]
      const fallback = file.data === undefined
        ? undefined
        : { content: file.data, diskContent: file.data, revision: 0, saving: false }
      const next = buffer ?? fallback
      if (!next || next.editor === editor) return current
      return { ...current, [selected]: { ...next, editor } }
    })
  }, [file.data, selected, setBuffers])

  const refreshTree = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: daemonKeys.workspaceTree(config?.address ?? 'disconnected', root ?? 'none', expanded, showHidden) })
  }, [config?.address, expanded, queryClient, root, showHidden])

  const submitFileOperation = useCallback(async (e?: React.FormEvent) => {
    e?.preventDefault()
    if (!client || !root || !fileOperationDialog) return
    const name = operationName.trim()
    if (!name || name.includes('/') || name.includes('\\')) {
      toast.error(t('files.invalid_name'))
      return
    }
    try {
      if (fileOperationDialog.kind === 'createFile') {
        const parent = fileOperationDialog.parent
        await createWorkspaceFile(client, root, `${parent ? `${parent}/` : ''}${name}`)
      } else if (fileOperationDialog.kind === 'createDirectory') {
        const parent = fileOperationDialog.parent
        await createWorkspaceDirectory(client, root, `${parent ? `${parent}/` : ''}${name}`)
      } else {
        const entry = fileOperationDialog.entry
        const parent = entry.relativePath.split('/').slice(0, -1).join('/')
        await renameWorkspacePath(client, root, entry.relativePath, `${parent ? `${parent}/` : ''}${name}`)
        if (selected === entry.relativePath) setSelected(`${parent ? `${parent}/` : ''}${name}`)
      }
      setFileOperationDialog(null)
      refreshTree()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }, [client, fileOperationDialog, operationName, refreshTree, root, selected, t])

  const confirmDelete = useCallback(async () => {
    if (!client || !root || !deleteTarget) return
    try {
      await deleteWorkspacePath(client, root, deleteTarget.relativePath)
      if (selected === deleteTarget.relativePath) setSelected(null)
      setDeleteTarget(null)
      refreshTree()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }, [client, deleteTarget, refreshTree, root, selected])

  const refreshSelectedBuffer = useCallback(() => {
    if (!selected) return
    const buffer = buffersRef.current[selected]
    if (!buffer || buffer.content === buffer.diskContent) void refetchFile()
  }, [refetchFile, selected])

  if (!root) return <PanelMessage title={t('files.no_project_open')} detail={t('files.no_project_open_description')} />

  const fileTree = (
    <div
      className={cn('relative flex min-h-0 flex-col', selected ? 'shrink-0 border-l' : 'flex-1')}
      style={selected ? { width: fittedTreeWidth } : undefined}
    >
      {selected && (
        <PanelResizeHandle
          edge="left"
          label={t('files.resize_tree')}
          max={maxTreeWidth}
          min={140}
          value={fittedTreeWidth}
          onChange={setTreeWidth}
        />
      )}
      <div className="flex h-[42px] shrink-0 items-center justify-between border-b px-3 text-[11.5px] font-medium text-[var(--text-secondary)]">
        <div className="flex min-w-0 flex-1 items-center gap-1">
          {!selected && <>
            <PaduIcon className="ml-1 size-[13px] text-[var(--text-tertiary)]" name="folder" />
            <span className="min-w-0 truncate px-1">
              {project ? projectDisplayName(project, t('project.no_project_name')) : ''}
            </span>
          </>}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <button
            aria-label={t('command_palette.find_file')}
            className="grid size-6 place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            type="button"
            onClick={onFindFile}
          >
            <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="search" />
          </button>
          <button
            aria-label={t('files.new_file')}
            className="grid size-6 place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            type="button"
            onClick={() => void createEntry(false, '')}
          >
            <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="file" />
          </button>
          <button
            aria-label={t('files.new_folder')}
            className="grid size-6 place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            type="button"
            onClick={() => void createEntry(true, '')}
          >
            <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="folderNew" />
          </button>
          <button aria-label={t('files.refresh')} className="grid size-6 place-items-center rounded hover:bg-accent" type="button" onClick={refreshTree}>
            <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="rotateCw" />
          </button>
          <button aria-label={showHidden ? t('files.hide_hidden') : t('files.show_hidden')} className="grid size-6 place-items-center rounded hover:bg-accent" type="button" onClick={() => setShowHidden((value) => !value)}>
            <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name={showHidden ? 'eye' : 'eyeOff'} />
          </button>
        </div>
      </div>
      <ContextMenu.Root>
        <ContextMenu.Trigger
          className="relative min-h-0 flex-1 outline-none"
          onContextMenu={(event) => {
            if ((event.target as HTMLElement).closest('[role="treeitem"]')) {
              event.preventDefault()
            }
          }}
        >
          {tree.isPending ? (
            <p className="p-3 text-[11px] text-[var(--text-tertiary)]">{t('files.loading')}</p>
          ) : tree.error ? (
            <p className="p-3 text-[11px] text-destructive">{errorMessage(tree.error)}</p>
          ) : (
            <Virtuoso
              aria-label={t('files.workspace_files')}
              className="size-full py-1 outline-none"
              computeItemKey={(_, entry) => entry.absolutePath}
              data={tree.data ?? []}
              fixedItemHeight={30}
              increaseViewportBy={180}
              itemContent={(index, entry) => (
                <TreeRow
                  entry={entry}
                  expanded={entry.isDir && expanded.includes(entry.absolutePath)}
                  id={workingTreeRowId(entry.absolutePath)}
                  selected={selected === entry.relativePath}
                  tabIndex={treeTabStop === entry.absolutePath ? 0 : -1}
                  onActivate={activateTreeEntry}
                  onAddToChat={onAddToChat}
                  onCreateFile={() => {
                    const parent = entry.isDir
                      ? entry.relativePath
                      : entry.relativePath.split('/').slice(0, -1).join('/')
                    void createEntry(false, parent)
                  }}
                  onCreateFolder={() => {
                    const parent = entry.isDir
                      ? entry.relativePath
                      : entry.relativePath.split('/').slice(0, -1).join('/')
                    void createEntry(true, parent)
                  }}
                  onCopyPath={() => {
                    void navigator.clipboard.writeText(entry.absolutePath)
                    toast.success(t('files.copied_path', { path: entry.name }))
                  }}
                  onCopyRelativePath={() => {
                    void navigator.clipboard.writeText(entry.relativePath)
                    toast.success(t('files.copied_path', { path: entry.name }))
                  }}
                  onDelete={() => void deleteEntry(entry)}
                  onFocus={() => setFocusedTreeEntry(entry.absolutePath)}
                  t={t}
                  onKeyDown={(event) => handleWorkingTreeKeyDown(event, entry, index)}
                  onRename={() => void renameEntry(entry)}
                />
              )}
              ref={treeList}
              role="tree"
            />
          )}
        </ContextMenu.Trigger>
        <ContextMenu.Portal>
          <ContextMenu.Positioner className="z-[100] outline-none">
            <ContextMenu.Popup className="padu-menu-surface">
              <ContextMenu.Item
                className="padu-menu-item"
                onClick={() => void createEntry(false, '')}
              >
                <PaduIcon className="size-3" name="file" /> {t('files.new_file')}
              </ContextMenu.Item>
              <ContextMenu.Item
                className="padu-menu-item"
                onClick={() => void createEntry(true, '')}
              >
                <PaduIcon className="size-3" name="folderNew" /> {t('files.new_folder')}
              </ContextMenu.Item>
            </ContextMenu.Popup>
          </ContextMenu.Positioner>
        </ContextMenu.Portal>
      </ContextMenu.Root>
    </div>
  )

  const dialogs = (
    <>
      <Dialog open={fileOperationDialog !== null} onOpenChange={(open) => !open && setFileOperationDialog(null)}>
        <DialogContent className="max-w-[420px] overflow-hidden rounded-[14px] bg-[var(--raised)] p-5">
          <DialogTitle className="flex items-center justify-between text-[15px] font-semibold text-foreground">
            <span>
              {fileOperationDialog?.kind === 'createFile'
                ? t('files.new_file')
                : fileOperationDialog?.kind === 'createDirectory'
                  ? t('files.new_folder')
                  : t('common.rename')}
            </span>
          </DialogTitle>
          <form className="mt-4 flex flex-col gap-4" onSubmit={(e) => void submitFileOperation(e)}>
            <label className="flex flex-col gap-1.5 text-[12.5px] font-medium text-[var(--text-secondary)]">
              <span>{t('files.name_placeholder')}</span>
              <Input
                autoFocus
                className="h-8 bg-card"
                placeholder={t('files.name_placeholder')}
                value={operationName}
                onChange={(e) => setOperationName(e.target.value)}
              />
            </label>
            <div className="flex items-center justify-end gap-2 pt-1">
              <Button
                className="gap-1.5"
                size="sm"
                type="button"
                variant="outline"
                onClick={() => setFileOperationDialog(null)}
              >
                <span>{t('common.cancel')}</span>
                <Kbd size="xs" variant="outline">Esc</Kbd>
              </Button>
              <Button className="gap-1.5" size="sm" type="submit">
                <span>
                  {fileOperationDialog?.kind === 'rename'
                    ? t('common.rename')
                    : t('files.confirm')}
                </span>
                <Kbd size="xs" variant="onPrimary" className="px-1">
                  <PaduIcon name="cornerDownLeft" className="size-2.5" />
                </Kbd>
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t('files.delete')}
        description={t('files.delete_confirm', { name: deleteTarget?.name ?? '' })}
        confirmLabel={t('files.delete')}
        cancelLabel={t('common.cancel')}
        variant="danger"
        icon="trash"
        onConfirm={confirmDelete}
        onCancel={() => setDeleteTarget(null)}
      />
    </>
  )

  if (!selected) {
    return (
      <>
        {fileTree}
        {dialogs}
      </>
    )
  }
  const buffer = buffers[selected] ?? (file.data === undefined
    ? undefined
    : {
        content: file.data,
        diskContent: file.data,
        revision: 0,
        saving: false,
      })
  const isDirty = Boolean(buffer && buffer.content !== buffer.diskContent)
  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <FileBreadcrumbs path={selected} dirty={isDirty} />
        {!buffer && file.isPending
          ? <PanelMessage title={t('files.loading_file')} detail={t('files.reading_from_daemon')} />
          : !buffer && file.error
            ? <PanelMessage title={t('files.file_unavailable')} detail={errorMessage(file.error)} danger />
            : buffer && (
                <Suspense fallback={<PanelMessage title={t('files.loading_editor')} detail={t('files.preparing_syntax')} />}>
                  <CodeFileSurface
                    cacheKey={`editor:${selected}:${buffer.revision}`}
                    contents={buffer.content}
                    editor={buffer.editor}
                    key={`${selected}:${buffer.revision}`}
                    path={selected}
                    onChange={updateSelectedBuffer}
                    onEditor={retainSelectedEditor}
                    onFocus={refreshSelectedBuffer}
                  />
                </Suspense>
              )}
      </div>
      {fileTree}
      {dialogs}
    </div>
  )
}

interface TreeRowProps {
  entry: WorkingTreeEntry
  expanded: boolean
  id: string
  selected: boolean
  tabIndex: number
  onActivate: (entry: WorkingTreeEntry) => void
  onAddToChat?: (name: string, isDir?: boolean) => void
  onCreateFile: () => void
  onCreateFolder: () => void
  onCopyPath: () => void
  onCopyRelativePath: () => void
  onDelete: () => void
  onFocus: () => void
  onKeyDown: (event: ReactKeyboardEvent<HTMLElement>) => void
  onRename: () => void
  t: Translator
}

function TreeRow({
  entry,
  expanded,
  id,
  selected,
  tabIndex,
  onActivate,
  onAddToChat,
  onCreateFile,
  onCreateFolder,
  onCopyPath,
  onCopyRelativePath,
  onDelete,
  onFocus,
  onKeyDown,
  onRename,
  t,
}: TreeRowProps) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger
        aria-expanded={entry.isDir ? expanded : undefined}
        aria-level={entry.depth + 1}
        className={cn(
          'mx-2 flex h-[30px] min-h-[30px] shrink-0 min-w-0 items-center gap-1.5 rounded-md pr-2 text-left text-[11.5px] outline-none hover:bg-accent focus-visible:bg-accent',
          selected && 'bg-accent',
          entry.isIgnored && 'opacity-55 text-[var(--text-ghost)]',
        )}
        id={id}
        role="treeitem"
        style={{ paddingLeft: `${8 + entry.depth * 16}px`, width: 'calc(100% - 16px)' }}
        tabIndex={tabIndex}
        onClick={() => onActivate(entry)}
        onContextMenu={(event) => {
          event.stopPropagation()
        }}
        onFocus={onFocus}
        onKeyDown={(event) => {
          if ((event.shiftKey && event.key === 'F10') || event.key === 'ContextMenu') {
            event.preventDefault()
            event.currentTarget.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true }))
            return
          }
          onKeyDown(event)
        }}
      >
        {entry.isDir
          ? expanded
            ? <PaduIcon className="size-2.5 shrink-0 text-[var(--text-ghost)]" name="chevronDown" />
            : <PaduIcon className="size-2.5 shrink-0 text-[var(--text-ghost)]" name="chevronRight" />
          : <span className="size-2.5 shrink-0" />}
        {entry.isDir
          ? <PaduIcon className="size-3.5 shrink-0 text-[var(--text-tertiary)]" name={expanded ? 'folderOpen' : 'folder'} />
          : <FileTypeIcon className="size-3.5" path={entry.name} />}
        <span className="truncate">{entry.name}</span>
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Positioner className="z-[100] outline-none">
          <ContextMenu.Popup className="padu-menu-surface">
            {onAddToChat && (
              <>
                <ContextMenu.Item className="padu-menu-item" onClick={() => onAddToChat(entry.name, entry.isDir)}>
                  <PaduIcon className="size-3" name="compose" /> {t('files.add_to_chat')}
                </ContextMenu.Item>
                <ContextMenu.Separator className="padu-menu-separator" />
              </>
            )}
            <ContextMenu.Item className="padu-menu-item" onClick={onCreateFile}><PaduIcon className="size-3" name="file" /> {t('files.new_file')}</ContextMenu.Item>
            <ContextMenu.Item className="padu-menu-item" onClick={onCreateFolder}><PaduIcon className="size-3" name="folderNew" /> {t('files.new_folder')}</ContextMenu.Item>
            <ContextMenu.Separator className="padu-menu-separator" />
            <ContextMenu.Item className="padu-menu-item" onClick={onCopyPath}><PaduIcon className="size-3" name="copy" /> {t('files.copy_path')}</ContextMenu.Item>
            <ContextMenu.Item className="padu-menu-item" onClick={onCopyRelativePath}><PaduIcon className="size-3" name="copy" /> {t('files.copy_relative_path')}</ContextMenu.Item>
            <ContextMenu.Item className="padu-menu-item" onClick={onRename}><PaduIcon className="size-3" name="pencil" /> {t('common.rename')}</ContextMenu.Item>
            <ContextMenu.Separator className="padu-menu-separator" />
            <ContextMenu.Item className="padu-menu-item text-destructive" onClick={onDelete}><PaduIcon className="size-3" name="trash" /> {t('files.delete')}</ContextMenu.Item>
          </ContextMenu.Popup>
        </ContextMenu.Positioner>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}

function ChangesPanel({
  session,
  project,
  diffSource,
  panelWidth,
  onDiffSourceChange,
}: {
  session: AgentSession | null
  project?: Project
  diffSource: ReviewDiffSource
  panelWidth: number
  onDiffSourceChange: (source: ReviewDiffSource) => void
}) {
  const { t } = useI18n()
  const { client, config, phase } = useDaemon()
  const diffView = useRef<CodeDiffSurfaceHandle>(null)
  const diffTreeList = useRef<VirtuosoHandle>(null)
  const [files, setFiles] = useState<DiffSurfaceFile[]>([])
  const [selectedFile, setSelectedFile] = useState<string | null>(null)
  const [focusedDiffRow, setFocusedDiffRow] = useState<string | null>(null)
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())
  const [filter, setFilter] = useState('')
  const [treeWidth, setTreeWidth] = useState(() => readStoredWidth('padu.diffTreeWidth', 184, 140, 360))
  const root = session && project ? sessionCwd(session, project) : undefined
  const maxTreeWidth = Math.max(140, Math.min(360, panelWidth - 140))
  const fittedTreeWidth = clamp(treeWidth, 140, maxTreeWidth)
  const latestTurnSource = latestReviewTurnSource(session)
  const sourceLabel = reviewDiffSourceLabel(diffSource, latestTurnSource, t)
  const diff = useQuery({
    queryKey: daemonKeys.workspaceDiff(config?.address ?? 'disconnected', root ?? 'none', diffSource),
    queryFn: () => collectWorkspaceDiff(requireClient(client), root!, diffSource),
    enabled: phase === 'connected' && Boolean(client && config && root),
  })
  const reviewFiles = mergeReviewDiffFiles(files, diff.data?.numstat ?? '') as DiffSurfaceFile[]

  useEffect(() => {
    setExpandedPaths(diffDirectoryPaths(reviewFiles))
    setSelectedFile((current) => reviewFiles.some((file) => file.id === current)
      ? current
      : reviewFiles[0]?.id ?? null)
    setFilter('')
  }, [files, diff.data?.numstat])

  useEffect(() => {
    setFiles([])
    setSelectedFile(null)
    setExpandedPaths(new Set())
    setFilter('')
  }, [diffSource])

  useEffect(() => {
    const timer = window.setTimeout(() => {
      window.localStorage.setItem('padu.diffTreeWidth', String(Math.round(treeWidth)))
    }, 150)
    return () => window.clearTimeout(timer)
  }, [treeWidth])

  const treeRows = buildDiffTreeRows(reviewFiles, expandedPaths, filter)
  const selectedDiffRow = selectedFile
    ? treeRows.find((row) => row.kind === 'file' && row.file.id === selectedFile)
    : undefined
  const diffTreeTabStop = focusedDiffRow && treeRows.some((row) => diffTreeRowKey(row) === focusedDiffRow)
    ? focusedDiffRow
    : selectedDiffRow ? diffTreeRowKey(selectedDiffRow) : treeRows[0] ? diffTreeRowKey(treeRows[0]) : null

  const toggleDiffDirectory = useCallback((path: string) => {
    setExpandedPaths((current) => {
      const next = new Set(current)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }, [])

  const focusDiffTreeIndex = useCallback((index: number) => {
    const row = treeRows[index]
    if (!row) return
    const key = diffTreeRowKey(row)
    setFocusedDiffRow(key)
    focusVirtualTreeRow(diffTreeList, index, diffTreeRowId(key))
  }, [treeRows])

  const handleDiffTreeKeyDown = useCallback((
    event: ReactKeyboardEvent<HTMLButtonElement>,
    row: DiffTreeRow,
    index: number,
  ) => {
    if (event.metaKey || event.ctrlKey || event.altKey) return
    if (!isTreeNavigationKey(event.key)) return
    event.preventDefault()
    const action = treeNavigationAction(
      treeRows.map((candidate) => ({
        depth: candidate.depth,
        directory: candidate.kind === 'directory',
        expanded: candidate.kind === 'directory' && candidate.expanded,
      })),
      index,
      event.key,
    )
    if (action.toggle && row.kind === 'directory') {
      setFocusedDiffRow(diffTreeRowKey(row))
      toggleDiffDirectory(row.path)
    } else {
      focusDiffTreeIndex(action.index)
    }
  }, [focusDiffTreeIndex, toggleDiffDirectory, treeRows])

  if (!root) return <PanelMessage title={t('files.no_project_open')} detail={t('files.no_project_open_description')} />
  const stats = parseNumstat(diff.data?.numstat ?? '')
  const selectSource = (source: ReviewDiffSource) => {
    if (!sameReviewDiffSource(diffSource, source)) onDiffSourceChange(source)
  }
  const sourceItems = [
    {
      id: 'last-turn',
      label: t('diff.source_last_turn'),
      disabled: !latestTurnSource,
      selected: Boolean(latestTurnSource && sameReviewDiffSource(diffSource, latestTurnSource)),
      onSelect: () => latestTurnSource && selectSource(latestTurnSource),
    },
    {
      id: 'uncommitted',
      label: t('diff.source_uncommitted'),
      separatorBefore: true,
      selected: diffSource === 'uncommitted',
      onSelect: () => selectSource('uncommitted'),
    },
    {
      id: 'unstaged',
      label: t('diff.source_unstaged'),
      selected: diffSource === 'unstaged',
      onSelect: () => selectSource('unstaged'),
    },
    {
      id: 'staged',
      label: t('diff.source_staged'),
      selected: diffSource === 'staged',
      onSelect: () => selectSource('staged'),
    },
    {
      id: 'committed',
      label: t('diff.source_committed'),
      separatorBefore: true,
      selected: diffSource === 'committed',
      onSelect: () => selectSource('committed'),
    },
    {
      id: 'branch',
      label: t('diff.source_branch'),
      selected: diffSource === 'branch',
      onSelect: () => selectSource('branch'),
    },
  ]

  let reviewContent: ReactNode
  if (diff.isPending) {
    reviewContent = <PanelMessage title={t('diff.loading')} detail={t('diff.loading_description')} />
  } else if (diff.error) {
    reviewContent = <PanelMessage title={t('diff.unavailable')} detail={errorMessage(diff.error)} danger />
  } else if (!diff.data?.patch.trim()) {
    reviewContent = <PanelMessage title={t('diff.no_changes')} detail={t('diff.no_changes_description')} />
  } else {
    const data = diff.data
    reviewContent = (
      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1">
          <Suspense fallback={<PanelMessage title={t('diff.loading')} detail={t('files.preparing_syntax')} />}>
            <CodeDiffSurface
              completeContext={data.completeContext}
              onFiles={setFiles}
              patch={data.patch}
              ref={diffView}
            />
          </Suspense>
        </div>
        <div
          className="relative flex min-h-0 shrink-0 flex-col border-l bg-background"
          style={{ width: fittedTreeWidth }}
        >
          <PanelResizeHandle
            edge="left"
            label={t('diff.resize_tree')}
            max={maxTreeWidth}
            min={140}
            value={fittedTreeWidth}
            onChange={setTreeWidth}
          />
          <label className="flex h-11 shrink-0 items-center gap-2 border-b px-2">
            <PaduIcon className="size-[13px] shrink-0 text-[var(--text-tertiary)]" name="search" />
            <input
              aria-label={t('diff.filter_files')}
              className="min-w-0 flex-1 bg-transparent text-[11px] outline-none placeholder:text-[var(--text-ghost)]"
              placeholder={t('diff.filter_files')}
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
            />
          </label>
          <Virtuoso
            aria-label={t('diff.changed_files')}
            className="min-h-0 flex-1 py-1"
            computeItemKey={(_, row) => row.kind === 'directory'
              ? `directory:${row.path}`
              : `file:${row.file.id}`}
            data={treeRows}
            fixedItemHeight={30}
            increaseViewportBy={180}
            itemContent={(index, row) => row.kind === 'directory' ? (
              <button
                aria-expanded={row.expanded}
                aria-level={row.depth + 1}
                className="mx-1.5 my-0.5 flex h-[26px] min-h-[26px] shrink-0 min-w-0 items-center gap-1.5 rounded-md pr-2 text-left text-[11px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:bg-accent"
                id={diffTreeRowId(diffTreeRowKey(row))}
                role="treeitem"
                style={{ paddingLeft: `${7 + row.depth * 14}px`, width: 'calc(100% - 12px)' }}
                tabIndex={diffTreeTabStop === diffTreeRowKey(row) ? 0 : -1}
                type="button"
                onClick={() => toggleDiffDirectory(row.path)}
                onFocus={() => setFocusedDiffRow(diffTreeRowKey(row))}
                onKeyDown={(event) => handleDiffTreeKeyDown(event, row, index)}
              >
                <PaduIcon className="size-2.5 shrink-0 text-[var(--text-ghost)]" name={row.expanded ? 'chevronDown' : 'chevronRight'} />
                <PaduIcon className="size-[13px] shrink-0 text-[var(--text-tertiary)]" name="folder" />
                <span className="min-w-0 flex-1 truncate font-medium">{row.name}</span>
              </button>
            ) : (
              <button
                aria-level={row.depth + 1}
                aria-selected={selectedFile === row.file.id}
                className={cn(
                  'mx-1.5 my-0.5 flex h-[26px] min-h-[26px] shrink-0 min-w-0 items-center gap-1.5 rounded-md pr-2 text-left text-[11px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:bg-accent',
                  selectedFile === row.file.id && 'bg-accent text-foreground',
                )}
                id={diffTreeRowId(diffTreeRowKey(row))}
                role="treeitem"
                style={{ paddingLeft: `${23 + row.depth * 14}px`, width: 'calc(100% - 12px)' }}
                tabIndex={diffTreeTabStop === diffTreeRowKey(row) ? 0 : -1}
                title={row.file.path}
                type="button"
                onClick={() => {
                  setSelectedFile(row.file.id)
                  diffView.current?.scrollToFile(row.file.id)
                }}
                onFocus={() => setFocusedDiffRow(diffTreeRowKey(row))}
                onKeyDown={(event) => handleDiffTreeKeyDown(event, row, index)}
              >
                <FileTypeIcon className="size-[13px]" path={row.file.path} />
                <span className="min-w-0 flex-1 truncate">{fileName(row.file.path)}</span>
                <DiffFileStatus status={row.file.status} />
              </button>
            )}
            ref={diffTreeList}
            role="tree"
          />
        </div>
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex min-h-10 items-center gap-2 border-b px-3 text-[11px]">
        <ControlMenu
          items={sourceItems}
          label={sourceLabel}
          menuClassName="w-44"
          placement="below"
          triggerClassName="h-7 max-w-44 bg-background px-2"
        />
        {diff.data && (
          <>
            <span className="font-medium text-[var(--success)]">+{stats.additions}</span>
            <span className="font-medium text-destructive">-{stats.deletions}</span>
            {!diff.data.completeContext && (
              <span className="truncate text-[var(--text-tertiary)]">{t('diff.truncated')}</span>
            )}
          </>
        )}
        <div className="flex-1" />
        <button aria-label={t('diff.refresh')} className="rounded p-1 hover:bg-accent" type="button" onClick={() => void diff.refetch()}>
          <PaduIcon className={cn('size-3.5', diff.isFetching && 'motion-safe:animate-spin')} name="rotateCw" />
        </button>
      </div>
      {reviewContent}
    </div>
  )
}

type DiffTreeRow = {
  kind: 'directory'
  path: string
  name: string
  depth: number
  expanded: boolean
} | {
  kind: 'file'
  file: DiffSurfaceFile
  depth: number
}

function buildDiffTreeRows(
  files: DiffSurfaceFile[],
  expandedPaths: Set<string>,
  filter: string,
): DiffTreeRow[] {
  const needle = filter.trim().toLocaleLowerCase()
  const filtering = needle.length > 0
  const rows: DiffTreeRow[] = []
  const emittedDirectories = new Set<string>()

  for (const file of [...files]
    .filter((candidate) => !filtering || candidate.path.toLocaleLowerCase().includes(needle))
    .sort((left, right) => left.path.localeCompare(right.path))) {
    const parts = file.path.split('/').filter(Boolean)
    let directory = ''
    let visible = true
    for (const [depth, part] of parts.slice(0, -1).entries()) {
      directory = directory ? `${directory}/${part}` : part
      const expanded = filtering || expandedPaths.has(directory)
      if (!emittedDirectories.has(directory) && visible) {
        emittedDirectories.add(directory)
        rows.push({ kind: 'directory', path: directory, name: part, depth, expanded })
      }
      if (!expanded) {
        visible = false
        break
      }
    }
    if (visible) rows.push({ kind: 'file', file, depth: Math.max(0, parts.length - 1) })
  }
  return rows
}

function diffDirectoryPaths(files: DiffSurfaceFile[]): Set<string> {
  const paths = new Set<string>()
  for (const file of files) {
    const parts = file.path.split('/').filter(Boolean)
    let directory = ''
    for (const part of parts.slice(0, -1)) {
      directory = directory ? `${directory}/${part}` : part
      paths.add(directory)
    }
  }
  return paths
}

function DiffFileStatus({ status }: { status: DiffSurfaceFile['status'] }) {
  return (
    <span className={cn(
      'grid size-4 shrink-0 place-items-center rounded border text-[9px] font-semibold',
      status === 'A' && 'border-[var(--success)]/60 text-[var(--success)]',
      status === 'D' && 'border-destructive/60 text-destructive',
      (status === 'B' || status === 'M') && 'border-[var(--warning)]/60 text-[var(--warning)]',
    )}>
      {status}
    </span>
  )
}

function fileName(path: string): string {
  return path.split('/').at(-1) ?? path
}

function BackgroundWorkPanel({
  session,
  workKey,
  onTitle,
}: {
  session: AgentSession | null
  workKey: BackgroundWorkKey
  onTitle: (title: string) => void
}) {
  const { t } = useI18n()
  const { backgroundWork, stopBackgroundWork } = useRuntime()
  const reportedTitle = useRef<string | null>(null)
  const item = session
    ? backgroundWork[session.id]?.find((candidate) => sameBackgroundWorkKey(candidate.key, workKey))
    : undefined

  useEffect(() => {
    if (!item?.title) return
    const reportKey = `${item.key.kind}:${item.key.providerId}:${item.title}`
    if (reportedTitle.current === reportKey) return
    reportedTitle.current = reportKey
    onTitle(item.title)
  }, [item?.title, onTitle])

  if (!session || !item) {
    return (
      <div className="grid min-h-0 flex-1 place-items-center p-6 text-center">
        <div>
          <PaduIcon className="mx-auto size-[22px] text-[var(--text-ghost)]" name={backgroundWorkKindIcon(workKey.kind)} />
          <p className="mt-2 text-[12px] text-[var(--text-secondary)]">{t('background.unavailable')}</p>
        </div>
      </div>
    )
  }

  const stoppable = isStoppableBackgroundStatus(item.status) && item.canStop && item.controlId
  const metadata = [
    [t(item.key.kind === 'subagent' ? 'background.prompt' : 'background.command'), item.command],
    [t('background.cwd'), item.cwd],
    [t('background.role'), item.role],
    [t('background.model'), item.model],
    [t('background.latest_update'), item.detail],
    [t('background.exit_code'), item.exitCode == null ? null : String(item.exitCode)],
  ].filter((entry): entry is [string, string] => Boolean(entry[1]))
  return (
    <div className="min-h-0 flex-1 overflow-auto p-3">
      <div className="overflow-hidden rounded-[9px] border bg-card">
        <div className="flex min-h-[54px] items-center gap-2.5 px-[11px] py-2">
          <PaduIcon className="size-[15px] text-[var(--text-secondary)]" name={backgroundWorkKindIcon(item.key.kind)} />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[12px] font-medium">{item.title}</div>
            <div className="mt-1 flex items-center gap-1.5 text-[10px] text-[var(--text-tertiary)]">
              <BackgroundStatusIcon status={item.status} />
              <span>{backgroundStatusLabel(item.status, t)}</span>
              <span>·</span>
              <span>{backgroundElapsed(item, t)}</span>
            </div>
          </div>
          {stoppable && (
            <Button
              className="h-[26px] gap-1.5 px-[9px] text-[10.5px] hover:bg-destructive/10"
              size="sm"
              variant="outline"
              onClick={() => void stopBackgroundWork(session.id, item).catch(() => {})}
            >
              <PaduIcon className="size-[11px] text-destructive" name="stopFilled" />
              {t('background.stop')}
            </Button>
          )}
        </div>
        {metadata.map(([label, value]) => (
          <div className="border-t px-2.5 py-[7px]" key={label}>
            <div className="text-[9.5px] text-[var(--text-tertiary)]">{label}</div>
            <div className="mt-[3px] whitespace-pre-wrap break-words font-mono text-[10.5px] text-[var(--text-secondary)]">{value}</div>
          </div>
        ))}
        <div className="border-t p-2.5">
          <div className="mb-[5px] flex items-center justify-between text-[9.5px] text-[var(--text-tertiary)]">
            <span>{t('background.output')}</span>
            {item.outputTruncated && <span>{t('background.output_truncated')}</span>}
          </div>
          <pre className="max-h-80 overflow-auto rounded-md bg-[var(--inset)] p-2 font-mono text-[10.5px] leading-[15px] text-[var(--text-secondary)]">
            {stripAnsi(item.output || t('background.no_output'))}
          </pre>
        </div>
      </div>
    </div>
  )
}

function BackgroundStatusIcon({ status }: { status: BackgroundWorkStatus }) {
  const icon: PaduIconName = status === 'completed'
    ? 'check'
    : status === 'failed'
      ? 'x'
      : status === 'lost'
        ? 'alert'
        : status === 'stopping' || status === 'stopped'
          ? 'stop'
          : 'loaderCircle'
  return (
    <PaduIcon
      className={cn(
        'size-[9px]',
        isStoppableBackgroundStatus(status) && 'motion-safe:animate-spin text-ring',
        status === 'completed' && 'text-[var(--success)]',
        (status === 'failed' || status === 'lost') && 'text-destructive',
      )}
      name={icon}
    />
  )
}

function TerminalPanel({
  terminalId,
  session,
  project,
  onTitle,
}: {
  terminalId: string
  session: AgentSession | null
  project?: Project
  onTitle: (title: string) => void
}) {
  const { t } = useI18n()
  const { client, phase } = useDaemon()
  const { ref, write, focus } = useTerminal()
  const [core, setCore] = useState<GhosttyCore | null>(null)
  const ready = useRef(false)
  const queuedOutput = useRef<Uint8Array[]>([])
  const titleScanner = useRef(new TerminalTitleScanner())
  const [error, setError] = useState<string | null>(null)
  const [exited, setExited] = useState(false)
  const cwd = session && project ? sessionCwd(session, project) : undefined

  useEffect(() => {
    let disposed = false
    void GhosttyCore.load()
      .then((loaded) => {
        if (!disposed) setCore(loaded)
      })
      .catch((cause) => {
        if (!disposed) setError(errorMessage(cause))
      })
    return () => {
      disposed = true
    }
  }, [])

  useEffect(() => {
    if (!client || phase !== 'connected' || !cwd) return
    let disposed = false
    titleScanner.current = new TerminalTitleScanner()
    const unsubscribe = client.subscribe(terminalId, terminalId, (event) => {
      if (event.event.kind === 'terminalOutput') {
        const payload = event.event.payload as { data?: string }
        if (!payload.data) return
        const bytes = decodeBase64(payload.data)
        const title = titleScanner.current.feed(bytes)
        if (title) onTitle(title)
        if (ready.current) write(bytes)
        else queuedOutput.current.push(bytes)
      } else if (event.event.kind === 'terminalExited') {
        setExited(true)
      } else if (event.event.kind === 'terminalError') {
        setError(typeof event.event.payload === 'string' ? event.event.payload : t('terminal.transport_failed'))
      }
    })

    void client.request(
      { type: 'openTerminal', cwd, cols: 80, rows: 24 },
      terminalId,
      terminalId,
    ).catch((cause) => {
      if (!disposed) setError(errorMessage(cause))
    })

    return () => {
      disposed = true
      ready.current = false
      queuedOutput.current = []
      unsubscribe()
      void client.notify({ type: 'closeTerminal' }, terminalId, terminalId).catch(() => {})
    }
  }, [client, cwd, phase, terminalId, write])

  if (!cwd) return <PanelMessage title={t('files.no_project_open')} detail={t('terminal.no_workspace_description')} />
  return (
    <div className="relative flex min-h-0 flex-1 bg-background">
      {core && (
        <Terminal
          autoResize
          className="padu-terminal size-full"
          core={core}
          cursorBlink
          ref={ref}
          onData={(data) => {
            if (!client || phase !== 'connected' || exited) return
            void client.notify(
              { type: 'writeTerminal', data: encodeBase64(new TextEncoder().encode(data)) },
              terminalId,
              terminalId,
            ).catch((cause) => setError(errorMessage(cause)))
          }}
          onError={(cause) => setError(errorMessage(cause))}
          onReady={() => {
            ready.current = true
            for (const bytes of queuedOutput.current) write(bytes)
            queuedOutput.current = []
            focus()
          }}
          onResize={(cols, rows) => {
            if (!client || phase !== 'connected') return
            void client.notify(
              { type: 'resizeTerminal', cols: clampU16(cols), rows: clampU16(rows) },
              terminalId,
              terminalId,
            ).catch(() => {})
          }}
        />
      )}
      {(error || exited) && (
        <div className="pointer-events-none absolute bottom-2 right-2 max-w-[calc(100%-16px)] rounded-md border bg-popover px-2 py-1 text-[10.5px] text-[var(--text-secondary)] shadow-sm">
          {error ?? t('terminal.process_exited')}
        </div>
      )}
    </div>
  )
}

class TerminalTitleScanner {
  private readonly decoder = new TextDecoder()
  private pending = ''

  feed(bytes: Uint8Array): string | null {
    const text = this.pending + this.decoder.decode(bytes, { stream: true })
    const pattern = /\x1b\](?:0|2);([^\x07\x1b]*)(?:\x07|\x1b\\)/g
    let title: string | null = null
    let consumed = 0
    for (const match of text.matchAll(pattern)) {
      title = match[1]?.replace(/[\x00-\x1f\x7f]/g, '').trim().slice(0, 80) || null
      consumed = (match.index ?? 0) + match[0].length
    }

    const incompleteOsc = text.lastIndexOf('\x1b]')
    if (incompleteOsc >= consumed) this.pending = text.slice(incompleteOsc, incompleteOsc + 1_024)
    else this.pending = text.endsWith('\x1b') ? '\x1b' : ''
    return title
  }
}

function PanelMessage({ title, detail, danger = false }: { title: string; detail: string; danger?: boolean }) {
  return (
    <div className="grid min-h-0 flex-1 place-items-center p-6 text-center">
      <div className="max-w-64">
        <div className={cn('text-[13px] font-medium', danger && 'text-destructive')}>{title}</div>
        <p className="mt-1.5 text-[11px] leading-4 text-[var(--text-tertiary)]">{detail}</p>
      </div>
    </div>
  )
}

function parseNumstat(numstat: string) {
  let files = 0
  let additions = 0
  let deletions = 0
  for (const line of numstat.trim().split('\n')) {
    if (!line) continue
    const [added, removed] = line.split('\t')
    files += 1
    additions += Number.parseInt(added || '0', 10) || 0
    deletions += Number.parseInt(removed || '0', 10) || 0
  }
  return { files, additions, deletions }
}

function encodeBase64(bytes: Uint8Array): string {
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000))
  }
  return btoa(binary)
}

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}

function clampU16(value: number): number {
  return Math.max(1, Math.min(65_535, Math.round(value)))
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

export function panelTabId(tabId: string): string {
  return `right-panel-tab-${tabId}`
}

export function panelContentId(tabId: string): string {
  return `right-panel-content-${tabId}`
}

function isTreeNavigationKey(key: string): key is TreeNavigationKey {
  return ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(key)
}

function workingTreeRowId(path: string): string {
  return `working-tree-row-${encodeURIComponent(path)}`
}

function diffTreeRowKey(row: DiffTreeRow): string {
  return row.kind === 'directory' ? `directory:${row.path}` : `file:${row.file.id}`
}

function diffTreeRowId(key: string): string {
  return `diff-tree-row-${encodeURIComponent(key)}`
}

function focusVirtualTreeRow(
  list: { current: VirtuosoHandle | null },
  index: number,
  id: string,
) {
  list.current?.scrollIntoView({ index, behavior: 'auto' })
  let attempts = 0
  const focus = () => {
    const row = document.getElementById(id)
    if (row) {
      row.focus()
      return
    }
    attempts += 1
    if (attempts < 4) window.requestAnimationFrame(focus)
  }
  window.requestAnimationFrame(focus)
}

function readStoredWidth(key: string, fallback: number, min: number, max: number): number {
  if (typeof window === 'undefined') return fallback
  const raw = window.localStorage.getItem(key)
  if (raw === null) return fallback
  const value = Number(raw)
  return Number.isFinite(value) ? clamp(value, min, max) : fallback
}

function absoluteParentPaths(root: string | undefined, relativePath: string) {
  if (!root) return []
  const separator = root.includes('\\') ? '\\' : '/'
  const normalizedRoot = root.replace(/[\\/]+$/, '') || separator
  const parts = relativePath.split(/[\\/]/).filter(Boolean)
  const paths: string[] = []
  let current = normalizedRoot
  for (const part of parts.slice(0, -1)) {
    current = current === separator ? `${current}${part}` : `${current}${separator}${part}`
    paths.push(current)
  }
  return paths
}

function useViewportWidth(): number {
  const [width, setWidth] = useState(() => typeof window === 'undefined' ? 1_440 : window.innerWidth)

  useEffect(() => {
    const update = () => setWidth(window.innerWidth)
    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [])

  return width
}

function sameBackgroundWorkKey(left: BackgroundWorkKey, right: BackgroundWorkKey) {
  return left.kind === right.kind && left.providerId === right.providerId
}

function backgroundWorkKindIcon(kind?: BackgroundWorkKey['kind']): PaduIconName {
  return kind === 'subagent' ? 'bot' : 'terminalSquare'
}

function backgroundWorkKindLabel(
  kind: BackgroundWorkKey['kind'] | undefined,
  t: Translator,
) {
  return t(kind === 'subagent'
    ? 'background.subagent'
    : kind === 'monitor'
      ? 'background.monitor'
      : 'background.process')
}

function isStoppableBackgroundStatus(status: BackgroundWorkStatus) {
  return status === 'starting' || status === 'running' || status === 'monitoring'
}

function backgroundStatusLabel(status: BackgroundWorkStatus, t: Translator) {
  return t(`background.status.${status}`)
}

function backgroundElapsed(item: BackgroundWorkItem, t: Translator) {
  const duration = item.durationMs ?? Math.max(0, Date.now() - item.startedAtMs)
  return formatWorkingElapsed(Math.floor(duration / 1_000), t)
}

function stripAnsi(value: string) {
  return value.replace(new RegExp('\\u001b\\[[0-?]*[ -/]*[@-~]', 'g'), '')
}

function requireClient<T>(client: T | null): T {
  if (!client) throw new Error('Padu daemon is disconnected')
  return client
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
