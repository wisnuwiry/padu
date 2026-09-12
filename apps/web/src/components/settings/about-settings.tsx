import { useState } from 'react'
import { PaduIcon } from '@/components/padu-icon'
import { Button } from '@/components/ui/button'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { SettingsCard } from './shared'
import type { SettingsPageId } from './types'

export function AboutSettings({
  onPageChange,
}: {
  onPageChange: (page: SettingsPageId) => void
}) {
  const { t } = useI18n()
  const { activeHost, config } = useDaemon()
  const [checkingUpdate, setCheckingUpdate] = useState(false)
  const [updateResult, setUpdateResult] = useState<string | null>(null)

  const currentVersion = '0.1.1'

  const hostName = activeHost
    ? activeHost.name || activeHost.address
    : t('about.local_daemon')
  const hostAddress = activeHost
    ? activeHost.address
    : config?.address || 'ws://127.0.0.1:47319'
  const isRemote = Boolean(activeHost)

  return (
    <div className="flex flex-col gap-3">
      {/* Unified About + Version + Updates Card */}
      <div className="mt-[15px] flex w-full items-center justify-between gap-4 rounded-[13px] bg-[var(--raised)] p-5">
        <div className="flex min-w-0 flex-1 items-center gap-4">
          <div className="flex size-[60px] shrink-0 items-center justify-center rounded-[15px] border border-border/40 bg-black text-white">
            <PaduIcon className="size-9" name="logo" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="text-[16px] font-semibold text-foreground">Padu</span>
              <span className="rounded-md border bg-background px-2 py-0.5 font-mono text-[11px] font-medium text-[var(--text-secondary)]">
                v{currentVersion}
              </span>
            </div>
            <div className="mt-1 text-[12px] leading-[17px] text-[var(--text-secondary)]">
              {t('about.tagline')}
            </div>
          </div>
        </div>
      </div>

      {/* Connected Host Card */}
      <SettingsCard>
        <div className='flex flex-row items-center gap-2'>
          <div className="flex min-w-0 flex-1 items-center gap-3">
            <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border bg-background text-[var(--primary)]">
              <PaduIcon className="size-4" name="server" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="truncate text-[13.5px] font-medium text-foreground">
                  {hostName}
                </span>
                <span className="rounded bg-[var(--success-soft)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--success)]">
                  {isRemote ? t('about.remote_host') : t('about.local_daemon')}
                </span>
              </div>
              <div className="truncate font-mono text-[12px] text-[var(--text-secondary)]">
                {hostAddress}
              </div>
            </div>
          </div>
          <Button
            size="sm"
            variant="outline"
            className="gap-1.5 text-[12px] text-[var(--text-secondary)]"
            onClick={() => onPageChange('daemon')}
          >
            {t('about.manage_hosts')}
            <PaduIcon className="size-3 text-[var(--text-tertiary)]" name="arrowRight" />
          </Button>
        </div>
      </SettingsCard>

      {/* Website, GitHub & Sponsor single row of plain buttons, aligned center */}
      <div className="flex items-center justify-center gap-2 pt-0.5">
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={() => window.open('https://padu.dev', '_blank')}
        >
          <PaduIcon className="size-3.5" name="globe" />
          {t('about.website')}
          <PaduIcon className="size-2.5 text-[var(--text-tertiary)]" name="arrowUpRight" />
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={() => window.open('https://github.com/wisnuwiry/padu', '_blank')}
        >
          <PaduIcon className="size-3.5" name="github" />
          GitHub
          <PaduIcon className="size-2.5 text-[var(--text-tertiary)]" name="arrowUpRight" />
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={() => window.open('https://github.com/sponsors/wisnuwiry', '_blank')}
        >
          <PaduIcon className="size-3.5 text-red-500" name="heart" />
          Sponsor
          <PaduIcon className="size-2.5 text-[var(--text-tertiary)]" name="arrowUpRight" />
        </Button>
      </div>
    </div>
  )
}
