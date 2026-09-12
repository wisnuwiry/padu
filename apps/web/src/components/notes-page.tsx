import { useNavigate, useSearch } from '@tanstack/react-router'
import type { Note, NoteSummary, PaduClient, Project } from '@padu/client'
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { ContextMenu } from '@base-ui/react/context-menu'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import { toast } from 'sonner'
import { ConnectionPanel } from '@/components/connection-panel'
import { MarkdownView } from '@/components/markdown-view'
import { PaduIcon } from '@/components/padu-icon'
import { Sidebar } from '@/components/sidebar'
import { StartupScreen } from '@/components/startup-screen'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { Tooltip } from '@/components/ui/tooltip'
import { useTaskState } from '@/hooks/use-daemon-data'
import {
  createNote,
  deleteNote,
  getNote,
  hydrateSession,
  listNotes,
  persistSession,
  removeSession,
  setSessionArchived,
  setSessionPinned,
  updateNote,
} from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { formatNoteTimeAgo, noteExcerpt } from '@/lib/notes-utils'
import { usePrimaryShortcut } from '@/lib/platform'
import { projectDisplayName } from '@/lib/project-presentation'
import { readSidebarGrouping, readSidebarOrdering, sidebarVisualSessions } from '@/lib/sidebar-presentation'

const ALL_NOTES_PROJECT_ID = '00000000-0000-0000-0000-000000000000'
const SAVE_DEBOUNCE_MS = 650

const CodeFileSurface = lazy(() => import('@/components/code-surfaces').then((module) => ({ default: module.CodeFileSurface })))

type NotesLayout = 'edit' | 'split' | 'preview'

export async function addContentToNewNote(
  client: PaduClient,
  projectId: string | undefined,
  title: string | undefined,
  content: string,
): Promise<void> {
  if (!projectId) throw new Error('Select a project before adding a response to Notes')
  await createNote(client, {
    projectId,
    title: title?.trim() || 'Untitled note',
    content: content.trim(),
  })
}

function readNotesLayout(): NotesLayout {
  if (typeof window === 'undefined') return 'edit'
  const saved = window.localStorage.getItem('padu.notesLayout')
  return saved === 'split' || saved === 'preview' ? saved : 'edit'
}

function readListCollapsed(): boolean {
  if (typeof window === 'undefined') return false
  return window.localStorage.getItem('padu.notesListCollapsed') === 'true'
}

export function NotesPage() {
  const { t } = useI18n()
  const { phase } = useDaemon()
  const taskState = useTaskState()

  if (phase !== 'connected') return <ConnectionPanel title={t('settings.notes')} />
  if (!taskState.data) {
    return (
      <StartupScreen
        error={taskState.error ? errorMessage(taskState.error) : undefined}
        onRetry={() => void taskState.refetch()}
      />
    )
  }
  return <NotesShell />
}

