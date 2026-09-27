import * as React from 'react'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

export interface TagInputProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'onChange'> {
  value: string[]
  onChange: (tags: string[]) => void
  placeholder?: string
  disabled?: boolean
  className?: string
}

export function TagInput({
  value = [],
  onChange,
  placeholder,
  disabled = false,
  className,
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
        'flex flex-wrap items-center gap-1.5 min-h-[36px] p-1.5 rounded-md bg-muted/40 cursor-text focus-within:ring-1 focus-within:ring-ring border border-input transition-colors',
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
          className="gap-1 text-xs px-2 py-0.5"
        >
          <span>{tag}</span>
          {!disabled && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                removeTag(idx)
              }}
              className="hover:text-destructive text-muted-foreground ml-0.5 cursor-pointer leading-none"
              aria-label={`Remove tag ${tag}`}
            >
              ×
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
        className="bg-transparent border-0 outline-none text-xs flex-1 min-w-[80px] h-6 px-1 text-foreground placeholder:text-muted-foreground focus:ring-0 shadow-none"
      />
    </div>
  )
}
