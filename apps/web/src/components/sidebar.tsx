import type { AgentSession, Project } from '@padu/client'
import { ContextMenu } from '@base-ui/react/context-menu'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Virtuoso } from 'react-virtuoso'
import { Button } from '@/components/ui/button'
import { ControlMenu, type ControlMenuItem } from '@/components/control-menu'
import { HostDialog } from '@/components/host-dialog'
import { Input } from '@/components/ui/input'
import { Kbd } from '@/components/ui/kbd'
import { Tooltip } from '@/components/ui/tooltip'
import { PanelResizeHandle } from '@/components/panel-resize-handle'
import { PaduIcon } from '@/components/padu-icon'
import { DeleteSessionDialog } from '@/components/delete-session-dialog'
import { displayHost } from '@/lib/connection'
import { displayTitle, type TaskState } from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { usePrimaryShortcut } from '@/lib/platform'
import {
  groupSessions,
  nextSidebarUpdateDelay,
  sessionTimeLabel,
  sidebarRows,
  type DateGroup,
  type SessionGroup,
  type SessionItem,
  type SidebarGrouping,
  type SidebarOrdering,
} from '@/lib/sidebar-presentation'
import { cn } from '@/lib/utils'
import paduAppIconUrl from '../../../landing/public/app-icon.png'

interface SidebarProps {
  taskState: TaskState
  selectedSessionId?: string
  mobileOpen: boolean
  width: number
  onMobileOpenChange: (open: boolean) => void
  onToggleSidebar: () => void
  onWidthChange: (width: number) => void
  onNewTask: (project?: Project) => void
  onAddProject: () => void
  onSelectSession: (sessionId: string) => void
  onRenameSession: (sessionId: string, title: string) => Promise<void>
  onRemoveSession: (sessionId: string) => Promise<void>
  onSetSessionPinned: (sessionId: string, pinned: boolean) => Promise<void>
  onSetSessionArchived: (sessionId: string, archived: boolean) => Promise<void>
  onSearch: () => void
  onSettings: () => void
  onUsage?: () => void
  onNotes?: () => void
}

const GROUP_TRANSLATION_KEYS: Record<DateGroup, string> = {
  today: 'sidebar.today',
  yesterday: 'sidebar.yesterday',
  week: 'sidebar.this_week',
  month: 'sidebar.this_month',
  year: 'sidebar.this_year',
  more: 'sidebar.more',
}

type Translator = (key: string, params?: Record<string, string | number>) => string

