import { useEffect, useRef } from 'react'
import type { AgentSession } from '@padu/client'
import { Button } from '@/components/ui/button'
import { PaduIcon, type PaduIconName } from '@/components/padu-icon'
import { useI18n } from '@/lib/i18n'
import { useRuntime, type BackgroundWorkItem, type BackgroundWorkKey, type BackgroundWorkStatus } from '@/lib/runtime-context'
import { formatWorkingElapsed, type Translator } from '@/lib/transcript-presentation'
import { cn } from '@/lib/utils'
import { PanelMessage } from './shared'

export function BackgroundWorkPanel({
  session,
  workKey,
  onTitle,
}: {
  session: AgentSession | null
  workKey: BackgroundWorkKey
  onTitle: (title: string) => void
}) {
  const { t } = useI18n()
  const { backgroundWork, stopBackgroundWork } = useRuntime()
  const reportedTitle = useRef<string | null>(null)
  const item = session
    ? backgroundWork[session.id]?.find((candidate) => sameBackgroundWorkKey(candidate.key, workKey))
    : undefined

  useEffect(() => {
    if (!item?.title) return
    const reportKey = `${item.key.kind}:${item.key.providerId}:${item.title}`
    if (reportedTitle.current === reportKey) return
    reportedTitle.current = reportKey
    onTitle(item.title)
  }, [item?.title, onTitle])

  if (!session || !item) {
    return (
      <div className="grid min-h-0 flex-1 place-items-center p-6 text-center">
        <div>
          <PaduIcon className="mx-auto size-[22px] text-[var(--text-ghost)]" name={backgroundWorkKindIcon(workKey.kind)} />
          <p className="mt-2 text-[12px] text-[var(--text-secondary)]">{t('background.unavailable')}</p>
        </div>
      </div>
    )
  }

  const stoppable = isStoppableBackgroundStatus(item.status) && item.canStop && item.controlId
  const metadata = [
    [t(item.key.kind === 'subagent' ? 'background.prompt' : 'background.command'), item.command],
    [t('background.cwd'), item.cwd],
    [t('background.role'), item.role],
    [t('background.model'), item.model],
    [t('background.latest_update'), item.detail],
    [t('background.exit_code'), item.exitCode == null ? null : String(item.exitCode)],
  ].filter((entry): entry is [string, string] => Boolean(entry[1]))
  return (
    <div className="min-h-0 flex-1 overflow-auto p-3">
      <div className="overflow-hidden rounded-[9px] border bg-card">
        <div className="flex min-h-[54px] items-center gap-2.5 px-[11px] py-2">
          <PaduIcon className="size-[15px] text-[var(--text-secondary)]" name={backgroundWorkKindIcon(item.key.kind)} />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[12px] font-medium">{item.title}</div>
            <div className="mt-1 flex items-center gap-1.5 text-[10px] text-[var(--text-tertiary)]">
              <BackgroundStatusIcon status={item.status} />
              <span>{backgroundStatusLabel(item.status, t)}</span>
              <span>·</span>
              <span>{backgroundElapsed(item, t)}</span>
            </div>
          </div>
          {stoppable && (
            <Button
              className="h-[26px] gap-1.5 px-[9px] text-[10.5px] hover:bg-destructive/10"
              size="sm"
              variant="outline"
              onClick={() => void stopBackgroundWork(session.id, item).catch(() => {})}
            >
              <PaduIcon className="size-[11px] text-destructive" name="stopFilled" />
              {t('background.stop')}
            </Button>
          )}
        </div>
        {metadata.map(([label, value]) => (
          <div className="border-t px-2.5 py-[7px]" key={label}>
            <div className="text-[9.5px] text-[var(--text-tertiary)]">{label}</div>
            <div className="mt-[3px] whitespace-pre-wrap break-words font-mono text-[10.5px] text-[var(--text-secondary)]">{value}</div>
          </div>
        ))}
        <div className="border-t p-2.5">
          <div className="mb-[5px] flex items-center justify-between text-[9.5px] text-[var(--text-tertiary)]">
            <span>{t('background.output')}</span>
            {item.outputTruncated && <span>{t('background.output_truncated')}</span>}
          </div>
          <pre className="max-h-80 overflow-auto rounded-md bg-[var(--inset)] p-2 font-mono text-[10.5px] leading-[15px] text-[var(--text-secondary)]">
            {stripAnsi(item.output || t('background.no_output'))}
          </pre>
        </div>
      </div>
    </div>
  )
}

function BackgroundStatusIcon({ status }: { status: BackgroundWorkStatus }) {
  const icon: PaduIconName = status === 'completed'
    ? 'check'
    : status === 'failed'
      ? 'x'
      : status === 'lost'
        ? 'alert'
        : status === 'stopping' || status === 'stopped'
          ? 'stop'
          : 'loaderCircle'
  return (
    <PaduIcon
      className={cn(
        'size-[9px]',
        isStoppableBackgroundStatus(status) && 'motion-safe:animate-spin text-ring',
        status === 'completed' && 'text-[var(--success)]',
        (status === 'failed' || status === 'lost') && 'text-destructive',
      )}
      name={icon}
    />
  )
}

export function sameBackgroundWorkKey(left: BackgroundWorkKey, right: BackgroundWorkKey) {
  return left.kind === right.kind && left.providerId === right.providerId
}

export function backgroundWorkKindIcon(kind?: BackgroundWorkKey['kind']): PaduIconName {
  return kind === 'subagent' ? 'bot' : 'terminalSquare'
}

export function backgroundWorkKindLabel(kind: BackgroundWorkKey['kind'] | undefined, t: Translator) {
  return t(kind === 'subagent'
    ? 'background.subagent'
    : kind === 'monitor'
      ? 'background.monitor'
      : 'background.process')
}

export function isStoppableBackgroundStatus(status: BackgroundWorkStatus) {
  return status === 'starting' || status === 'running' || status === 'monitoring'
}

export function backgroundStatusLabel(status: BackgroundWorkStatus, t: Translator) {
  return t(`background.status.${status}`)
}

export function backgroundElapsed(item: BackgroundWorkItem, t: Translator) {
  const duration = item.durationMs ?? Math.max(0, Date.now() - item.startedAtMs)
  return formatWorkingElapsed(Math.floor(duration / 1_000), t)
}

export function stripAnsi(value: string) {
  return value.replace(new RegExp('\\u001b\\[[0-?]*[ -/]*[@-~]', 'g'), '')
}
