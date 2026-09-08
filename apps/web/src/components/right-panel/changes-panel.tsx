import { useQuery } from '@tanstack/react-query'
import type { AgentSession, Project, ReviewDiffSource } from '@padu/client'
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import { ControlMenu } from '@/components/control-menu'
import { PanelResizeHandle } from '@/components/panel-resize-handle'
import { FileTypeIcon, PaduIcon } from '@/components/padu-icon'
import type { CodeDiffSurfaceHandle, DiffSurfaceFile } from '@/components/code-surfaces'
import { collectWorkspaceDiff, daemonKeys, sessionCwd } from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { latestReviewTurnSource, reviewDiffSourceLabel, sameReviewDiffSource } from '@/lib/review-diff'
import { mergeReviewDiffFiles, treeNavigationAction } from '@/lib/right-panel-state'
import { cn } from '@/lib/utils'
import { clamp, diffTreeRowId, errorMessage, focusVirtualTreeRow, isTreeNavigationKey, PanelMessage, parseNumstat, readStoredWidth, requireClient } from './shared'

const CodeDiffSurface = lazy(() => import('@/components/code-surfaces').then((module) => ({ default: module.CodeDiffSurface })))

export function ChangesPanel({
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
  const [layout, setLayout] = useState<'tree' | 'flat'>('tree')
  const [showNavigator, setShowNavigator] = useState(true)
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

  const navigatorRows = layout === 'tree'
    ? buildDiffTreeRows(reviewFiles, expandedPaths, filter)
    : buildDiffFlatRows(reviewFiles, filter)
  const selectedDiffRow = selectedFile
    ? navigatorRows.find((row) => row.kind === 'file' && row.file.id === selectedFile)
    : undefined
  const diffTreeTabStop = focusedDiffRow && navigatorRows.some((row) => diffTreeRowKey(row) === focusedDiffRow)
    ? focusedDiffRow
    : selectedDiffRow ? diffTreeRowKey(selectedDiffRow) : navigatorRows[0] ? diffTreeRowKey(navigatorRows[0]) : null

  const toggleDiffDirectory = useCallback((path: string) => {
    setExpandedPaths((current) => {
      const next = new Set(current)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }, [])

  const focusDiffTreeIndex = useCallback((index: number) => {
    const row = navigatorRows[index]
    if (!row) return
    const key = diffTreeRowKey(row)
    setFocusedDiffRow(key)
    focusVirtualTreeRow(diffTreeList, index, diffTreeRowId(key))
  }, [navigatorRows])

  const handleDiffTreeKeyDown = useCallback((
    event: ReactKeyboardEvent<HTMLButtonElement>,
    row: DiffTreeRow,
    index: number,
  ) => {
    if (event.metaKey || event.ctrlKey || event.altKey) return
    if (!isTreeNavigationKey(event.key)) return
    event.preventDefault()
    const action = treeNavigationAction(
      navigatorRows.map((candidate) => ({
        depth: candidate.depth,
        directory: candidate.kind === 'directory',
        expanded: candidate.kind === 'directory' && candidate.expanded,
      })),
      index,
      event.key,
    )
    if (action.toggle && row.kind === 'directory' && layout === 'tree') {
      setFocusedDiffRow(diffTreeRowKey(row))
      toggleDiffDirectory(row.path)
    } else {
      focusDiffTreeIndex(action.index)
    }
  }, [focusDiffTreeIndex, layout, navigatorRows, toggleDiffDirectory])

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
        {showNavigator && <div
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
            data={navigatorRows}
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
        </div>}
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
        {diff.data && (
          <>
            <div className="flex items-center rounded-md border p-0.5" role="group" aria-label="Changed-file layout">
              <button
                aria-label="Tree layout"
                aria-pressed={layout === 'tree'}
                className={cn('rounded px-1.5 py-1 text-[10px] hover:bg-accent', layout === 'tree' && 'bg-accent text-foreground')}
                type="button"
                onClick={() => setLayout('tree')}
              >
                <PaduIcon className="size-3.5" name="folder" />
              </button>
              <button
                aria-label="Flat layout"
                aria-pressed={layout === 'flat'}
                className={cn('rounded px-1.5 py-1 text-[10px] hover:bg-accent', layout === 'flat' && 'bg-accent text-foreground')}
                type="button"
                onClick={() => setLayout('flat')}
              >
                <PaduIcon className="size-3.5" name="list" />
              </button>
            </div>
            {layout === 'tree' && (
              <>
                <button
                  aria-label="Expand all changed-file folders"
                  className="rounded p-1 text-[var(--text-tertiary)] hover:bg-accent hover:text-foreground"
                  title="Expand all"
                  type="button"
                  onClick={() => setExpandedPaths(diffDirectoryPaths(reviewFiles))}
                >
                  <PaduIcon className="size-3.5" name="chevronDown" />
                </button>
                <button
                  aria-label="Collapse all changed-file folders"
                  className="rounded p-1 text-[var(--text-tertiary)] hover:bg-accent hover:text-foreground"
                  title="Collapse all"
                  type="button"
                  onClick={() => setExpandedPaths(new Set())}
                >
                  <PaduIcon className="size-3.5" name="chevronUp" />
                </button>
              </>
            )}
            <button
              aria-pressed={showNavigator}
              aria-label={showNavigator ? 'Hide changed-file navigator' : 'Show changed-file navigator'}
              className="rounded p-1 text-[var(--text-tertiary)] hover:bg-accent hover:text-foreground"
              title={showNavigator ? 'Hide changed-file navigator' : 'Show changed-file navigator'}
              type="button"
              onClick={() => setShowNavigator((visible) => !visible)}
            >
              <PaduIcon className="size-3.5" name="panelRight" />
            </button>
          </>
        )}
        <button aria-label={t('diff.refresh')} className="rounded p-1 hover:bg-accent" type="button" onClick={() => void diff.refetch()}>
          <PaduIcon className={cn('size-3.5', diff.isFetching && 'motion-safe:animate-spin')} name="rotateCw" />
        </button>
      </div>
      {reviewContent}
    </div>
  )
}

function diffTreeRowKey(row: DiffTreeRow): string {
  return row.kind === 'directory' ? `directory:${row.path}` : `file:${row.file.id}`
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

function buildDiffFlatRows(files: DiffSurfaceFile[], filter: string): DiffTreeRow[] {
  const needle = filter.trim().toLocaleLowerCase()
  return [...files]
    .filter((file) => !needle || file.path.toLocaleLowerCase().includes(needle))
    .sort((left, right) => left.path.localeCompare(right.path))
    .map((file) => ({ kind: 'file' as const, file, depth: 0 }))
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
