export type Translator = (key: string, params?: Record<string, string | number>) => string

export function noteExcerpt(preview: string): string {
  const text = preview.replace(/\s+/gu, ' ').trim()
  return text.length <= 140 ? text : `${text.slice(0, 139)}…`
}

export function formatNoteTimeAgo(
  value: number,
  t: Translator,
  now = Date.now(),
): string {
  const seconds = Math.max(0, Math.floor(now / 1_000) - value)
  if (seconds < 60) return t('notes.time_just_now')
  if (seconds < 3_600) return t('notes.time_minutes_ago', { count: Math.floor(seconds / 60) })
  if (seconds < 86_400) return t('notes.time_hours_ago', { count: Math.floor(seconds / 3_600) })
  return t('notes.time_days_ago', { count: Math.floor(seconds / 86_400) })
}