export function Sidebar({
  taskState,
  selectedSessionId,
  mobileOpen,
  width,
  onMobileOpenChange,
  onToggleSidebar,
  onWidthChange,
  onNewTask,
  onAddProject,
  onSelectSession,
  onRenameSession,
  onRemoveSession,
  onSetSessionPinned,
  onSetSessionArchived,
  onSearch,
  onSettings,
  onUsage,
  onNotes,
}: SidebarProps) {
  const { t } = useI18n()
  const [grouping, setGrouping] = useState<SidebarGrouping>(() => {
    if (typeof window !== 'undefined') {
      const saved = window.localStorage.getItem('padu:sidebar_grouping')
      if (saved === 'project' || saved === 'updated') return saved
    }
    return 'project'
  })
  const [ordering, setOrdering] = useState<SidebarOrdering>(() => {
    if (typeof window !== 'undefined') {
      const saved = window.localStorage.getItem('padu:sidebar_ordering')
      if (saved === 'newest' || saved === 'oldest') return saved
    }
    return 'newest'
  })
  const [collapsed, setCollapsed] = useState<Set<string>>(() => {
    if (typeof window !== 'undefined') {
      try {
        const saved = window.localStorage.getItem('padu:sidebar_collapsed_groups')
        if (saved) return new Set(JSON.parse(saved))
      } catch {}
    }
    return new Set()
  })
  const [revealedOlderCounts, setRevealedOlderCounts] = useState<Record<string, number>>({})
  const [liveWidth, setLiveWidth] = useState(width)
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1_000))
  const [sessionToDelete, setSessionToDelete] = useState<{ id: string; title?: string } | null>(null)
  const { hosts, activeHost, activeHostId, config, phase, switchHost, addHost } = useDaemon()
  const [hostDialogOpen, setHostDialogOpen] = useState(false)

  const currentHostDisplayName = phase === 'connecting'
    ? t('host.connecting')
    : (activeHostId === null ? t('host.local') : (activeHost?.name || (config ? displayHost(config.address) : t('host.local'))))

  const isLocalActive = activeHostId === null

  const hostMenuItems: ControlMenuItem[] = [
    {
      id: 'host-local',
      label: t('host.local'),
      icon: 'server' as const,
      selected: isLocalActive,
      onSelect: () => {
        void switchHost(null).catch(() => {})
      },
    },
    ...hosts.map((host) => ({
      id: host.id,
      label: host.name || host.address,
      icon: 'server' as const,
      selected: activeHostId === host.id,
      onSelect: () => {
        void switchHost(host.id).catch(() => {})
      },
    })),
    {
      id: 'add-host',
      label: t('host.add_host'),
      icon: 'plus' as const,
      separatorBefore: true,
      onSelect: () => setHostDialogOpen(true),
    },
    {
      id: 'host-settings',
      label: t('host.settings'),
      icon: 'settings' as const,
      onSelect: () => onSettings(),
    },
  ]

  const sidebarShortcut = usePrimaryShortcut('⌘B', 'Ctrl+B')
  const settingsShortcut = usePrimaryShortcut('⌘,', 'Ctrl+,')
  const usageShortcut = usePrimaryShortcut('⌘U', 'Ctrl+U')
  const projectShortcut = usePrimaryShortcut('⌘O', 'Ctrl+O')
  const newTaskShortcut = usePrimaryShortcut('⌘N', 'Ctrl+N')
  const searchShortcut = usePrimaryShortcut('⌘K', 'Ctrl+K')
  const notesShortcut = usePrimaryShortcut('⌘⇧M', 'Ctrl+Shift+M')

  useEffect(() => {
    try {
      window.localStorage.setItem('padu:sidebar_grouping', grouping)
    } catch {}
  }, [grouping])

  useEffect(() => {
    try {
      window.localStorage.setItem('padu:sidebar_ordering', ordering)
    } catch {}
  }, [ordering])

  useEffect(() => {
    try {
      window.localStorage.setItem('padu:sidebar_collapsed_groups', JSON.stringify(Array.from(collapsed)))
    } catch {}
  }, [collapsed])

  const groups = groupSessions(
    taskState.projects,
    taskState.sessions,
    new Date(nowSeconds * 1_000),
    t('sidebar.unknown_project'),
    t('project.no_project_name'),
    grouping,
    ordering,
    revealedOlderCounts,
  )
  const rows = sidebarRows(groups, collapsed)

  useEffect(() => setLiveWidth(width), [width])
  useEffect(() => {
    const delay = nextSidebarUpdateDelay(taskState.sessions, nowSeconds)
    const timer = window.setTimeout(
      () => setNowSeconds(Math.floor(Date.now() / 1_000)),
      delay * 1_000,
    )
    return () => window.clearTimeout(timer)
  }, [nowSeconds, taskState.sessions])

  return (
    <>
      <button
        aria-label={t('sidebar.close')}
        aria-hidden={!mobileOpen}
        className={cn(
          'pointer-events-none fixed inset-0 z-30 bg-black/25 opacity-0 transition-opacity motion-reduce:transition-none lg:hidden',
          mobileOpen && 'pointer-events-auto opacity-100',
        )}
        tabIndex={mobileOpen ? 0 : -1}
        type="button"
        onClick={() => onMobileOpenChange(false)}
      />
      <aside
        className={cn(
          'pointer-events-none invisible fixed inset-y-0 left-0 z-40 flex shrink-0 -translate-x-full flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-transform motion-reduce:transition-none lg:pointer-events-auto lg:visible lg:relative lg:z-auto lg:translate-x-0',
          mobileOpen && 'pointer-events-auto visible translate-x-0',
        )}
        style={{ width: `min(${liveWidth}px, 92vw)` }}
      >
        <header className="flex h-12 shrink-0 items-center px-2.5">
          <img
            alt="Padu"
            className="size-6 rounded-md"
            draggable={false}
            src={paduAppIconUrl}
          />
          <div className="flex-1" />
          <Tooltip content={t('sidebar.toggle')} shortcut={sidebarShortcut}>
            <Button
              aria-label={t('sidebar.hide')}
              size="icon-sm"
              variant="ghost"
              onClick={onToggleSidebar}
            >
              <PaduIcon name="panelLeft" />
            </Button>
          </Tooltip>
        </header>
        <div className="px-2.5">
          <SidebarAction
            icon={<PaduIcon name="pencil" />}
            label={t('menu.new_task')}
            shortcut={newTaskShortcut}
            onClick={() => {
              onNewTask()
              onMobileOpenChange(false)
            }}
          />

        </div>

        <nav aria-label={t('sidebar.tasks')} className="min-h-0 flex-1">
          <Virtuoso
            className="size-full"
            computeItemKey={(_, row) => row.key}
            data={rows}
            defaultItemHeight={52}
            increaseViewportBy={200}
            itemContent={(_, row) => {
              if (row.kind === 'search') {
                return (
                  <div className="h-8 px-2.5">
                    <SidebarAction
                      icon={<PaduIcon name="search" />}
                      label={t('sidebar.search')}
                      shortcut={searchShortcut}
                      onClick={onSearch}
                    />
                  </div>
                )
              }
              if (row.kind === 'notes') {
                return (
                  <div className="h-8 px-2.5">
                    <SidebarAction
                      icon={<PaduIcon name="note" />}
                      label={t('settings.notes')}
                      shortcut={notesShortcut}
                      onClick={() => {
                        onNotes?.()
                        onMobileOpenChange(false)
                      }}
                    />
                  </div>
                )
              }
              if (row.kind === 'spacer') return <div className="h-2.5" />
              if (row.kind === 'separator') {
                return (
                  <div className="flex h-[9px] items-center px-2.5">
                    <div className="h-px w-full bg-sidebar-border" />
                  </div>
                )
              }
              if (row.kind === 'showMore') {
                return (
                  <div className="relative px-2.5 pb-1">
                    <div className="pl-5 pr-2">
                      <button
                        className="flex h-7 items-center justify-start rounded-[5px] px-2 text-[12px] font-medium text-[var(--text-tertiary)] hover:bg-sidebar-accent hover:text-foreground focus-visible:ring-1 focus-visible:ring-ring cursor-pointer"
                        type="button"
                        onClick={() => {
                          setRevealedOlderCounts((current) => ({
                            ...current,
                            [row.groupId]: (current[row.groupId] ?? 0) + 10,
                          }))
                        }}
                        onKeyDown={(event) => {
                          if (event.key === 'ArrowDown') {
                            event.preventDefault()
                            navigateSidebarItem('next', event.currentTarget)
                          } else if (event.key === 'ArrowUp') {
                            event.preventDefault()
                            navigateSidebarItem('prev', event.currentTarget)
                          }
                        }}
                      >
                        {t('sidebar.show_more')}
                      </button>
                    </div>
                  </div>
                )
              }
              if (row.kind === 'group') {
                const isProjectGroup = row.group.kind === 'project' || row.group.kind === 'projectless'
                const label = row.group.kind === 'updated' && row.group.dateGroup
                  ? t(GROUP_TRANSLATION_KEYS[row.group.dateGroup])
                  : row.group.kind === 'pinned'
                  ? t('sidebar.pinned')
                  : row.group.label
                return (
                  <div className="relative px-2.5">
                    <div className="group/header flex h-7 items-center justify-between px-1.5">
                      <button
                        aria-expanded={!row.collapsed}
                        className="group flex h-[22px] min-w-0 flex-1 items-center gap-[5px] rounded px-1 text-[12.5px] font-medium text-[var(--text-tertiary)] outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-ring"
                        type="button"
                        onClick={() => {
                          setCollapsed((current) => {
                            const next = new Set(current)
                            if (next.has(row.group.id)) next.delete(row.group.id)
                            else next.add(row.group.id)
                            return next
                          })
                        }}
                        onKeyDown={(event) => {
                          if (event.key === 'ArrowLeft' && !row.collapsed) {
                            event.preventDefault()
                            setCollapsed((current) => new Set(current).add(row.group.id))
                          } else if (event.key === 'ArrowRight' && row.collapsed) {
                            event.preventDefault()
                            setCollapsed((current) => {
                              const next = new Set(current)
                              next.delete(row.group.id)
                              return next
                            })
                          } else if (event.key === 'ArrowDown') {
                            event.preventDefault()
                            navigateSidebarItem('next', event.currentTarget)
                          } else if (event.key === 'ArrowUp') {
                            event.preventDefault()
                            navigateSidebarItem('prev', event.currentTarget)
                          } else if (event.key === 'Home') {
                            event.preventDefault()
                            navigateSidebarItem('first', event.currentTarget)
                          } else if (event.key === 'End') {
                            event.preventDefault()
                            navigateSidebarItem('last', event.currentTarget)
                          }
                        }}
                      >
                        {isProjectGroup && (
                          <PaduIcon
                            className="size-3.5 shrink-0 text-[var(--text-secondary)]"
                            name={row.collapsed ? 'folder' : 'folderOpen'}
                          />
                        )}
                        {row.group.kind === 'pinned' && (
                          <PaduIcon
                            className="size-3.5 shrink-0 text-[var(--text-secondary)]"
                            name="pin"
                          />
                        )}
                        <span className="truncate">{label}</span>
                        <PaduIcon
                          className="size-3 opacity-0 transition-opacity motion-reduce:transition-none group-hover:opacity-100 group-focus-visible:opacity-100"
                          name={row.collapsed ? 'chevronRight' : 'chevronDown'}
                        />
                      </button>
                      <div className="flex items-center gap-0.5">
                        {row.first && (
                          <>
                            <ControlMenu
                              align="right"
                              caret={false}
                              items={[
                                {
                                  id: 'grouping-project',
                                  section: t('sidebar.grouping'),
                                  label: t('sidebar.grouping_project'),
                                  selected: grouping === 'project',
                                  onSelect: () => setGrouping('project'),
                                },
                                {
                                  id: 'grouping-updated',
                                  section: t('sidebar.grouping'),
                                  label: t('sidebar.grouping_updated'),
                                  selected: grouping === 'updated',
                                  onSelect: () => setGrouping('updated'),
                                },
                                {
                                  id: 'ordering-newest',
                                  section: t('sidebar.ordering'),
                                  separatorBefore: true,
                                  label: t('sidebar.ordering_newest'),
                                  selected: ordering === 'newest',
                                  onSelect: () => setOrdering('newest'),
                                },
                                {
                                  id: 'ordering-oldest',
                                  section: t('sidebar.ordering'),
                                  label: t('sidebar.ordering_oldest'),
                                  selected: ordering === 'oldest',
                                  onSelect: () => setOrdering('oldest'),
                                },
                              ]}
                              label={t('sidebar.options')}
                              placement="below"
                              selectionMode="choice"
                              triggerClassName="size-8 max-w-none p-0 justify-center text-[var(--text-tertiary)] hover:bg-sidebar-accent hover:text-foreground cursor-pointer"
                            >
                              <Tooltip content={t('sidebar.options')}>
                                <span className="grid size-8 place-items-center">
                                  <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="listFilter" />
                                </span>
                              </Tooltip>
                            </ControlMenu>
                            <Tooltip content={t('project.new_project')} shortcut={projectShortcut}>
                              <Button
                                aria-label={t('sidebar.add_project')}
                                className="text-[var(--text-tertiary)]"
                                size="icon-sm"
                                variant="ghost"
                                onClick={onAddProject}
                              >
                                <PaduIcon name="folderNew" />
                              </Button>
                            </Tooltip>
                          </>
                        )}
                      </div>
                    </div>
                  </div>
                )
              }
              return (
                <div className="relative px-2.5 pb-px">
                  <SessionRow
                    groupedByProject={grouping === 'project'}
                    item={row.item}
                    nowSeconds={nowSeconds}
                    selected={selectedSessionId === row.item.session.id}
                    t={t}
                    onRemove={(sessionId) => {
                      const session = taskState.sessions.find((s) => s.id === sessionId)
                      setSessionToDelete({ id: sessionId, title: session?.title })
                    }}
                    onRename={onRenameSession}
                    onSetPinned={onSetSessionPinned}
                    onSetArchived={onSetSessionArchived}
                    onSelect={(sessionId) => {
                      onSelectSession(sessionId)
                      onMobileOpenChange(false)
                    }}
                  />
                </div>
              )
            }}
          />
        </nav>

        <div className="flex h-10 shrink-0 items-center gap-1 px-2.5">
          <Tooltip content={t('common.settings')} shortcut={settingsShortcut}>
            <Button
              aria-label={t('common.settings')}
              className="text-[var(--text-tertiary)]"
              size="icon-sm"
              variant="ghost"
              onClick={onSettings}
            >
              <PaduIcon name="settings" />
            </Button>
          </Tooltip>
          <ControlMenu
            align="left"
            icon="server"
            items={hostMenuItems}
            label={currentHostDisplayName}
            placement="above"
            triggerClassName="h-[26px] max-w-[130px] px-1.5 text-[12px]"
          />
          <div className="flex-1" />
          {onUsage && (
            <Tooltip content={t('settings.usage')} shortcut={usageShortcut}>
              <Button
                aria-label={t('settings.usage')}
                className="text-[var(--text-tertiary)]"
                size="icon-sm"
                variant="ghost"
                onClick={onUsage}
              >
                <PaduIcon name="chartColumn" />
              </Button>
            </Tooltip>
          )}
          <ConnectionDot />
        </div>
        <PanelResizeHandle
          className="hidden lg:block"
          edge="right"
          label={t('sidebar.resize')}
          max={420}
          min={180}
          value={liveWidth}
          onChange={setLiveWidth}
          onCommit={onWidthChange}
        />
      </aside>

      <HostDialog
        editingHost={null}
        open={hostDialogOpen}
        onOpenChange={setHostDialogOpen}
        onSave={async (data) => {
          const newHost = await addHost(data)
          await switchHost(newHost.id)
        }}
      />

      <DeleteSessionDialog
        session={sessionToDelete}
        onClose={() => setSessionToDelete(null)}
        onConfirm={onRemoveSession}
      />
    </>
  )
}

