import { useNavigate, useSearch } from '@tanstack/react-router'
import type { Note, NoteSummary, PaduClient } from '@padu/client'
import { useEffect, useMemo, useState } from 'react'
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

function noteExcerpt(preview: string): string {
  const text = preview.replace(/\s+/gu, ' ').trim()
  return text.length <= 140 ? text : `${text.slice(0, 139)}…`
}

export async function addContentToNote(
  client: PaduClient,
  projectId: string | undefined,
  content: string,
): Promise<void> {
  if (!projectId) throw new Error('Select a project before adding a response to Notes')
  const summaries = await listNotes(client, projectId)
  const summary = summaries[0]
  const current = summary ? await getNote(client, projectId, summary.id) : null
  if (current) {
    await updateNote(client, {
      projectId,
      noteId: current.id,
      title: current.title,
      content: `${current.content}${current.content ? '\n\n' : ''}${content.trim()}`,
      expectedRevision: current.revision,
    })
  } else {
    await createNote(client, {
      projectId,
      title: 'Assistant notes',
      content: content.trim(),
    })
  }
}

function timestamp(value: number): string {
  if (!value) return '—'
  return new Date(value * 1000).toLocaleString()
}

export function NotesPage() {
  const navigate = useNavigate()
  const search = useSearch({ from: '/notes' })
  const { client } = useDaemon()
  const projectId = search.projectId
  const [notes, setNotes] = useState<NoteSummary[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selected, setSelected] = useState<Note | null>(null)
  const [filter, setFilter] = useState(search.q ?? '')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [preview, setPreview] = useState(false)

  async function refresh(preferredId?: string) {
    if (!client || !projectId) {
      setNotes([])
      setSelected(null)
      setLoading(false)
      return
    }
    setLoading(true)
    try {
      const next = await listNotes(client, projectId)
      setNotes(next)
      const id = preferredId ?? selectedId ?? next[0]?.id ?? null
      setSelectedId(id)
      setSelected(id ? await getNote(client, projectId, id) : null)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not load notes')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void refresh()
    // The daemon client and project identify this resource; refresh when either changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, projectId])

  const visibleNotes = useMemo(() => {
    const query = filter.trim().toLocaleLowerCase()
    return notes.filter((note) => !query || `${note.title} ${note.preview}`.toLocaleLowerCase().includes(query))
  }, [filter, notes])

  async function selectNote(id: string) {
    if (!client || !projectId) return
    setSelectedId(id)
    try {
      setSelected(await getNote(client, projectId, id))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not load note')
    }
  }

  async function addNote() {
    if (!client || !projectId) return
    try {
      const note = await createNote(client, { projectId, title: 'Untitled note', content: '' })
      await refresh(note.id)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not create note')
    }
  }

  async function save(patch: Partial<Pick<Note, 'title' | 'content'>>) {
    if (!client || !projectId || !selected) return
    const optimistic = { ...selected, ...patch }
    setSelected(optimistic)
    setSaving(true)
    try {
      const saved = await updateNote(client, {
        projectId,
        noteId: selected.id,
        title: optimistic.title.trim() || 'Untitled note',
        content: optimistic.content,
        expectedRevision: selected.revision,
      })
      setSelected(saved)
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

  async function removeNote() {
    if (!client || !projectId || !selected) return
    if (!window.confirm(`Delete “${selected.title}”?`)) return
    try {
      await deleteNote(client, projectId, selected.id, selected.revision)
      await refresh()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Could not delete note')
    }
  }

  return (
    <div className="flex h-dvh min-h-0 flex-col bg-background text-foreground">
      <header className="flex h-12 shrink-0 items-center gap-3 border-b px-4">
        <button className="rounded-md p-1.5 outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-label="Back to tasks" onClick={() => navigate({ to: '/', search: { session: undefined } })}>
          <PaduIcon name="arrowLeft" />
        </button>
        <h1 className="text-sm font-semibold">Notes</h1>
        {projectId && <span className="text-xs text-[var(--text-tertiary)]">Project notes</span>}
      </header>
      {!projectId ? (
        <div className="grid flex-1 place-items-center p-6 text-center text-sm text-[var(--text-tertiary)]">Select a project before opening Notes.</div>
      ) : (
        <main className="flex min-h-0 flex-1">
          <aside aria-label="Notes navigation" className="flex w-72 shrink-0 flex-col border-r bg-sidebar p-3">
            <div className="flex gap-2">
              <label className="sr-only" htmlFor="note-search">Search notes</label>
              <input id="note-search" className="min-w-0 flex-1 rounded-md border bg-background px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring" placeholder="Search notes" value={filter} onChange={(event) => setFilter(event.target.value)} />
              <button className="grid size-9 shrink-0 place-items-center rounded-md bg-primary text-primary-foreground outline-none hover:opacity-90 focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-label="New note" onClick={() => void addNote()}><PaduIcon name="plus" /></button>
            </div>
            <nav className="mt-3 min-h-0 flex-1 space-y-1 overflow-y-auto" aria-label="Notes">
              {loading ? <p className="px-2 py-6 text-center text-sm text-[var(--text-tertiary)]">Loading notes…</p> : visibleNotes.length ? visibleNotes.map((note) => (
                <button key={note.id} className={`w-full rounded-md px-3 py-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring ${selected?.id === note.id ? 'bg-accent' : 'hover:bg-accent/70'}`} type="button" onClick={() => void selectNote(note.id)}>
                  <div className="truncate text-sm font-medium">{note.title || 'Untitled note'}</div>
                  <div className="mt-0.5 truncate text-xs text-[var(--text-tertiary)]">{noteExcerpt(note.preview) || 'Empty note'}</div>
                </button>
              )) : <p className="px-2 py-6 text-center text-sm text-[var(--text-tertiary)]">No notes yet.</p>}
            </nav>
          </aside>
          <section aria-label="Note editor" className="min-w-0 flex-1">
            {selected ? (
              <div className="mx-auto flex h-full max-w-3xl flex-col gap-4 p-6">
                <div className="flex items-center gap-2">
                  <label className="sr-only" htmlFor="note-title">Note title</label>
                  <input id="note-title" className="min-w-0 flex-1 bg-transparent text-2xl font-semibold outline-none focus-visible:ring-2 focus-visible:ring-ring" value={selected.title} onChange={(event) => setSelected({ ...selected, title: event.target.value })} onBlur={() => void save({ title: selected.title })} />
                  <button className="rounded-md p-2 text-[var(--text-tertiary)] outline-none hover:bg-destructive/10 hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-label="Delete note" onClick={() => void removeNote()}><PaduIcon name="trash" /></button>
                </div>
                <div className="flex items-center gap-2 text-xs text-[var(--text-tertiary)]"><span>Project note</span><span>·</span><span>Created {timestamp(selected.createdAt)}</span><span>·</span><span>{saving ? 'Saving…' : `Updated ${timestamp(selected.updatedAt)}`}</span></div>
                <div className="flex items-center gap-2">
                  <button className="rounded-md border px-2.5 py-1.5 text-xs outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-pressed={!preview} onClick={() => setPreview(false)}>Edit</button>
                  <button className="rounded-md border px-2.5 py-1.5 text-xs outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring" type="button" aria-pressed={preview} onClick={() => setPreview(true)}>Preview</button>
                </div>
                {preview ? (
                  <article className="markdown min-h-0 flex-1 overflow-y-auto text-[15px] leading-7">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{selected.content}</ReactMarkdown>
                  </article>
                ) : (
                  <>
                    <label className="sr-only" htmlFor="note-body">Note body</label>
                    <textarea id="note-body" className="min-h-0 flex-1 resize-none bg-transparent text-[15px] leading-7 outline-none focus-visible:ring-2 focus-visible:ring-ring" placeholder="Write a note…" value={selected.content} onChange={(event) => setSelected({ ...selected, content: event.target.value })} onBlur={() => void save({ content: selected.content })} />
                  </>
                )}
                <div className="flex flex-wrap items-center gap-2 border-t pt-3 text-xs text-[var(--text-tertiary)]"><span>Use <kbd className="rounded border px-1">/note</kbd> in the composer to search and embed notes.</span></div>
              </div>
            ) : <div className="grid h-full place-items-center text-sm text-[var(--text-tertiary)]">Create a note to get started.</div>}
          </section>
        </main>
      )}
    </div>
  )
}
