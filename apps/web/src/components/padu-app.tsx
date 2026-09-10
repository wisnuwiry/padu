import { useQueryClient } from '@tanstack/react-query'
import { useNavigate, useSearch } from '@tanstack/react-router'
import { Popover } from '@base-ui/react/popover'
import type {
  AgentSession,
  ComposerDraft,
  ComposerDraftChange,
  EmbeddedNote,
  ComposerDrafts,
  ComposerDraftTarget,
  MessageAttachment,
  NoteSummary,
  Project,
  ProviderSessionSummary,
  ReviewDiffSource,
} from '@padu/client'
import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Tooltip } from '@/components/ui/tooltip'
import {
  CommandPalette,
  type CommandPaletteActions,
  type CommandPaletteView,
} from '@/components/command-palette'
import { CommitDialog } from '@/components/commit-dialog'
import { Composer } from '@/components/composer'
import { ControlMenu } from '@/components/control-menu'
import { DaemonFilePicker } from '@/components/daemon-file-picker'
import { OnboardingModal } from '@/components/onboarding-modal'
import {
  RightPanel,
  PanelTabStrip,
  panelTabId,
  type PanelSurface,
  type PanelTab,
  type RightPanelHandle,
} from '@/components/right-panel'
import {
  tabNavigationIndex,
  type TabNavigationKey,
} from '@/lib/right-panel-state'
import { cn } from '@/lib/utils'
import { Sidebar } from '@/components/sidebar'
import { StartupScreen } from '@/components/startup-screen'
import type { SettingsPageId } from '@/components/settings-view'
import { Transcript } from '@/components/transcript'
import { addContentToNewNote } from '@/components/notes-page'
import { PaduIcon } from '@/components/padu-icon'
import {
  useComposerDrafts,
  useComposerFiles,
  useSession,
  useSessionTurnRefs,
  useTaskState,
  useWorkspaceBranches,
} from '@/hooks/use-daemon-data'
import { useDocumentTitle } from '@/hooks/use-document-title'
import {
  applyComposerDraftChanges,
  createResumedSession,
  createSession,
  createProject,
  createProjectlessWorkspace,
  daemonKeys,
  displayTitle,
  getNote,
  hydrateSession,
  loadProviderSessionHistory,
  persistProject,
  removeSession,
  selectableProjects,
  sameProviderSession,
  sessionCwd,
  setSessionArchived,
  setSessionPinned,
  type TaskState,
} from '@/lib/daemon-api'
import {
  composerDraftFor,
  composerDraftId,
  moveComposerDraftToEmpty,
  setComposerDraft,
} from '@/lib/composer-drafts'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import {
  browserComposerPreferenceStorage,
  readComposerPreferences,
} from '@/lib/composer-preferences'
import {
  browserNavigationStorage,
  readRememberedNavigation,
  routeDestinationTransition,
  taskRemovalDestination,
  writeRememberedNavigation,
  type RememberedNavigation,
} from '@/lib/navigation-memory'
import { transcriptLinkRoute } from '@/lib/transcript-links'
import { sessionHasStarted, sortSidebarSessions } from '@/lib/sidebar-presentation'
import { shouldShowInitialDestination } from '@/lib/workspace-presentation'
import { usePrimaryShortcut } from '@/lib/platform'
import { agentPresetIdLabel } from '@/lib/agent-preset-presentation'
import { isProjectlessProject, projectDisplayName } from '@/lib/project-presentation'
import {
  useRuntime,
  type BackgroundWorkItem,
  type BackgroundWorkKey,
  type BackgroundWorkStatus,
} from '@/lib/runtime-context'

interface RetainedPanelSession {
  session: AgentSession
  project?: Project
}

const DEFAULT_SIDEBAR_WIDTH = 252
const DEFAULT_RIGHT_PANEL_WIDTH = 460

type Translator = (key: string, params?: Record<string, string | number>) => string

