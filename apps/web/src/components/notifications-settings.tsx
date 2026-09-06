import { useEffect, useState, type ReactNode } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { PaduIcon } from '@/components/padu-icon'
import { useI18n } from '@/lib/i18n'
import { cn } from '@/lib/utils'

function playChime() {
  try {
    const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
    if (!AudioCtx) return
    const ctx = new AudioCtx()
    const osc = ctx.createOscillator()
    const gain = ctx.createGain()

    osc.type = 'sine'
    // Two-tone ascending chime (D5 -> A5)
    osc.frequency.setValueAtTime(587.33, ctx.currentTime)
    osc.frequency.exponentialRampToValueAtTime(880, ctx.currentTime + 0.08)

    gain.gain.setValueAtTime(0.18, ctx.currentTime)
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.45)

    osc.connect(gain)
    gain.connect(ctx.destination)

    osc.start()
    osc.stop(ctx.currentTime + 0.45)
  } catch (err) {
    console.warn('Unable to play notification chime', err)
  }
}

type PermissionStatusType = 'granted' | 'denied' | 'default' | 'unsupported'

export function NotificationsSettings() {
  const { t } = useI18n()
  const [notificationsEnabled, setNotificationsEnabled] = useStoredBoolean(
    'padu.notifications-enabled',
    true,
  )
  const [soundEnabled, setSoundEnabled] = useStoredBoolean(
    'padu.notification-sound-enabled',
    true,
  )
  const [permission, setPermission] = useState<PermissionStatusType>(() => {
    if (typeof window === 'undefined' || !('Notification' in window)) {
      return 'unsupported'
    }
    return Notification.permission
  })

  useEffect(() => {
    if (typeof window === 'undefined' || !('Notification' in window)) {
      setPermission('unsupported')
      return
    }
    setPermission(Notification.permission)
  }, [])

  const handleRequestPermission = async () => {
    if (typeof window === 'undefined' || !('Notification' in window)) {
      toast.error('Notifications are not supported in this browser.')
      return
    }
    try {
      const res = await Notification.requestPermission()
      setPermission(res)
      if (res === 'granted') {
        toast.success(t('settings.notification_permission_status_allowed'))
      } else if (res === 'denied') {
        toast.error(t('settings.notification_permission_status_denied'))
      }
    } catch (err) {
      console.error('Error requesting notification permission:', err)
    }
  }

  const handlePreviewSound = () => {
    playChime()
  }

  const handleSendTest = () => {
    if (soundEnabled) {
      playChime()
    }

    if (permission === 'granted') {
      try {
        new Notification(t('settings.test_notification_title'), {
          body: t('settings.test_notification_body'),
          icon: '/favicon.ico',
        })
      } catch (err) {
        console.warn('Failed to construct Notification object', err)
      }
    }

    toast.info(t('settings.test_notification_title'), {
      description: t('settings.test_notification_body'),
    })
  }

  return (
    <div className="flex flex-col gap-3 pt-2">
      {/* Permission Status Card */}
      <SettingsCard>
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0 flex-1">
            <div className="text-[13.5px] font-medium">
              {t('settings.notification_permission_title')}
            </div>
            <p className="mt-1 text-[12.5px] leading-[18px] text-[var(--text-secondary)]">
              {permission === 'granted' && t('settings.notification_permission_status_allowed')}
              {permission === 'denied' && t('settings.notification_permission_status_denied')}
              {permission === 'default' && t('settings.notification_permission_status_prompt')}
              {permission === 'unsupported' && 'Web notifications are not supported in this browser.'}
            </p>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            {permission === 'granted' && (
              <span className="inline-flex items-center gap-1.5 rounded-md bg-[var(--success-soft)] px-2.5 py-1 text-[12px] font-medium text-[var(--success)]">
                <PaduIcon className="size-3.5" name="check" />
                {t('settings.notification_permission_allowed')}
              </span>
            )}

            {permission === 'denied' && (
              <span className="inline-flex items-center gap-1.5 rounded-md bg-[var(--danger-soft)] px-2.5 py-1 text-[12px] font-medium text-[var(--danger)]">
                <PaduIcon className="size-3.5" name="alert" />
                {t('settings.notification_permission_denied')}
              </span>
            )}

            {permission === 'default' && (
              <Button
                size="sm"
                variant="default"
                className="gap-1.5 text-[12px]"
                onClick={handleRequestPermission}
              >
                <PaduIcon className="size-3.5" name="bell" />
                {t('settings.request_permission')}
              </Button>
            )}
          </div>
        </div>
      </SettingsCard>

      {/* Preferences Card */}
      <SettingsCard>
        <div className="flex flex-col divide-y divide-border">
          {/* Notifications Toggle */}
          <div className="flex items-center justify-between gap-6 pb-3 pt-1">
            <SettingText
              title={t('settings.task_completion_notifications')}
              description={t('settings.task_completion_notifications_description')}
            />
            <Toggle
              checked={notificationsEnabled}
              label={t('settings.task_completion_notifications')}
              onChange={setNotificationsEnabled}
            />
          </div>

          {/* Sound Toggle */}
          <div className="flex items-center justify-between gap-6 py-3">
            <SettingText
              title={t('settings.notification_sound')}
              description={t('settings.notification_sound_description')}
            />
            <Toggle
              checked={soundEnabled}
              label={t('settings.notification_sound')}
              onChange={setSoundEnabled}
            />
          </div>

          {/* Sound Preview Row */}
          <div className="flex items-center justify-between gap-6 pt-3">
            <SettingText
              title={t('settings.preview_sound')}
              description="Play a preview of the audio alert that chimes upon task completion."
            />
            <Button
              size="sm"
              variant="outline"
              className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
              onClick={handlePreviewSound}
            >
              <PaduIcon className="size-3.5" name="gauge" />
              {t('settings.preview_sound')}
            </Button>
          </div>
        </div>
      </SettingsCard>

      {/* Test Card */}
      <SettingsCard row>
        <SettingText
          title={t('settings.send_test_notification')}
          description={t('settings.test_notification_subtitle')}
        />
        <Button
          size="sm"
          variant="outline"
          className="gap-1.5 text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={handleSendTest}
        >
          <PaduIcon className="size-3.5" name="bell" />
          {t('settings.send_test_notification')}
        </Button>
      </SettingsCard>
    </div>
  )
}

function SettingsCard({ children, row = false }: { children: ReactNode; row?: boolean }) {
  return (
    <section className={cn('w-full rounded-[13px] bg-[var(--raised)] px-5 py-[14px]', row && 'flex items-center gap-6')}>
      {children}
    </section>
  )
}

function SettingText({ title, description }: { title: string; description: string }) {
  return (
    <div className="min-w-0 flex-1">
      <div className="text-[13.5px] font-medium">{title}</div>
      <p className="mt-[5px] text-[12.5px] leading-[18px] text-[var(--text-secondary)]">{description}</p>
    </div>
  )
}

function Toggle({ checked, label, onChange }: { checked: boolean; label: string; onChange: (checked: boolean) => void }) {
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

function useStoredBoolean(key: string, fallback: boolean) {
  const [value, setValue] = useState<boolean>(() => {
    if (typeof window === 'undefined') return fallback
    const raw = window.localStorage.getItem(key)
    return raw === null ? fallback : raw === 'true'
  })

  useEffect(() => {
    window.localStorage.setItem(key, String(value))
  }, [key, value])

  return [value, setValue] as const
}