function navigateSidebarItem(
  direction: 'next' | 'prev' | 'first' | 'last',
  currentElement: HTMLElement | null,
) {
  if (!currentElement) return
  const container = currentElement.closest('[data-virtuoso-scroller]') ?? currentElement.closest('aside')
  if (!container) return
  const focusables = Array.from(
    container.querySelectorAll<HTMLElement>(
      'button:not([disabled]):not([tabindex="-1"]), [tabindex="0"]',
    ),
  ).filter((el) => el.offsetParent !== null)

  const currentIndex = focusables.indexOf(currentElement)
  if (currentIndex === -1) return

  let targetIndex = currentIndex
  if (direction === 'next') {
    targetIndex = Math.min(currentIndex + 1, focusables.length - 1)
  } else if (direction === 'prev') {
    targetIndex = Math.max(currentIndex - 1, 0)
  } else if (direction === 'first') {
    targetIndex = 0
  } else if (direction === 'last') {
    targetIndex = focusables.length - 1
  }

  const target = focusables[targetIndex]
  if (target && target !== currentElement) {
    target.focus()
  }
}

function SidebarAction({
  icon,
  label,
  shortcut,
  onClick,
}: {
  icon: ReactNode
  label: string
  shortcut?: string
  onClick: () => void
}) {
  return (
    <button
      className="flex h-8 w-full items-center gap-2.5 rounded-[7px] px-1 text-left text-[13px] text-[var(--text-secondary)] outline-none hover:bg-sidebar-accent focus-visible:ring-1 focus-visible:ring-ring active:bg-sidebar-accent cursor-pointer"
      type="button"
      onClick={onClick}
    >
      <span className="grid size-5 place-items-center [&>svg]:size-4">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {shortcut && <Kbd size="sm">{shortcut}</Kbd>}
    </button>
  )
}

