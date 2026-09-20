import type { HostKind, HostProfile } from '@padu/client'
import { useEffect, useState } from 'react'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Kbd } from '@/components/ui/kbd'
import { PaduIcon } from '@/components/padu-icon'
import { useI18n } from '@/lib/i18n'
import { normalizeDaemonAddress } from '@/lib/connection'
import { WEB_EDITABLE_KINDS, transportLabelKey } from '@/lib/daemon-transport'
import { cn } from '@/lib/utils'

export function HostDialog({
  open,
  editingHost,
  onOpenChange,
  onSave,
  onDelete,
}: {
  open: boolean
  editingHost: HostProfile | null
  onOpenChange: (open: boolean) => void
  onSave: (data: {
    name: string
    address: string
    token?: string
    kind?: HostKind
  }) => Promise<void>
  onDelete?: (id: string) => Promise<void>
}) {
  const { t } = useI18n()
  const [name, setName] = useState('')
  const [address, setAddress] = useState('')
  const [token, setToken] = useState('')
  const [kind, setKind] = useState<HostKind>('direct')
  const [tokenRevealed, setTokenRevealed] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [confirmDeleteOpen, setConfirmDeleteOpen] = useState(false)

  useEffect(() => {
    if (!open) return
    setName(editingHost?.name ?? '')
    setAddress(editingHost?.address ?? '')
    setToken(editingHost?.token ?? '')
    setKind(editingHost?.kind ?? 'direct')
    setTokenRevealed(false)
    setError(null)
    setBusy(false)
    setConfirmDeleteOpen(false)
  }, [open, editingHost])

  // A relay host was provisioned by the desktop. Editing it here would let the
  // user save an address the desktop's tunnel no longer backs, so the selector
  // shows the transport read-only instead.
  const isRelayHost = kind === 'cloudflare' || kind === 'ssh_relay'

  const isEditing = Boolean(editingHost)
  const title = isEditing ? t('host.edit_host') : t('host.add_host')
  const saveLabel = isEditing ? t('host.save') : t('host.save_and_connect')

  async function handleSave() {
    setError(null)
    try {
      normalizeDaemonAddress(address, (k) => t(k))
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
      return
    }

    setBusy(true)
    try {
      await onSave({
        name: name.trim(),
        address: address.trim(),
        token: token.trim() || undefined,
        kind,
      })
      onOpenChange(false)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusy(false)
    }
  }

  async function handleDelete() {
    if (!editingHost || !onDelete) return
    setBusy(true)
    try {
      await onDelete(editingHost.id)
      setConfirmDeleteOpen(false)
      onOpenChange(false)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-[460px] overflow-hidden rounded-[14px] bg-[var(--raised)] p-5">
          <DialogTitle className="flex items-center justify-between text-[15px] font-semibold text-foreground">
            <span>{title}</span>
          </DialogTitle>

          <form
            className="mt-4 flex flex-col gap-3"
            onSubmit={(e) => {
              e.preventDefault()
              void handleSave()
            }}
          >
            <label className="flex flex-col gap-1 text-[12.5px] font-medium text-[var(--text-secondary)]">
              <span>{t('host.name')}</span>
              <Input
                className="h-8 bg-card"
                placeholder={t('host.name_placeholder')}
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>

            <div className="flex flex-col gap-1">
              <span className="text-[12.5px] font-medium text-[var(--text-secondary)]">
                {t('host.transport')}
              </span>
              <div className="flex items-center gap-1.5">
                {isRelayHost ? (
                  <span className="rounded-md border border-border px-2.5 py-1 text-[12px] text-[var(--text-secondary)]">
                    {t(transportLabelKey(kind)!)}
                  </span>
                ) : (
                  WEB_EDITABLE_KINDS.map((option) => {
                    const selected = option === kind
                    return (
                      <button
                        key={option}
                        type="button"
                        aria-pressed={selected}
                        onClick={() => setKind(option)}
                        className={cn(
                          'rounded-md border px-2.5 py-1 text-[12px] transition-colors',
                          selected
                            ? 'border-ring bg-[var(--accent)] font-medium text-foreground'
                            : 'border-border bg-card text-[var(--text-secondary)] hover:bg-[var(--overlay)]',
                        )}
                      >
                        {t(`host.transport_${option}`)}
                      </button>
                    )
                  })
                )}
              </div>
              <p className="text-[11.5px] leading-[15px] text-[var(--text-tertiary)]">
                {isRelayHost
                  ? t('host.transport_relay_hint')
                  : t('host.transport_web_hint')}
              </p>
            </div>

            <label className="flex flex-col gap-1 text-[12.5px] font-medium text-[var(--text-secondary)]">
              <span>{t('host.address')}</span>
              <Input
                autoCapitalize="none"
                autoCorrect="off"
                className="h-8 bg-card"
                inputMode="url"
                placeholder={t('host.address_placeholder')}
                value={address}
                onChange={(e) => setAddress(e.target.value)}
              />
            </label>

            <label className="flex flex-col gap-1 text-[12.5px] font-medium text-[var(--text-secondary)]">
              <span>{t('host.token')}</span>
              <div className="relative">
                <Input
                  autoComplete="off"
                  className="h-8 bg-card pr-8"
                  placeholder={t('host.token_placeholder')}
                  type={tokenRevealed ? 'text' : 'password'}
                  value={token}
                  onChange={(e) => setToken(e.target.value)}
                />
                <Button
                  aria-label={t(tokenRevealed ? 'daemon.hide_token' : 'daemon.reveal_token')}
                  className="absolute right-0.5 top-0.5 size-7 text-[var(--text-tertiary)]"
                  size="icon-sm"
                  type="button"
                  variant="ghost"
                  onClick={() => setTokenRevealed((v) => !v)}
                >
                  <PaduIcon name={tokenRevealed ? 'eyeOff' : 'eye'} />
                </Button>
              </div>
            </label>

            {error && (
              <div role="alert" className="rounded-lg bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-destructive">
                {error}
              </div>
            )}

            <div className="mt-2 flex items-center justify-between pt-1">
              {isEditing && onDelete ? (
                <Button
                  className="text-destructive hover:bg-[var(--danger-soft)] hover:text-destructive"
                  disabled={busy}
                  size="sm"
                  type="button"
                  variant="ghost"
                  onClick={() => setConfirmDeleteOpen(true)}
                >
                  {t('host.remove_host')}
                </Button>
              ) : (
                <div />
              )}

              <div className="flex items-center gap-2">
                <Button
                  className="gap-1.5"
                  disabled={busy}
                  size="sm"
                  type="button"
                  variant="outline"
                  onClick={() => onOpenChange(false)}
                >
                  <span>{t('common.cancel')}</span>
                  <Kbd size="xs" variant="outline">Esc</Kbd>
                </Button>
                <Button className="gap-1.5" disabled={busy} size="sm" type="submit">
                  <span>{saveLabel}</span>
                  <Kbd size="xs" variant="onPrimary" className="px-1">
                    <PaduIcon name="cornerDownLeft" className="size-2.5" />
                  </Kbd>
                </Button>
              </div>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      {isEditing && editingHost && (
        <ConfirmDialog
          confirmLabel={t('host.remove_host')}
          description={t('host.delete_confirm_message', {
            name: editingHost.name || editingHost.address,
          })}
          open={confirmDeleteOpen}
          title={t('host.delete_confirm_title')}
          variant="danger"
          onConfirm={() => void handleDelete()}
          onOpenChange={setConfirmDeleteOpen}
        />
      )}
    </>
  )
}
