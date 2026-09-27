import { useNavigate, useSearch } from '@tanstack/react-router'
import type { AgentSession, CreateTask, ProviderKind, Task, TaskStatus, TaskSummary } from '@padu/client'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { PaduIcon, PROVIDERS, ProviderIcon } from '@/components/padu-icon'
import { Sidebar } from '@/components/sidebar'
import { ConnectionPanel } from '@/components/connection-panel'
import { StartupScreen } from '@/components/startup-screen'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { Tooltip } from '@/components/ui/tooltip'
import { TagInput } from '@/components/ui/tag-input'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { MarkdownView } from '@/components/markdown-view'
import { useTasks, useTaskHydrated, useTaskState, useProviderProbes, useAgentProfiles } from '@/hooks/use-daemon-data'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import {
  createTask,
  deleteTask,
  moveTask,
  updateTask,
  persistSession,
  removeSession,
  setSessionArchived,
  setSessionPinned,
} from '@/lib/daemon-api'
import { projectDisplayName } from '@/lib/project-presentation'
import { formatDurationShort, formatTokens, taskLiveBadges } from '@/lib/task-live-badges'
import { cn } from '@/lib/utils'

const DEFAULT_PROVIDER_MODELS: Record<string, string[]> = {
  claude: ['claude-3-7-sonnet', 'claude-3-5-sonnet', 'claude-3-5-haiku'],
  codex: ['gpt-4o', 'o3-mini', 'o1', 'gpt-4o-mini'],
  cursor: ['claude-3.5-sonnet', 'gpt-4o'],
  agy: ['gemini-2.0-flash', 'gemini-2.0-pro', 'gemini-1.5-pro'],
  deepSeek: ['deepseek-chat', 'deepseek-reasoner'],
  openCode: ['default'],
  grok: ['grok-2', 'grok-beta'],
  kimi: ['moonshot-v1-auto', 'moonshot-v1-128k'],
  pi: ['default'],
}

const COLUMNS: { id: TaskStatus; labelKey: string; color: string }[] = [
  { id: 'backlog', labelKey: 'board.backlog', color: 'bg-zinc-400' },
  { id: 'queued', labelKey: 'board.queued', color: 'bg-blue-400' },
  { id: 'running', labelKey: 'board.running', color: 'bg-amber-400' },
  { id: 'review', labelKey: 'board.review', color: 'bg-purple-400' },
  { id: 'done', labelKey: 'board.done', color: 'bg-emerald-400' },
]

