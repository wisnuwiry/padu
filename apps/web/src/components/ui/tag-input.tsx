import * as React from 'react'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

export interface TagInputProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'onChange'> {
  value: string[]
  onChange: (tags: string[]) => void
  placeholder?: string
  disabled?: boolean
  className?: string
  small?: boolean
  focusOutline?: boolean
  accentChips?: boolean
  /** Localized accessible name for a tag's remove button. Defaults to English. */
  getRemoveLabel?: (tag: string) => string
}

export function TagInput({
  value = [],
  onChange,
  placeholder,
  disabled = false,
  className,
  small = false,
  focusOutline = true,
  accentChips = false,
  getRemoveLabel,
  ...props
}: TagInputProps) {
  const [input, setInput] = React.useState('')
  const inputRef = React.useRef<HTMLInputElement>(null)

  const addTokens = (raw: string) => {
    const tokens = raw
      .split(/[,，;]/)
      .map((t) => t.trim())
      .filter((t) => t.length > 0)
    if (tokens.length === 0) return
    const next = [...value]
    for (const t of tokens) {
      if (!next.includes(t)) {
        next.push(t)
      }
    }
    onChange(next)
    setInput('')
  }

  const removeTag = (index: number) => {
    onChange(value.filter((_, i) => i !== index))
  }

  return (
    <div
      className={cn(
        'flex flex-wrap items-center cursor-text transition-colors',
        small
          ? 'gap-1 min-h-[24px] py-0.5 px-1.5 rounded text-[10.5px] bg-muted/30'
          : 'gap-1.5 min-h-[36px] p-1.5 rounded-md bg-muted/40',
        focusOutline && 'focus-within:ring-1 focus-within:ring-ring border border-input',
        !focusOutline && 'border-0 outline-none focus-within:ring-0',
        disabled && 'opacity-50 cursor-not-allowed',
        className,
      )}
      onClick={() => inputRef.current?.focus()}
      {...props}
    >
      {value.map((tag, idx) => (
        <Badge
          key={`${tag}-${idx}`}
          variant="secondary"
          className={cn(
            'gap-1',
            small ? 'text-[9.5px] px-1.5 py-0' : 'text-xs px-2 py-0.5',
            accentChips && 'border border-primary/45 bg-primary/10 text-primary hover:bg-primary/15',
          )}
        >
          <span>{tag}</span>
          {!disabled && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                removeTag(idx)
              }}
              className={cn(
                'ml-0.5 cursor-pointer leading-none hover:text-destructive',
                accentChips ? 'text-primary/70' : 'text-muted-foreground',
              )}
              aria-label={getRemoveLabel ? getRemoveLabel(tag) : `Remove tag ${tag}`}
            >
              <span aria-hidden="true">×</span>
            </button>
          )}
        </Badge>
      ))}
      <input
        ref={inputRef}
        value={input}
        disabled={disabled}
        onChange={(e) => {
          const val = e.target.value
          if (val.includes(',') || val.includes('，') || val.includes(';')) {
            addTokens(val)
          } else {
            setInput(val)
          }
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ',') {
            e.preventDefault()
            addTokens(input)
          } else if (e.key === 'Backspace' && !input && value.length > 0) {
            removeTag(value.length - 1)
          }
        }}
        onBlur={() => {
          if (input.trim()) {
            addTokens(input)
          }
        }}
        placeholder={value.length === 0 ? placeholder : ''}
        className={cn(
          'bg-transparent border-0 outline-none flex-1 px-1 text-foreground placeholder:text-muted-foreground focus:ring-0 shadow-none',
          small ? 'text-[10.5px] min-w-[60px] h-[18px]' : 'text-xs min-w-[80px] h-6',
        )}
      />
    </div>
  )
}
