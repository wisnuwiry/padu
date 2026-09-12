import { useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { PaduIcon } from '@/components/padu-icon'
import { useCopyFeedback } from '@/hooks/use-copy-feedback'
import { useI18n } from '@/lib/i18n'
import { cn } from '@/lib/utils'

export type Translator = (key: string, params?: Record<string, string | number>) => string

export function SettingsCard({ children, row = false }: { children: ReactNode; row?: boolean }) {
  return (
    <section className={cn('mt-[15px] w-full rounded-[13px] bg-[var(--raised)] px-5 py-[14px]', row && 'flex items-center gap-6')}>
      {children}
    </section>
  )
}

export function SettingText({ title, description }: { title: string; description: string }) {
  return (
    <div className="min-w-0 flex-1">
      <div className="text-[13.5px] font-medium">{title}</div>
      <p className="mt-[5px] text-[12.5px] leading-[18px] text-[var(--text-secondary)]">{description}</p>
    </div>
  )
}

export function Toggle({ checked, label, onChange }: { checked: boolean; label: string; onChange: (checked: boolean) => void }) {
  return (
    <button
      aria-checked={checked}
      aria-label={label}
      className={cn(
        'flex h-5 w-9 shrink-0 items-center rounded-full border p-0.5 outline-none transition-colors focus-visible:ring-1 focus-visible:ring-ring',
        checked ? 'justify-end border-foreground bg-foreground' : 'justify-start border-input bg-[var(--inset)]',
      )}
      role="switch"
      type="button"
      onClick={() => onChange(!checked)}
    >
      <span className={cn('size-3.5 rounded-full', checked ? 'bg-background' : 'bg-[var(--text-tertiary)]')} />
    </button>
  )
}

export function DetailRow({
  label,
  value,
  copy = false,
  secret = false,
}: {
  label: string
  value: string
  copy?: boolean
  secret?: boolean
}) {
  const { t } = useI18n()
  const copyFeedback = useCopyFeedback()
  const [revealed, setRevealed] = useState(false)
  return (
    <div className="flex min-h-12 items-center gap-4 text-[11.5px]">
      <span className="w-28 shrink-0 text-[var(--text-tertiary)]">{label}</span>
      <span className="min-w-0 flex-1 truncate font-mono">
        {secret && !revealed ? '••••••••••••••••••••••••' : value}
      </span>
      {secret && (
        <Button
          aria-label={t(revealed ? 'daemon.hide_token' : 'daemon.reveal_token')}
          aria-pressed={revealed}
          size="icon-sm"
          title={t(revealed ? 'daemon.hide_token' : 'daemon.reveal_token')}
          type="button"
          variant="outline"
          onClick={() => setRevealed((current) => !current)}
        >
          <PaduIcon name={revealed ? 'eyeOff' : 'eye'} />
        </Button>
      )}
      {copy && (
        <Button size="sm" variant="outline" onClick={() => void copyFeedback.copyText(value)}>
          <PaduIcon name={copyFeedback.copied ? 'check' : 'copy'} />
          {t(copyFeedback.copied ? 'common.copied' : 'common.copy')}
        </Button>
      )}
    </div>
  )
}

export function useStoredBoolean(key: string, fallback: boolean) {
  const [value, setValue] = useState(() => typeof window === 'undefined' ? fallback : window.localStorage.getItem(key) !== 'false')
  const update = (next: boolean) => {
    setValue(next)
    window.localStorage.setItem(key, String(next))
  }
  return [value, update] as const
}

export function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

export function abbreviateHomePath(path: string) {
  return path
    .replace(/^\/Users\/[^/]+(?=\/|$)/, '~')
    .replace(/^\/home\/[^/]+(?=\/|$)/, '~')
    .replace(/^\/root(?=\/|$)/, '~')
}

export function formatHostLastConnected(
  lastConnectedAt: number | null | undefined,
  t: (key: string, params?: Record<string, string | number>) => string,
) {
  if (!lastConnectedAt) {
    return t('host.never_connected')
  }
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - lastConnectedAt))
  let timeStr: string
  if (seconds < 90) {
    timeStr = t('providers.checked_just_now')
  } else if (seconds < 3600) {
    timeStr = t('providers.checked_minutes_ago', { count: Math.floor(seconds / 60) })
  } else if (seconds < 86400) {
    timeStr = t('providers.checked_hours_ago', { count: Math.floor(seconds / 3600) })
  } else {
    timeStr = `${Math.floor(seconds / 86400)}d ago`
  }
  return t('host.last_connected', { time: timeStr })
}