export function PaduApp() {
  const { t } = useI18n()
  const navigate = useNavigate({ from: '/' })
  const search = useSearch({ from: '/' })
  const queryClient = useQueryClient()
  const { client, config } = useDaemon()
  const {
    attachSession,
    backgroundWork,
    closeSession,
    forkSessionFromResponse,
    messageRewinds,
    responseForks,
    rewindSessionToMessage,
    runtimes,
    saveSession,
  } = useRuntime()
  const taskState = useTaskState()
  const loadedComposerDrafts = useComposerDrafts()
  const selected = useSession(search.session)
  const [displayed, setDisplayed] = useState<AgentSession | null>(null)
  const [mobileSidebar, setMobileSidebar] = useState(false)
  const [sidebarVisible, setSidebarVisible] = useState(readSidebarVisible)
  const [sidebarWidth, setSidebarWidth] = useState(readSidebarWidth)
  const [rightPanelWidth, setRightPanelWidth] = useState(readRightPanelWidth)
  const [rightPanelOpenBySession, setRightPanelOpenBySession] = useState<Record<string, boolean>>({})
  const [panelFullscreenBySession, setPanelFullscreenBySession] = useState<
    Record<string, { expanded: boolean, conversation: boolean }>
  >({})
  const [panelExpandableBySession, setPanelExpandableBySession] = useState<Record<string, boolean>>({})
  const [panelTabsBySession, setPanelTabsBySession] = useState<
    Record<string, { tabs: PanelTab[]; activeId: string | null }>
  >({})
  const rightPanelRefs = useRef<Record<string, RightPanelHandle | null>>({})
  const retainedPanelSessions = useRef(new Map<string, RetainedPanelSession>())
  const [retainedPanelSessionIds, setRetainedPanelSessionIds] = useState<string[]>([])
  const [paletteOpen, setPaletteOpen] = useState(false)
  const [paletteInitialView, setPaletteInitialView] = useState<CommandPaletteView>('commands')
  const [paletteInitialQuery, setPaletteInitialQuery] = useState('')
  const [focusComposerSignal, setFocusComposerSignal] = useState(0)
  const [modelPickerSignal, setModelPickerSignal] = useState(0)
  const [usagePanelSignal, setUsagePanelSignal] = useState(0)
  const [composerPrefill, setComposerPrefill] = useState<{
    sessionId: string
    text: string
    signal: number
  } | null>(null)
  const [composerAttachment, setComposerAttachment] = useState<{
    sessionId: string
    paths: string[]
    signal: number
  } | null>(null)
  const [composerMention, setComposerMention] = useState<{
    sessionId: string
    mention: string
    signal: number
  } | null>(null)
  const [composerEmbeddedNote, setComposerEmbeddedNote] = useState<{
    sessionId: string
    note: EmbeddedNote
  } | null>(null)
  const [requestedPanel, setRequestedPanel] = useState<PanelSurface>('files')
  const [requestedFile, setRequestedFile] = useState<string | null>(null)
  const [requestedDiffSource, setRequestedDiffSource] = useState<ReviewDiffSource>('uncommitted')
  const [requestedBackgroundWorkKey, setRequestedBackgroundWorkKey] = useState<BackgroundWorkKey | null>(null)
  const [panelRequestSessionId, setPanelRequestSessionId] = useState<string | null>(null)
  const [panelRequestSignal, setPanelRequestSignal] = useState(0)
  const [commitDialogOpen, setCommitDialogOpen] = useState(false)
  const commitDialogReturnFocus = useRef<HTMLElement | null>(null)
  const [projectPickerOpen, setProjectPickerOpen] = useState(false)
  const projectPickerReturnFocus = useRef<HTMLElement | null>(null)
  const [projectlessPending, setProjectlessPending] = useState(false)
  const projectlessRequest = useRef(false)
  const [onboardingOpen, setOnboardingOpen] = useState(() => {
    try { return !localStorage.getItem('padu.has-completed-onboarding') } catch { return false }
  })
  const composerDraftState = useRef<ComposerDrafts>({})
  const composerDraftTimers = useRef(new Map<string, number>())
  const composerDraftWriteQueue = useRef<Promise<void>>(Promise.resolve())
  const [composerDraftAddress, setComposerDraftAddress] = useState<string | null>(null)
  const rememberedNavigation = useRef(config
    ? readRememberedNavigation(browserNavigationStorage(), config.address)
    : null)
  const previousRouteSession = useRef(search.session)
  const pendingPaletteFocusSession = useRef<string | null>(null)
  const presentedWorkspaceFor = useRef<string | null>(null)
  const enteringNewTask = useRef(false)
  const [newTaskMode, setNewTaskMode] = useState(
    () => !search.session && rememberedNavigation.current?.kind === 'newTask',
  )
  const [draft, setDraft] = useState<AgentSession | null>(null)
  const [draftProject, setDraftProject] = useState<Project | null>(null)
  const hydratedSelected = selected.data?.id === search.session ? selected.data : null
  const current = newTaskMode ? null : hydratedSelected ?? displayed
  useEffect(() => {
    if (!client || typeof window === 'undefined') return
    const pending = window.sessionStorage.getItem('padu.pending-composer-note')
    if (!pending) return
    const [targetSession, projectId, noteId] = pending.split(':')
    if (targetSession !== (current?.id ?? 'new') || !projectId || !noteId) return
    let cancelled = false
    void getNote(client, projectId, noteId).then((note) => {
      if (cancelled || !note) return
      setComposerEmbeddedNote({
        sessionId: targetSession,
        note: { id: note.id, title: note.title, content: note.content, revision: note.revision },
      })
      window.sessionStorage.removeItem('padu.pending-composer-note')
    }).catch((error) => toast.error(error instanceof Error ? error.message : 'Could not load note'))
    return () => { cancelled = true }
  }, [client, current?.id])
  useDocumentTitle(newTaskMode ? t('menu.new_task') : current ? displayTitle(current) : null)
  const currentProject = current
    ? taskState.data?.projects.find((project) => project.id === current.project_id)
    : undefined
  const currentCwd = current && currentProject
    ? sessionCwd(current, currentProject)
    : undefined
  const composerFiles = useComposerFiles(currentCwd)
  const sessionTurnRefs = useSessionTurnRefs(currentCwd, current?.id)
  const selectedReady = current?.id === search.session
  const activeSession = current ?? draft
  const activeProject = currentProject ?? draftProject ?? undefined
  const activePanelSessionId = activeSession?.id ?? null
  const rightPanelVisible = activePanelSessionId
    ? rightPanelOpenBySession[activePanelSessionId] ?? false
    : false
  const panelFullscreen = activePanelSessionId
    ? panelFullscreenBySession[activePanelSessionId] ?? { expanded: false, conversation: false }
    : { expanded: false, conversation: false }
  const panelExpandable = activePanelSessionId
    ? panelExpandableBySession[activePanelSessionId] ?? false
    : false
  // Fullscreen surface mode hides the transcript column (kept mounted, so
  // composer drafts and scroll survive); conversation mode keeps it and
  // floats just the tab strip above it.
  const panelTakesCenter = rightPanelVisible
    && panelFullscreen.expanded
    && panelExpandable
    && !panelFullscreen.conversation
  const choosingInitialDestination = Boolean(
    taskState.data && !search.session && !newTaskMode,
  )
  const restoringNewTask = Boolean(
    taskState.data
      && newTaskMode
      && taskState.data.projects.length
      && (!draft || !draftProject),
  )
  const hydratingInitialSession = Boolean(
    search.session
      && !newTaskMode
      && !displayed
      && !hydratedSelected
      && selected.isPending,
  )
  const hasPresentedWorkspace = Boolean(
    config && presentedWorkspaceFor.current === config.address,
  )
  const showingInitialDestination = shouldShowInitialDestination(
    hasPresentedWorkspace,
    {
      choosing: choosingInitialDestination,
      restoringNewTask,
      hydratingSession: hydratingInitialSession,
    },
  )

  useEffect(() => {
    if (!client || !config || !taskState.data) return
    let cancelled = false
    const candidates = new Set(
      taskState.data.sessions
        .filter((session) => ['connecting', 'working', 'waiting'].includes(session.status))
        .map((session) => session.id),
    )
    if (current?.id) candidates.add(current.id)

    for (const sessionId of candidates) {
      if (runtimes[sessionId]) continue
      const loaded = current?.id === sessionId
        ? Promise.resolve(current)
        : queryClient.fetchQuery({
            queryKey: daemonKeys.session(config.address, sessionId),
            queryFn: () => hydrateSession(client, sessionId),
            staleTime: 1_000,
          })
      void loaded
        .then((session) => {
          if (!cancelled && session) return attachSession(session)
        })
        .catch(() => {
          // Connection state owns visible errors. A catalog revision or the
          // next selected-session hydration will retry an attachment race.
        })
    }
    return () => {
      cancelled = true
    }
  }, [attachSession, client, config, current, queryClient, runtimes, taskState.data])

  useEffect(() => {
    if (!config || !current || !taskState.data) return
    const summary = taskState.data.sessions.find((session) => session.id === current.id)
    if (!summary) return
    queryClient.setQueryData<AgentSession>(
      daemonKeys.session(config.address, current.id),
      (session) => {
        if (!session) return session
        const status = runtimes[current.id] ? session.status : summary.status
        if (
          session.title === summary.title
          && session.auto_title === summary.auto_title
          && session.model === summary.model
          && session.status === status
          && session.last_reply_at === summary.last_reply_at
        ) return session
        return {
          ...session,
          title: summary.title,
          auto_title: summary.auto_title,
          model: summary.model,
          status,
          last_reply_at: summary.last_reply_at,
          updated_at: Math.max(session.updated_at, summary.updated_at),
        }
      },
    )
  }, [config, current, queryClient, runtimes, taskState.data])

  useEffect(() => {
    const preload = () => {
      void import('@/components/code-surfaces')
        .then((module) => module.preloadCodeSurfaces())
        .catch(() => {
          // Code surfaces retain their own visible error handling if an
          // optional syntax-highlighting chunk cannot be warmed in advance.
        })
    }
    if (typeof window.requestIdleCallback === 'function') {
      const idle = window.requestIdleCallback(preload, { timeout: 2_000 })
      return () => window.cancelIdleCallback(idle)
    }
    const timer = globalThis.setTimeout(preload, 750)
    return () => globalThis.clearTimeout(timer)
  }, [])

  useEffect(() => {
    if (!config || !taskState.data || composerDraftAddress !== config.address) return
    if (showingInitialDestination) return
    presentedWorkspaceFor.current = config.address
  }, [composerDraftAddress, config, showingInitialDestination, taskState.data])

  useEffect(() => {
    const transition = routeDestinationTransition(
      previousRouteSession.current,
      search.session,
      newTaskMode,
    )
    previousRouteSession.current = search.session
    if (transition === 'session') {
      enteringNewTask.current = false
      return
    }
    if (transition !== 'newTask') return

    enteringNewTask.current = true
    const project = currentProject ?? draftProject ?? taskState.data?.projects[0]
    setDisplayed(null)
    setNewTaskMode(true)
    setDraftProject(project ?? null)
    setDraft(project ? createRememberedSession(project.id) : null)
    rememberNavigation({ kind: 'newTask', projectId: project?.id })
  }, [search.session])

  useEffect(() => {
    if (!search.session && newTaskMode) enteringNewTask.current = false
  }, [newTaskMode, search.session])

  useEffect(() => {
    // Replay-onboarding flag set by the settings page before navigating home.
    try {
      if (localStorage.getItem('padu.open-onboarding') === '1') {
        localStorage.removeItem('padu.open-onboarding')
        localStorage.removeItem('padu.has-completed-onboarding')
        setOnboardingOpen(true)
      }
    } catch { /* noop */ }
    const handler = (event: StorageEvent) => {
      if (event.key === 'padu.open-onboarding' && event.newValue === '1') {
        try { localStorage.removeItem('padu.open-onboarding') } catch { /* noop */ }
        setOnboardingOpen(true)
      }
    }
    window.addEventListener('storage', handler)
    return () => window.removeEventListener('storage', handler)
  }, [])

  useEffect(() => {
    const hydrated = selected.data
    if (search.session && hydrated?.id === search.session) {
      setDisplayed(hydrated)
      // A click from New Task keeps that complete surface mounted while the
      // transcript is fetched. Commit the activation only when hydration has
      // succeeded, just like the desktop pending-session transition.
      if (newTaskMode && !enteringNewTask.current) {
        setNewTaskMode(false)
        setDraft(null)
        setDraftProject(null)
      }
    }
    else if (selected.isSuccess) setDisplayed(null)
  }, [newTaskMode, search.session, selected.data, selected.isSuccess])

  useEffect(() => {
    if (!current || pendingPaletteFocusSession.current !== current.id) return
    pendingPaletteFocusSession.current = null
    setFocusComposerSignal((value) => value + 1)
  }, [current?.id])

  const daemonAddress = config?.address
  useEffect(() => {
    if (!daemonAddress || !loadedComposerDrafts.data) return
    for (const timer of new Set(composerDraftTimers.current.values())) {
      window.clearTimeout(timer)
    }
    composerDraftTimers.current.clear()
    composerDraftWriteQueue.current = Promise.resolve()
    composerDraftState.current = {
      new_sessions: { ...loadedComposerDrafts.data.new_sessions },
      sessions: { ...loadedComposerDrafts.data.sessions },
    }
    setComposerDraftAddress(daemonAddress)
  }, [daemonAddress, loadedComposerDrafts.data])

  useEffect(() => () => {
    for (const timer of new Set(composerDraftTimers.current.values())) {
      window.clearTimeout(timer)
    }
    composerDraftTimers.current.clear()
  }, [])

  useEffect(() => {
    window.localStorage.setItem('padu.sidebarVisible', String(sidebarVisible))
  }, [sidebarVisible])

  useEffect(() => {
    if (sidebarWidth === DEFAULT_SIDEBAR_WIDTH) return
    const timer = window.setTimeout(() => {
      window.localStorage.setItem('padu.sidebarWidth', String(Math.round(sidebarWidth)))
    }, 150)
    return () => window.clearTimeout(timer)
  }, [sidebarWidth])

  useEffect(() => {
    if (rightPanelWidth === DEFAULT_RIGHT_PANEL_WIDTH) return
    const timer = window.setTimeout(() => {
      window.localStorage.setItem('padu.rightPanelWidth', String(Math.round(rightPanelWidth)))
    }, 150)
    return () => window.clearTimeout(timer)
  }, [rightPanelWidth])

  useEffect(() => {
    if (!activeSession) return
    if (!retainedPanelSessionIds.includes(activeSession.id)) return
    retainedPanelSessions.current.set(activeSession.id, {
      session: activeSession,
      project: activeProject,
    })
  }, [activeProject, activeSession, retainedPanelSessionIds])

  useEffect(() => {
    if (!taskState.data) return
    if (search.session) return
    if (enteringNewTask.current && !newTaskMode) return
    if (newTaskMode) {
      const rememberedProjectId = rememberedNavigation.current?.kind === 'newTask'
        ? rememberedNavigation.current.projectId
        : undefined
      const project = taskState.data.projects.find((item) => item.id === rememberedProjectId)
        ?? taskState.data.projects[0]
      if (!project) {
        setDraft(null)
        setDraftProject(null)
        return
      }
      setDraft((currentDraft) => currentDraft?.project_id === project.id
        ? currentDraft
        : createRememberedSession(project.id))
      setDraftProject(project)
      return
    }
    const remembered = rememberedNavigation.current
    const rememberedSession = remembered?.kind === 'session'
      ? taskState.data.sessions.find((session) => session.id === remembered.sessionId)
      : undefined
    const newest = rememberedSession && !rememberedSession.archived_at
      ? rememberedSession
      : [...taskState.data.sessions]
      .filter((session) => !session.archived_at && (session.last_reply_at || session.turns.length || session.messages.length))
      .sort((a, b) => (b.last_reply_at ?? b.created_at) - (a.last_reply_at ?? a.created_at))[0]
    if (newest) void navigate({ search: { session: newest.id }, replace: true })
    else startNewTask(taskState.data.projects[0])
  }, [taskState.data, search.session, newTaskMode, navigate])

  useEffect(() => {
    if (!config || !search.session) return
    rememberNavigation({
      kind: 'session',
      sessionId: search.session,
    })
  }, [config, search.session])

  useEffect(() => {
    const keyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return
      const key = event.key.toLowerCase()
      if (key === 'p') {
        event.preventDefault()
        setPaletteInitialView('findFile')
        setPaletteOpen(true)
        return
      }
      if (key === 'k') {
        event.preventDefault()
        if (paletteOpen) setPaletteOpen(false)
        else openCommandPalette('commands')
        return
      }
      if (key === ',') {
        event.preventDefault()
        openSettings('general')
        return
      }
      if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
        event.preventDefault()
        selectAdjacentSession(event.key === 'ArrowDown' ? 1 : -1)
        return
      }
      if (event.altKey && (event.key === 'ArrowRight' || event.key === 'ArrowLeft')) {
        if (activePanelSessionId && rightPanelVisible) {
          event.preventDefault()
          rightPanelRefs.current[activePanelSessionId]?.cycleTabs(event.key === 'ArrowRight' ? 1 : -1)
          return
        }
      }
      if (event.shiftKey && (event.key === '[' || event.key === ']')) {
        event.preventDefault()
        selectAdjacentSession(event.key === ']' ? 1 : -1)
        return
      }
      if (key === 'b' && event.shiftKey) {
        event.preventDefault()
        toggleRightPanel()
        return
      }
      if (key === 't') {
        event.preventDefault()
        if (rightPanelVisible && requestedPanel === 'terminal') {
          if (panelFullscreen.expanded) {
            if (activeSession) {
              setPanelFullscreenForSession(activeSession.id, {
                conversation: !panelFullscreen.conversation,
              })
            }
          } else {
            toggleRightPanel()
          }
        } else {
          if (panelFullscreen.expanded && activeSession) {
            setPanelFullscreenForSession(activeSession.id, { conversation: false })
          }
          openPanel('terminal')
        }
        return
      }
      if (key === 'e' && event.shiftKey) {
        if (!activeProject || isProjectlessProject(activeProject)) return
        event.preventDefault()
        if (rightPanelVisible && requestedPanel === 'files') {
          if (panelFullscreen.expanded) {
            if (activeSession) {
              setPanelFullscreenForSession(activeSession.id, {
                conversation: !panelFullscreen.conversation,
              })
            }
          } else {
            toggleRightPanel()
          }
        } else {
          if (panelFullscreen.expanded && activeSession) {
            setPanelFullscreenForSession(activeSession.id, { conversation: false })
          }
          openPanel('files')
        }
        return
      }
      if (key === 'd') {
        if (!activeProject || isProjectlessProject(activeProject)) return
        event.preventDefault()
        if (rightPanelVisible && requestedPanel === 'changes') {
          if (panelFullscreen.expanded) {
            if (activeSession) {
              setPanelFullscreenForSession(activeSession.id, {
                conversation: !panelFullscreen.conversation,
              })
            }
          } else {
            toggleRightPanel()
          }
        } else {
          if (panelFullscreen.expanded && activeSession) {
            setPanelFullscreenForSession(activeSession.id, { conversation: false })
          }
          openPanel('changes')
        }
        return
      }
      if (key === 'b') {
        event.preventDefault()
        setSidebarVisible((value) => !value)
        return
      }
      if (!taskState.data) return
      if (key === 'n' && event.shiftKey) {
        event.preventDefault()
        void navigate({ to: '/notes', search: { q: undefined, noteId: undefined, projectId: activeProject?.id } })
      } else if (key === 'n') {
        event.preventDefault()
        startNewTask()
      } else if (key === 'o') {
        event.preventDefault()
        openProjectPicker()
      } else if (key === 'l') {
        event.preventDefault()
        setFocusComposerSignal((value) => value + 1)
      } else if (key === '/') {
        event.preventDefault()
        setModelPickerSignal((value) => value + 1)
      } else if (key === 'u') {
        event.preventDefault()
        setUsagePanelSignal((value) => value + 1)
      }
    }
    window.addEventListener('keydown', keyDown)
    return () => window.removeEventListener('keydown', keyDown)
  })

  const composerDraftsReady = Boolean(config && composerDraftAddress === config.address)
  if (!taskState.data || !composerDraftsReady) {
    const startupError = taskState.error ?? loadedComposerDrafts.error
    return <StartupScreen error={startupError ? errorMessage(startupError) : undefined} onRetry={() => {
      void taskState.refetch()
      void loadedComposerDrafts.refetch()
    }} />
  }

  if (showingInitialDestination) {
    return <StartupScreen />
  }

  if (search.session && !newTaskMode && !displayed && !hydratedSelected && selected.error) {
    return <StartupScreen error={errorMessage(selected.error)} onRetry={() => void selected.refetch()} />
  }

  function openProjectPicker() {
    projectPickerReturnFocus.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null
    setProjectPickerOpen(true)
  }

  function openCommandPalette(view: CommandPaletteView = 'commands', query = '') {
    setPaletteInitialView(view)
    setPaletteInitialQuery(query)
    setPaletteOpen(true)
  }

  function embedNoteIntoComposer(note: NoteSummary) {
    if (!client) return
    const targetSession = current?.id ?? 'new'
    void getNote(client, note.projectId, note.id)
      .then((selected) => {
        if (!selected) return
        setComposerEmbeddedNote({
          sessionId: targetSession,
          note: {
            id: selected.id,
            title: selected.title,
            content: selected.content,
            revision: selected.revision,
          },
        })
      })
      .catch((error) => toast.error(error instanceof Error ? error.message : 'Could not load note'))
  }

  function rememberNavigation(navigation: RememberedNavigation) {
    if (!config) return
    rememberedNavigation.current = navigation
    writeRememberedNavigation(browserNavigationStorage(), config.address, navigation)
  }

  function openSettings(page: SettingsPageId) {
    void navigate({
      to: '/settings/$page',
      params: { page },
      search: { session: search.session },
    })
  }

  function showSidebar() {
    setSidebarVisible(true)
    if (window.matchMedia('(max-width: 1023px)').matches) setMobileSidebar(true)
  }

  function hideSidebar() {
    if (window.matchMedia('(max-width: 1023px)').matches) setMobileSidebar(false)
    else setSidebarVisible(false)
  }

  function openPanel(
    surface: PanelSurface,
    source: ReviewDiffSource = 'uncommitted',
    file: string | null = null,
  ) {
    if (!activeSession) return
    retainPanelSession(activeSession, activeProject)
    setRequestedPanel(surface)
    setRequestedFile(surface === 'files' ? file : null)
    if (surface === 'changes') setRequestedDiffSource(source)
    setPanelRequestSessionId(activeSession.id)
    setPanelRequestSignal((value) => value + 1)
    setRightPanelForSession(activeSession.id, true)
  }

  function openTranscriptLink(target: string) {
    const workspace = activeSession && activeProject
      ? sessionCwd(activeSession, activeProject)
      : undefined
    const route = transcriptLinkRoute(target, workspace)
    if (route.kind === 'external') return false
    if (route.kind === 'remoteFile') {
      toast.error(t('errors.path_outside_workspace'))
      return true
    }
    openPanel('files', 'uncommitted', route.path)
    return true
  }

  function openBackgroundWork(key: BackgroundWorkKey) {
    if (!activeSession) return
    retainPanelSession(activeSession, activeProject)
    setRequestedBackgroundWorkKey(key)
    setRequestedPanel('backgroundWork')
    setPanelRequestSessionId(activeSession.id)
    setPanelRequestSignal((value) => value + 1)
    setRightPanelForSession(activeSession.id, true)
  }

  function retainPanelSession(session: AgentSession, project?: Project) {
    retainedPanelSessions.current.set(session.id, { session, project })
    setRetainedPanelSessionIds((currentIds) => currentIds.includes(session.id)
      ? currentIds
      : [...currentIds, session.id])
  }

  function setRightPanelForSession(
    sessionId: string,
    next: boolean | ((current: boolean) => boolean),
  ) {
    setRightPanelOpenBySession((current) => {
      const previous = current[sessionId] ?? false
      const value = typeof next === 'function' ? next(previous) : next
      return value === previous ? current : { ...current, [sessionId]: value }
    })
  }

  function setActiveRightPanel(next: boolean | ((current: boolean) => boolean)) {
    if (!activeSession) return
    retainPanelSession(activeSession, activeProject)
    setRightPanelForSession(activeSession.id, next)
  }

  function toggleRightPanel() {
    setActiveRightPanel((current) => !current)
  }

  function setPanelFullscreenForSession(
    sessionId: string,
    next: Partial<{ expanded: boolean, conversation: boolean }>,
  ) {
    setPanelFullscreenBySession((current) => {
      const previous = current[sessionId] ?? { expanded: false, conversation: false }
      const value = { ...previous, ...next }
      if (value.expanded === previous.expanded && value.conversation === previous.conversation) {
        return current
      }
      return { ...current, [sessionId]: value }
    })
  }

  function togglePanelFullscreen() {
    if (!activeSession || !panelExpandable) return
    retainPanelSession(activeSession, activeProject)
    setRightPanelForSession(activeSession.id, true)
    const next = !panelFullscreen.expanded
    setPanelFullscreenForSession(activeSession.id, { expanded: next, conversation: false })
  }

  function forgetRightPanelSession(sessionId: string) {
    retainedPanelSessions.current.delete(sessionId)
    setRetainedPanelSessionIds((current) => current.filter((id) => id !== sessionId))
    setRightPanelOpenBySession((current) => {
      if (!(sessionId in current)) return current
      const next = { ...current }
      delete next[sessionId]
      return next
    })
    setPanelFullscreenBySession((current) => {
      if (!(sessionId in current)) return current
      const next = { ...current }
      delete next[sessionId]
      return next
    })
    setPanelExpandableBySession((current) => {
      if (!(sessionId in current)) return current
      const next = { ...current }
      delete next[sessionId]
      return next
    })
    setPanelTabsBySession((current) => {
      if (!(sessionId in current)) return current
      const next = { ...current }
      delete next[sessionId]
      return next
    })
    delete rightPanelRefs.current[sessionId]
    setPanelRequestSessionId((current) => current === sessionId ? null : current)
  }

  function startNewTask(preferred?: Project | null) {
    const project = preferred === null
      ? undefined
      : preferred ?? currentProject ?? draftProject ?? taskState.data?.projects[0]
    if (newTaskMode && draft) forgetRightPanelSession(draft.id)
    const nextDraft = project
      ? createRememberedSession(project.id)
      : null
    enteringNewTask.current = true
    pendingPaletteFocusSession.current = null
    setFocusComposerSignal((value) => value + 1)
    setNewTaskMode(true)
    setDisplayed(null)
    if (project) {
      setDraftProject(project)
      setDraft(nextDraft)
    } else {
      setDraft(null)
      setDraftProject(null)
    }
    rememberNavigation({ kind: 'newTask', projectId: project?.id })
    void navigate({ search: { session: undefined }, replace: false })
  }

  function selectSession(sessionId: string) {
    if (sessionId === search.session && !newTaskMode) return
    if (newTaskMode && draft && draft.id !== sessionId) forgetRightPanelSession(draft.id)
    enteringNewTask.current = false
    if (!newTaskMode) {
      setDraft(null)
      setDraftProject(null)
    }
    rememberNavigation({ kind: 'session', sessionId })
    void navigate({ search: { session: sessionId } })
  }

  function selectAdjacentSession(delta: number) {
    if (!taskState.data) return
    const started = sortSidebarSessions(
      taskState.data.sessions.filter((session) => !session.archived_at && sessionHasStarted(session)),
      'newest',
    )
    if (!started.length) return
    const currentId = search.session
    const currentIndex = currentId ? started.findIndex((s) => s.id === currentId) : -1
    let nextIndex: number
    if (currentIndex >= 0) {
      nextIndex = delta > 0
        ? Math.min(currentIndex + 1, started.length - 1)
        : Math.max(currentIndex - 1, 0)
    } else {
      nextIndex = delta > 0 ? 0 : started.length - 1
    }
    const nextSession = started[nextIndex]
    if (nextSession) {
      selectSession(nextSession.id)
    }
  }

  async function resumeProviderSession(summary: ProviderSessionSummary) {
    if (!client || !config || !taskState.data) {
      throw new Error(t('errors.daemon_disconnected'))
    }
    const existing = taskState.data.sessions.find((session) =>
      session.provider_cursor
        ? sameProviderSession(session.provider_cursor, summary.cursor)
        : false)
    if (existing) {
      pendingPaletteFocusSession.current = existing.id
      selectSession(existing.id)
      return
    }

    const history = await loadProviderSessionHistory(client, summary)
    const existingProject = taskState.data.projects.find((project) => project.path === summary.cwd)
    const project = existingProject ?? createProject(summary.cwd)
    const session = createResumedSession(
      project.id,
      summary,
      history,
      activeSession?.runtime_mode ?? 'fullAccess',
    )
    const saved = await saveSession(session, existingProject ? undefined : project)
    pendingPaletteFocusSession.current = saved.id
    selectSession(saved.id)
  }

  async function forkResponse(session: AgentSession, turnCount: number) {
    try {
      const result = await forkSessionFromResponse(session, turnCount)
      selectSession(result.session.id)
      if (result.checkpointWarning) {
        toast.warning(t('session.forked_with_checkpoint_warning', {
          error: result.checkpointWarning,
        }))
      } else {
        toast.success(t('session.forked_from_response'))
      }
    } catch (error) {
      toast.error(t('errors.fork_task', { error: errorMessage(error) }))
    }
  }

  async function rewindMessage(
    session: AgentSession,
    turnCount: number,
    prompt: string,
    attachments: MessageAttachment[],
  ) {
    try {
      const result = await rewindSessionToMessage(
        session,
        turnCount,
        prompt,
        attachments,
      )
      if (result.cleanupWarning) {
        toast.warning(t('session.rewound_with_stale_refs', {
          turn: turnCount,
          error: result.cleanupWarning,
        }))
      } else {
        toast(t('session.rewound', { turn: turnCount }))
      }
    } catch (error) {
      toast.error(t('errors.rewind_task', { error: errorMessage(error) }))
      throw error
    }
  }

  async function renameSession(sessionId: string, title: string) {
    if (!client || !config) throw new Error(t('errors.daemon_disconnected'))
    const stateKey = daemonKeys.taskState(config.address)
    const sessionKey = daemonKeys.session(config.address, sessionId)
    const previousState = queryClient.getQueryData<TaskState>(stateKey)
    const previousSummary = previousState?.sessions.find((session) => session.id === sessionId)
    const previousHydrated = queryClient.getQueryData<AgentSession>(sessionKey)
    const updatedAt = Math.floor(Date.now() / 1_000)
    queryClient.setQueryData<TaskState>(stateKey, (currentState) => currentState && ({
      ...currentState,
      sessions: currentState.sessions.map((session) => session.id === sessionId
        ? { ...session, title, updated_at: updatedAt }
        : session),
    }))
    queryClient.setQueryData<AgentSession>(sessionKey, (session) => session
      ? { ...session, title, updated_at: updatedAt }
      : session)
    try {
      const hydrated = await hydrateSession(client, sessionId)
      if (!hydrated) throw new Error(t('errors.task_not_found'))
      await saveSession({ ...hydrated, title, updated_at: updatedAt })
    } catch (error) {
      if (previousSummary) {
        queryClient.setQueryData<TaskState>(stateKey, (currentState) => currentState && ({
          ...currentState,
          sessions: currentState.sessions.map((session) => session.id === sessionId
            ? {
                ...session,
                title: previousSummary.title,
                updated_at: previousSummary.updated_at,
              }
            : session),
        }))
      }
      if (previousHydrated) {
        queryClient.setQueryData<AgentSession>(sessionKey, (session) => session && ({
          ...session,
          title: previousHydrated.title,
          updated_at: previousHydrated.updated_at,
        }))
      }
      toast.error(t('errors.rename_task', { error: errorMessage(error) }))
      throw error
    }
  }

  async function updateSessionFlag(
    sessionId: string,
    flag: 'pinned_at' | 'archived_at',
    enabled: boolean,
  ) {
    if (!client || !config) throw new Error(t('errors.daemon_disconnected'))
    const stateKey = daemonKeys.taskState(config.address)
    const sessionKey = daemonKeys.session(config.address, sessionId)
    const previousState = queryClient.getQueryData<TaskState>(stateKey)
    const previousSession = queryClient.getQueryData<AgentSession>(sessionKey)
    const timestamp = enabled ? Math.floor(Date.now() / 1_000) : null
    const update = <T extends { id: string } & Partial<Record<'pinned_at' | 'archived_at', number | null>>>(value: T) => ({
      ...value,
      [flag]: timestamp,
      ...(flag === 'archived_at' && enabled ? { pinned_at: null } : {}),
    }) as T
    queryClient.setQueryData<TaskState>(stateKey, (currentState) => currentState && ({
      ...currentState,
      sessions: currentState.sessions.map((session) => session.id === sessionId ? update(session) : session),
    }))
    queryClient.setQueryData<AgentSession>(sessionKey, (session) => session ? update(session) : session)
    try {
      const updated = flag === 'pinned_at'
        ? await setSessionPinned(client, sessionId, enabled)
        : await setSessionArchived(client, sessionId, enabled)
      queryClient.setQueryData<TaskState>(stateKey, (currentState) => currentState && ({
        ...currentState,
        sessions: currentState.sessions.map((session) => session.id === sessionId
          ? { ...session, pinned_at: updated.pinned_at, archived_at: updated.archived_at }
          : session),
      }))
      queryClient.setQueryData<AgentSession>(sessionKey, (session) => session
        ? { ...session, pinned_at: updated.pinned_at, archived_at: updated.archived_at }
        : session)
      if (flag === 'archived_at' && enabled && search.session === sessionId) startNewTask(null)
    } catch (error) {
      if (previousState) queryClient.setQueryData(stateKey, previousState)
      if (previousSession) queryClient.setQueryData(sessionKey, previousSession)
      toast.error(t('errors.update_task', { error: errorMessage(error) }))
      throw error
    }
  }

  async function removeSessionById(sessionId: string) {
    if (!client || !config) throw new Error(t('errors.daemon_disconnected'))
    const stateKey = daemonKeys.taskState(config.address)
    const previousState = queryClient.getQueryData<TaskState>(stateKey)
    const removedIndex = previousState?.sessions.findIndex((session) => session.id === sessionId) ?? -1
    const removed = removedIndex >= 0 ? previousState?.sessions[removedIndex] : undefined
    queryClient.setQueryData<TaskState>(stateKey, (currentState) => currentState && ({
      ...currentState,
      sessions: currentState.sessions.filter((session) => session.id !== sessionId),
    }))
    try {
      await closeSession(sessionId)
      const next = await removeSession(client, sessionId)
      queryClient.setQueryData(stateKey, next)
      queryClient.removeQueries({ queryKey: daemonKeys.session(config.address, sessionId) })
      removeStoredComposerDraft({ type: 'session', sessionId })
      forgetRightPanelSession(sessionId)
      if (search.session === sessionId && removed && previousState) {
        const destination = taskRemovalDestination(
          previousState.projects,
          next.projects,
          next.sessions,
          removed,
        )
        if (destination.kind === 'session') {
          selectSession(destination.sessionId)
        } else if (destination.kind === 'newTask') {
          startNewTask(destination.project)
        } else if (destination.kind === 'projectless') {
          startNewTask(null)
          await createProjectlessTask()
        } else {
          startNewTask(null)
        }
      } else if (search.session === sessionId) {
        startNewTask(null)
      }
    } catch (error) {
      if (removed) {
        queryClient.setQueryData<TaskState>(stateKey, (currentState) => {
          if (!currentState || currentState.sessions.some((session) => session.id === sessionId)) {
            return currentState
          }
          const sessions = [...currentState.sessions]
          sessions.splice(Math.min(removedIndex, sessions.length), 0, removed)
          return { ...currentState, sessions }
        })
      }
      toast.error(t('errors.remove_task', { error: errorMessage(error) }))
      throw error
    }
  }

  function chooseProject(project: Project) {
    if (draftProject && draftProject.id !== project.id) {
      moveStoredComposerDraft(
        { type: 'newSession', projectId: draftProject.id },
        { type: 'newSession', projectId: project.id },
      )
    }
    enteringNewTask.current = true
    setDisplayed(null)
    setDraftProject(project)
    setDraft((currentDraft) => currentDraft
      ? { ...currentDraft, project_id: project.id, workspace: { kind: 'local' } }
      : createRememberedSession(project.id))
    setNewTaskMode(true)
    rememberNavigation({ kind: 'newTask', projectId: project.id })
    void navigate({ search: { session: undefined }, replace: false })
  }

  async function persistAndChooseProject(project: Project): Promise<boolean> {
    if (!client || !config) return false
    try {
      const saved = await persistProject(client, project)
      queryClient.setQueryData(daemonKeys.taskState(config.address), saved.taskState)
      chooseProject(saved.project)
      return true
    } catch (error) {
      toast.error(errorMessage(error))
      return false
    }
  }

  async function addRemoteProject(path: string): Promise<boolean> {
    try {
      return await persistAndChooseProject(createProject(path))
    } catch (error) {
      toast.error(errorMessage(error))
      return false
    }
  }

  async function createProjectlessTask() {
    if (!client || projectlessRequest.current) return
    projectlessRequest.current = true
    setProjectlessPending(true)
    try {
      const path = await createProjectlessWorkspace(client)
      await persistAndChooseProject({ ...createProject(path), name: 'No project' })
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      projectlessRequest.current = false
      setProjectlessPending(false)
    }
  }

  function createRememberedSession(projectId: string): AgentSession {
    const preferences = readComposerPreferences(
      browserComposerPreferenceStorage(),
      config?.address ?? 'disconnected',
    )
    return {
      ...createSession(projectId, preferences.lastProvider, false),
      model: preferences.lastModel,
      reasoning_effort: preferences.lastReasoningEffort,
      service_tier: preferences.lastServiceTier,
      context_window: preferences.lastContextWindow,
    }
  }

  function updateStoredComposerDraft(
    target: ComposerDraftTarget,
    draft: ComposerDraft,
  ) {
    const change = setComposerDraft(composerDraftState.current, target, draft)
    if (!change) return
    const id = composerDraftId(target)
    const pending = composerDraftTimers.current.get(id)
    if (pending !== undefined) window.clearTimeout(pending)
    const timer = window.setTimeout(() => {
      if (composerDraftTimers.current.get(id) === timer) {
        composerDraftTimers.current.delete(id)
      }
      enqueueComposerDraftChanges([change])
    }, 250)
    composerDraftTimers.current.set(id, timer)
  }

  function removeStoredComposerDraft(target: ComposerDraftTarget) {
    const change = setComposerDraft(
      composerDraftState.current,
      target,
      { text: '', attachments: [] },
    )
    if (!change) return
    cancelComposerDraftTimer(target)
    enqueueComposerDraftChanges([change])
  }

  function moveStoredComposerDraft(
    source: ComposerDraftTarget,
    destination: ComposerDraftTarget,
  ) {
    const changes = moveComposerDraftToEmpty(composerDraftState.current, source, destination)
    if (!changes.length) return
    cancelComposerDraftTimer(source)
    cancelComposerDraftTimer(destination)
    enqueueComposerDraftChanges(changes)
  }

  function cancelComposerDraftTimer(target: ComposerDraftTarget) {
    const id = composerDraftId(target)
    const timer = composerDraftTimers.current.get(id)
    if (timer !== undefined) window.clearTimeout(timer)
    composerDraftTimers.current.delete(id)
  }

  function enqueueComposerDraftChanges(changes: ComposerDraftChange[]) {
    if (!client || !changes.length) return
    composerDraftWriteQueue.current = composerDraftWriteQueue.current
      .catch(() => {})
      .then(() => applyComposerDraftChanges(client, changes))
      .catch((error) => {
        toast.error(t('errors.save_draft', { error: errorMessage(error) }))
      })
  }

  const paletteActions: CommandPaletteActions = {
    newTask: () => startNewTask(),
    openProject: openProjectPicker,
    openFile: (path) => openPanel('files', 'uncommitted', path),
    openNotes: (query, noteId) => {
      void navigate({
        to: '/notes',
        search: { q: query || undefined, noteId: noteId || undefined, projectId: activeProject?.id },
      })
    },
    embedNote: embedNoteIntoComposer,
    chooseModel: () => {
      setModelPickerSignal((value) => value + 1)
    },
    focusComposer: () => {
      setFocusComposerSignal((value) => value + 1)
    },
    toggleUsage: () => {
      setUsagePanelSignal((value) => value + 1)
    },
    toggleSidebar: () => setSidebarVisible((value) => !value),
    toggleRightPanel,
    toggleRightPanelFullscreen: togglePanelFullscreen,
    cycleRightPanelTab: (direction) => {
      if (!activePanelSessionId) return
      rightPanelRefs.current[activePanelSessionId]?.cycleTabs(direction)
    },
    openSettings,
    openOnboarding: () => setOnboardingOpen(true),
    selectTask: (sessionId) => {
      if (!newTaskMode && current?.id === sessionId) {
        setFocusComposerSignal((value) => value + 1)
        return
      }
      pendingPaletteFocusSession.current = sessionId
      selectSession(sessionId)
    },
    selectPreviousTask: () => selectAdjacentSession(-1),
    selectNextTask: () => selectAdjacentSession(1),
    resumeProviderSession,
  }

  const palette = (
    <CommandPalette
      actions={paletteActions}
      canChooseModel={Boolean(activeSession && !['connecting', 'working', 'waiting'].includes(activeSession.status))}
      canToggleUsage={Boolean(activeSession)}
      currentProvider={activeSession?.provider ?? 'codex'}
      files={composerFiles.data ?? []}
      filesLoading={composerFiles.isLoading}
      initialQuery={paletteInitialQuery}
      initialView={paletteInitialView}
      open={paletteOpen}
      rightPanelVisible={rightPanelVisible}
      rightPanelExpanded={panelFullscreen.expanded && panelExpandable}
      canExpandRightPanel={panelExpandable}
      selectedSessionId={search.session}
      sidebarVisible={sidebarVisible}
      taskState={taskState.data}
      onOpenChange={setPaletteOpen}
    />
  )

  return (
    <div className="flex h-dvh w-full overflow-hidden bg-background">
      <OnboardingModal
        open={onboardingOpen}
        onOpenChange={(open) => {
          setOnboardingOpen(open)
          if (!open) {
            try { localStorage.setItem('padu.has-completed-onboarding', '1') } catch { /* noop */ }
          }
        }}
      />
      {sidebarVisible && (
        <Sidebar
          mobileOpen={mobileSidebar}
          onAddProject={openProjectPicker}
          onMobileOpenChange={setMobileSidebar}
          onNewTask={(project) => startNewTask(project)}
          onNotes={() => void navigate({ to: '/notes', search: { q: undefined, noteId: undefined, projectId: activeProject?.id } })}
          onRemoveSession={removeSessionById}
          onRenameSession={renameSession}
          onSetSessionArchived={(sessionId, archived) => updateSessionFlag(sessionId, 'archived_at', archived)}
          onSetSessionPinned={(sessionId, pinned) => updateSessionFlag(sessionId, 'pinned_at', pinned)}
          onSearch={() => openCommandPalette('commands')}
          onSelectSession={selectSession}
          onSettings={() => openSettings('general')}
          onUsage={() => openSettings('usage')}
          onToggleSidebar={hideSidebar}
          onWidthChange={setSidebarWidth}
          selectedSessionId={search.session}
          taskState={taskState.data}
          width={sidebarWidth}
        />
      )}

      <main className={panelTakesCenter ? 'hidden' : 'relative flex min-w-0 flex-1 flex-col'}>
        <TaskHeader
          activeTabId={activePanelSessionId ? panelTabsBySession[activePanelSessionId]?.activeId : null}
          fullscreen={panelFullscreen.expanded && panelExpandable}
          onActivateTab={(tabId) => {
            if (!activePanelSessionId) return
            setPanelFullscreenForSession(activePanelSessionId, { conversation: false })
            rightPanelRefs.current[activePanelSessionId]?.activateTab(tabId)
          }}
          onAddSurface={(surface) => {
            if (!activePanelSessionId) return
            setPanelFullscreenForSession(activePanelSessionId, { conversation: false })
            rightPanelRefs.current[activePanelSessionId]?.openSurface(surface)
          }}
          onCloseTab={(tabId) => {
            if (!activePanelSessionId) return
            rightPanelRefs.current[activePanelSessionId]?.closeTab(tabId)
          }}
          onCommit={(returnFocus) => {
            commitDialogReturnFocus.current = returnFocus
            setCommitDialogOpen(true)
          }}
          onCompareBranch={() => openPanel('changes', 'branch')}
          onMenu={showSidebar}
          onOpenBackgroundWork={openBackgroundWork}
          onOpenChanges={() => openPanel('changes', 'uncommitted')}
          onToggleFullscreen={() => {
            if (!activePanelSessionId) return
            setPanelFullscreenForSession(activePanelSessionId, { expanded: false, conversation: false })
          }}
          onTogglePanel={toggleRightPanel}
          panelVisible={rightPanelVisible}
          project={activeProject}
          session={activeSession}
          showingConversation={panelFullscreen.expanded && panelExpandable && panelFullscreen.conversation}
          sidebarVisible={sidebarVisible}
          tabs={activePanelSessionId ? panelTabsBySession[activePanelSessionId]?.tabs ?? [] : []}
          title={newTaskMode ? t('menu.new_task') : current ? displayTitle(current) : 'Padu'}
        />

        {newTaskMode ? (
          activeSession && activeProject ? (
            <>
              <NewTaskCanvas
                project={activeProject}
                projects={taskState.data.projects}
                onAddProject={openProjectPicker}
                onProject={(project) => chooseProject(project)}
                onProjectless={() => void createProjectlessTask()}
              />
              <Composer
                draft
                focusSignal={focusComposerSignal}
                initialComposerDraft={composerDraftFor(composerDraftState.current, {
                  type: 'newSession',
                  projectId: activeProject.id,
                })}
                embeddedNote={composerEmbeddedNote?.sessionId === 'new' ? composerEmbeddedNote.note : undefined}
                key={composerDraftId({
                  type: 'newSession',
                  projectId: activeProject.id,
                })}
                modelPickerSignal={modelPickerSignal}
                attachmentSignal={composerAttachment?.sessionId === activeSession.id ? composerAttachment.signal : 0}
                pendingAttachmentPaths={composerAttachment?.sessionId === activeSession.id ? composerAttachment.paths : undefined}
                onAttachmentSignalHandled={() => setComposerAttachment((value) =>
                  value?.sessionId === activeSession.id ? null : value)}
                mentionSignal={composerMention?.sessionId === activeSession.id ? composerMention : undefined}
                onMentionSignalHandled={() => setComposerMention((value) =>
                  value?.sessionId === activeSession.id ? null : value)}
                onAddProject={openProjectPicker}
                onNoteCommand={(query) => openCommandPalette('notes', query)}
                onFocusSignalHandled={() => setFocusComposerSignal(0)}
                onModelPickerSignalHandled={() => setModelPickerSignal(0)}
                onProjectless={() => void createProjectlessTask()}
                onResume={() => openCommandPalette('resume')}
                onUsagePanelSignalHandled={() => setUsagePanelSignal(0)}
                project={activeProject}
                projects={selectableProjects(taskState.data.projects, activeProject)}
                session={activeSession}
                usagePanelSignal={usagePanelSignal}
                onComposerDraftChange={(nextDraft) => updateStoredComposerDraft(
                  { type: 'newSession', projectId: activeProject.id },
                  nextDraft,
                )}
                onComposerDraftSubmitted={() => {
                  setComposerEmbeddedNote(null)
                  removeStoredComposerDraft({
                    type: 'newSession',
                    projectId: activeProject.id,
                  })
                }}
                onActivated={(session) => {
                  setDisplayed(session)
                  setNewTaskMode(false)
                  setDraft(null)
                  setDraftProject(null)
                  rememberNavigation({ kind: 'session', sessionId: session.id })
                  void navigate({ search: { session: session.id } })
                }}
                onDraftChange={(next) => {
                  if (next.project_id !== activeProject.id) {
                    moveStoredComposerDraft(
                      { type: 'newSession', projectId: activeProject.id },
                      { type: 'newSession', projectId: next.project_id },
                    )
                  }
                  setDraft(next)
                  const project = taskState.data?.projects.find((item) => item.id === next.project_id)
                  if (project) setDraftProject(project)
                }}
              />
            </>
          ) : (
            <NoProjectState
              projectlessPending={projectlessPending}
              onAddProject={openProjectPicker}
              onProjectless={() => void createProjectlessTask()}
            />
          )
        ) : current ? (
          <>
            <Transcript
              backgroundWork={backgroundWork[current.id] ?? []}
              forkingTurnCount={responseForks[current.id]}
              rewindingTurnCount={messageRewinds[current.id]}
              rewindTurnCounts={sessionTurnRefs.data ?? []}
              session={current}
              onAddToNote={(text) => {
                if (!client) {
                  toast.error('The daemon is not connected')
                  return
                }
                void addContentToNewNote(client, activeProject?.id, displayTitle(current), text)
                  .then(() => toast.success('Added assistant response to note'))
                  .catch((error) => toast.error(error instanceof Error ? error.message : 'Could not add response to note'))
              }}
              onCopyToComposer={(text) => setComposerPrefill((previous) => ({
                sessionId: current.id,
                text,
                signal: (previous?.signal ?? 0) + 1,
              }))}
              onOpenBackgroundWork={openBackgroundWork}
              onOpenLink={openTranscriptLink}
              onForkResponse={(turnCount) => void forkResponse(current, turnCount)}
              onRewindMessage={(turnCount, prompt, attachments) =>
                rewindMessage(current, turnCount, prompt, attachments)}
              onReviewChanges={(source) => openPanel('changes', source)}
            />
            {currentProject && (
              <div aria-busy={!selectedReady} className="shrink-0" inert={!selectedReady}>
                <Composer
                  focusSignal={focusComposerSignal}
                  initialComposerDraft={composerDraftFor(composerDraftState.current, {
                    type: 'session',
                    sessionId: current.id,
                  })}
                  embeddedNote={composerEmbeddedNote?.sessionId === current.id ? composerEmbeddedNote.note : undefined}
                  key={composerDraftId({ type: 'session', sessionId: current.id })}
                  modelPickerSignal={modelPickerSignal}
                  prefillSignal={composerPrefill?.sessionId === current.id ? composerPrefill.signal : 0}
                  prefillText={composerPrefill?.sessionId === current.id ? composerPrefill.text : undefined}
                  attachmentSignal={composerAttachment?.sessionId === current.id ? composerAttachment.signal : 0}
                  pendingAttachmentPaths={composerAttachment?.sessionId === current.id ? composerAttachment.paths : undefined}
                  onAttachmentSignalHandled={() => setComposerAttachment((value) =>
                    value?.sessionId === current.id ? null : value)}
                  mentionSignal={composerMention?.sessionId === current.id ? composerMention : undefined}
                  onMentionSignalHandled={() => setComposerMention((value) =>
                    value?.sessionId === current.id ? null : value)}
                  onAddProject={openProjectPicker}
                  onNoteCommand={(query) => openCommandPalette('notes', query)}
                  onFocusSignalHandled={() => setFocusComposerSignal(0)}
                  onModelPickerSignalHandled={() => setModelPickerSignal(0)}
                  onPrefillSignalHandled={() => setComposerPrefill((value) =>
                    value?.sessionId === current.id ? null : value)}
                  onProjectless={() => void createProjectlessTask()}
                  onResume={() => openCommandPalette('resume')}
                  onUsagePanelSignalHandled={() => setUsagePanelSignal(0)}
                  project={currentProject}
                  projects={taskState.data.projects}
                  session={current}
                  usagePanelSignal={usagePanelSignal}
                  onComposerDraftChange={(nextDraft) => updateStoredComposerDraft(
                    { type: 'session', sessionId: current.id },
                    nextDraft,
                  )}
                  onComposerDraftSubmitted={() => {
                    setComposerEmbeddedNote(null)
                    removeStoredComposerDraft({
                      type: 'session',
                      sessionId: current.id,
                    })
                  }}
                />
              </div>
            )}
            {selected.error && selectedReady && (
              <div role="alert" className="absolute left-1/2 top-14 z-20 -translate-x-1/2 rounded-lg bg-destructive px-3 py-2 text-xs text-white shadow-lg">
                {errorMessage(selected.error)}
              </div>
            )}
          </>
        ) : (
          <NoProjectState
            projectlessPending={projectlessPending}
            onAddProject={openProjectPicker}
            onProjectless={() => void createProjectlessTask()}
          />
        )}

        {newTaskMode && search.session && selected.error && (
          <div
            className="absolute left-1/2 top-14 z-20 flex -translate-x-1/2 items-center gap-2 rounded-lg bg-destructive px-3 py-2 text-xs text-white shadow-lg"
            role="alert"
          >
            <span>{errorMessage(selected.error)}</span>
            <button
              className="rounded px-1.5 py-0.5 font-medium hover:bg-white/15 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-white"
              type="button"
              onClick={() => void selected.refetch()}
            >
              {t('common.retry')}
            </button>
          </div>
        )}
      </main>

      {panelSessionIds(activeSession, retainedPanelSessionIds).map((sessionId) => {
        const isActive = activeSession?.id === sessionId
        const retained = retainedPanelSessions.current.get(sessionId)
        const panelSession = isActive ? activeSession : retained?.session
        const panelProject = isActive ? activeProject : retained?.project
        if (!panelSession) return null
        const fullscreen = panelFullscreenBySession[sessionId] ?? {
          expanded: false,
          conversation: false,
        }
        return (
          <RightPanel
            ref={(instance) => {
              rightPanelRefs.current[sessionId] = instance
            }}
            onTabsReport={(tabs, activeId) => {
              setPanelTabsBySession((current) => {
                const existing = current[sessionId]
                if (existing && existing.tabs === tabs && existing.activeId === activeId) return current
                return { ...current, [sessionId]: { tabs, activeId } }
              })
            }}
            active={isActive}
            key={sessionId}
            open={isActive && (rightPanelOpenBySession[sessionId] ?? false)}
            panelWidth={rightPanelWidth}
            project={panelProject}
            requestedDiffSource={requestedDiffSource}
            requestedBackgroundWorkKey={requestedBackgroundWorkKey}
            requestedFile={requestedFile}
            requestedSurface={requestedPanel}
            requestSignal={panelRequestSessionId === sessionId ? panelRequestSignal : 0}
            session={panelSession}
            sidebarVisible={sidebarVisible}
            sidebarWidth={sidebarVisible ? sidebarWidth : 0}
            onToggleSidebar={() => setSidebarVisible((value) => !value)}
            onFindFile={() => openCommandPalette('findFile')}
            expanded={isActive && fullscreen.expanded}
            showConversation={isActive && fullscreen.conversation}
            onExpandedChange={(expanded) => {
              if (expanded) retainPanelSession(panelSession, panelProject)
              setPanelFullscreenForSession(sessionId, expanded
                ? { expanded: true }
                : { expanded: false, conversation: false })
            }}
            onShowConversationChange={(conversation) => {
              setPanelFullscreenForSession(sessionId, { conversation })
            }}
            onExpandableChange={(expandable) => {
              setPanelExpandableBySession((current) => current[sessionId] === expandable
                ? current
                : { ...current, [sessionId]: expandable })
            }}
            onOpenChange={(open) => {
              setRightPanelForSession(sessionId, open)
              if (!open) {
                setPanelFullscreenForSession(sessionId, { expanded: false, conversation: false })
              }
            }}
            onPanelWidthChange={setRightPanelWidth}
            onAddToChat={(name, isDir) => {
              const mention = isDir ? `${name}/` : name
              setComposerMention({
                sessionId: panelSession.id,
                mention,
                signal: Date.now(),
              })
              setFocusComposerSignal((value) => value + 1)
              toast.success(t('files.added_to_chat', { path: name }))
            }}
          />
        )
      })}

      <CommitDialog
        open={commitDialogOpen}
        project={activeProject}
        returnFocus={commitDialogReturnFocus}
        session={activeSession}
        onOpenChange={setCommitDialogOpen}
      />

      {projectPickerOpen && (
        <DaemonFilePicker
          root={null}
          returnFocus={projectPickerReturnFocus}
          selectionMode="directory"
          onClose={() => setProjectPickerOpen(false)}
          onSelect={addRemoteProject}
        />
      )}
      {palette}
    </div>
  )
}

