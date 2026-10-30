import { PaduIcon } from '@/components/padu-icon'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import type { SettingsPageId } from './types'

export function AboutSettings({
  _onPageChange,
}: {
  onPageChange?: (page: SettingsPageId) => void
  _onPageChange?: (page: SettingsPageId) => void
} = {}) {
  const { t } = useI18n()
  const currentVersion = '0.1.1'

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
