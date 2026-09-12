import type { HostProfile } from '@padu/client'
import { useState } from 'react'
import { HostDialog } from '@/components/host-dialog'
import { PaduIcon } from '@/components/padu-icon'
import { Button } from '@/components/ui/button'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { cn } from '@/lib/utils'
import { DetailRow, SettingsCard, SettingText, errorMessage, formatHostLastConnected } from './shared'

export function RemoteHostsCard() {
  const { t } = useI18n()
  const { hosts, activeHostId, switchHost, addHost, updateHost, removeHost } = useDaemon()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingHost, setEditingHost] = useState<HostProfile | null>(null)

  return (
    <SettingsCard>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="text-[13.5px] font-medium text-foreground">{t('host.remote_hosts')}</div>
          <div className="mt-1 text-[12.5px] leading-[18px] text-[var(--text-secondary)]">
            {t('host.remote_hosts_description')}
          </div>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="shrink-0 gap-1.5"
          onClick={() => {
            setEditingHost(null)
            setDialogOpen(true)
          }}
        >
          <PaduIcon className="size-3 text-[var(--text-secondary)]" name="plus" />
          {t('host.add_host')}
        </Button>
      </div>

      {hosts.length === 0 ? (
        <div className="mt-3.5 flex flex-col items-center justify-center gap-2 rounded-lg border border-border bg-[var(--overlay)]/40 px-4 py-6 text-center">
          <div className="flex size-9 items-center justify-center rounded-full bg-[var(--overlay)]">
            <PaduIcon className="size-4.5 text-[var(--text-tertiary)]" name="server" />
          </div>
          <p className="text-[13px] font-medium text-[var(--text-secondary)]">{t('host.no_remote_hosts')}</p>
        </div>
      ) : (
        <div className="mt-3.5 flex flex-col gap-2.5">
          {hosts.map((host) => {
            const isActive = activeHostId === host.id
            const hasToken = Boolean(host.token && host.token.trim().length > 0)
            const lastConnLabel = formatHostLastConnected(host.lastConnectedAt, t)

            return (
              <div
                key={host.id}
                className={cn(
                  'flex items-center justify-between gap-3 rounded-lg border bg-card p-3.5 transition-colors',
                  isActive ? 'border-ring ring-1 ring-ring/20' : 'border-border hover:border-border-strong',
                )}
              >
                <div className="flex min-w-0 flex-1 items-center gap-3">
                  <div className="relative flex size-9 shrink-0 items-center justify-center rounded-lg bg-[var(--overlay)]">
                    <PaduIcon
                      className={cn('size-4', isActive ? 'text-ring' : 'text-[var(--text-secondary)]')}
                      name="server"
                    />
                    <span
                      className={cn(
                        'absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-card',
                        isActive ? 'bg-[var(--success)]' : 'bg-[var(--text-ghost)]',
                      )}
                    />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[13.5px] font-medium text-foreground">
                        {host.name || host.address}
                      </span>
                      {isActive && (
                        <span className="rounded bg-[var(--success-soft)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--success)]">
                          {t('host.active')}
                        </span>
                      )}
                    </div>
                    <div className="truncate font-mono text-[12px] text-[var(--text-secondary)]">
                      {host.address}
                    </div>
                    <div className="mt-0.5 flex items-center gap-1.5 text-[11.5px] text-[var(--text-tertiary)]">
                      <span className="inline-flex items-center gap-1">
                        <PaduIcon className="size-2.5" name="lock" />
                        {hasToken ? t('host.authenticated') : t('host.no_auth')}
                      </span>
                      <span>·</span>
                      <span>{lastConnLabel}</span>
                    </div>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {!isActive && (
                    <Button
                      size="sm"
                      variant="default"
                      onClick={() => void switchHost(host.id).catch(() => {})}
                    >
                      {t('host.connect')}
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="outline"
                    className="gap-1 text-[var(--text-secondary)]"
                    onClick={() => {
                      setEditingHost(host)
                      setDialogOpen(true)
                    }}
                  >
                    <PaduIcon className="size-3 text-[var(--text-tertiary)]" name="pencil" />
                    {t('common.edit')}
                  </Button>
                </div>
              </div>
            )
          })}
        </div>
      )}

      <HostDialog
        editingHost={editingHost}
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onDelete={async (id) => {
          await removeHost(id)
        }}
        onSave={async (data) => {
          if (editingHost) {
            await updateHost(editingHost.id, data)
            if (activeHostId === editingHost.id) {
              await switchHost(editingHost.id)
            }
          } else {
            const created = await addHost(data)
            await switchHost(created.id)
          }
        }}
      />
    </SettingsCard>
  )
}

export function DaemonSettings() {
  const { t } = useI18n()
  const { config, phase, reconnect, disconnect, forget } = useDaemon()
  const [error, setError] = useState<string | null>(null)
  return (
    <div>
      <RemoteHostsCard />
      <SettingsCard>
        <SettingText title={t('daemon.credentials_title')} description={t('daemon.web_connection_description')} />
        <div className="mt-4 divide-y rounded-xl border bg-background px-3">
          <DetailRow label={t('daemon.websocket_url')} value={config?.address ?? t('daemon.not_configured')} copy />
          <DetailRow
            copy={Boolean(config?.token)}
            label={t('daemon.token')}
            secret={Boolean(config?.token)}
            value={config?.token ?? t('daemon.not_configured')}
          />
          <DetailRow label={t('daemon.status')} value={t(`daemon.phase_${phase}`)} />
        </div>
        {error && <p className="mt-3 text-[11.5px] text-destructive">{error}</p>}
        <div className="mt-4 flex flex-wrap justify-end gap-2">
          <Button variant="ghost" onClick={disconnect}>{t('daemon.disconnect')}</Button>
          <Button variant="destructive" onClick={forget}>{t('daemon.forget')}</Button>
          <Button
            disabled={phase === 'connecting'}
            onClick={() => {
              setError(null)
              void reconnect().catch((cause) => setError(errorMessage(cause)))
            }}
          >
            {t('daemon.reconnect')}
          </Button>
        </div>
      </SettingsCard>
    </div>
  )
}