function readSidebarWidth(): number {
  if (typeof window === 'undefined') return DEFAULT_SIDEBAR_WIDTH
  const raw = window.localStorage.getItem('padu.sidebarWidth')
  if (raw === null) return DEFAULT_SIDEBAR_WIDTH
  const stored = Number(raw)
  return Number.isFinite(stored) ? Math.min(420, Math.max(180, stored)) : DEFAULT_SIDEBAR_WIDTH
}

function readSidebarVisible(): boolean {
  if (typeof window === 'undefined') return true
  return window.localStorage.getItem('padu.sidebarVisible') !== 'false'
}

function readRightPanelWidth(): number {
  if (typeof window === 'undefined') return DEFAULT_RIGHT_PANEL_WIDTH
  const raw = window.localStorage.getItem('padu.rightPanelWidth')
  if (raw === null) return DEFAULT_RIGHT_PANEL_WIDTH
  const stored = Number(raw)
  return Number.isFinite(stored) ? Math.min(1_000, Math.max(280, stored)) : DEFAULT_RIGHT_PANEL_WIDTH
}

function panelSessionIds(
  activeSession: AgentSession | null,
  retainedSessionIds: string[],
): string[] {
  if (!activeSession || retainedSessionIds.includes(activeSession.id)) return retainedSessionIds
  return [...retainedSessionIds, activeSession.id]
}

