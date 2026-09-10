import { useNavigate, useSearch } from '@tanstack/react-router'
import type { Note, NoteSummary, PaduClient } from '@padu/client'
import { useEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { toast } from 'sonner'
import { PaduIcon } from '@/components/padu-icon'
import {
  createNote,
  deleteNote,
  getNote,
  listNotes,
  updateNote,
} from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { formatNoteTimeAgo, noteExcerpt } from '@/lib/notes-utils'

const ALL_NOTES_PROJECT_ID = '00000000-0000-0000-0000-000000000000'

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


export function NotesPage() {
  const navigate = useNavigate()
  const search = useSearch({ from: '/notes' })
  const { client } = useDaemon()
  const { t } = useI18n()
  const projectId = search.projectId
  const [notes, setNotes] = useState<NoteSummary[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selected, setSelected] = useState<Note | null>(null)
  const [filter, setFilter] = useState(search.q ?? '')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [preview, setPreview] = useState(false)
  const [listCollapsed, setListCollapsed] = useState(false)
  const [contextMenu, setContextMenu] = useState<{ id: string; x: number; y: number } | null>(null)
  const autosaveKey = useRef<string | null>(null)
  // Bumped per refresh; a completion from a superseded request (project switch,
  // a newer refresh, or an add-triggered reload) is discarded so it cannot
  // replace newer notes or selections with stale data.
  const refreshGeneration = useRef(0)

  async function refresh(preferredId?: string) {
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
      const id = preferredId ?? selectedId ?? next[0]?.id ?? null
      const summary = next.find((note) => note.id === id)
      if (generation !== refreshGeneration.current) return
      setSelectedId(id)
      setSelected(id && summary ? await getNote(client, summary.projectId, id) : null)
    } catch (error) {
      if (generation !== refreshGeneration.current) return
      toast.error(error instanceof Error ? error.message : 'Could not load notes')
    } finally {
      if (generation === refreshGeneration.current) setLoading(false)
    }
  }

  useEffect(() => {
    void refresh(search.noteId)
    // Notes are project-independent; only the daemon connection identifies this resource.
    // refresh intentionally reads current state when it fires.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, search.noteId])

  const visibleNotes = useMemo(() => {
    const query = filter.trim().toLocaleLowerCase()
    return notes.filter((note) => !query || `${note.title} ${note.preview}`.toLocaleLowerCase().includes(query))
  }, [filter, notes])

  async function selectNote(id: string) {
    if (!client) return
    const summary = notes.find((note) => note.id === id)
    if (!summary) return
    setSelectedId(id)
    try {
      setSelected(await getNote(client, summary.projectId, id))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not load note')
    }
  }

  async function addNote() {
    if (!client || !projectId) return
    setPreview(false)
    try {
      const note = await createNote(client, { projectId, title: 'Untitled note', content: '' })
      await refresh(note.id)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not create note')
    }
  }

  async function save(patch: Partial<Pick<Note, 'title' | 'content'>>) {
    if (!client || !selected) return
    const optimistic = { ...selected, ...patch }
    setSelected(optimistic)
    setSaving(true)
    try {
      const saved = await updateNote(client, {
        projectId: selected.projectId,
        noteId: selected.id,
        title: optimistic.title.trim() || 'Untitled note',
        content: optimistic.content,
        expectedRevision: selected.revision,
      })
      setSelected(saved)
      autosaveKey.current = `${saved.id}:${saved.title}:${saved.content}`
      setNotes((current) => current.map((note) => note.id === saved.id
        ? { ...note, title: saved.title, preview: noteExcerpt(saved.content), revision: saved.revision, updatedAt: saved.updatedAt }
        : note))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not save note')
      await refresh(selected.id)
    } finally {
      setSaving(false)
    }
  }

  function addNoteToChat(note: Pick<Note, 'projectId' | 'id'>) {
    if (typeof window === 'undefined') return
    const targetSession = window.sessionStorage.getItem('padu.note-target-session') ?? 'new'
    window.sessionStorage.setItem('padu.pending-composer-note', `${targetSession}:${note.projectId}:${note.id}`)
    window.sessionStorage.removeItem('padu.note-target-session')
    setContextMenu(null)
    void navigate({ to: '/', search: { session: targetSession === 'new' ? undefined : targetSession } })
  }

  async function removeNote(noteId = selectedId) {
    if (!client || !noteId) return
    const summary = notes.find((item) => item.id === noteId)
    if (!summary && selected?.id !== noteId) return
    try {
      const note = selected?.id === noteId
        ? selected
        : summary
          ? await getNote(client, summary.projectId, noteId)
          : null
      if (!note || !window.confirm(`Delete “${note.title || 'Untitled note'}”?`)) return
      await deleteNote(client, note.projectId, note.id, note.revision)
      await refresh()
      setContextMenu(null)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not delete note')
    }
  }

  useEffect(() => {
    if (!selected) return
    const key = `${selected.id}:${selected.title}:${selected.content}`
    if (autosaveKey.current === null) {
      autosaveKey.current = key
      return
    }
    if (autosaveKey.current === key) return
    autosaveKey.current = key
    const timer = window.setTimeout(() => void save({
      title: selected.title,
      content: selected.content,
    }), 650)
    return () => window.clearTimeout(timer)
    // save intentionally reads the current selected note when the debounce fires.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected?.id, selected?.title, selected?.content])

  useEffect(() => {
    const close = () => setContextMenu(null)
    window.addEventListener('click', close)
    return () => window.removeEventListener('click', close)
  }, [])

  return (
    <div className="flex h-dvh min-h-0 flex-col bg-background text-foreground">
      <header className="flex h-12 shrink-0 items-center gap-3 px-4">
        <button className="rounded-md p-1.5 outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-label="Back to tasks" onClick={() => navigate({ to: '/', search: { session: undefined } })}>
          <PaduIcon name="arrowLeft" />
        </button>
        <h1 className="text-sm font-semibold">Notes</h1>
        <button className="rounded-md p-1.5 text-[var(--text-tertiary)] outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-label={listCollapsed ? 'Show notes list' : 'Hide notes list'} title={listCollapsed ? 'Show notes list' : 'Hide notes list'} onClick={() => setListCollapsed((value) => !value)}><PaduIcon name="list" /></button>
        <span className="text-xs text-[var(--text-tertiary)]">All projects</span>
        <div className="flex-1" />
      </header>
      <>
        <main className="flex min-h-0 flex-1">
          {!listCollapsed && <aside aria-label="Notes navigation" className="flex w-72 shrink-0 flex-col border-r bg-sidebar p-3">
            <button className="flex h-8 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm text-primary-foreground outline-none hover:opacity-90 focus-visible:ring-2 focus-visible:ring-ring" type="button" onClick={() => void addNote()} disabled={!projectId}><PaduIcon name="plus" /> New Note <kbd className="rounded border border-primary-foreground/30 px-1 text-[11px]">⌘⌥N</kbd></button>
            <div className="mt-2">
              <label className="sr-only" htmlFor="note-search">Search notes</label>
              <div className="flex items-center rounded-md border bg-background px-2 focus-within:ring-2 focus-within:ring-ring"><PaduIcon name="search" className="size-4 text-[var(--text-tertiary)]" /><input id="note-search" className="min-w-0 flex-1 bg-transparent px-2 py-1.5 text-sm outline-none" placeholder="Search notes" value={filter} onChange={(event) => setFilter(event.target.value)} /></div>
            </div>
            <nav className="mt-3 min-h-0 flex-1 space-y-1 overflow-y-auto" aria-label="Notes">
              {loading ? <p className="px-2 py-6 text-center text-sm text-[var(--text-tertiary)]">Loading notes…</p> : visibleNotes.length ? visibleNotes.map((note) => (
                <div key={note.id} className="relative">
                  <button className={`flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring ${selected?.id === note.id ? 'bg-accent' : 'hover:bg-accent/70'}`} type="button" aria-current={selected?.id === note.id ? 'page' : undefined} onContextMenu={(event) => { event.preventDefault(); setContextMenu({ id: note.id, x: event.clientX, y: event.clientY }) }} onClick={() => void selectNote(note.id)}>
                    <span className="grid size-[26px] shrink-0 place-items-center rounded-md bg-muted"><PaduIcon name="file" /></span>
                    <span className="flex min-w-0 flex-1 flex-col gap-px"><span className="block truncate text-[12.5px]">{note.title || 'Untitled note'}</span>{noteExcerpt(note.preview) && <span className="block truncate text-[11px] text-[var(--text-tertiary)]">{noteExcerpt(note.preview)}</span>}<span className="block truncate text-[10px] text-[var(--text-tertiary)]">{note.projectId ? 'Project note' : 'No project'} · Updated {formatNoteTimeAgo(note.updatedAt, t)}</span></span>
                  </button>
                </div>
              )) : <p className="px-2 py-6 text-center text-sm text-[var(--text-tertiary)]">{filter ? 'No matching notes' : 'No notes yet.'}</p>}
            </nav>
          </aside>}
          <section aria-label="Note editor" className="min-w-0 flex-1">
            {selected ? (
              <div className="mx-auto flex h-full max-w-3xl flex-col gap-4 p-6">
                <div className="flex items-center gap-2">
                  <label className="sr-only" htmlFor="note-title">Note title</label>
                  <input id="note-title" className="min-w-0 flex-1 border-0 bg-transparent px-0 text-3xl font-semibold outline-none focus-visible:ring-2 focus-visible:ring-ring" value={selected.title} onChange={(event) => setSelected({ ...selected, title: event.target.value })} />

                </div>
                <div className="flex items-center justify-between gap-3 text-xs text-[var(--text-tertiary)]"><div className="flex min-w-0 items-center gap-2"><button className="flex h-8 items-center gap-1.5 rounded-md bg-black px-3 text-xs font-medium text-white outline-none focus-visible:ring-2 focus-visible:ring-ring" type="button" onClick={() => addNoteToChat(selected)}><PaduIcon name="compose" /> {t('files.add_to_chat')}</button><span className="min-w-0 truncate">Created {formatNoteTimeAgo(selected.createdAt, t)} · {saving ? 'Saving…' : `Updated ${formatNoteTimeAgo(selected.updatedAt, t)}`}</span></div><div className="flex items-center gap-1 rounded-md bg-muted p-1"><button className={`rounded px-2 py-1 outline-none focus-visible:ring-2 focus-visible:ring-ring ${!preview ? 'bg-background shadow-sm' : 'hover:bg-accent'}`} type="button" aria-label="Edit mode" aria-pressed={!preview} onClick={() => setPreview(false)}><PaduIcon name="pencil" /></button><button className={`rounded px-2 py-1 outline-none focus-visible:ring-2 focus-visible:ring-ring ${preview ? 'bg-background shadow-sm' : 'hover:bg-accent'}`} type="button" aria-label="Preview mode" aria-pressed={preview} onClick={() => setPreview(true)}><PaduIcon name="eye" /></button><button className="rounded px-2 py-1 text-destructive outline-none hover:bg-destructive/10 focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-label="Delete note" onClick={() => void removeNote()}><PaduIcon name="trash" /></button></div></div>
                {preview ? (
                  <article className="markdown min-h-full min-w-0 flex-1 overflow-x-hidden overflow-y-auto break-words px-2 text-[15px] leading-7">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{selected.content}</ReactMarkdown>
                  </article>
                ) : (
                  <>
                    <label className="sr-only" htmlFor="note-body">Note body</label>
                    <textarea id="note-body" className="min-h-0 flex-1 resize-none bg-transparent text-[15px] leading-7 outline-none focus-visible:ring-2 focus-visible:ring-ring" placeholder="Write a note…" value={selected.content} onChange={(event) => setSelected({ ...selected, content: event.target.value })} />
                  </>
                )}
                <div className="flex flex-wrap items-center gap-2 border-t pt-3 text-xs text-[var(--text-tertiary)]"><span>Use <kbd className="rounded border px-1">/note</kbd> in the composer to search and embed notes.</span></div>
              </div>
            ) : <div className="grid h-full place-items-center text-sm text-[var(--text-tertiary)]">Create a note to get started.</div>}
          </section>
        </main>
        {contextMenu && <div className="fixed z-50 min-w-36 rounded-md border bg-popover p-1 shadow-lg" style={{ left: contextMenu.x, top: contextMenu.y }} onClick={(event) => event.stopPropagation()}><button className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring" type="button" onClick={() => { const note = notes.find((item) => item.id === contextMenu.id); if (note) addNoteToChat(note) }}><PaduIcon name="compose" /> {t('files.add_to_chat')}</button><button className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm text-destructive outline-none hover:bg-destructive/10 focus-visible:ring-2 focus-visible:ring-ring" type="button" onClick={() => void removeNote(contextMenu.id)}><PaduIcon name="trash" /> Delete</button></div>}
      </>
    </div>
  )
}
