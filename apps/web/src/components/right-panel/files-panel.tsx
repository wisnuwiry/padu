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
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { Input } from '@/components/ui/input'
import { Tooltip } from '@/components/ui/tooltip'
import { FileTypeIcon, PaduIcon } from '@/components/padu-icon'
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
import { cn } from '@/lib/utils'
import { Kbd } from '../ui/kbd'
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
