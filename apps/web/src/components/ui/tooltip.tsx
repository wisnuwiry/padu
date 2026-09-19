import { Tooltip as TooltipPrimitive } from '@base-ui/react/tooltip'
import type { ReactNode } from 'react'
import { Kbd } from '@/components/ui/kbd'
import { cn } from '@/lib/utils'

export interface TooltipProps {
  content?: ReactNode
  shortcut?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
  align?: 'start' | 'center' | 'end'
  sideOffset?: number
  delay?: number
  children: ReactNode
  disabled?: boolean
  className?: string
  triggerClassName?: string
}

export function Tooltip({
  content,
  shortcut,
  side = 'top',
  align = 'center',
  sideOffset = 6,
  delay = 300,
  children,
  disabled = false,
  className,
  triggerClassName,
}: TooltipProps) {
  if (!content || disabled) return <>{children}</>

  return (
    <TooltipPrimitive.Provider delay={delay}>
      <TooltipPrimitive.Root>
        <TooltipPrimitive.Trigger
          render={(props) => (
            <span {...props} className={cn('inline-flex', triggerClassName, props.className)}>
              {children}
            </span>
          )}
        />
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Positioner
            align={align}
            className="z-[120] outline-none pointer-events-none"
            side={side}
            sideOffset={sideOffset}
          >
            <TooltipPrimitive.Popup
              className={cn(
                'flex items-center gap-2 rounded-md border border-[var(--input)] bg-[var(--raised)] px-2 py-1 text-[12px] leading-4 text-foreground shadow-md transition-[transform,opacity] motion-reduce:transition-none data-[ending-style]:scale-95 data-[ending-style]:opacity-0 data-[starting-style]:scale-95 data-[starting-style]:opacity-0',
                className,
              )}
            >
              <div className="min-w-0">{content}</div>
              {shortcut && <Kbd size="xs">{shortcut}</Kbd>}
            </TooltipPrimitive.Popup>
          </TooltipPrimitive.Positioner>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  )
}