function SessionRow({
  item,
  nowSeconds,
  selected,
  groupedByProject,
  onSelect,
  onRename,
  onRemove,
  onSetPinned,
  onSetArchived,
  t,
}: {
  item: SessionItem
  nowSeconds: number
  selected: boolean
  groupedByProject?: boolean
  onSelect: (sessionId: string) => void
  onRename: (sessionId: string, title: string) => Promise<void>
  onRemove: (sessionId: string) => void | Promise<void>
  onSetPinned: (sessionId: string, pinned: boolean) => Promise<void>
  onSetArchived: (sessionId: string, archived: boolean) => Promise<void>
  t: Translator
}) {
  const [menuOpen, setMenuOpen] = useState(false)
  const [renaming, setRenaming] = useState(false)
  const [title, setTitle] = useState(displayTitle(item.session))
  const skipRenameCommit = useRef(false)
  const rowButton = useRef<HTMLButtonElement>(null)
  const restoreMenuFocus = useRef(false)
  const currentTitle = displayTitle(item.session)
  const timeLabel = sessionTimeLabel(item.session, nowSeconds, t)

  async function commitRename() {
    if (skipRenameCommit.current) {
      skipRenameCommit.current = false
      return
    }
    const next = title.trim()
    setRenaming(false)
    if (!next || next === currentTitle) {
      setTitle(currentTitle)
      return
    }
    try {
      await onRename(item.session.id, next)
    } catch {
      setTitle(currentTitle)
    }
  }

  useEffect(() => {
    if (!renaming) setTitle(currentTitle)
  }, [currentTitle, renaming])

  return (
    <ContextMenu.Root
      open={menuOpen}
      onOpenChange={(open) => {
        setMenuOpen(renaming ? false : open)
        if (!open && restoreMenuFocus.current) {
          restoreMenuFocus.current = false
          requestAnimationFrame(() => rowButton.current?.focus())
        }
      }}
    >
      <ContextMenu.Trigger
        className={cn(
          'group relative rounded-[7px] hover:bg-sidebar-accent',
          selected && 'bg-sidebar-accent',
        )}
      >
        {renaming ? (
          <div
            className={cn(
              'flex w-full min-w-0 flex-col rounded-[7px]',
              groupedByProject ? 'h-[36px] justify-center pl-1.5 pr-2 py-1' : 'h-[51px] gap-1 px-1.5 py-[7px]',
            )}
          >
            <span className="flex min-w-0 w-full items-center gap-[5px] leading-[18px]">
              <span className="grid size-3.5 shrink-0 place-items-center">
                <span className="size-1.5 rounded-full bg-primary" />
              </span>
              <Input
                autoFocus
                className="h-[22px] min-w-0 flex-1 rounded border-0 border-none bg-[var(--inset)] px-1 text-[13px] shadow-none outline-none focus-visible:border-0 focus-visible:border-none focus-visible:ring-0 focus-visible:outline-none"
                value={title}
                onBlur={() => void commitRename()}
                onChange={(event) => setTitle(event.target.value)}
                onClick={(event) => event.stopPropagation()}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault()
                    event.currentTarget.blur()
                  }
                  if (event.key === 'Escape') {
                    event.preventDefault()
                    skipRenameCommit.current = true
                    setTitle(currentTitle)
                    setRenaming(false)
                  }
                }}
              />
              <SessionStatus status={item.session.status} t={t} />
            </span>
          </div>
        ) : groupedByProject ? (
          <button
            aria-current={selected ? 'page' : undefined}
            aria-haspopup="menu"
            className="flex h-[34px] w-full min-w-0 items-center justify-between gap-2 rounded-[6px] pl-1.5 pr-2 text-left outline-none focus-visible:ring-1 focus-visible:ring-ring cursor-pointer"
            ref={rowButton}
            type="button"
            onClick={() => onSelect(item.session.id)}
            onKeyDown={(event) => {
              if (event.key === 'F2') {
                event.preventDefault()
                setRenaming(true)
              } else if (
                event.key === 'Delete' ||
                ((event.metaKey || event.ctrlKey) && event.key === 'Backspace')
              ) {
                event.preventDefault()
                onRemove(item.session.id)
              } else if ((event.shiftKey && event.key === 'F10') || event.key === 'ContextMenu') {
                event.preventDefault()
                restoreMenuFocus.current = true
                setMenuOpen(true)
              } else if (event.key === 'ArrowDown') {
                event.preventDefault()
                navigateSidebarItem('next', event.currentTarget)
              } else if (event.key === 'ArrowUp') {
                event.preventDefault()
                navigateSidebarItem('prev', event.currentTarget)
              } else if (event.key === 'Home') {
                event.preventDefault()
                navigateSidebarItem('first', event.currentTarget)
              } else if (event.key === 'End') {
                event.preventDefault()
                navigateSidebarItem('last', event.currentTarget)
              }
            }}
          >
            <span className="flex min-w-0 flex-1 items-center gap-[5px]">
              <span className="grid size-3.5 shrink-0 place-items-center">
                <span
                  className={cn(
                    'size-1.5 rounded-full',
                    selected ? 'bg-primary' : 'bg-[var(--text-tertiary)]',
                  )}
                />
              </span>
              <span
                className={cn(
                  'min-w-0 flex-1 truncate text-[13px] leading-tight text-[var(--text-secondary)] group-hover:text-foreground',
                  selected && 'font-medium text-foreground',
                )}
              >
                {currentTitle}
              </span>
              <SessionStatus status={item.session.status} t={t} />
            </span>
            {timeLabel && (
              <span
                className={cn(
                  'shrink-0 text-[11px] text-[var(--text-ghost)] group-hover:text-[var(--text-tertiary)]',
                  item.session.status !== 'idle' && 'text-[var(--text-tertiary)]',
                  selected && 'text-[var(--text-tertiary)]',
                )}
              >
                {timeLabel}
              </span>
            )}
          </button>
        ) : (
          <button
            aria-current={selected ? 'page' : undefined}
            aria-haspopup="menu"
            className="flex h-[51px] w-full min-w-0 flex-col gap-1 rounded-[7px] px-1.5 py-[7px] text-left outline-none focus-visible:ring-1 focus-visible:ring-ring cursor-pointer"
            ref={rowButton}
            type="button"
            onClick={() => onSelect(item.session.id)}
            onKeyDown={(event) => {
              if (event.key === 'F2') {
                event.preventDefault()
                setRenaming(true)
              } else if (
                event.key === 'Delete' ||
                ((event.metaKey || event.ctrlKey) && event.key === 'Backspace')
              ) {
                event.preventDefault()
                onRemove(item.session.id)
              } else if ((event.shiftKey && event.key === 'F10') || event.key === 'ContextMenu') {
                event.preventDefault()
                restoreMenuFocus.current = true
                setMenuOpen(true)
              } else if (event.key === 'ArrowDown') {
                event.preventDefault()
                navigateSidebarItem('next', event.currentTarget)
              } else if (event.key === 'ArrowUp') {
                event.preventDefault()
                navigateSidebarItem('prev', event.currentTarget)
              } else if (event.key === 'Home') {
                event.preventDefault()
                navigateSidebarItem('first', event.currentTarget)
              } else if (event.key === 'End') {
                event.preventDefault()
                navigateSidebarItem('last', event.currentTarget)
              }
            }}
          >
            <span className="flex min-w-0 w-full items-center gap-[5px] leading-[18px]">
              <span className="grid size-3.5 shrink-0 place-items-center">
                <span
                  className={cn(
                    'size-1.5 rounded-full',
                    selected ? 'bg-primary' : 'bg-[var(--text-tertiary)]',
                  )}
                />
              </span>
              <span
                className={cn(
                  'min-w-0 flex-1 truncate text-[13.5px] text-foreground',
                  selected && 'font-medium',
                )}
              >
                {currentTitle}
              </span>
              <SessionStatus status={item.session.status} t={t} />
            </span>
            <SessionMetadata item={item} nowSeconds={nowSeconds} t={t} />
          </button>
        )}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Positioner className="z-[100] outline-none">
          <ContextMenu.Popup
            className="padu-menu-surface"
            finalFocus={false}
          >
            <ContextMenu.Item
              className="padu-menu-item"
              onClick={() => {
                restoreMenuFocus.current = false
                setMenuOpen(false)
                skipRenameCommit.current = false
                setTitle(currentTitle)
                setRenaming(true)
              }}
            >
              <PaduIcon className="size-3" name="pencil" /> {t('common.rename')}
            </ContextMenu.Item>
            <ContextMenu.Item
              className="padu-menu-item"
              onClick={() => {
                restoreMenuFocus.current = true
                setMenuOpen(false)
                void onSetPinned(item.session.id, !Boolean(item.session.pinned_at)).catch(() => {})
              }}
            >
              <PaduIcon className="size-3" name="pin" /> {t(item.session.pinned_at ? 'session.unpin' : 'session.pin')}
            </ContextMenu.Item>
            <ContextMenu.Item
              className="padu-menu-item"
              disabled={item.session.status === 'connecting' || item.session.status === 'working' || item.session.status === 'waiting'}
              onClick={() => {
                restoreMenuFocus.current = true
                setMenuOpen(false)
                void onSetArchived(item.session.id, true).catch(() => {})
              }}
            >
              <PaduIcon className="size-3" name="package" /> {t('session.archive')}
            </ContextMenu.Item>
            <ContextMenu.Separator className="padu-menu-separator" />
            <ContextMenu.Item
              className="padu-menu-item text-destructive data-[highlighted]:bg-[var(--danger-soft)]"
              onClick={() => {
                restoreMenuFocus.current = false
                setMenuOpen(false)
                void Promise.resolve(onRemove(item.session.id)).catch(() => {})
              }}
            >
              <PaduIcon className="size-3" name="trash" /> {t('common.remove')}
            </ContextMenu.Item>
          </ContextMenu.Popup>
        </ContextMenu.Positioner>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}