function TaskHeader({
  title,
  session,
  project,
  sidebarVisible,
  panelVisible = false,
  fullscreen = false,
  showingConversation = false,
  tabs = [],
  activeTabId,
  onMenu,
  onOpenChanges,
  onCommit,
  onCompareBranch,
  onOpenBackgroundWork,
  onTogglePanel,
  onActivateTab,
  onCloseTab,
  onAddSurface,
  onToggleFullscreen,
}: {
  title: string
  session: AgentSession | null
  project?: Project
  sidebarVisible: boolean
  panelVisible?: boolean
  fullscreen?: boolean
  showingConversation?: boolean
  tabs?: PanelTab[]
  activeTabId?: string | null
  onMenu: () => void
  onOpenChanges: () => void
  onCommit: (returnFocus: HTMLElement | null) => void
  onCompareBranch: () => void
  onOpenBackgroundWork: (key: BackgroundWorkKey) => void
  onTogglePanel: () => void
  onActivateTab?: (tabId: string) => void
  onCloseTab?: (tabId: string) => void
  onAddSurface?: (surface: PanelSurface) => void
  onToggleFullscreen?: () => void
}) {
  const { t } = useI18n()
  const sidebarShortcut = usePrimaryShortcut('⌘B', 'Ctrl+B')
  const rightPanelShortcut = usePrimaryShortcut('⇧⌘B', 'Ctrl+Shift+B')
  const fullscreenShortcut = usePrimaryShortcut('⌘J', 'Ctrl+J')
  const cwd = session && project ? sessionCwd(session, project) : undefined
  const branches = useWorkspaceBranches(cwd)
  const preset = session?.provider === 'deepSeek' && session.messages.length
    ? agentPresetIdLabel(session.agent_preset || 'standard', t)
    : null
  const isFullscreenConversation = fullscreen && showingConversation
  const hasProject = Boolean(project)

  function navigateStrip(stripIndex: number, key: TabNavigationKey) {
    const next = tabNavigationIndex(tabs.length + 1, stripIndex, key)
    if (next === 0) {
      document.getElementById('right-panel-tab-conversation')?.focus()
    } else {
      const tab = tabs[next - 1]
      if (tab) {
        onActivateTab?.(tab.id)
        window.requestAnimationFrame(() => {
          document.getElementById(panelTabId(tab.id))?.focus()
        })
      }
    }
  }

  return (
    <header className="flex h-12 shrink-0 items-center gap-2 px-3 lg:px-3.5">
      <Tooltip content={t('sidebar.toggle')} shortcut={sidebarShortcut}>
        <Button
          aria-label={t('sidebar.open')}
          className={sidebarVisible ? 'lg:hidden' : undefined}
          size="icon-sm"
          variant="ghost"
          onClick={onMenu}
        >
          <PaduIcon name="panelLeft" />
        </Button>
      </Tooltip>
      <div className={cn('flex min-w-0 items-center gap-2', isFullscreenConversation && 'max-w-56 shrink-0')}>
        <h1 className="min-w-0 truncate text-[13px] font-medium">{title}</h1>
        {preset && (
          <span className="max-w-44 truncate rounded-md bg-accent px-1.5 py-1 text-[11px] font-medium text-[var(--text-secondary)]">
            {preset}
          </span>
        )}
      </div>

      {isFullscreenConversation ? (
        <div className="flex min-w-0 flex-1 justify-center overflow-hidden">
          <PanelTabStrip
            activeId={activeTabId}
            fullscreen={true}
            showingConversation={true}
            tabs={tabs}
            onActivateTab={(tabId) => onActivateTab?.(tabId)}
            onCloseTab={(tabId) => onCloseTab?.(tabId)}
            onNavigateStrip={navigateStrip}
            onSelectConversation={() => {}}
          />
        </div>
      ) : (
        <div className="flex-1" />
      )}

      {branches.data && (branches.data.additions > 0 || branches.data.deletions > 0) && (
        <button
          className="flex shrink-0 items-center gap-1 rounded px-1.5 py-1 text-[11px] hover:bg-accent cursor-pointer"
          type="button"
          onClick={onOpenChanges}
        >
          <span className="text-[var(--success)]">+{branches.data.additions}</span>
          <span className="text-destructive">-{branches.data.deletions}</span>
        </button>
      )}

      {session && (
        <EnvironmentPopover
          sessionId={session.id}
          onCommit={onCommit}
          onCompareBranch={onCompareBranch}
          onOpenBackgroundWork={onOpenBackgroundWork}
        />
      )}

      {isFullscreenConversation && onAddSurface && tabs.length > 0 && (
        <ControlMenu
          align="right"
          caret={false}
          highlightTriggerWhenOpen={false}
          label={t('right_panel.add_tab')}
          placement="below"
          selectionMode="status"
          triggerClassName="size-7 justify-center px-0 shrink-0"
          items={[
            {
              id: 'terminal',
              label: t('right_panel.terminal'),
              icon: 'terminal',
              onSelect: () => onAddSurface('terminal'),
            },
            ...(hasProject
              ? [
                  {
                    id: 'files',
                    label: t('right_panel.files'),
                    icon: 'folder' as const,
                    selected: tabs.some((tab) => tab.surface === 'files'),
                    onSelect: () => onAddSurface('files'),
                  },
                  {
                    id: 'changes',
                    label: t('right_panel.diff'),
                    icon: 'fileDiff' as const,
                    selected: tabs.some((tab) => tab.surface === 'changes'),
                    onSelect: () => onAddSurface('changes'),
                  },
                ]
              : []),
          ]}
        >
          <PaduIcon className="size-3.5" name="plus" />
        </ControlMenu>
      )}

      {isFullscreenConversation && onToggleFullscreen && (
        <Tooltip content={t('right_panel.collapse')} shortcut={fullscreenShortcut}>
          <Button
            aria-label={t('right_panel.collapse')}
            aria-pressed={true}
            className="shrink-0"
            size="icon-sm"
            variant="ghost"
            onClick={onToggleFullscreen}
          >
            <PaduIcon name="minimize" />
          </Button>
        </Tooltip>
      )}

      {!fullscreen && !panelVisible && (
        <Tooltip content={t('right_panel.toggle')} shortcut={rightPanelShortcut}>
          <Button aria-label={t('right_panel.toggle')} size="icon-sm" variant="ghost" onClick={onTogglePanel}>
            <PaduIcon name="panelRight" />
          </Button>
        </Tooltip>
      )}
    </header>
  )
}