function NotesShell() {
  const navigate = useNavigate({ from: '/notes' })
  const search = useSearch({ from: '/notes' })
  const { client } = useDaemon()
  const { t } = useI18n()
  const taskState = useTaskState()
  const [mobileSidebar, setMobileSidebar] = useState(false)
  const [sidebarVisible, setSidebarVisible] = useState(readSidebarVisible)
  const [sidebarWidth, setSidebarWidth] = useState(readSidebarWidth)

  useEffect(() => {
    try {
      window.localStorage.setItem('padu.sidebarVisible', String(sidebarVisible))
    } catch { /* noop */ }
  }, [sidebarVisible])
  useEffect(() => {
    const timer = window.setTimeout(() => {
      try {
        window.localStorage.setItem('padu.sidebarWidth', String(Math.round(sidebarWidth)))
      } catch { /* noop */ }
    }, 150)
    return () => window.clearTimeout(timer)
  }, [sidebarWidth])

  const goSession = useCallback((sessionId: string | undefined) => {
    window.sessionStorage.removeItem('padu.note-target-session')
    void navigate({ to: '/', search: { session: sessionId } })
  }, [navigate])

  const selectAdjacentSession = useCallback((delta: number) => {
    const data = taskState.data
    if (!data) return
    const started = sidebarVisualSessions(
      data.projects,
      data.sessions,
      readSidebarGrouping(),
      readSidebarOrdering(),
      t('sidebar.unknown_project'),
      t('project.no_project_name'),
    )
    if (!started.length) return
    const targetSession = window.sessionStorage.getItem('padu.note-target-session')
    const currentId = targetSession && targetSession !== 'new' ? targetSession : undefined
    const currentIndex = currentId ? started.findIndex((session) => session.id === currentId) : -1
    const nextIndex = currentIndex >= 0
      ? (delta > 0 ? Math.min(currentIndex + 1, started.length - 1) : Math.max(currentIndex - 1, 0))
      : (delta > 0 ? 0 : started.length - 1)
    const next = started[nextIndex]
    if (next) {
      window.sessionStorage.setItem('padu.note-target-session', next.id)
      void navigate({ to: '/', search: { session: next.id } })
    }
  }, [navigate, t, taskState.data])

  // Global shortcuts that stay available on the notes page, mirroring
  // padu-app: ⌘B toggles the sidebar, ⌘⇧M returns to the conversation that
  // opened notes, and ⌘⌥↑/↓ (or ⌘⇧[/]) steps through conversations.
  // padu-app unmounts on this route, so without these the shortcuts die here.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || !taskState.data) return
      const key = event.key.toLowerCase()
      if (key === 'b' && !event.shiftKey && !event.altKey) {
        event.preventDefault()
        setSidebarVisible((value) => !value)
        return
      }
      if (key === 'm' && event.shiftKey && !event.altKey) {
        event.preventDefault()
        const target = window.sessionStorage.getItem('padu.note-target-session')
        window.sessionStorage.removeItem('padu.note-target-session')
        void navigate({ to: '/', search: { session: target && target !== 'new' ? target : undefined } })
        return
      }
      if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
        event.preventDefault()
        selectAdjacentSession(event.key === 'ArrowDown' ? 1 : -1)
        return
      }
      if (event.shiftKey && (event.code === 'BracketLeft' || event.code === 'BracketRight')) {
        event.preventDefault()
        selectAdjacentSession(event.code === 'BracketRight' ? 1 : -1)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [navigate, selectAdjacentSession, taskState.data])

  if (!taskState.data) {
    return (
      <StartupScreen
        error={taskState.error ? errorMessage(taskState.error) : undefined}
        onRetry={() => void taskState.refetch()}
      />
    )
  }

  return (
    <div className="flex h-dvh w-full overflow-hidden bg-background text-foreground">
      {sidebarVisible && (
        <Sidebar
          mobileOpen={mobileSidebar}
          onAddProject={() => goSession(undefined)}
          onMobileOpenChange={setMobileSidebar}
          onNewTask={() => goSession(undefined)}
          onNotes={() => setMobileSidebar(false)}
          onRemoveSession={async (sessionId) => {
            if (!client) return
            try {
              await removeSession(client, sessionId)
              await taskState.refetch()
            } catch (error) {
              toast.error(error instanceof Error ? error.message : t('errors.update_task', { error: '' }))
            }
          }}
          onRenameSession={async (sessionId, title) => {
            if (!client) throw new Error(t('errors.daemon_disconnected'))
            const hydrated = await hydrateSession(client, sessionId)
            if (!hydrated) throw new Error(t('errors.task_not_found'))
            await persistSession(client, { ...hydrated, title, updated_at: Math.floor(Date.now() / 1_000) })
            await taskState.refetch()
          }}
          onSearch={() => goSession(undefined)}
          onSelectSession={(sessionId) => {
            setMobileSidebar(false)
            goSession(sessionId)
          }}
          onSettings={() => {
            const target = window.sessionStorage.getItem('padu.note-target-session')
            void navigate({ to: '/settings/$page', params: { page: 'general' }, search: { session: target && target !== 'new' ? target : undefined } })
          }}
          onSetSessionArchived={async (sessionId, archived) => {
            if (!client) return
            await setSessionArchived(client, sessionId, archived)
            await taskState.refetch()
          }}
          onSetSessionPinned={async (sessionId, pinned) => {
            if (!client) return
            await setSessionPinned(client, sessionId, pinned)
            await taskState.refetch()
          }}
          onToggleSidebar={() => {
            if (window.matchMedia('(max-width: 1023px)').matches) setMobileSidebar(false)
            else setSidebarVisible(false)
          }}
          onUsage={() => {
            const target = window.sessionStorage.getItem('padu.note-target-session')
            void navigate({ to: '/settings/$page', params: { page: 'usage' }, search: { session: target && target !== 'new' ? target : undefined } })
          }}
          onWidthChange={setSidebarWidth}
          selectedSessionId={undefined}
          taskState={taskState.data}
          width={sidebarWidth}
        />
      )}
      <NotesWorkspace
        projectId={search.projectId}
        initialNoteId={search.noteId}
        initialQuery={search.q}
        projects={taskState.data.projects}
        sidebarVisible={sidebarVisible}
        onShowSidebar={() => {
          setSidebarVisible(true)
          if (window.matchMedia('(max-width: 1023px)').matches) setMobileSidebar(true)
        }}
      />
    </div>
  )
}

