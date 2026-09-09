export function noteExcerpt(preview: string): string {
  const text = preview.replace(/\s+/gu, ' ').trim()
  return text.length <= 140 ? text : `${text.slice(0, 139)}…`
}


export function formatNoteTimeAgo(value: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor(now / 1_000) - value)
  if (seconds < 60) return 'just now'
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86_400) return `${Math.floor(seconds / 3_600)}h ago`
  return `${Math.floor(seconds / 86_400)}d ago`
}