function EnvironmentPopover({
  sessionId,
  onCommit,
  onCompareBranch,
  onOpenBackgroundWork,
}: {
  sessionId: string
  onCommit: (returnFocus: HTMLElement | null) => void
  onCompareBranch: () => void
  onOpenBackgroundWork: (key: BackgroundWorkKey) => void
}) {
  const { t } = useI18n()
  const { backgroundWork, refreshBackgroundWork, stopBackgroundWork } = useRuntime()
  const [open, setOpen] = useState(false)
  const popup = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  function focusTrigger() {
    requestAnimationFrame(() => trigger.current?.focus())
  }
  useEffect(() => {
    if (!open) return
    void refreshBackgroundWork(sessionId).catch(() => {})
    const interval = window.setInterval(() => {
      void refreshBackgroundWork(sessionId).catch(() => {})
    }, 2_000)
    return () => window.clearInterval(interval)
  }, [open, refreshBackgroundWork, sessionId])

  const items = [...(backgroundWork[sessionId] ?? [])].reverse()
  const processes = items.filter((item) => item.key.kind !== 'subagent')
  const agents = items.filter((item) => item.key.kind === 'subagent')
  const hasBackgroundWork = processes.length > 0 || agents.length > 0

  function focusMenuItem(position: 'first' | 'last') {
    requestAnimationFrame(() => {
      const items = popup.current?.querySelectorAll<HTMLElement>('[role="menuitem"]')
      const target = position === 'first' ? items?.[0] : items?.[Math.max(0, (items?.length ?? 1) - 1)]
      target?.focus()
    })
  }

  return (
    <Popover.Root modal={false} open={open} onOpenChange={setOpen}>
      <Popover.Trigger
        aria-label={t('environment.summary')}
        ref={trigger}
        render={<Button className={open ? 'bg-accent' : undefined} size="icon-sm" variant="ghost" />}
        onKeyDown={(event) => {
          if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
          event.preventDefault()
          setOpen(true)
          focusMenuItem(event.key === 'ArrowDown' ? 'first' : 'last')
        }}
        title={backgroundWorkCountSummary(items, t)}
      >
        <PaduIcon name="info" />
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Positioner
          align="end"
          className="z-[100] outline-none"
          collisionPadding={8}
          side="bottom"
          sideOffset={4}
        >
          <Popover.Popup
            aria-label={t('environment.title')}
            className="padu-popover-surface max-h-[420px] w-[min(300px,calc(100vw-24px))] overflow-y-auto rounded-[12px] p-2 text-popover-foreground outline-none"
            initialFocus={false}
            ref={popup}
            role="menu"
            onKeyDown={(event) => {
              if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
              const menuItems = [...event.currentTarget.querySelectorAll<HTMLElement>('[role="menuitem"]')]
              if (!menuItems.length) return
              const current = menuItems.indexOf(document.activeElement as HTMLElement)
              const target = event.key === 'Home'
                ? 0
                : event.key === 'End'
                  ? menuItems.length - 1
                  : event.key === 'ArrowDown'
                    ? (current + 1 + menuItems.length) % menuItems.length
                    : (current - 1 + menuItems.length) % menuItems.length
              event.preventDefault()
              menuItems[target]?.focus()
            }}
          >
            <div className="flex h-[30px] items-center px-2 text-[13.5px] text-[var(--text-tertiary)]">
              {t('environment.title')}
            </div>
            <EnvironmentAction
              icon="gitCommitHorizontal"
              label={t('environment.commit_or_push')}
              onClick={() => {
                setOpen(false)
                onCommit(trigger.current)
              }}
            />
            <EnvironmentAction
              icon="github"
              label={t('environment.compare_branch')}
              trailing="arrowUpRight"
              onClick={() => {
                setOpen(false)
                onCompareBranch()
                focusTrigger()
              }}
            />
            {hasBackgroundWork && <div className="mx-2 my-2 h-px bg-border" />}
            {processes.length > 0 && (
              <EnvironmentWorkSection
                items={processes}
                label={t('background.processes')}
                onOpen={(key) => {
                  setOpen(false)
                  onOpenBackgroundWork(key)
                  focusTrigger()
                }}
                onStop={(item) => void stopBackgroundWork(sessionId, item).catch(() => {})}
                t={t}
              />
            )}
            {agents.length > 0 && (
              <EnvironmentWorkSection
                items={agents}
                label={t('background.agents')}
                onOpen={(key) => {
                  setOpen(false)
                  onOpenBackgroundWork(key)
                  focusTrigger()
                }}
                onStop={(item) => void stopBackgroundWork(sessionId, item).catch(() => {})}
                t={t}
              />
            )}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  )
}

