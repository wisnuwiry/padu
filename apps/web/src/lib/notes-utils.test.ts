import { describe, expect, test } from 'bun:test'
import {
  formatNoteMeta,
  formatNoteTimeAgo,
  nextLayoutOnCreate,
  noteExcerpt,
} from './notes-utils'

describe('notes utils', () => {
  const t = (key: string, params?: Record<string, string | number>) => {
    if (key === 'notes.time_just_now') return 'just now'
    if (key === 'notes.time_minutes_ago') return `${params?.count}m`
    if (key === 'notes.time_hours_ago') return `${params?.count}h`
    if (key === 'notes.time_days_ago') return `${params?.count}d`
    return key
  }

  test('noteExcerpt trims and truncates long previews', () => {
    expect(noteExcerpt('  short text  ')).toBe('short text')
    const long = 'a'.repeat(200)
    expect(noteExcerpt(long)).toBe(`${'a'.repeat(139)}…`)
  })

  test('formatNoteTimeAgo handles time boundaries', () => {
    const now = 1_000_000_000
    expect(formatNoteTimeAgo(now - 30, t, now * 1_000)).toBe('just now')
    expect(formatNoteTimeAgo(now - 120, t, now * 1_000)).toBe('2m')
    expect(formatNoteTimeAgo(now - 7_200, t, now * 1_000)).toBe('2h')
    expect(formatNoteTimeAgo(now - 172_800, t, now * 1_000)).toBe('2d')
  })

  test('formatNoteMeta formats metadata with tags', () => {
    expect(formatNoteMeta('Padu', 'just now', undefined)).toBe('Padu · just now')
    expect(formatNoteMeta('Padu', 'just now', [])).toBe('Padu · just now')
    expect(formatNoteMeta('Padu', '1h', ['design', 'v1'])).toBe('Padu · 1h · #design #v1')
    expect(
      formatNoteMeta('Padu', '2d', ['tag1', 'tag2', 'tag3', 'tag4', 'tag5']),
    ).toBe('Padu · 2d · #tag1 #tag2 #tag3 +2')
  })

  test('nextLayoutOnCreate changes preview to edit only', () => {
    expect(nextLayoutOnCreate('preview')).toBe('edit')
    expect(nextLayoutOnCreate('split')).toBe('split')
    expect(nextLayoutOnCreate('edit')).toBe('edit')
  })
})