function NotesWorkspace({
  projectId,
  initialNoteId,
  initialQuery,
  projects,
  sidebarVisible,
  onShowSidebar,
}: {
  projectId: string | undefined
  initialNoteId: string | undefined
  initialQuery: string | undefined
  projects: Project[]
  sidebarVisible: boolean
  onShowSidebar: () => void
}) {
  const navigate = useNavigate({ from: '/notes' })
  const { client } = useDaemon()
  const { t } = useI18n()
  const newShortcut = usePrimaryShortcut('⌘⌥N', 'Ctrl+Alt+N')
  const editShortcut = usePrimaryShortcut('⌘1', 'Ctrl+1')
  const splitShortcut = usePrimaryShortcut('⌘2', 'Ctrl+2')
  const previewShortcut = usePrimaryShortcut('⌘3', 'Ctrl+3')
  const deleteShortcut = usePrimaryShortcut('⌘⌫', 'Ctrl+Backspace')
  const chatShortcut = usePrimaryShortcut('⌘↵', 'Ctrl+Enter')
  const listShortcut = usePrimaryShortcut('⌘⇧L', 'Ctrl+Shift+L')
  const toggleSidebarShortcut = usePrimaryShortcut('⌘B', 'Ctrl+B')

  const [notes, setNotes] = useState<NoteSummary[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selected, setSelected] = useState<Note | null>(null)
  const [filter, setFilter] = useState(initialQuery ?? '')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [layout, setLayout] = useState<NotesLayout>(readNotesLayout)
  const [listCollapsed, setListCollapsed] = useState(readListCollapsed)
  const [splitRatio, setSplitRatio] = useState(() => readSplitRatio())
  const [deleteTarget, setDeleteTarget] = useState<NoteSummary | null>(null)
  const listRef = useRef<VirtuosoHandle>(null)
  const autosaveKey = useRef<string | null>(null)
  const saveTimer = useRef<number | null>(null)
  const pendingSave = useRef<{ id: string; title: string; content: string; revision: number; projectId: string } | null>(null)
  // Bumped per refresh; a completion from a superseded request is discarded so
  // it cannot replace newer notes or selections with stale data.
  const refreshGeneration = useRef(0)

  const defaultProjectId = projectId ?? projects[0]?.id
  const projectById = useMemo(() => new Map(projects.map((project) => [project.id, project])), [projects])

  useEffect(() => {
    try {
      window.localStorage.setItem('padu.notesLayout', layout)
    } catch { /* noop */ }
  }, [layout])
  useEffect(() => {
    try {
      window.localStorage.setItem('padu.notesListCollapsed', String(listCollapsed))
    } catch { /* noop */ }
  }, [listCollapsed])
  useEffect(() => {
    const timer = window.setTimeout(() => {
      try {
        window.localStorage.setItem('padu.notesSplitRatio', String(splitRatio))
      } catch { /* noop */ }
    }, 150)
    return () => window.clearTimeout(timer)
  }, [splitRatio])

  const refresh = useCallback(async (preferredId?: string | null) => {
    const generation = ++refreshGeneration.current
    if (!client) {
      if (generation !== refreshGeneration.current) return
      setNotes([])
      setSelected(null)
      setLoading(false)
      return
    }
    setLoading(true)
    try {
      const next = await listNotes(client, ALL_NOTES_PROJECT_ID)
      if (generation !== refreshGeneration.current) return
      setNotes(next)
      const id = preferredId ?? selectedId ?? initialNoteId ?? next[0]?.id ?? null
      const summary = next.find((note) => note.id === id)
      if (generation !== refreshGeneration.current) return
      setSelectedId(id)
      setSelected(id && summary ? await getNote(client, summary.projectId, id) : null)
    } catch (error) {
      if (generation !== refreshGeneration.current) return
      toast.error(error instanceof Error ? error.message : t('notes.save_failed', { error: '' }))
    } finally {
      if (generation === refreshGeneration.current) setLoading(false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, initialNoteId, selectedId, t])

  useEffect(() => {
    void refresh(initialNoteId)
    // Notes are project-independent; only the daemon connection identifies this resource.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client])

  const visibleNotes = useMemo(() => {
    const query = filter.trim().toLocaleLowerCase()
    if (!query) return notes
    return notes.filter((note) => `${note.title} ${note.preview}`.toLocaleLowerCase().includes(query))
  }, [filter, notes])

  useEffect(() => {
    if (!selected) return
    if (selectedId && selected.id !== selectedId) return
    pendingSave.current = {
      id: selected.id,
      title: selected.title,
      content: selected.content,
      revision: selected.revision,
      projectId: selected.projectId,
    }
    if (autosaveKey.current === null) {
      autosaveKey.current = `${selected.id}:${selected.title}:${selected.content}`
      return
    }
    const key = `${selected.id}:${selected.title}:${selected.content}`
    if (autosaveKey.current === key) return
    autosaveKey.current = key
    if (saveTimer.current !== null) window.clearTimeout(saveTimer.current)
    saveTimer.current = window.setTimeout(() => {
      saveTimer.current = null
      void persistEdit()
    }, SAVE_DEBOUNCE_MS)
    // persistEdit reads pendingSave when the debounce fires.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected?.id, selected?.title, selected?.content])

  useEffect(() => () => {
    if (saveTimer.current !== null) window.clearTimeout(saveTimer.current)
  }, [])

  async function persistEdit(retry = true) {
    const pending = pendingSave.current
    if (!client || !pending) return
    const normalizedTitle = pending.title.trim() || t('notes.untitled')
    setSaving(true)
    try {
      const saved = await updateNote(client, {
        projectId: pending.projectId,
        noteId: pending.id,
        title: normalizedTitle,
        content: pending.content,
        expectedRevision: pending.revision,
      })
      pendingSave.current = { ...pending, title: saved.title, content: saved.content, revision: saved.revision }
      autosaveKey.current = `${saved.id}:${saved.title}:${saved.content}`
      setSelected((current) => current && current.id === saved.id ? saved : current)
      setNotes((current) => current.map((note) => note.id === saved.id
        ? { ...note, title: saved.title, preview: noteExcerpt(saved.content), revision: saved.revision, updatedAt: saved.updatedAt }
        : note))
    } catch {
      // A revision conflict means the daemon accepted a different write.
      // Fetch the current revision and retry the still-pending content so the
      // editor's newer text is not silently abandoned.
      if (retry) {
        try {
          const fresh = await getNote(client, pending.projectId, pending.id)
          if (fresh) {
            pendingSave.current = { ...pending, revision: fresh.revision }
            await persistEdit(false)
            return
          }
        } catch {
          // Fall through to the failure path below.
        }
      }
      toast.error(t('notes.save_failed', { error: '' }))
      await refresh(pending.id)
    } finally {
      setSaving(false)
    }
  }

  function flushPendingSave() {
    if (saveTimer.current !== null) {
      window.clearTimeout(saveTimer.current)
      saveTimer.current = null
    }
    const pending = pendingSave.current
    if (!pending || !selected || pending.id !== selected.id) return
    const key = `${selected.id}:${selected.title}:${selected.content}`
    if (autosaveKey.current === key) return
    autosaveKey.current = key
    pendingSave.current = {
      id: selected.id,
      title: selected.title,
      content: selected.content,
      revision: selected.revision,
      projectId: selected.projectId,
    }
    void persistEdit()
  }

  async function selectNote(id: string) {
    if (!client || id === selectedId) return
    flushPendingSave()
    // Selecting a note returns to the editor, matching desktop.
    setLayout('edit')
    const summary = notes.find((note) => note.id === id)
    if (!summary) return
    setSelectedId(id)
    try {
      setSelected(await getNote(client, summary.projectId, id))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t('notes.save_failed', { error: '' }))
    }
  }

  function selectAdjacent(direction: 1 | -1) {
    if (!visibleNotes.length) return
    const currentIndex = visibleNotes.findIndex((note) => note.id === selectedId)
    const nextIndex = currentIndex === -1
      ? (direction < 0 ? visibleNotes.length - 1 : 0)
      : Math.min(visibleNotes.length - 1, Math.max(0, currentIndex + direction))
    const next = visibleNotes[nextIndex]
    if (next) void selectNote(next.id)
  }

  async function addNote() {
    if (!client) return
    if (!defaultProjectId) {
      toast.error(t('notes.add_failed'))
      return
    }
    flushPendingSave()
    setLayout('edit')
    try {
      const note = await createNote(client, { projectId: defaultProjectId, title: t('notes.untitled'), content: '' })
      autosaveKey.current = `${note.id}:${note.title}:${note.content}`
      pendingSave.current = { id: note.id, title: note.title, content: note.content, revision: note.revision, projectId: note.projectId }
      await refresh(note.id)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t('notes.add_failed'))
    }
  }

  function addNoteToChat(note: Pick<Note, 'projectId' | 'id'>) {
    if (typeof window === 'undefined') return
    const targetSession = window.sessionStorage.getItem('padu.note-target-session') ?? 'new'
    window.sessionStorage.setItem('padu.pending-composer-note', `${targetSession}:${note.projectId}:${note.id}`)
    window.sessionStorage.removeItem('padu.note-target-session')
    void navigate({ to: '/', search: { session: targetSession === 'new' ? undefined : targetSession } })
  }

  async function confirmDeleteNote() {
    if (!client || !deleteTarget) return
    const target = deleteTarget
    try {
      const note = selected?.id === target.id
        ? selected
        : await getNote(client, target.projectId, target.id)
      if (!note) return
      if (saveTimer.current !== null) {
        window.clearTimeout(saveTimer.current)
        saveTimer.current = null
      }
      pendingSave.current = null
      autosaveKey.current = null
      await deleteNote(client, note.projectId, note.id, note.revision)
      setDeleteTarget(null)
      const remaining = notes.filter((item) => item.id !== note.id)
      const nextId = remaining[0]?.id ?? null
      await refresh(nextId)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t('notes.save_failed', { error: '' }))
    }
  }

  // Desktop parity shortcuts: Alt+N new, Cmd/Ctrl+1/2/3 layout,
  // Cmd/Ctrl+Enter add to chat, Cmd/Ctrl+Backspace delete,
  // Cmd/Ctrl+Shift+L toggle list, Cmd/Ctrl+Up/Down navigate.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null
      const typing = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement
      const primary = event.metaKey || event.ctrlKey
      if (event.altKey && !event.shiftKey && event.key.toLowerCase() === 'n') {
        event.preventDefault()
        void addNote()
        return
      }
      if (!primary || event.altKey) return
      const key = event.key.toLowerCase()
      if (key === '1' && !event.shiftKey) {
        event.preventDefault()
        setLayout('edit')
      } else if (key === '2' && !event.shiftKey) {
        event.preventDefault()
        setLayout('split')
      } else if (key === '3' && !event.shiftKey) {
        event.preventDefault()
        setLayout('preview')
      } else if (key === 'enter' && !event.shiftKey) {
        if (typing) return
        event.preventDefault()
        if (selected) addNoteToChat(selected)
      } else if ((key === 'backspace' || key === 'delete') && !event.shiftKey) {
        if (typing) return
        event.preventDefault()
        const summary = notes.find((note) => note.id === selectedId)
        if (summary) setDeleteTarget(summary)
      } else if (key === 'l' && event.shiftKey) {
        event.preventDefault()
        setListCollapsed((value) => !value)
      } else if (key === 'arrowdown' && !event.shiftKey) {
        if (typing) return
        event.preventDefault()
        selectAdjacent(1)
      } else if (key === 'arrowup' && !event.shiftKey) {
        if (typing) return
        event.preventDefault()
        selectAdjacent(-1)
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [notes, selected, selectedId, visibleNotes])

  const selectedSummary = selectedId ? notes.find((note) => note.id === selectedId) ?? null : null
  const projectName = selected?.projectId
    ? projectById.has(selected.projectId)
      ? projectDisplayName(projectById.get(selected.projectId)!, t('project.no_project_name'))
      : t('project.no_project_name')
    : t('project.no_project_name')

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col bg-background">
      <header className="flex h-12 shrink-0 items-center gap-1.5 px-3.5">
        {!sidebarVisible && (
          <Tooltip content={t('menu.toggle_sidebar')} shortcut={toggleSidebarShortcut}>
            <button
              aria-label={t('menu.toggle_sidebar')}
              className="grid size-7 cursor-pointer place-items-center rounded-md text-[var(--text-tertiary)] outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
              type="button"
              onClick={onShowSidebar}
            >
              <PaduIcon name="panelLeft" />
            </button>
          </Tooltip>
        )}
        <span className="grid size-7 place-items-center text-[var(--text-secondary)]">
          <PaduIcon name="note" />
        </span>
        <h1 className="text-[13px] font-medium">{t('settings.notes')}</h1>
        <Tooltip content={t(listCollapsed ? 'notes.expand' : 'notes.collapse')} shortcut={listShortcut}>
          <button
            aria-label={t(listCollapsed ? 'notes.expand' : 'notes.collapse')}
            className="grid size-7 cursor-pointer place-items-center rounded-md text-[var(--text-tertiary)] outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
            type="button"
            onClick={() => setListCollapsed((value) => !value)}
          >
            <PaduIcon name={listCollapsed ? 'expand2' : 'collapse2'} />
          </button>
        </Tooltip>
        <div className="flex-1" />
        {saving && <span className="text-[11px] text-[var(--text-tertiary)]">{t('notes.save')}</span>}
      </header>

      <div className="flex min-h-0 flex-1 gap-4 px-2 pb-6">
        {!listCollapsed && (
          <aside aria-label={t('settings.notes')} className="flex w-60 shrink-0 flex-col gap-2">
            <Tooltip content={t('notes.new')} shortcut={newShortcut}>
              <button
                className="flex h-7 w-full cursor-pointer items-center justify-center gap-1.5 rounded-md px-2.5 text-[12.5px] text-[var(--text-secondary)] outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
                type="button"
                disabled={!defaultProjectId}
                onClick={() => void addNote()}
              >
                <PaduIcon name="plus" className="size-3.5" />
                {t('notes.new')}
              </button>
            </Tooltip>
            <div className="flex h-[30px] shrink-0 items-center gap-1.5 rounded-md bg-muted px-2">
              <PaduIcon name="search" className="size-3.5 shrink-0 text-[var(--text-tertiary)]" />
              <label className="sr-only" htmlFor="note-search">{t('notes.search_placeholder')}</label>
              <input
                id="note-search"
                className="min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-[var(--text-ghost)]"
                placeholder={t('notes.search_placeholder')}
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
              />
            </div>
            <div className="relative min-h-0 flex-1">
              {loading ? (
                <p className="px-2 py-7 text-center text-[12px] text-[var(--text-tertiary)]">{t('files.loading')}</p>
              ) : visibleNotes.length ? (
                <Virtuoso
                  aria-label={t('settings.notes')}
                  className="size-full outline-none"
                  computeItemKey={(_, note) => note.id}
                  data={visibleNotes}
                  fixedItemHeight={56}
                  increaseViewportBy={240}
                  itemContent={(_, note) => (
                    <NoteRow
                      active={note.id === selectedId}
                      note={note}
                      projectName={projectById.has(note.projectId)
                        ? projectDisplayName(projectById.get(note.projectId)!, t('project.no_project_name'))
                        : t('project.no_project_name')}
                      t={t}
                      onAddToChat={() => addNoteToChat(note)}
                      onDelete={() => setDeleteTarget(note)}
                      onSelect={() => void selectNote(note.id)}
                    />
                  )}
                  ref={listRef}
                />
              ) : (
                <div className="flex flex-col items-center gap-2 px-3 py-7 text-center">
                  <PaduIcon name="note" className="size-[22px] text-[var(--text-tertiary)]" />
                  <p className="text-[13px] text-[var(--text-secondary)]">
                    {notes.length ? t('notes.no_matches') : t('notes.empty_title')}
                  </p>
                  <p className="text-[11px] text-[var(--text-tertiary)]">
                    {notes.length ? filter.trim() : t('notes.empty_message')}
                  </p>
                </div>
              )}
            </div>
          </aside>
        )}

        <section aria-label={t('notes.label')} className="flex min-h-0 min-w-0 flex-1 flex-col gap-2.5">
          {selected ? (
            <>
              <label className="sr-only" htmlFor="note-title">{t('notes.title_placeholder')}</label>
              <input
                id="note-title"
                className="h-[38px] w-full shrink-0 rounded-lg px-1 text-[22px] font-bold outline-none placeholder:text-[var(--text-ghost)] focus-visible:ring-2 focus-visible:ring-ring"
                placeholder={t('notes.title_placeholder')}
                value={selected.title}
                onChange={(event) => setSelected({ ...selected, title: event.target.value })}
              />
              <div className="flex w-full shrink-0 items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-[var(--text-tertiary)]">
                  <Tooltip content={t('files.add_to_chat')} shortcut={chatShortcut}>
                    <button
                      className="flex h-7 cursor-pointer items-center gap-1.5 rounded-md bg-foreground px-2.5 text-[12px] font-medium text-background outline-none hover:opacity-90 focus-visible:ring-2 focus-visible:ring-ring"
                      type="button"
                      onClick={() => addNoteToChat(selected)}
                    >
                      <PaduIcon name="addToChat" className="size-3.5" />
                      {t('files.add_to_chat')}
                    </button>
                  </Tooltip>
                  <span className="hidden min-w-0 truncate xl:inline">
                    {t('notes.created', { time: formatNoteTimeAgo(selected.createdAt, t) })} · {t('notes.updated', { time: formatNoteTimeAgo(selected.updatedAt, t) })}
                  </span>
                </div>
                <div className="flex shrink-0 items-center gap-0.5">
                  <div className="flex items-center gap-px rounded-[7px] bg-muted p-0.5" role="group" aria-label={t('notes.details')}>
                    <LayoutButton
                      active={layout === 'edit'}
                      icon="pencil"
                      label={t('notes.edit')}
                      shortcut={editShortcut}
                      onClick={() => setLayout('edit')}
                    />
                    <LayoutButton
                      active={layout === 'split'}
                      icon="split"
                      label={t('notes.side_by_side')}
                      shortcut={splitShortcut}
                      onClick={() => setLayout('split')}
                    />
                    <LayoutButton
                      active={layout === 'preview'}
                      icon="eye"
                      label={t('notes.preview')}
                      shortcut={previewShortcut}
                      onClick={() => setLayout('preview')}
                    />
                  </div>
                  <Tooltip content={t('notes.delete')} shortcut={deleteShortcut}>
                    <button
                      aria-label={t('notes.delete')}
                      className="grid size-7 cursor-pointer place-items-center rounded-md text-[var(--text-tertiary)] outline-none hover:bg-accent hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring"
                      type="button"
                      onClick={() => {
                        const summary = notes.find((note) => note.id === selected.id)
                        if (summary) setDeleteTarget(summary)
                      }}
                    >
                      <PaduIcon name="trash" className="size-3.5" />
                    </button>
                  </Tooltip>
                </div>
              </div>

              {layout === 'edit' && (
                <div className="min-h-0 flex-1 overflow-hidden rounded-lg">
                  <Suspense fallback={<EditorLoading />}>
                    <CodeFileSurface
                      key={selected.id}
                      path="note.md"
                      contents={selected.content}
                      cacheKey={`note:${selected.id}`}
                      onChange={(content) => setSelected({ ...selected, content })}
                    />
                  </Suspense>
                </div>
              )}
              {layout === 'preview' && (
                <MarkdownView
                  className="min-h-0 flex-1 overflow-y-auto rounded-lg bg-muted/60 p-3.5 text-[14px] leading-7 break-words"
                  text={selected.content}
                />
              )}
              {layout === 'split' && (
                <SplitEditor
                  key={selected.id}
                  noteId={selected.id}
                  content={selected.content}
                  onChange={(content) => setSelected({ ...selected, content })}
                  ratio={splitRatio}
                  onRatioChange={setSplitRatio}
                />
              )}
            </>
          ) : (
            <div className="grid min-h-0 flex-1 place-items-center">
              <div className="flex flex-col items-center gap-2 px-3 py-7 text-center">
                <PaduIcon name="note" className="size-[22px] text-[var(--text-tertiary)]" />
                <p className="text-[13px] text-[var(--text-secondary)]">{t('notes.empty_title')}</p>
                <p className="text-[11px] text-[var(--text-tertiary)]">{t('notes.empty_message')}</p>
              </div>
            </div>
          )}
        </section>
      </div>

      <ConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        title={t('notes.delete_title')}
        description={t('notes.delete_message', { title: deleteTarget?.title || t('notes.untitled') })}
        confirmLabel={t('notes.delete')}
        cancelLabel={t('common.cancel')}
        icon="trash"
        onConfirm={() => void confirmDeleteNote()}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  )
}

function LayoutButton({
  active,
  icon,
  label,
  shortcut,
  onClick,
}: {
  active: boolean
  icon: 'pencil' | 'split' | 'eye'
  label: string
  shortcut: string
  onClick: () => void
}) {
  return (
    <Tooltip content={label} shortcut={shortcut}>
      <button
        aria-label={label}
        aria-pressed={active}
        className={`grid size-7 cursor-pointer place-items-center rounded-md outline-none focus-visible:ring-2 focus-visible:ring-ring ${active ? 'bg-background text-foreground shadow-sm' : 'text-[var(--text-tertiary)] hover:bg-accent hover:text-foreground'}`}
        type="button"
        onClick={onClick}
      >
        <PaduIcon name={icon} className="size-3.5" />
      </button>
    </Tooltip>
  )
}

function NoteRow({
  note,
  active,
  projectName,
  t,
  onSelect,
  onAddToChat,
  onDelete,
}: {
  note: NoteSummary
  active: boolean
  projectName: string
  t: (key: string, params?: Record<string, string | number>) => string
  onSelect: () => void
  onAddToChat: () => void
  onDelete: () => void
}) {
  const firstLine = noteExcerpt(note.preview).split('\n')[0]?.trim() ?? ''
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger
        aria-current={active ? 'page' : undefined}
        className={`mb-1 flex min-h-[48px] w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring ${active ? 'bg-accent' : 'hover:bg-accent/70'}`}
        onClick={onSelect}
        onKeyDown={(event) => {
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            onSelect()
          }
        }}
        tabIndex={0}
      >
        <span className="grid size-[26px] shrink-0 place-items-center rounded-md bg-muted">
          <PaduIcon name="note" className="size-3.5 text-[var(--text-secondary)]" />
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-px">
          <span className="block truncate text-[12.5px]">{note.title || t('notes.untitled')}</span>
          {firstLine && <span className="block truncate text-[11px] text-[var(--text-tertiary)]">{firstLine}</span>}
          <span className="block truncate text-[10px] text-[var(--text-tertiary)]">
            {projectName} · {formatNoteTimeAgo(note.updatedAt, t)}
          </span>
        </span>
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Positioner className="z-[100] outline-none">
          <ContextMenu.Popup className="padu-menu-surface">
            <ContextMenu.Item className="padu-menu-item" onClick={onAddToChat}>
              <PaduIcon className="size-3" name="addToChat" /> {t('files.add_to_chat')}
            </ContextMenu.Item>
            <ContextMenu.Separator className="padu-menu-separator" />
            <ContextMenu.Item className="padu-menu-item text-destructive" onClick={onDelete}>
              <PaduIcon className="size-3" name="trash" /> {t('notes.delete')}
            </ContextMenu.Item>
          </ContextMenu.Popup>
        </ContextMenu.Positioner>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}

function EditorLoading() {
  const { t } = useI18n()
  return (
    <div className="grid size-full place-items-center bg-muted/60 text-[11px] text-[var(--text-tertiary)]">
      {t('files.preparing_syntax')}
    </div>
  )
}

function SplitEditor({
  noteId,
  content,
  onChange,
  ratio,
  onRatioChange,
}: {
  noteId: string
  content: string
  onChange: (content: string) => void
  ratio: number
  onRatioChange: (ratio: number) => void
}) {
  const containerRef = useRef<HTMLDivElement>(null)
  const dragging = useRef(false)

  return (
    <div ref={containerRef} className="flex min-h-0 flex-1 gap-1.5">
      <div className="min-h-0 min-w-0 overflow-hidden rounded-lg" style={{ flexGrow: ratio, flexShrink: 1, flexBasis: 0 }}>
        <Suspense fallback={<EditorLoading />}>
          <CodeFileSurface
            path="note.md"
            contents={content}
            cacheKey={`note-split:${noteId}`}
            onChange={onChange}
          />
        </Suspense>
      </div>
      <div
        aria-label="Resize editor and preview"
        aria-orientation="vertical"
        aria-valuemax={80}
        aria-valuemin={20}
        aria-valuenow={Math.round(ratio * 100)}
        className="group relative w-2 shrink-0 cursor-col-resize touch-none outline-none"
        role="separator"
        tabIndex={0}
        onKeyDown={(event) => {
          const step = event.shiftKey ? 0.1 : 0.04
          if (event.key === 'ArrowLeft') {
            event.preventDefault()
            onRatioChange(clampRatio(ratio - step))
          } else if (event.key === 'ArrowRight') {
            event.preventDefault()
            onRatioChange(clampRatio(ratio + step))
          }
        }}
        onPointerDown={(event) => {
          if (event.button !== 0) return
          dragging.current = true
          event.currentTarget.setPointerCapture(event.pointerId)
          event.preventDefault()
        }}
        onPointerMove={(event) => {
          if (!dragging.current || !containerRef.current) return
          const rect = containerRef.current.getBoundingClientRect()
          if (rect.width <= 0) return
          onRatioChange(clampRatio((event.clientX - rect.left) / rect.width))
        }}
        onPointerUp={(event) => {
          dragging.current = false
          if (event.currentTarget.hasPointerCapture(event.pointerId)) {
            event.currentTarget.releasePointerCapture(event.pointerId)
          }
        }}
        onPointerCancel={() => {
          dragging.current = false
        }}
      >
        <span className="absolute inset-y-0 left-[3px] w-0.5 bg-transparent transition-colors motion-reduce:transition-none group-hover:bg-ring/70 group-focus-visible:bg-ring group-active:bg-ring" />
      </div>
      <MarkdownView
        className="min-h-0 min-w-0 overflow-y-auto rounded-lg bg-muted/60 p-3.5 text-[14px] leading-7 break-words"
        style={{ flexGrow: 1 - ratio, flexShrink: 1, flexBasis: 0 }}
        text={content}
      />
    </div>
  )
}

function clampRatio(value: number): number {
  return Math.min(0.8, Math.max(0.2, value))
}

function readSplitRatio(): number {
  if (typeof window === 'undefined') return 0.5
  const raw = Number(window.localStorage.getItem('padu.notesSplitRatio'))
  return Number.isFinite(raw) ? clampRatio(raw) : 0.5
}

function readSidebarWidth(): number {
  if (typeof window === 'undefined') return 252
  const raw = window.localStorage.getItem('padu.sidebarWidth')
  if (raw === null) return 252
  const stored = Number(raw)
  return Number.isFinite(stored) ? Math.min(420, Math.max(180, stored)) : 252
}

function readSidebarVisible(): boolean {
  if (typeof window === 'undefined') return true
  return window.localStorage.getItem('padu.sidebarVisible') !== 'false'
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