function EnvironmentWorkSection({
  label,
  items,
  onOpen,
  onStop,
  t,
}: {
  label: string
  items: BackgroundWorkItem[]
  onOpen: (key: BackgroundWorkKey) => void
  onStop: (item: BackgroundWorkItem) => void
  t: Translator
}) {
  return (
    <section className="flex flex-col gap-[5px]" aria-label={label}>
      <div className="px-2 text-[13px] text-[var(--text-tertiary)]">{label}</div>
      <div className="flex flex-col gap-0.5">
        {items.map((item) => {
          const stoppable = isStoppableBackgroundStatus(item.status) && item.canStop && item.controlId
          return (
            <div
              className="group flex h-8 w-full cursor-pointer items-center gap-[9px] rounded-[8px] px-2 outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
              key={`${item.key.kind}:${item.key.providerId}`}
              role="menuitem"
              tabIndex={0}
              onClick={() => onOpen(item.key)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault()
                  onOpen(item.key)
                }
              }}
            >
              <PaduIcon
                className="size-3.5 text-[var(--text-secondary)]"
                name={item.key.kind === 'subagent' ? 'bot' : 'terminalSquare'}
              />
              <span className={item.key.kind === 'subagent'
                ? 'min-w-0 flex-1 truncate text-left text-[13.5px]'
                : 'min-w-0 flex-1 truncate text-left text-[12px] text-[var(--text-secondary)]'}>
                {item.title}
              </span>
              {(item.key.kind !== 'subagent' || stoppable) && (
                <span className="relative grid size-6 shrink-0 place-items-center">
                  {item.key.kind !== 'subagent' && (
                    <EnvironmentWorkStatus
                      className={stoppable ? 'group-hover:invisible group-focus-within:invisible' : undefined}
                      status={item.status}
                    />
                  )}
                  {stoppable && (
                    <button
                      aria-label={t('background.stop_named', { name: item.title })}
                      className="absolute inset-0 grid place-items-center rounded-md opacity-0 outline-none hover:bg-accent group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:ring-1 focus-visible:ring-ring"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation()
                        onStop(item)
                      }}
                      onPointerDown={(event) => event.stopPropagation()}
                    >
                      <PaduIcon className="size-3 text-[var(--text-tertiary)]" name="stopFilled" />
                    </button>
                  )}
                </span>
              )}
            </div>
          )
        })}
      </div>
    </section>
  )
}