function SessionMetadata({
  item,
  nowSeconds,
  t,
}: {
  item: SessionItem
  nowSeconds: number
  t: Translator
}) {
  const timeLabel = sessionTimeLabel(item.session, nowSeconds, t)
  return (
    <span className="flex w-full min-w-0 items-center gap-[5px] text-[11.5px] leading-[15px] text-[var(--text-tertiary)]">
      <span className="grid size-3.5 shrink-0 place-items-center">
        <PaduIcon className="size-[12px] shrink-0" name="folder" />
      </span>
      <span className="min-w-0 flex-1 truncate">{item.projectName}</span>
      {timeLabel && (
        <span
          className={cn(
            'shrink-0 text-[var(--text-ghost)]',
            item.session.status !== 'idle' && 'text-[var(--text-tertiary)]',
          )}
        >
          {timeLabel}
        </span>
      )}
    </span>
  )
}

function SessionStatus({ status, t }: { status: AgentSession['status']; t: Translator }) {
  if (status === 'idle') return null
  if (status === 'working' || status === 'connecting') {
    return <PaduIcon label={t('sidebar.status_working')} className="size-3 text-[var(--success)] motion-safe:animate-spin" name="loaderCircle" />
  }
  if (status === 'waiting') {
    return <PaduIcon label={t('sidebar.status_waiting')} className="size-3 text-[var(--warning)]" name="alert" />
  }
  return <PaduIcon label={t('sidebar.status_failed')} className="size-3 text-destructive" name="x" />
}

function ConnectionDot() {
  const { t } = useI18n()
  const { phase } = useDaemon()
  return (
    <span
      aria-label={`${t('settings.daemon')} · ${t(`daemon.phase_${phase}`)}`}
      className={cn(
        'block size-1.5 rounded-full bg-[var(--text-ghost)]',
        phase === 'connected' && 'bg-[var(--success)]',
        phase === 'error' && 'bg-destructive',
      )}
      role="img"
    />
  )
}