export function BoardPage() {
  const { t } = useI18n()
  const navigate = useNavigate()
  const search = useSearch({ from: '/board' })
  const { client, phase } = useDaemon()
  const taskState = useTaskState()

  const [mobileSidebar, setMobileSidebar] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useState(260)
  const [sidebarVisible, setSidebarVisible] = useState(true)

  // Filters
  const [searchQuery, setSearchQuery] = useState(search.q ?? '')
  const [selectedProjectId, setSelectedProjectId] = useState<string | 'all'>(
    search.projectId ?? 'all',
  )
  const [selectedAgent, setSelectedAgent] = useState<string | 'all'>(
    search.agent ?? 'all',
  )
  const [flagFilter, setFlagFilter] = useState<'all' | 'needs_attention' | 'sync_failed'>('all')

  // Task selection & Modals
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(search.taskId ?? null)
  const [isNewTaskOpen, setIsNewTaskOpen] = useState(false)
  const [newTaskInitialStatus, setNewTaskInitialStatus] = useState<TaskStatus>('backlog')
  const [draggedTaskId, setDraggedTaskId] = useState<string | null>(null)
  const [dragOverColumn, setDragOverColumn] = useState<TaskStatus | null>(null)
  const [collapsedColumns, setCollapsedColumns] = useState<Set<TaskStatus>>(() => new Set())

  const toggleColumnCollapse = useCallback((status: TaskStatus) => {
    setCollapsedColumns((prev) => {
      const next = new Set(prev)
      if (next.has(status)) {
        next.delete(status)
      } else {
        next.add(status)
      }
      return next
    })
  }, [])

  // Queries
  const tasksQuery = useTasks()
  const tasks = tasksQuery.data ?? []

  // Live usage badges (P1-08): join each card's sessionId to the in-memory
  // session catalog, which refreshes on the daemon's task-state/card-updated
  // events. No polling — a missing session renders no badges.
  const sessionsById = useMemo(() => {
    const map = new Map<string, AgentSession>()
    for (const session of taskState.data?.sessions ?? []) {
      map.set(session.id, session)
    }
    return map
  }, [taskState.data])
  const nowSec = Math.floor(Date.now() / 1000)

  // Filtered tasks
  const filteredTasks = useMemo(() => {
    return tasks.filter((task) => {
      if (selectedProjectId !== 'all' && task.projectId !== selectedProjectId) {
        return false
      }
      if (selectedAgent !== 'all' && task.assignedAgent !== selectedAgent) {
        return false
      }
      if (flagFilter === 'needs_attention' && !task.needsAttention) {
        return false
      }
      if (flagFilter === 'sync_failed' && !task.syncFailed) {
        return false
      }
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase()
        const titleMatch = task.title.toLowerCase().includes(q)
        const descMatch = task.descriptionPreview.toLowerCase().includes(q)
        const labelMatch = task.labels.some((l) => l.toLowerCase().includes(q))
        if (!titleMatch && !descMatch && !labelMatch) return false
      }
      return true
    })
  }, [tasks, selectedProjectId, selectedAgent, flagFilter, searchQuery])

  // Drag and Drop
  const handleDragStart = (e: React.DragEvent, taskId: string) => {
    e.dataTransfer.setData('text/plain', taskId)
    setDraggedTaskId(taskId)
  }

  const handleDragOver = (e: React.DragEvent, status: TaskStatus) => {
    e.preventDefault()
    setDragOverColumn(status)
  }

  const handleDragLeave = () => {
    setDragOverColumn(null)
  }

  const handleDrop = async (e: React.DragEvent, targetStatus: TaskStatus) => {
    e.preventDefault()
    setDragOverColumn(null)
    const taskId = e.dataTransfer.getData('text/plain') || draggedTaskId
    setDraggedTaskId(null)
    if (!taskId || !client) return

    const task = tasks.find((t) => t.id === taskId)
    if (!task) return
    if (task.status === targetStatus) return

    if (targetStatus === 'running') {
      toast.error(t('board.drag_running_rejected'))
      return
    }

    try {
      await moveTask(client, taskId, targetStatus, task.version)
      void tasksQuery.refetch()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
  }

  const handleQuickMove = async (task: TaskSummary, targetStatus: TaskStatus) => {
    if (!client) return
    try {
      await moveTask(client, task.id, targetStatus, task.version)
      void tasksQuery.refetch()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
  }

  const projects = taskState.data?.projects ?? []

  if (phase === 'booting' || phase === 'connecting') {
    return <StartupScreen />
  }

  if (phase === 'disconnected' || phase === 'error' || !taskState.data) {
    return <ConnectionPanel />
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
      {sidebarVisible && (
        <Sidebar
          mobileOpen={mobileSidebar}
          onAddProject={() => {}}
          onMobileOpenChange={setMobileSidebar}
          onNewTask={() => {
            void navigate({ to: '/', search: { session: undefined } as any })
          }}
          onNotes={() => {
            void navigate({ to: '/notes', search: { q: undefined, noteId: undefined, projectId: undefined } as any })
          }}
          onBoard={() => {}}
          onRemoveSession={async (id) => {
            if (client) await removeSession(client, id)
          }}
          onRenameSession={async () => {}}
          onSetSessionArchived={async (id, archived) => {
            if (client) await setSessionArchived(client, id, archived)
          }}
          onSetSessionPinned={async (id, pinned) => {
            if (client) await setSessionPinned(client, id, pinned)
          }}
          onSearch={() => {}}
          onSelectSession={(sessionId) => {
            void navigate({ to: '/', search: { session: sessionId } as any })
          }}
          onSettings={() => {
            void navigate({ to: '/settings/$page', params: { page: 'general' }, search: {} as any })
          }}
          onToggleSidebar={() => setSidebarVisible(false)}
          onWidthChange={setSidebarWidth}
          selectedSessionId={undefined}
          taskState={taskState.data}
          width={sidebarWidth}
        />
      )}

      {/* Main Board View */}
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
        {/* Top Header */}
        <header className="flex h-14 flex-none items-center justify-between border-b border-border px-3 sm:px-4 gap-2 sm:gap-3 bg-card/40 backdrop-blur-xs">
          <div className="flex items-center gap-2 sm:gap-2.5 min-w-0 overflow-x-auto no-scrollbar py-1">
            {!sidebarVisible && (
              <Button
                variant="ghost"
                size="sm"
                className="h-8 w-8 p-0 flex-none"
                onClick={() => setSidebarVisible(true)}
              >
                <PaduIcon name="panelLeft" className="size-4" />
              </Button>
            )}
            <div className="flex items-center gap-2 flex-none">
              <PaduIcon name="listChecks" className="size-5 text-primary" />
              <h1 className="text-sm sm:text-base font-semibold text-foreground whitespace-nowrap">
                {t('board.title')}
              </h1>
              <Badge variant="outline" className="px-1.5 py-0 text-[11px] font-mono">
                {filteredTasks.length}
              </Badge>
            </div>

            <div className="h-4 w-px bg-border flex-none" />

            {/* Filter Toolbar (Responsive and grouped alongside Title) */}
            <div className="flex items-center gap-2 flex-none">
              {/* Search */}
              <div className="relative w-32 sm:w-40 flex-none">
                <PaduIcon
                  name="search"
                  className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none"
                />
                <Input
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder={t('board.filter_search_placeholder')}
                  className="h-8 pl-8 pr-7 text-xs bg-muted/40 border-border"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => setSearchQuery('')}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground cursor-pointer"
                  >
                    <PaduIcon name="x" className="size-3" />
                  </button>
                )}
              </div>

              {/* Project Filter */}
              <div className="relative flex items-center flex-none">
                <PaduIcon
                  name="folder"
                  className={cn(
                    'pointer-events-none absolute left-2.5 size-3.5',
                    selectedProjectId !== 'all' ? 'text-primary' : 'text-muted-foreground',
                  )}
                />
                <select
                  value={selectedProjectId}
                  onChange={(e) => setSelectedProjectId(e.target.value)}
                  className={cn(
                    'h-8 rounded-md border text-xs pl-8 pr-6 max-w-[130px] sm:max-w-[160px] truncate appearance-none cursor-pointer transition-colors focus:outline-none focus:ring-1 focus:ring-primary',
                    selectedProjectId !== 'all'
                      ? 'border-primary/50 bg-primary/10 text-primary font-medium'
                      : 'border-border bg-muted/40 text-foreground',
                  )}
                >
                  <option value="all">{t('board.all_projects')}</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {projectDisplayName(p, t('project.no_project_name'))}
                    </option>
                  ))}
                </select>
                <PaduIcon
                  name="chevronDown"
                  className="pointer-events-none absolute right-2 size-3 text-muted-foreground"
                />
              </div>

              {/* Agent Filter */}
              <div className="relative flex items-center flex-none">
                <PaduIcon
                  name="bot"
                  className={cn(
                    'pointer-events-none absolute left-2.5 size-3.5',
                    selectedAgent !== 'all' ? 'text-primary' : 'text-muted-foreground',
                  )}
                />
                <select
                  value={selectedAgent}
                  onChange={(e) => setSelectedAgent(e.target.value)}
                  className={cn(
                    'h-8 rounded-md border text-xs pl-8 pr-6 max-w-[110px] sm:max-w-[140px] truncate appearance-none cursor-pointer transition-colors focus:outline-none focus:ring-1 focus:ring-primary',
                    selectedAgent !== 'all'
                      ? 'border-primary/50 bg-primary/10 text-primary font-medium'
                      : 'border-border bg-muted/40 text-foreground',
                  )}
                >
                  <option value="all">{t('board.all_agents')}</option>
                  {PROVIDERS.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
                <PaduIcon
                  name="chevronDown"
                  className="pointer-events-none absolute right-2 size-3 text-muted-foreground"
                />
              </div>

              <div className="h-4 w-px bg-border flex-none" />

              {/* Flags Filter Segmented Control */}
              <div className="flex h-8 items-center rounded-lg border border-border bg-muted/40 p-0.5 gap-0.5 flex-none">
                <button
                  type="button"
                  onClick={() =>
                    setFlagFilter((current) =>
                      current === 'needs_attention' ? 'all' : 'needs_attention',
                    )
                  }
                  className={cn(
                    'flex h-7 items-center gap-1.5 rounded-md px-2 sm:px-2.5 text-xs font-medium transition-colors cursor-pointer',
                    flagFilter === 'needs_attention'
                      ? 'bg-amber-500/20 text-amber-500'
                      : 'text-muted-foreground hover:text-foreground hover:bg-background/50',
                  )}
                >
                  <span className="size-1.5 rounded-full bg-amber-500 shrink-0" />
                  <span className="hidden sm:inline">{t('board.needs_attention')}</span>
                  <span className="sm:hidden text-[11px]">Attention</span>
                </button>
                <button
                  type="button"
                  onClick={() =>
                    setFlagFilter((current) => (current === 'sync_failed' ? 'all' : 'sync_failed'))
                  }
                  className={cn(
                    'flex h-7 items-center gap-1.5 rounded-md px-2 sm:px-2.5 text-xs font-medium transition-colors cursor-pointer',
                    flagFilter === 'sync_failed'
                      ? 'bg-rose-500/20 text-rose-500'
                      : 'text-muted-foreground hover:text-foreground hover:bg-background/50',
                  )}
                >
                  <PaduIcon
                    name="syncFailed"
                    className={cn(
                      'size-3.5 shrink-0',
                      flagFilter === 'sync_failed' ? 'text-rose-500' : 'text-muted-foreground',
                    )}
                  />
                  <span className="hidden sm:inline">{t('board.sync_failed')}</span>
                  <span className="sm:hidden text-[11px]">Failed</span>
                </button>
              </div>

              {/* Clear Filters (Vertically centered with matching h-8 height) */}
              {(searchQuery ||
                selectedProjectId !== 'all' ||
                selectedAgent !== 'all' ||
                flagFilter !== 'all') && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    setSearchQuery('')
                    setSelectedProjectId('all')
                    setSelectedAgent('all')
                    setFlagFilter('all')
                  }}
                  className="h-8 px-2.5 flex items-center justify-center gap-1.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted/60 border-border flex-none"
                >
                  <PaduIcon name="x" className="size-3 text-muted-foreground" />
                  <span className="hidden sm:inline">{t('board.clear_filters')}</span>
                  <span className="sm:hidden text-[11px]">Clear</span>
                </Button>
              )}
            </div>
          </div>

          {/* Right Header Actions */}
          <div className="flex items-center gap-1.5 sm:gap-2 flex-none">
            <Button
              variant="outline"
              size="sm"
              className="h-8 w-8 p-0"
              onClick={() => tasksQuery.refetch?.()}
              title={t('common.refresh')}
            >
              <PaduIcon name="rotateCw" className="size-3.5 text-muted-foreground" />
            </Button>
            <Button
              size="sm"
              className="h-8 gap-1.5 text-xs font-medium shadow-xs"
              onClick={() => {
                setNewTaskInitialStatus('backlog')
                setIsNewTaskOpen(true)
              }}
            >
              <PaduIcon name="plus" className="size-3.5" />
              <span className="hidden sm:inline">{t('board.new_task')}</span>
            </Button>
          </div>
        </header>

        {/* Kanban Board Columns Container */}
        <div className="flex flex-1 overflow-x-auto p-4 gap-3 bg-muted/20">
          {COLUMNS.map((column) => {
            const columnTasks = filteredTasks.filter((t) => t.status === column.id)
            const isDragOver = dragOverColumn === column.id
            const isCollapsed = collapsedColumns.has(column.id)

            if (isCollapsed) {
              return (
                <div
                  key={column.id}
                  onClick={() => toggleColumnCollapse(column.id)}
                  className="flex flex-col items-center flex-none w-11 py-3 px-1 rounded-xl border border-border bg-card/60 hover:bg-card hover:border-primary/40 cursor-pointer transition-all select-none gap-3 shadow-xs"
                  title={t('board.expand_column')}
                >
                  <Button
                    variant="ghost"
                    size="sm"
                    className="size-6 p-0 text-muted-foreground hover:text-foreground"
                    onClick={(e) => {
                      e.stopPropagation()
                      toggleColumnCollapse(column.id)
                    }}
                  >
                    <PaduIcon name="chevronRight" className="size-3.5" />
                  </Button>
                  <span className={`size-2.5 rounded-full ${column.color}`} />
                  <Badge variant="secondary" className="h-5 px-1 text-[10px] font-mono">
                    {columnTasks.length}
                  </Badge>
                  <div className="flex-1 flex items-center justify-center my-2">
                    <span
                      className="text-[11px] font-medium text-muted-foreground tracking-wider uppercase whitespace-nowrap"
                      style={{ writingMode: 'vertical-rl', transform: 'rotate(180deg)' }}
                    >
                      {t(column.labelKey)}
                    </span>
                  </div>
                </div>
              )
            }

            return (
              <div
                key={column.id}
                onDragOver={(e) => handleDragOver(e, column.id)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, column.id)}
                className={`flex flex-col flex-none w-76 rounded-xl border border-border bg-card/60 transition-colors ${
                  isDragOver ? 'border-primary ring-2 ring-primary/20 bg-primary/5' : ''
                }`}
              >
                {/* Column Header */}
                <div className="flex items-center justify-between px-3 py-2.5 border-b border-border/60">
                  <div className="flex items-center gap-2">
                    <span className={`size-2.5 rounded-full ${column.color}`} />
                    <span className="text-xs font-semibold text-foreground">
                      {t(column.labelKey)}
                    </span>
                    <Badge variant="secondary" className="h-5 px-1.5 text-[11px] font-mono">
                      {columnTasks.length}
                    </Badge>
                  </div>
                  <div className="flex items-center gap-1">
                    {column.id === 'backlog' && (
                      <Button
                        variant="ghost"
                        size="sm"
                        className="size-6 p-0 text-muted-foreground hover:text-foreground"
                        onClick={() => {
                          setNewTaskInitialStatus('backlog')
                          setIsNewTaskOpen(true)
                        }}
                      >
                        <PaduIcon name="plus" className="size-3.5" />
                      </Button>
                    )}
                    <Button
                      variant="ghost"
                      size="sm"
                      className="size-6 p-0 text-muted-foreground hover:text-foreground"
                      onClick={() => toggleColumnCollapse(column.id)}
                      title={t('board.collapse_column')}
                    >
                      <PaduIcon name="chevronLeft" className="size-3.5" />
                    </Button>
                  </div>
                </div>

                {/* Cards Container */}
                <div className="flex-1 overflow-y-auto p-2.5 space-y-2.5 min-h-[150px]">
                  {columnTasks.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-28 rounded-lg border border-dashed border-border/60 text-muted-foreground p-3 text-center">
                      <p className="text-xs">{t(`board.empty_${column.id}`)}</p>
                    </div>
                  ) : (
                    columnTasks.map((task) => (
                      <TaskCard
                        key={task.id}
                        task={task}
                        project={projects.find((p) => p.id === task.projectId)}
                        session={task.sessionId ? sessionsById.get(task.sessionId) : undefined}
                        nowSec={nowSec}
                        onSelect={() => setSelectedTaskId(task.id)}
                        onDragStart={(e) => handleDragStart(e, task.id)}
                        onQuickMove={handleQuickMove}
                      />
                    ))
                  )}
                </div>
              </div>
            )
          })}
        </div>
      </div>

      {/* Task Detail Flyout / Drawer */}
      {selectedTaskId && (
        <TaskDetailDrawer
          taskId={selectedTaskId}
          projects={projects}
          onClose={() => setSelectedTaskId(null)}
          onTaskUpdated={() => void tasksQuery.refetch()}
        />
      )}

      {/* New Task Dialog */}
      {isNewTaskOpen && (
        <NewTaskDialog
          projects={projects}
          defaultProjectId={selectedProjectId !== 'all' ? selectedProjectId : projects[0]?.id}
          initialStatus={newTaskInitialStatus}
          onClose={() => setIsNewTaskOpen(false)}
          onCreated={(created) => {
            setIsNewTaskOpen(false)
            void tasksQuery.refetch()
            setSelectedTaskId(created.id)
          }}
        />
      )}
    </div>
  )
}