function EnvironmentWorkStatus({ status, className }: { status: BackgroundWorkStatus; className?: string }) {
  const live = status === 'starting' || status === 'running' || status === 'monitoring'
  const icon = live
    ? 'loaderCircle'
    : status === 'completed'
      ? 'check'
      : status === 'failed'
        ? 'x'
        : status === 'lost'
          ? 'alert'
          : 'stop'
  return (
    <PaduIcon
      className={`${className ?? ''} size-3 ${live ? 'text-ring motion-safe:animate-spin' : status === 'completed' ? 'text-[var(--success)]' : status === 'failed' || status === 'lost' ? 'text-destructive' : 'text-[var(--text-tertiary)]'}`}
      name={icon}
    />
  )
}

function backgroundWorkCountSummary(items: BackgroundWorkItem[], t: Translator) {
  const processes = items.filter((item) => item.key.kind !== 'subagent' && isLiveBackgroundStatus(item.status)).length
  const agents = items.filter((item) => item.key.kind === 'subagent' && isLiveBackgroundStatus(item.status)).length
  const parts = []
  if (processes) parts.push(t(processes === 1 ? 'background.process_count_one' : 'background.process_count', { count: processes }))
  if (agents) parts.push(t(agents === 1 ? 'background.agent_count_one' : 'background.agent_count', { count: agents }))
  return parts.length ? parts.join(' · ') : t('environment.summary')
}

