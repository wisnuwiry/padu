import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { PaduIcon } from '@/components/padu-icon'
import { useI18n } from '@/lib/i18n'
import {
  SettingText,
  Toggle,
  useStoredBoolean,
} from '@/components/settings/shared'

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
      toast.error(t('notifications.permission_unsupported'))
      return
    }
    try {
      const res = await Notification.requestPermission()
      setPermission(res)
      if (res === 'granted') {
        toast.success(t('notifications.permission_granted'))
      } else if (res === 'denied') {
        toast.error(t('notifications.permission_denied'))
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
        new Notification(t('notifications.test_notification_title'), {
          body: t('notifications.test_notification_body'),
          icon: '/favicon.ico',
        })
      } catch (err) {
        console.warn('Failed to construct Notification object', err)
      }
    }

    toast.info(t('notifications.test_notification_title'), {
      description: t('notifications.test_notification_body'),
    })
  }

  return (
    <div className="mt-[15px] w-full overflow-hidden rounded-[13px] bg-[var(--raised)]">
      {/* Permission status */}
      <div className="flex items-center gap-3 px-4 py-3">
        <span className="grid size-8 shrink-0 place-items-center rounded-lg border border-[var(--border)] bg-background text-[var(--text-secondary)]">
          <PaduIcon className="size-4" name="bell" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-medium">
            {t('notifications.system_permission')}
          </div>
          <p className="mt-0.5 text-[12px] leading-[17px] text-[var(--text-secondary)]">
            {t('notifications.system_permission_desc')}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {permission === 'granted' && (
            <Badge variant="success" className="h-5 gap-1 px-2 text-[11px]">
              <PaduIcon className="size-3" name="check" />
              {t('notifications.permission_granted')}
            </Badge>
          )}
          {permission === 'denied' && (
            <Badge variant="destructive" className="h-5 gap-1 px-2 text-[11px]">
              <PaduIcon className="size-3" name="alert" />
              {t('notifications.permission_denied')}
            </Badge>
          )}
          {permission === 'default' && (
            <>
              <Badge variant="warning" className="h-5 gap-1 px-2 text-[11px]">
                <PaduIcon className="size-3" name="bell" />
                {t('notifications.permission_not_determined')}
              </Badge>
              <Button
                size="sm"
                variant="default"
                className="h-7 gap-1.5 text-[12px]"
                onClick={handleRequestPermission}
              >
                {t('notifications.request_permission')}
              </Button>
            </>
          )}
          {permission === 'unsupported' && (
            <Badge variant="secondary" className="h-5 px-2 text-[11px]">
              {t('notifications.permission_unsupported')}
            </Badge>
          )}
        </div>
      </div>

      <div className="mx-4 border-t border-[var(--border)]" />

      {/* Task completion toggle */}
      <div className="flex items-center justify-between gap-4 px-4 py-2.5">
        <SettingText
          title={t('notifications.task_completion')}
          description={t('notifications.task_completion_desc')}
        />
        <Toggle
          checked={notificationsEnabled}
          label={t('notifications.task_completion')}
          onChange={setNotificationsEnabled}
        />
      </div>

      <div className="mx-4 border-t border-[var(--border)]" />

      {/* Sound toggle + preview */}
      <div className="flex items-center justify-between gap-4 px-4 py-2.5">
        <SettingText
          title={t('notifications.sound')}
          description={t('notifications.sound_desc')}
        />
        <div className="flex shrink-0 items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            className="h-7 gap-1.5 border-[var(--border)] bg-background text-[12px] text-[var(--text-secondary)] hover:text-foreground"
            onClick={handlePreviewSound}
          >
            <PaduIcon className="size-3" name="gauge" />
            {t('notifications.preview_sound')}
          </Button>
          <Toggle
            checked={soundEnabled}
            label={t('notifications.sound')}
            onChange={setSoundEnabled}
          />
        </div>
      </div>

      <div className="mx-4 border-t border-[var(--border)]" />

      {/* Test notification */}
      <div className="flex items-center justify-between gap-4 px-4 py-2.5">
        <SettingText
          title={t('notifications.test_title')}
          description={t('notifications.test_desc')}
        />
        <Button
          size="sm"
          variant="outline"
          className="h-7 shrink-0 gap-1.5 border-[var(--border)] bg-background text-[12px] text-[var(--text-secondary)] hover:text-foreground"
          onClick={handleSendTest}
        >
          <PaduIcon className="size-3" name="bell" />
          {t('notifications.send_test')}
        </Button>
      </div>
    </div>
  )
}
