import { useQueryClient } from '@tanstack/react-query'
import type { Project } from '@padu/client'
import { useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { DeleteSessionDialog } from '@/components/delete-session-dialog'
import { PaduIcon } from '@/components/padu-icon'
import { useTaskState } from '@/hooks/use-daemon-data'
import {
  daemonKeys,
  displayTitle,
  removeSession,
  setSessionArchived,
} from '@/lib/daemon-api'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { projectDisplayName } from '@/lib/project-presentation'
import { errorMessage } from './shared'

export function ArchivedSettings({
  projects,
  onRestoreSession,
}: {
  projects: Project[]
  onRestoreSession?: (sessionId: string) => void
}) {
  const { t } = useI18n()
  const { client, config } = useDaemon()
  const queryClient = useQueryClient()
  const taskState = useTaskState()
  const [query, setQuery] = useState('')
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  const searchRef = useRef<HTMLInputElement>(null)

  const archived = useMemo(() => {
    const needle = query.trim().toLowerCase()
    const sessions = taskState.data?.sessions.filter((session) => Boolean(session.archived_at)) ?? []
    const filtered = needle
      ? sessions.filter((session) => {
        const project = projects.find((candidate) => candidate.id === session.project_id)
        const projectName = project ? projectDisplayName(project, t('project.no_project_name')) : ''
        const projectPath = project?.path ?? ''
        return displayTitle(session).toLowerCase().includes(needle)
          || session.id.toLowerCase().includes(needle)
          || projectName.toLowerCase().includes(needle)
          || projectPath.toLowerCase().includes(needle)
      })
      : sessions
    return filtered
      .slice()
      .sort((left, right) => (right.archived_at ?? 0) - (left.archived_at ?? 0))
  }, [projects, query, t, taskState.data?.sessions])

  const empty = archived.length === 0
  const hasQuery = query.trim().length > 0
  const pendingDelete = pendingDeleteId
    ? archived.find((session) => session.id === pendingDeleteId)
      ?? taskState.data?.sessions.find((session) => session.id === pendingDeleteId)
      ?? null
    : null

  async function restore(sessionId: string) {
    if (!client || !config) return
    try {
      await setSessionArchived(client, sessionId, false)
      queryClient.setQueryData(daemonKeys.taskState(config.address), (current: typeof taskState.data) => current && ({
        ...current,
        sessions: current.sessions.map((session) => session.id === sessionId
          ? { ...session, archived_at: null }
          : session),
      }))
      onRestoreSession?.(sessionId)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  async function confirmDelete(sessionId: string) {
    if (!client || !config) return
    try {
      const next = await removeSession(client, sessionId)
      queryClient.setQueryData(daemonKeys.taskState(config.address), next)
      queryClient.removeQueries({ queryKey: daemonKeys.session(config.address, sessionId) })
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setPendingDeleteId(null)
    }
  }

  return (
    <div className="text-[13px] text-[var(--text-secondary)]">
      <p>{t('settings.archived_description')}</p>
      <label className="mt-3 flex h-8 items-center gap-2 rounded-lg border bg-[var(--inset)] px-2.5 focus-within:border-ring">
        <PaduIcon className="size-[13px] shrink-0 text-[var(--text-tertiary)]" name="search" />
        <input
          ref={searchRef}
          aria-label={t('settings.archived_search')}
          className="min-w-0 flex-1 bg-transparent text-[12px] text-foreground outline-none placeholder:text-[var(--text-ghost)]"
          placeholder={t('settings.archived_search')}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape' && query) {
              event.stopPropagation()
              setQuery('')
            }
          }}
        />
        {query && (
          <button
            aria-label={t('common.clear')}
            className="grid size-5 shrink-0 place-items-center rounded-md text-[var(--text-tertiary)] outline-none hover:bg-accent hover:text-foreground focus-visible:ring-1 focus-visible:ring-ring"
            type="button"
            onClick={() => {
              setQuery('')
              searchRef.current?.focus()
            }}
          >
            <PaduIcon className="size-3" name="x" />
          </button>
        )}
      </label>
      {empty ? (
        <p className="mt-4 text-[13px] text-[var(--text-tertiary)]">
          {hasQuery ? t('settings.archived_no_matches') : t('settings.archived_empty')}
        </p>
      ) : (
        <div className="mt-4 flex flex-col gap-[1px]">
          {archived.map((session) => {
            const project = projects.find((candidate) => candidate.id === session.project_id)
            const projectName = project
              ? projectDisplayName(project, t('project.no_project_name'))
              : t('project.no_project_name')
            const projectPath = project?.path ?? ''
            return (
              <div
                className="flex min-h-[66px] w-full items-center gap-2.5 rounded-[7px] px-3 py-[7px] hover:bg-sidebar-accent"
                key={session.id}
              >
                <PaduIcon className="size-[14px] shrink-0 text-[var(--text-tertiary)]" name="archive" />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] font-medium text-foreground">{displayTitle(session)}</div>
                  <div className="truncate text-[11px] text-[var(--text-secondary)]">
                    {t('settings.archived_project')} · {projectName}
                  </div>
                  <div className="truncate text-[10px] text-[var(--text-tertiary)]" title={projectPath || session.id}>
                    {projectPath ? `${t('settings.archived_project_path')} · ${projectPath}` : session.id}
                  </div>
                </div>
                <button
                  className="shrink-0 rounded-[5px] px-[9px] py-[5px] text-[12px] text-[var(--text-secondary)] outline-none hover:bg-[var(--overlay)] focus-visible:ring-1 focus-visible:ring-ring"
                  type="button"
                  onClick={() => void restore(session.id)}
                >
                  {t('settings.restore')}
                </button>
                <button
                  className="shrink-0 rounded-[5px] px-[9px] py-[5px] text-[12px] text-destructive outline-none hover:bg-destructive/12 focus-visible:ring-1 focus-visible:ring-ring"
                  type="button"
                  onClick={() => setPendingDeleteId(session.id)}
                >
                  {t('common.remove')}
                </button>
              </div>
            )
          })}
        </div>
      )}
      <DeleteSessionDialog
        session={pendingDelete ? { id: pendingDelete.id, title: displayTitle(pendingDelete) } : null}
        onClose={() => setPendingDeleteId(null)}
        onConfirm={(sessionId) => void confirmDelete(sessionId)}
      />
    </div>
  )
}
