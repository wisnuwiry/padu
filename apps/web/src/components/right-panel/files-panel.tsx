import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query'
import type { Editor } from '@pierre/diffs/edit'
import type { AgentSession, Project, WorkingTreeEntry } from '@padu/client'
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type KeyboardEvent as ReactKeyboardEvent,
  type SetStateAction,
} from 'react'
import { ContextMenu } from '@base-ui/react/context-menu'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import { toast } from 'sonner'
import { PanelResizeHandle } from '@/components/panel-resize-handle'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { Tooltip } from '@/components/ui/tooltip'
import { FileTypeIcon, PaduIcon } from '@/components/padu-icon'
import { MarkdownView } from '@/components/markdown-view'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { projectDisplayName } from '@/lib/project-presentation'
import {
  createWorkspaceDirectory,
  createWorkspaceFile,
  daemonKeys,
  deleteWorkspacePath,
  listWorkspaceTree,
  readWorkspaceTextFile,
  renameWorkspacePath,
  sessionCwd,
  writeWorkspaceTextFile,
} from '@/lib/daemon-api'
import { treeNavigationAction } from '@/lib/right-panel-state'
import { usePrimaryShortcut } from '@/lib/platform'
import { cn } from '@/lib/utils'
import { absoluteParentPaths, clamp, errorMessage, focusVirtualTreeRow, isTreeNavigationKey, PanelMessage, readStoredWidth, requireClient, workingTreeRowId } from './shared'
import type { Translator } from '@/lib/transcript-presentation'


const CodeFileSurface = lazy(() => import('@/components/code-surfaces').then((module) => ({ default: module.CodeFileSurface })))

export interface FileBuffer {
  content: string
  diskContent: string
  revision: number
  saving: boolean
  editor?: Editor<undefined>
}

/// The files whose contents are rendered (view mode) rather than only edited.
/// Mirrors the desktop client's `file_highlighter_language(...) == "markdown"`.
function isMarkdownFile(path: string): boolean {
  const extension = path.split('/').at(-1)?.split('.').at(-1)?.toLowerCase()
  return extension === 'md' || extension === 'markdown' || extension === 'mdx'
}