function TaskCard({
  task,
  project,
  session,
  nowSec,
  onSelect,
  onDragStart,
  onQuickMove,
}: {
  task: TaskSummary
  project: { name: string } | undefined
  session: AgentSession | undefined
  nowSec: number
  onSelect: () => void
  onDragStart: (e: React.DragEvent) => void
  onQuickMove: (task: TaskSummary, targetStatus: TaskStatus) => void
}) {
  const { t } = useI18n()
  const live = taskLiveBadges(session, nowSec)

  return (
    <div
      draggable={task.status !== 'running'}
      onDragStart={onDragStart}
      onClick={onSelect}
      role="button"
      tabIndex={0}
      aria-label={task.title}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect()
        }
      }}
      className="group relative flex flex-col gap-2 rounded-lg border border-border bg-card p-3 shadow-xs hover:border-primary/50 hover:shadow-sm cursor-pointer transition-all outline-none focus-visible:ring-1 focus-visible:ring-ring"
    >
      {/* Title & Flags */}
      <div className="flex items-start justify-between gap-1.5">
        <h4 className="text-xs font-medium text-foreground line-clamp-2 leading-relaxed">
          {task.title}
        </h4>
        <div className="flex items-center gap-1 flex-none">
          {task.needsAttention && (
            <Tooltip content={t('board.needs_attention')}>
              <span className="flex size-4 items-center justify-center rounded-full bg-amber-500/10 text-amber-500">
                <PaduIcon name="alert" className="size-3" />
              </span>
            </Tooltip>
          )}
          {task.syncFailed && (
            <Tooltip content={task.syncFailed}>
              <span className="flex size-4 items-center justify-center rounded-full bg-rose-500/10 text-rose-500">
                <PaduIcon name="syncFailed" className="size-3" />
              </span>
            </Tooltip>
          )}
        </div>
      </div>

      {/* Description Preview */}
      {task.descriptionPreview && (
        <p className="text-[11px] text-muted-foreground line-clamp-2 leading-normal">
          {task.descriptionPreview}
        </p>
      )}

      {/* Labels */}
      {task.labels.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {task.labels.map((label) => (
            <Badge
              key={label}
              variant="outline"
              className="px-1.5 py-0 text-[10px] font-normal"
            >
              {label}
            </Badge>
          ))}
        </div>
      )}

      {/* Meta Bar */}
      <div className="flex items-center justify-between pt-1 border-t border-border/40 text-[10px] text-muted-foreground">
        <div className="flex items-center gap-2">
          {project && <span className="truncate max-w-[100px]">{project.name}</span>}
          {task.assignedAgent && (
            <span className="flex items-center gap-1 font-mono uppercase text-foreground/80">
              <ProviderIcon provider={task.assignedAgent} className="size-3" />
              {task.assignedAgent}
            </span>
          )}
          {task.model && (
            <span className="truncate max-w-[85px] font-mono text-[9px] px-1 py-0.5 rounded bg-muted text-muted-foreground border border-border/40" title={task.model}>
              {task.model}
            </span>
          )}
          {live.tokens !== undefined && (
            <span
              className="flex items-center gap-0.5 font-mono text-foreground/80"
              title={t('board.tokens_tooltip')}
            >
              <PaduIcon name="zap" className="size-3" />
              {formatTokens(live.tokens)}
            </span>
          )}
          {live.durationSecs !== undefined && (
            <span
              className="font-mono tabular-nums"
              title={t('board.duration_tooltip')}
            >
              {formatDurationShort(live.durationSecs, t)}
            </span>
          )}
        </div>

        {/* Quick action buttons */}
        <div className="opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-within:opacity-100 transition-opacity flex items-center gap-1">
          {task.status === 'backlog' && (
            <Button
              variant="secondary"
              size="sm"
              className="h-5 px-1.5 text-[10px]"
              onClick={(e) => {
                e.stopPropagation()
                onQuickMove(task, 'queued')
              }}
            >
              {t('board.queue_task')}
            </Button>
          )}
          {task.status === 'review' && (
            <Button
              variant="secondary"
              size="sm"
              className="h-5 px-1.5 text-[10px] text-emerald-500"
              onClick={(e) => {
                e.stopPropagation()
                onQuickMove(task, 'done')
              }}
            >
              {t('board.mark_done')}
            </Button>
          )}
          {(task.status === 'done' || task.status === 'review') && (
            <Button
              variant="ghost"
              size="sm"
              className="h-5 px-1.5 text-[10px]"
              onClick={(e) => {
                e.stopPropagation()
                onQuickMove(task, 'backlog')
              }}
            >
              {t('board.reopen_task')}
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}

function TaskDetailDrawer({
  taskId,
  projects,
  onClose,
  onTaskUpdated,
}: {
  taskId: string
  projects: { id: string; name: string }[]
  onClose: () => void
  onTaskUpdated: () => void
}) {
  const { t } = useI18n()
  const { client } = useDaemon()
  const navigate = useNavigate()
  const profiles = useAgentProfiles()
  const disabledAgentIdsForDrawer = useMemo(
    () =>
      new Set(
        (profiles.data ?? []).filter((p) => !p.enabled).map((p) => p.agentId),
      ),
    [profiles.data],
  )

  const taskQuery = useTaskHydrated(taskId)
  const task = taskQuery.data?.task
  const session = taskQuery.data?.session

  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [status, setStatus] = useState<TaskStatus>('backlog')
  const [assignedAgent, setAssignedAgent] = useState<ProviderKind | 'none'>('none')
  const [labels, setLabels] = useState<string[]>([])
  const [showPreview, setShowPreview] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)
  const [isSaving, setIsSaving] = useState(false)

  // Initialize fields once loaded
  const [loadedTaskId, setLoadedTaskId] = useState<string | null>(null)
  if (task && task.id !== loadedTaskId) {
    setLoadedTaskId(task.id)
    setTitle(task.title)
    setDescription(task.description)
    setStatus(task.status)
    setAssignedAgent(task.assignedAgent ?? 'none')
    setLabels([...task.labels])
  }

  const handleSave = async () => {
    if (!client || !task) return
    setIsSaving(true)
    try {
      // Status rides the guarded moveTask path (transition checks, lifecycle
      // side effects); updateTask carries content only so a stale drawer
      // status can never regress a concurrent daemon transition.
      const updated: Task = {
        ...task,
        title: title.trim(),
        description: description.trim(),
        assignedAgent: assignedAgent === 'none' ? null : assignedAgent,
        labels,
      }
      const saved = await updateTask(client, updated, task.version)
      if (status !== task.status) {
        await moveTask(client, task.id, status, saved.version)
      }
      toast.success(t('common.saved'))
      onTaskUpdated()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    } finally {
      setIsSaving(false)
    }
  }

  const handleDelete = async () => {
    if (!client || !task) return
    try {
      await deleteTask(client, task.id, task.version)
      toast.success(t('common.deleted'))
      onTaskUpdated()
      onClose()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
  }

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onClose])


  return (
    <div className="fixed inset-y-0 right-0 z-40 flex w-full max-w-lg flex-col border-l border-border bg-sidebar-background shadow-2xl animate-in slide-in-from-right duration-200">
      {/* Drawer Header */}
      <div className="flex h-14 flex-none items-center justify-between border-b border-border px-5">
        <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          {t('board.edit_task')}
        </span>
        <Button
          variant="ghost"
          size="sm"
          className="size-8 p-0"
          onClick={onClose}
        >
          <PaduIcon name="x" className="size-4" />
        </Button>
      </div>

      {/* Drawer Content */}
      <div className="flex-1 overflow-y-auto p-5 space-y-5">
        {!task ? (
          <div className="flex h-40 items-center justify-center text-xs text-muted-foreground">
            {t('common.loading')}
          </div>
        ) : (
          <>
            {/* Status & Alerts */}
            {task.syncFailed && (
              <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 p-3 text-xs text-rose-500 flex items-start gap-2">
                <PaduIcon name="syncFailed" className="size-4 shrink-0 mt-0.5" />
                <div>
                  <span className="font-semibold">{t('board.sync_failed')}: </span>
                  {task.syncFailed}
                </div>
              </div>
            )}

            {/* Title */}
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-foreground">
                {t('board.task_title')}
              </label>
              <Input
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder={t('board.task_title_placeholder')}
                className="text-sm font-medium border-0 bg-muted/40 shadow-none focus-visible:ring-1"
              />
            </div>

            {/* Status & Agent Row */}
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-foreground">
                  {t('board.status')}
                </label>
                <select
                  value={status}
                  onChange={(e) => {
                    const next = e.target.value as TaskStatus
                    if (next === 'running') {
                      toast.error(t('board.drag_running_rejected'))
                      return
                    }
                    setStatus(next)
                  }}
                  className="w-full h-9 rounded-md border border-input bg-background px-3 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
                >
                  {COLUMNS.map((col) => (
                    <option key={col.id} value={col.id}>
                      {t(col.labelKey)}
                    </option>
                  ))}
                </select>
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-foreground">
                  {t('board.assigned_agent')}
                </label>
                <select
                  value={assignedAgent}
                  onChange={(e) => setAssignedAgent(e.target.value as ProviderKind | 'none')}
                  className="w-full h-9 rounded-md border border-input bg-background px-3 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
                >
                  <option value="none">{t('board.no_agent')}</option>
                  {PROVIDERS.map((p) => (
                    <option
                      key={p.id}
                      value={p.id}
                      disabled={disabledAgentIdsForDrawer.has(p.id)}
                      title={disabledAgentIdsForDrawer.has(p.id) ? t('board.agent_disabled_tooltip') : undefined}
                    >
                      {p.name}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {/* Description */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <label className="text-xs font-medium text-foreground">
                  {t('board.task_description')}
                </label>
                <button
                  type="button"
                  onClick={() => setShowPreview(!showPreview)}
                  className="text-[11px] text-primary hover:underline"
                >
                  {showPreview ? t('common.edit') : t('common.preview')}
                </button>
              </div>
              {showPreview ? (
                <div className="min-h-[120px] rounded-md border border-input bg-background p-3 text-xs">
                  <MarkdownView text={description || '_No description provided._'} />
                </div>
              ) : (
                <Textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder={t('board.task_description_placeholder')}
                  rows={6}
                  className="text-xs"
                />
              )}
            </div>

            {/* Labels (TagInput) */}
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-foreground">
                {t('board.labels')}
              </label>
              <TagInput
                value={labels}
                onChange={setLabels}
                placeholder={t('board.labels_placeholder')}
                className="border-0 bg-muted/40 shadow-none"
              />
            </div>

            {/* Live Usage (P1-08): same streamed session join as the cards */}
            {(() => {
              const live = taskLiveBadges(session ?? undefined, Math.floor(Date.now() / 1000))
              if (live.tokens === undefined && live.durationSecs === undefined) return null
              const parts: string[] = []
              if (live.tokens !== undefined) parts.push(formatTokens(live.tokens))
              if (live.durationSecs !== undefined) {
                parts.push(formatDurationShort(live.durationSecs, t))
              }
              return (
                <div className="flex items-center justify-between rounded-lg border border-border/80 bg-muted/30 px-3 py-2">
                  <span className="text-xs text-muted-foreground">
                    {t('board.live_usage')}
                  </span>
                  <span
                    className="flex items-center gap-1 text-xs font-medium text-foreground"
                    title={t('board.tokens_tooltip')}
                  >
                    <PaduIcon name="zap" className="size-3.5" />
                    {parts.join(' · ')}
                  </span>
                </div>
              )
            })()}

            {/* Linked Session Info */}
            {task.sessionId && (
              <div className="rounded-lg border border-border/80 bg-muted/30 p-3 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-foreground">
                    {t('common.session')}
                  </span>
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7 text-xs gap-1"
                    onClick={() => {
                      onClose()
                      void navigate({ to: '/', search: { session: task.sessionId! } as any })
                    }}
                  >
                    <PaduIcon name="externalLink" className="size-3.5" />
                    {t('board.open_session')}
                  </Button>
                </div>
                <div className="text-[11px] text-muted-foreground font-mono truncate">
                  ID: {task.sessionId}
                </div>
                {session && (
                  <div className="text-[11px] text-muted-foreground">
                    Status: <span className="font-semibold text-foreground">{session.status}</span> ·{' '}
                    Turns: <span className="font-semibold text-foreground">{session.turns.length}</span>
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>

      {/* Drawer Footer (Sticky on bottom) */}
      <div className="flex h-14 flex-none items-center justify-between border-t border-border px-5 bg-sidebar-background sticky bottom-0 z-10">
        <Button
          variant="destructive"
          size="sm"
          onClick={() => setIsDeleting(true)}
          className="text-xs"
        >
          {t('board.delete_task')}
        </Button>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={onClose} className="text-xs">
            {t('common.cancel')}
          </Button>
          <Button
            size="sm"
            onClick={handleSave}
            disabled={isSaving || !title.trim()}
            className="text-xs"
          >
            {isSaving ? t('common.saving') : t('common.save')}
          </Button>
        </div>
      </div>

      {isDeleting && (
        <ConfirmDialog
          open={isDeleting}
          onOpenChange={setIsDeleting}
          title={t('board.delete_task')}
          description={t('board.delete_task_confirm')}
          confirmLabel={t('common.delete')}
          variant="danger"
          onConfirm={handleDelete}
          onCancel={() => setIsDeleting(false)}
        />
      )}
    </div>
  )
}

function NewTaskDialog({
  projects,
  defaultProjectId,
  initialStatus,
  onClose,
  onCreated,
}: {
  projects: { id: string; name: string }[]
  defaultProjectId: string | undefined
  initialStatus: TaskStatus
  onClose: () => void
  onCreated: (task: Task) => void
}) {
  const { t } = useI18n()
  const { client } = useDaemon()
  const probes = useProviderProbes()
  const profiles = useAgentProfiles()
  // Disabled agents are unselectable with a re-enable tooltip (§6.4); the
  // daemon remains the enforcing source of truth at queue time.
  const disabledAgentIds = useMemo(
    () =>
      new Set(
        (profiles.data ?? []).filter((p) => !p.enabled).map((p) => p.agentId),
      ),
    [profiles.data],
  )

  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [showPreview, setShowPreview] = useState(false)
  const [projectId, setProjectId] = useState(defaultProjectId ?? projects[0]?.id ?? '')
  const [assignedAgent, setAssignedAgent] = useState<ProviderKind | 'none'>('none')
  const [selectedModel, setSelectedModel] = useState<string>('')
  const [labels, setLabels] = useState<string[]>([])
  const [isSubmitting, setIsSubmitting] = useState(false)

  // Compute available models for the assigned agent
  const availableModels = useMemo(() => {
    if (assignedAgent === 'none') return []
    const probeModels = probes.data?.[assignedAgent]?.models?.map((m) => m.id) ?? []
    if (probeModels.length > 0) return probeModels
    return DEFAULT_PROVIDER_MODELS[assignedAgent] ?? []
  }, [assignedAgent, probes.data])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!client || !projectId || !title.trim()) return

    setIsSubmitting(true)
    try {
      const input: CreateTask = {
        projectId,
        title: title.trim(),
        description: description.trim(),
        labels,
        assignedAgent: assignedAgent === 'none' ? null : assignedAgent,
        model: assignedAgent !== 'none' && selectedModel ? selectedModel : undefined,
      }

      const created = await createTask(client, input)
      if (initialStatus !== 'backlog') {
        const moved = await moveTask(client, created.id, initialStatus, created.version)
        onCreated(moved)
      } else {
        onCreated(created)
      }
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 animate-in fade-in duration-150">
      <div className="w-full max-w-md rounded-xl border border-border bg-sidebar-background p-5 shadow-2xl space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-foreground">
            {t('board.create_task')}
          </h3>
          <Button variant="ghost" size="sm" className="size-7 p-0" onClick={onClose}>
            <PaduIcon name="x" className="size-4" />
          </Button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-3.5">
          {/* Project Selector */}
          <div className="space-y-1">
            <label className="text-xs font-medium text-foreground">
              {t('board.project')}
            </label>
            <select
              value={projectId}
              onChange={(e) => setProjectId(e.target.value)}
              required
              className="w-full h-8 rounded-md border border-input bg-background px-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            >
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>

          {/* Title */}
          <div className="space-y-1">
            <label className="text-xs font-medium text-foreground">
              {t('board.task_title')}
            </label>
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t('board.task_title_placeholder')}
              required
              autoFocus
              className="h-8 text-xs"
            />
          </div>

          {/* Assigned Agent */}
          <div className="space-y-1">
            <label className="text-xs font-medium text-foreground">
              {t('board.assigned_agent')}
            </label>
            <select
              value={assignedAgent}
              onChange={(e) => {
                const next = e.target.value as ProviderKind | 'none'
                setAssignedAgent(next)
                setSelectedModel('')
              }}
              className="w-full h-8 rounded-md border border-input bg-background px-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            >
              <option value="none">{t('board.no_agent')}</option>
              {PROVIDERS.map((p) => (
                <option
                  key={p.id}
                  value={p.id}
                  disabled={disabledAgentIds.has(p.id)}
                  title={disabledAgentIds.has(p.id) ? t('board.agent_disabled_tooltip') : undefined}
                >
                  {p.name}
                </option>
              ))}
            </select>
          </div>

          {/* Model based on Assigned Agent */}
          {assignedAgent !== 'none' && (
            <div className="space-y-1 animate-in fade-in duration-100">
              <label className="text-xs font-medium text-foreground">
                {t('board.model')}
              </label>
              <select
                value={selectedModel}
                onChange={(e) => setSelectedModel(e.target.value)}
                className="w-full h-8 rounded-md border border-input bg-background px-2.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
              >
                <option value="">{t('board.default_model')}</option>
                {availableModels.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
          )}

          {/* Description */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <label className="text-xs font-medium text-foreground">
                {t('board.task_description')}
              </label>
              <button
                type="button"
                onClick={() => setShowPreview(!showPreview)}
                className="text-[11px] text-primary hover:underline"
              >
                {showPreview ? t('common.edit') : t('common.preview')}
              </button>
            </div>
            {showPreview ? (
              <div className="min-h-[100px] rounded-md border border-input bg-background p-3 text-xs">
                <MarkdownView text={description || '_No description provided._'} />
              </div>
            ) : (
              <Textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t('board.task_description_placeholder')}
                rows={4}
                className="text-xs"
              />
            )}
          </div>

          {/* Labels (TagInput) */}
          <div className="space-y-1">
            <label className="text-xs font-medium text-foreground">
              {t('board.labels')}
            </label>
            <TagInput
              value={labels}
              onChange={setLabels}
              placeholder={t('board.labels_placeholder')}
              className="border-0 bg-muted/40 shadow-none min-h-[32px] py-1"
            />
          </div>

          {/* Footer Actions */}
          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="outline" size="sm" onClick={onClose} className="h-8 text-xs">
              {t('common.cancel')}
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={isSubmitting || !title.trim()}
              className="h-8 text-xs"
            >
              {isSubmitting ? t('common.creating') : t('board.create_task')}
            </Button>
          </div>
        </form>
      </div>
    </div>
  )
}
