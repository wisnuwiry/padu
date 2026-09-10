import type { Note } from './generated/Note'

export function noteExcerpt(content: string, maxLength = 140): string {
  const text = content.replace(/\s+/gu, ' ').trim()
  return text.length <= maxLength ? text : `${text.slice(0, maxLength - 1)}…`
}

export function parseNoteCommand(input: string): string | null {
  const match = input.trim().match(/^\/note(?:\s+([\s\S]*))?$/u)
  return match ? (match[1] ?? '').trim() : null
}

export function noteEmbed(note: Note): string {
  return `## Attached note: ${note.title}\n\n${note.content}`
}