function FileBreadcrumbs({
  path,
  dirty,
  markdown = false,
  preview = false,
  onTogglePreview,
}: {
  path: string
  dirty?: boolean
  markdown?: boolean
  preview?: boolean
  onTogglePreview?: () => void
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
      {markdown && (
        <Tooltip content={preview ? t('files.edit_markdown_source') : t('files.preview_markdown')}>
          <button
            type="button"
            aria-label={preview ? t('files.edit_markdown_source') : t('files.preview_markdown')}
            aria-pressed={preview}
            className="grid size-6 shrink-0 cursor-pointer place-items-center rounded hover:bg-accent text-[var(--text-tertiary)] hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring transition-colors"
            onClick={onTogglePreview}
          >
            <PaduIcon className="size-3.5" name={preview ? 'pencil' : 'eye'} />
          </button>
        </Tooltip>
      )}
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

export function FilesPanel({
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
  onAddToChat?: (path: string, isDir?: boolean) => void
  onFindFile?: () => void
}) {
  const { t } = useI18n()
  const findShortcut = usePrimaryShortcut('⌘P', 'Ctrl+P')
  const { client, config, phase } = useDaemon()
  const queryClient = useQueryClient()
  const [expanded, setExpanded] = useState<string[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [focusedTreeEntry, setFocusedTreeEntry] = useState<string | null>(null)
  const [showHidden, setShowHidden] = useState(false)
  const [inlineOp, setInlineOp] = useState<
    | { kind: 'createFile'; parent: string; depth: number; value: string }
    | { kind: 'createDirectory'; parent: string; depth: number; value: string }
    | { kind: 'rename'; absolutePath: string; relativePath: string; name: string; value: string }
    | null
  >(null)
  const treeList = useRef<VirtuosoHandle>(null)
  const buffersRef = useRef(buffers)
  buffersRef.current = buffers
  const [treeWidth, setTreeWidth] = useState(() => readStoredWidth('padu.fileTreeWidth', 184, 140, 360))
  const treeWidthRef = useRef(treeWidth)
  treeWidthRef.current = treeWidth
  // Global markdown source/preview toggle, mirroring the desktop client's
  // persisted choice so the mode follows the user across files and sessions.
  const [markdownPreview, setMarkdownPreview] = useState(
    () => typeof window !== 'undefined' && window.localStorage.getItem('padu.markdownPreview') === '1',
  )
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
    setInlineOp(null)
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

  useEffect(() => {
    window.localStorage.setItem('padu.markdownPreview', markdownPreview ? '1' : '0')
  }, [markdownPreview])

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

  const [deleteTarget, setDeleteTarget] = useState<WorkingTreeEntry | null>(null)

  const beginInlineCreate = useCallback((directory: boolean, parent: string, depth: number) => {
    setExpanded((current) => {
      if (!parent || !root) return current
      const separator = root.includes('\\') ? '\\' : '/'
      const normalizedRoot = root.replace(/[\\/]+$/, '')
      const parentAbsolute = `${normalizedRoot}${separator}${parent.split('/').join(separator)}`
      if (current.includes(parentAbsolute)) return current
      return [...current, ...absoluteParentPaths(root, `${parent}/x`), parentAbsolute]
    })
    setInlineOp({ kind: directory ? 'createDirectory' : 'createFile', parent, depth, value: '' })
  }, [root])

  const createEntry = useCallback((directory: boolean, parent: string, depth: number) => {
    beginInlineCreate(directory, parent, depth)
  }, [beginInlineCreate])

  const renameEntry = useCallback((entry: WorkingTreeEntry) => {
    setInlineOp({ kind: 'rename', absolutePath: entry.absolutePath, relativePath: entry.relativePath, name: entry.name, value: entry.name })
  }, [])

  const cancelInlineOp = useCallback(() => {
    setInlineOp(null)
  }, [])

  const deleteEntry = useCallback((entry: WorkingTreeEntry) => {
    setDeleteTarget(entry)
  }, [])

  const handleWorkingTreeKeyDown = useCallback((
    event: ReactKeyboardEvent<HTMLElement>,
    entry: WorkingTreeEntry,
    index: number,
  ) => {
    if (event.key === 'Escape' && inlineOp) {
      event.preventDefault()
      cancelInlineOp()
      return
    }
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
  }, [activateTreeEntry, cancelInlineOp, deleteEntry, expanded, focusWorkingTreeIndex, inlineOp, renameEntry, workingTreeEntries])

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

  const submitInlineOp = useCallback(async () => {
    if (!client || !root || !inlineOp) return
    const name = inlineOp.value.trim()
    if (!name) {
      setInlineOp(null)
      return
    }
    if (name.includes('/') || name.includes('\\')) {
      toast.error(t('files.invalid_name'))
      return
    }
    try {
      if (inlineOp.kind === 'createFile' || inlineOp.kind === 'createDirectory') {
        const parent = inlineOp.parent
        const relativePath = `${parent ? `${parent}/` : ''}${name}`
        if (inlineOp.kind === 'createFile') {
          await createWorkspaceFile(client, root, relativePath)
        } else {
          await createWorkspaceDirectory(client, root, relativePath)
        }
        setInlineOp(null)
        refreshTree()
        if (inlineOp.kind === 'createFile') {
          setSelected(relativePath)
          onOpenFile(tabId, relativePath, treeWidthRef.current)
        }
      } else {
        if (name === inlineOp.name) {
          setInlineOp(null)
          return
        }
        const parent = inlineOp.relativePath.split('/').slice(0, -1).join('/')
        const nextRelative = `${parent ? `${parent}/` : ''}${name}`
        await renameWorkspacePath(client, root, inlineOp.relativePath, nextRelative)
        if (selected === inlineOp.relativePath) setSelected(nextRelative)
        setInlineOp(null)
        refreshTree()
      }
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }, [client, inlineOp, onOpenFile, refreshTree, root, selected, t, tabId])

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

  const focusedEntry = workingTreeEntries.find((entry) => entry.absolutePath === focusedTreeEntry)
    ?? workingTreeEntries.find((entry) => entry.relativePath === selected)
  const createAtFocus = (directory: boolean) => {
    const parent = createParentPath(focusedEntry)
    const depth = !focusedEntry ? 0 : focusedEntry.isDir ? focusedEntry.depth + 1 : focusedEntry.depth
    createEntry(directory, parent, depth)
  }
  const createAtEntry = (entry: WorkingTreeEntry, directory: boolean) => {
    const parent = entry.isDir
      ? entry.relativePath
      : entry.relativePath.split('/').slice(0, -1).join('/')
    const depth = entry.isDir ? entry.depth + 1 : entry.depth
    createEntry(directory, parent, depth)
  }
  type TreeListItem =
    | { kind: 'entry'; entry: WorkingTreeEntry }
    | { kind: 'create'; parent: string; depth: number; isDir: boolean }
  const treeListItems: TreeListItem[] = (() => {
    const items = workingTreeEntries.map((entry): TreeListItem => ({ kind: 'entry', entry }))
    if (!inlineOp || (inlineOp.kind !== 'createFile' && inlineOp.kind !== 'createDirectory')) return items
    const createItem: TreeListItem = {
      kind: 'create',
      parent: inlineOp.parent,
      depth: inlineOp.depth,
      isDir: inlineOp.kind === 'createDirectory',
    }
    if (!inlineOp.parent) return [createItem, ...items]
    const parentIndex = items.findIndex(
      (item) => item.kind === 'entry' && item.entry.relativePath === inlineOp.parent,
    )
    if (parentIndex === -1) return [...items, createItem]
    return [...items.slice(0, parentIndex + 1), createItem, ...items.slice(parentIndex + 1)]
  })()
  const handleFilesPanelKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape' && inlineOp) {
      event.preventDefault()
      cancelInlineOp()
      return
    }
    const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement
    if (typing || event.metaKey || event.ctrlKey || event.altKey) return
    const key = event.key.toLowerCase()
    if (key === 'n') {
      event.preventDefault()
      createAtFocus(event.shiftKey)
    } else if (key === 'r') {
      event.preventDefault()
      refreshTree()
    } else if (key === 'h') {
      event.preventDefault()
      setShowHidden((value) => !value)
    }
  }
  const fileTree = (
    <div
      className={cn('relative flex min-h-0 flex-col outline-none', selected ? 'shrink-0 border-l' : 'flex-1')}
      style={selected ? { width: fittedTreeWidth } : undefined}
      tabIndex={0}
      onKeyDown={handleFilesPanelKeyDown}
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
          <Tooltip content={t('command_palette.find_file')} shortcut={findShortcut}>
            <button
              aria-label={t('command_palette.find_file')}
              className="grid size-6 cursor-pointer place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              type="button"
              onClick={onFindFile}
            >
              <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="search" />
            </button>
          </Tooltip>
          <Tooltip content={t('files.new_file')} shortcut="N">
            <button
              aria-label={t('files.new_file')}
              className="grid size-6 cursor-pointer place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              type="button"
              onClick={() => createAtFocus(false)}
            >
              <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="fileAdd" />
            </button>
          </Tooltip>
          <Tooltip content={t('files.new_folder')} shortcut="⇧N">
            <button
              aria-label={t('files.new_folder')}
              className="grid size-6 cursor-pointer place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              type="button"
              onClick={() => createAtFocus(true)}
            >
              <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="folderNew" />
            </button>
          </Tooltip>
          <Tooltip content={t('files.refresh')} shortcut="R">
            <button aria-label={t('files.refresh')} className="grid size-6 cursor-pointer place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" type="button" onClick={refreshTree}>
              <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name="rotateCw" />
            </button>
          </Tooltip>
          <Tooltip content={showHidden ? t('files.hide_hidden') : t('files.show_hidden')} shortcut="H">
            <button aria-label={showHidden ? t('files.hide_hidden') : t('files.show_hidden')} className="grid size-6 cursor-pointer place-items-center rounded hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" type="button" onClick={() => setShowHidden((value) => !value)}>
              <PaduIcon className="size-3.5 text-[var(--text-tertiary)]" name={showHidden ? 'eye' : 'eyeOff'} />
            </button>
          </Tooltip>
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
              computeItemKey={(_, item) => item.kind === 'create'
                ? `inline-create:${item.parent}:${item.isDir ? 'dir' : 'file'}`
                : item.entry.absolutePath}
              data={treeListItems}
              fixedItemHeight={30}
              increaseViewportBy={180}
              itemContent={(index, item) => {
                if (item.kind === 'create') {
                  return (
                    <InlineCreateRow
                      depth={item.depth}
                      isDir={item.isDir}
                      value={inlineOp?.kind === 'createFile' || inlineOp?.kind === 'createDirectory' ? inlineOp.value : ''}
                      onChange={(value) => setInlineOp((current) => current && (current.kind === 'createFile' || current.kind === 'createDirectory')
                        ? { ...current, value }
                        : current)}
                      onCancel={cancelInlineOp}
                      onSubmit={() => void submitInlineOp()}
                      t={t}
                    />
                  )
                }
                const entry = item.entry
                const entryIndex = workingTreeEntries.findIndex((candidate) => candidate.absolutePath === entry.absolutePath)
                return (
                <TreeRow
                  entry={entry}
                  expanded={entry.isDir && expanded.includes(entry.absolutePath)}
                  id={workingTreeRowId(entry.absolutePath)}
                  selected={selected === entry.relativePath}
                  tabIndex={treeTabStop === entry.absolutePath ? 0 : -1}
                  renaming={inlineOp?.kind === 'rename' && inlineOp.absolutePath === entry.absolutePath}
                  renameValue={inlineOp?.kind === 'rename' && inlineOp.absolutePath === entry.absolutePath ? inlineOp.value : ''}
                  onRenameChange={(value) => setInlineOp((current) => current?.kind === 'rename' ? { ...current, value } : current)}
                  onRenameCancel={cancelInlineOp}
                  onRenameSubmit={() => void submitInlineOp()}
                  onActivate={activateTreeEntry}
                  onAddToChat={onAddToChat}
                  onCreateFile={() => createAtEntry(entry, false)}
                  onCreateFolder={() => createAtEntry(entry, true)}
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
                  onKeyDown={(event) => handleWorkingTreeKeyDown(event, entry, entryIndex === -1 ? index : entryIndex)}
                  onRename={() => void renameEntry(entry)}
                />
                )
              }}
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
                onClick={() => void createEntry(false, '', 0)}
              >
                <PaduIcon className="size-3" name="fileAdd" /> {t('files.new_file')}
              </ContextMenu.Item>
              <ContextMenu.Item
                className="padu-menu-item"
                onClick={() => void createEntry(true, '', 0)}
              >
                <PaduIcon className="size-3" name="folderNew" /> {t('files.new_folder')}
              </ContextMenu.Item>
            </ContextMenu.Popup>
          </ContextMenu.Positioner>
        </ContextMenu.Portal>
      </ContextMenu.Root>
    </div>
  )

  const deleteDialog = (
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
  )

  if (!selected) {
    return (
      <>
        {fileTree}
        {deleteDialog}
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
  const selectedIsMarkdown = isMarkdownFile(selected)
  const preview = selectedIsMarkdown && markdownPreview
  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <FileBreadcrumbs
          path={selected}
          dirty={isDirty}
          markdown={selectedIsMarkdown}
          preview={preview}
          onTogglePreview={() => setMarkdownPreview((value) => !value)}
        />
        {!buffer && file.isPending
          ? <PanelMessage title={t('files.loading_file')} detail={t('files.reading_from_daemon')} />
          : !buffer && file.error
            ? <PanelMessage title={t('files.file_unavailable')} detail={errorMessage(file.error)} danger />
            : buffer && preview
              ? (
                <div className="h-full min-h-0 min-w-0 flex-1 overflow-auto bg-card">
                  <div className="min-w-0 px-4 pt-3.5 pb-6">
                    <MarkdownView text={buffer.content} />
                  </div>
                </div>
              )
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
      {deleteDialog}
    </div>
  )
}

interface TreeRowProps {
  entry: WorkingTreeEntry
  expanded: boolean
  id: string
  selected: boolean
  tabIndex: number
  renaming?: boolean
  renameValue?: string
  onRenameChange?: (value: string) => void
  onRenameCancel?: () => void
  onRenameSubmit?: () => void
  onActivate: (entry: WorkingTreeEntry) => void
  onAddToChat?: (path: string, isDir?: boolean) => void
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
  renaming,
  renameValue,
  onRenameChange,
  onRenameCancel,
  onRenameSubmit,
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
  if (renaming) {
    return (
      <div
        className={cn(
          'mx-2 flex h-[30px] min-h-[30px] min-w-0 shrink-0 cursor-pointer items-center gap-1.5 rounded-md bg-accent pr-1.5 text-left text-[11.5px] outline-none',
          entry.isIgnored && 'opacity-55 text-[var(--text-ghost)]',
        )}
        id={id}
        role="treeitem"
        aria-level={entry.depth + 1}
        style={{ paddingLeft: `${8 + entry.depth * 16}px`, width: 'calc(100% - 16px)' }}
        onClick={(event) => event.stopPropagation()}
        onContextMenu={(event) => event.stopPropagation()}
      >
        {entry.isDir
          ? expanded
            ? <PaduIcon className="size-2.5 shrink-0 text-[var(--text-ghost)]" name="chevronDown" />
            : <PaduIcon className="size-2.5 shrink-0 text-[var(--text-ghost)]" name="chevronRight" />
          : <span className="size-2.5 shrink-0" />}
        {entry.isDir
          ? <PaduIcon className="size-3.5 shrink-0 text-[var(--text-tertiary)]" name={expanded ? 'folderOpen' : 'folder'} />
          : <FileTypeIcon className="size-3.5 shrink-0" path={entry.name} />}
        <InlineNameField
          autoSelectStem={!entry.isDir}
          initialValue={renameValue ?? entry.name}
          placeholder={t('files.name_placeholder')}
          onCancel={() => onRenameCancel?.()}
          onChange={onRenameChange}
          onSubmit={() => onRenameSubmit?.()}
        />
      </div>
    )
  }
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger
        aria-expanded={entry.isDir ? expanded : undefined}
        aria-level={entry.depth + 1}
        className={cn(
          'mx-2 flex h-[30px] min-h-[30px] shrink-0 min-w-0 cursor-pointer items-center gap-1.5 rounded-md pr-2 text-left text-[11.5px] outline-none hover:bg-accent focus-visible:bg-accent',
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
                <ContextMenu.Item className="padu-menu-item" onClick={() => onAddToChat(entry.relativePath, entry.isDir)}>
                  <PaduIcon className="size-3" name="compose" /> {t('files.add_to_chat')}
                </ContextMenu.Item>
                <ContextMenu.Separator className="padu-menu-separator" />
              </>
            )}
            <ContextMenu.Item className="padu-menu-item" onClick={onCreateFile}><PaduIcon className="size-3" name="fileAdd" /> {t('files.new_file')}</ContextMenu.Item>
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

function InlineCreateRow({
  depth,
  isDir,
  value,
  onChange,
  onCancel,
  onSubmit,
  t,
}: {
  depth: number
  isDir: boolean
  value: string
  onChange: (value: string) => void
  onCancel: () => void
  onSubmit: () => void
  t: Translator
}) {
  return (
    <div
      className="mx-2 flex h-[30px] min-h-[30px] min-w-0 shrink-0 cursor-pointer items-center gap-1.5 rounded-md bg-accent py-0 pr-1.5 text-left text-[11.5px] outline-none"
      role="treeitem"
      aria-level={depth + 1}
      style={{ paddingLeft: `${8 + depth * 16}px`, width: 'calc(100% - 16px)' }}
      onClick={(event) => event.stopPropagation()}
      onContextMenu={(event) => event.stopPropagation()}
    >
      <span className="size-2.5 shrink-0" />
      <PaduIcon className="size-3.5 shrink-0 text-[var(--text-tertiary)]" name={isDir ? 'folder' : 'file'} />
      <InlineNameField
        initialValue={value}
        placeholder={isDir ? t('files.new_folder') : t('files.name_placeholder')}
        onCancel={onCancel}
        onChange={onChange}
        onSubmit={onSubmit}
      />
    </div>
  )
}

function InlineNameField({
  initialValue,
  placeholder,
  autoSelectStem,
  onCancel,
  onChange,
  onSubmit,
}: {
  initialValue: string
  placeholder: string
  autoSelectStem?: boolean
  onCancel: () => void
  onChange?: (value: string) => void
  onSubmit: () => void
}) {
  const inputRef = useRef<HTMLInputElement>(null)
  const [value, setValue] = useState(initialValue)
  const mountedRef = useRef(false)

  // Focus once on mount. Deliberately not re-running when the parent mirrors
  // the typed value back through `initialValue` — that would steal selection
  // on every keystroke.
  useEffect(() => {
    if (mountedRef.current) return
    mountedRef.current = true
    const input = inputRef.current
    if (!input) return
    input.focus()
    if (autoSelectStem && initialValue) {
      const dot = initialValue.lastIndexOf('.')
      const end = dot > 0 ? dot : initialValue.length
      input.setSelectionRange(0, end)
    } else {
      input.select()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <span className="flex min-w-0 flex-1 items-center gap-1">
      <input
        ref={inputRef}
        aria-label={placeholder}
        className="h-6 min-w-0 flex-1 rounded-[5px] border border-ring bg-card px-1.5 text-[11.5px] text-foreground outline-none placeholder:text-[var(--text-ghost)]"
        placeholder={placeholder}
        value={value}
        onChange={(event) => {
          setValue(event.target.value)
          onChange?.(event.target.value)
        }}
        onKeyDown={(event) => {
          event.stopPropagation()
          if (event.key === 'Enter') {
            event.preventDefault()
            onSubmit()
          } else if (event.key === 'Escape') {
            event.preventDefault()
            onCancel()
          }
        }}
        onClick={(event) => event.stopPropagation()}
      />
      <button
        type="button"
        aria-label="Confirm"
        className="grid size-5 shrink-0 cursor-pointer place-items-center rounded hover:bg-accent"
        onClick={(event) => {
          event.stopPropagation()
          onSubmit()
        }}
      >
        <PaduIcon className="size-3 text-[var(--accent)]" name="check" />
      </button>
      <button
        type="button"
        aria-label="Cancel"
        className="grid size-5 shrink-0 cursor-pointer place-items-center rounded hover:bg-accent"
        onClick={(event) => {
          event.stopPropagation()
          onCancel()
        }}
      >
        <PaduIcon className="size-3 text-[var(--text-tertiary)]" name="x" />
      </button>
    </span>
  )
}

function createParentPath(entry?: WorkingTreeEntry) {
  if (!entry) return ''
  if (entry.isDir) return entry.relativePath
  const slash = entry.relativePath.lastIndexOf('/')
  return slash === -1 ? '' : entry.relativePath.slice(0, slash)
}
