import { File, type FileOptions } from '@pierre/diffs/react'
import { useEffect, useState } from 'react'
import { FileTypeIcon, PaduIcon } from '@/components/padu-icon'
import { Tooltip } from '@/components/ui/tooltip'
import { useI18n } from '@/lib/i18n'
import { useResolvedTheme } from '@/lib/theme'

/// A fenced code block rendered like a read-only file in the files panel:
/// syntax-highlighted by the shared Diffs highlighter with line numbers, a
/// header carrying the language's file-type icon, and a copy button.
export function MarkdownCodeBlock({ language, code }: { language?: string; code: string }) {
  const { t } = useI18n()
  const themeType = useResolvedTheme()
  const [copied, setCopied] = useState(false)
  const name = snippetFilename(language)
  const label = language?.toLowerCase() || 'text'

  useEffect(() => {
    if (!copied) return
    const timer = window.setTimeout(() => setCopied(false), 2000)
    return () => window.clearTimeout(timer)
  }, [copied])

  const copy = () => {
    void navigator.clipboard.writeText(code)
    setCopied(true)
  }

  const options: FileOptions<undefined> = {
    overflow: 'wrap',
    preferredHighlighter: 'shiki-js',
    disableFileHeader: true,
    themeType,
  }

  const copyLabel = copied ? t('common.copied') : t('common.copy_code')
  return (
    <div className="max-w-full overflow-hidden rounded-xl border bg-muted/45">
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-border px-3">
        <FileTypeIcon className="size-4 shrink-0" path={name} />
        <span className="min-w-0 truncate font-mono text-[11px] text-[var(--text-secondary)]">{label}</span>
        <div className="flex-1" />
        <Tooltip content={copyLabel}>
          <button
            aria-label={copyLabel}
            className="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md text-[var(--text-tertiary)] outline-none transition-colors hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
            type="button"
            onClick={copy}
          >
            <PaduIcon className="size-3.5" name={copied ? 'check' : 'copy'} />
          </button>
        </Tooltip>
      </div>
      <File
        className="padu-code-surface bg-muted/45"
        disableWorkerPool
        file={{ name, contents: code }}
        options={options}
      />
    </div>
  )
}

/// A representative filename for a fenced language tag. Diffs infers the
/// highlighting language from the extension, and `FileTypeIcon` derives the
/// icon from the same path, so one mapping feeds both.
function snippetFilename(language?: string): string {
  switch (language?.toLowerCase()) {
    case 'js': case 'javascript': case 'jsx': case 'mjs': case 'cjs': return 'snippet.jsx'
    case 'ts': case 'typescript': case 'mts': case 'cts': return 'snippet.ts'
    case 'tsx': return 'snippet.tsx'
    case 'rust': case 'rs': return 'snippet.rs'
    case 'go': case 'golang': return 'snippet.go'
    case 'py': case 'python': case 'pyi': return 'snippet.py'
    case 'rb': case 'ruby': return 'snippet.rb'
    case 'java': return 'snippet.java'
    case 'kt': case 'kotlin': case 'kts': return 'snippet.kt'
    case 'c': return 'snippet.c'
    case 'cc': case 'cpp': case 'cxx': case 'c++': case 'h': case 'hpp': return 'snippet.cpp'
    case 'cs': case 'csharp': return 'snippet.cs'
    case 'swift': return 'snippet.swift'
    case 'go.mod': return 'go.mod'
    case 'html': case 'htm': case 'vue': case 'svelte': case 'xml': case 'svg': return 'snippet.html'
    case 'css': case 'scss': case 'sass': case 'less': return 'snippet.scss'
    case 'json': case 'jsonc': case 'json5': return 'snippet.json'
    case 'yaml': case 'yml': return 'snippet.yml'
    case 'toml': return 'snippet.toml'
    case 'sql': return 'snippet.sql'
    case 'sh': case 'bash': case 'zsh': case 'shell': case 'fish': return 'snippet.sh'
    case 'md': case 'markdown': case 'mdx': return 'snippet.md'
    case 'diff': case 'patch': return 'snippet.diff'
    case 'dockerfile': return 'Dockerfile'
    case 'make': case 'makefile': return 'Makefile'
    default: return 'snippet.txt'
  }
}