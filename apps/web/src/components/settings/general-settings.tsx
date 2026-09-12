import { useI18n } from '@/lib/i18n'
import { SettingsCard, SettingText, Toggle, useStoredBoolean } from './shared'

export function GeneralSettings({ onOpenOnboarding }: { onOpenOnboarding?: () => void }) {
  const { t } = useI18n()
  const [analytics, setAnalytics] = useStoredBoolean('padu.analytics-enabled', true)
  return (
    <div>
      <SettingsCard>
        <SettingText
          title={t('settings.local_by_default')}
          description={t('settings.local_by_default_web_description')}
        />
      </SettingsCard>
      <SettingsCard row>
        <SettingText
          title={t('settings.share_anonymous_usage_data')}
          description={t('settings.share_anonymous_usage_data_description')}
        />
        <Toggle checked={analytics} label={t('settings.share_anonymous_usage_data')} onChange={setAnalytics} />
      </SettingsCard>
      {onOpenOnboarding && (
        <SettingsCard row>
          <SettingText
            title={t('onboarding.command_title')}
            description={t('onboarding.welcome_subtitle')}
          />
          <button
            className="flex h-8 shrink-0 items-center justify-center rounded-lg bg-accent px-3.5 text-[12.5px] font-medium text-foreground outline-none hover:bg-accent/80 active:opacity-80 focus-visible:ring-1 focus-visible:ring-ring"
            type="button"
            onClick={onOpenOnboarding}
          >
            {t('onboarding.replay_button')}
          </button>
        </SettingsCard>
      )}
    </div>
  )
}