function isLiveBackgroundStatus(status: BackgroundWorkStatus) {
  return isStoppableBackgroundStatus(status) || status === 'stopping'
}

function isStoppableBackgroundStatus(status: BackgroundWorkStatus) {
  return status === 'starting' || status === 'running' || status === 'monitoring'
}

function EnvironmentAction({
  icon,
  label,
  trailing,
  onClick,
}: {
  icon: 'gitCommitHorizontal' | 'github'
  label: string
  trailing?: 'arrowUpRight'
  onClick: () => void
}) {
  return (
    <button
      className="flex min-h-8 w-full items-center gap-2.5 rounded-[8px] px-2 text-[13.5px] outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
      role="menuitem"
      type="button"
      onClick={onClick}
    >
      <PaduIcon className="size-3.5 text-[var(--text-secondary)]" name={icon} />
      <span className="min-w-0 flex-1 truncate text-left">{label}</span>
      {trailing && <PaduIcon className="size-[13px] text-[var(--text-tertiary)]" name={trailing} />}
    </button>
  )
}

function NewTaskCanvas({
  project,
  projects,
  onProject,
  onAddProject,
  onProjectless,
}: {
  project: Project
  projects: Project[]
  onProject: (project: Project) => void
  onAddProject: () => void
  onProjectless: () => void
}) {
  const { t } = useI18n()
  const projectless = isProjectlessProject(project)
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center px-8 pb-12">
      <div className="text-center">
        <PaduIcon
          className="mx-auto size-10 text-foreground dark:text-white sm:size-12 lg:size-14"
          name="logo"
        />
        <div className="mt-3 flex flex-wrap items-baseline justify-center gap-1 text-xl font-medium">
          {projectless ? (
            <span>{t('onboarding.what_should_we_build')}</span>
          ) : (
            <>
              <span>{t('onboarding.what_should_we_build_in')}</span>
              <ControlMenu
                caret={false}
                items={[
                  ...selectableProjects(projects, project)
                    .filter((item) => item.name !== 'No project')
                    .map((item) => ({
                      id: item.id,
                      label: item.name,
                      selected: item.id === project.id,
                      onSelect: () => onProject(item),
                    })),
                  { id: 'add', label: t('project.new_project'), icon: 'folderNew' as const, onSelect: onAddProject },
                  { id: 'projectless', label: t('project.no_project'), icon: 'x' as const, onSelect: onProjectless },
                ]}
                label={projectDisplayName(project, t('project.no_project_name'))}
                menuClassName="w-60 text-sm"
                placement="below"
                triggerClassName="h-auto max-w-64 rounded-none border-b border-dashed border-[var(--text-tertiary)] px-0 text-[20px] font-medium leading-6 text-foreground hover:bg-transparent focus-visible:ring-0 focus-visible:border-ring"
              />
              <span>{t('onboarding.question_mark')}</span>
            </>
          )}
        </div>
      </div>
    </div>
  )
}

function NoProjectState({
  projectlessPending,
  onAddProject,
  onProjectless,
}: {
  projectlessPending: boolean
  onAddProject: () => void
  onProjectless: () => void
}) {
  const { t } = useI18n()
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center px-8 pb-12">
      <div className="max-w-sm text-center">
        <PaduIcon className="mx-auto size-10 text-ring sm:size-12 lg:size-14" name="logo" />
        <h2 className="mt-4 text-xl font-medium">{t('onboarding.open_project_to_begin')}</h2>
        <p className="mt-2 text-[12.5px] leading-[19px] text-[var(--text-tertiary)]">
          {t('onboarding.web_description')}
        </p>
        <div className="mt-5 flex flex-col items-center gap-2">
          <Button className="rounded-full" onClick={onAddProject}>
            <PaduIcon name="plus" /> {t('sidebar.add_project')}
          </Button>
          <Button
            className="rounded-full"
            disabled={projectlessPending}
            variant="ghost"
            onClick={onProjectless}
          >
            {projectlessPending ? t('common.creating') : t('project.no_project_name')}
          </Button>
        </div>
      </div>
    </div>
  )
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
