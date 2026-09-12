import type { MouseEvent, ReactNode } from 'react'
import { FileTypeIcon, PaduIcon } from '@/components/padu-icon'
import { Tooltip } from '@/components/ui/tooltip'
import { displayFileTarget, fileReferenceBasename } from '@/lib/inline-file-references'
import { cn } from '@/lib/utils'

export function InlineFileChip({
  target,
  label,
  href,
  className,
  composer = false,
  legacy = false,
  onOpen,
}: {
  target: string
  label?: ReactNode
  href?: string
  className?: string
  composer?: boolean
  legacy?: boolean
  onOpen?: (target: string) => boolean
}) {
  const displayTarget = displayFileTarget(target)
  const isDirectory = /[/\\]$/u.test(displayTarget)
  const contents = (
    <>
      {isDirectory
        ? <PaduIcon className="size-3.5 shrink-0 text-[var(--text-secondary)]" name="folder" />
        : <FileTypeIcon className="size-3.5 shrink-0" path={displayTarget} />}
      <span className="min-w-0 truncate">{label ?? (legacy ? displayTarget.replace(/[/\\]$/u, '') : fileReferenceBasename(displayTarget))}</span>
    </>
  )
  const classes = cn(
    'inline-flex max-w-[240px] items-center gap-2 whitespace-nowrap rounded-[8px] border border-border bg-[var(--inset)] px-2.5 py-1.5 align-baseline font-mono text-[12px] font-medium leading-4 text-foreground no-underline',
    composer && 'mx-1 select-all',
    className,
  )

  const chip = !href ? (
    <span className={classes}>{contents}</span>
  ) : (
    <a
      className={classes}
      href={href}
      rel="noreferrer noopener"
      target="_blank"
      onClick={(event: MouseEvent<HTMLAnchorElement>) => {
        if (onOpen?.(target)) event.preventDefault()
      }}
    >
      {contents}
    </a>
  )

  return <Tooltip content={displayTarget}>{chip}</Tooltip>
}
