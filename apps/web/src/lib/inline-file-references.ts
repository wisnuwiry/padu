export type InlineFileReference = {
  start: number
  end: number
  canonical: string
  target: string
  basename: string
  isDirectory: boolean
}

export type InlineComposerToken =
  | { kind: 'text'; start: number; end: number; value: string }
  | ({ kind: 'file' } & InlineFileReference)
  | { kind: 'mention'; start: number; end: number; canonical: string; target: string; isDirectory: boolean }

export function displayFileTarget(target: string): string {
  return percentDecode(unescapeMarkdown(target))
}

export function fileReferenceBasename(target: string): string {
  const decoded = displayFileTarget(target)
  const trimmed = stripLineLocation(decoded.trim().replace(/[/\\]+$/u, ''))
  return trimmed.split(/[/\\]/u).at(-1) || trimmed
}

export function markdownFileReference(target: string): string {
  const normalized = target.replaceAll('\\', '/')
  const label = escapeMarkdownLabel(fileReferenceBasename(normalized))
  const destination = normalized
    .replaceAll('\\', '\\\\')
    .replaceAll('(', '\\(')
    .replaceAll(')', '\\)')
    .replaceAll(' ', '%20')
  return `[${label}](${destination})`
}

export function isLocalFileTarget(target: string): boolean {
  const trimmed = target.trim()
  const lower = trimmed.toLocaleLowerCase()
  const scheme = trimmed.match(/^([a-z][a-z0-9+.-]*):/iu)?.[1]
  const isWindowsDrivePath = /^[a-z]:[/\\]/iu.test(trimmed)
  return Boolean(
    trimmed
      && !trimmed.startsWith('#')
      && (!scheme || isWindowsDrivePath)
      && !lower.startsWith('mailto:'),
  )
}

/** Mirrors the desktop input parser: a local Markdown link is an inline file
 * reference, and its canonical source range remains the editor value. */
export function parseInlineFileReferences(text: string): InlineFileReference[] {
  const references: InlineFileReference[] = []
  let cursor = 0
  while (cursor < text.length) {
    const start = text.indexOf('[', cursor)
    if (start < 0) break
    if (start > 0 && text[start - 1] === '!') {
      cursor = start + 1
      continue
    }
    const labelEnd = findUnescaped(text, start + 1, ']')
    if (labelEnd < 0) break
    if (text[labelEnd + 1] !== '(') {
      cursor = start + 1
      continue
    }
    const targetStart = labelEnd + 2
    const destination = markdownDestinationEnd(text, targetStart)
    if (!destination) {
      cursor = start + 1
      continue
    }
    const { targetEnd, end } = destination
    const raw = text.slice(targetStart, targetEnd)
    const enclosed = raw.startsWith('<') && raw.endsWith('>') ? raw.slice(1, -1) : raw
    const target = displayFileTarget(enclosed)
    if (isLocalFileTarget(target)) {
      references.push({
        start,
        end,
        canonical: text.slice(start, end),
        target,
        basename: fileReferenceBasename(target),
        isDirectory: /[/\\]$/u.test(target),
      })
      cursor = end
    } else {
      cursor = start + 1
    }
  }
  return references
}

function escapeMarkdownLabel(label: string): string {
  return label
    .replaceAll('\\', '\\\\')
    .replaceAll('[', '\\[')
    .replaceAll(']', '\\]')
}

function findUnescaped(source: string, start: number, needle: string): number {
  for (let cursor = start; cursor < source.length; cursor++) {
    if (source[cursor] === '\\') cursor++
    else if (source[cursor] === needle) return cursor
  }
  return -1
}

function markdownDestinationEnd(
  source: string,
  start: number,
): { targetEnd: number; end: number } | null {
  const angle = source[start] === '<'
  let depth = 0
  for (let cursor = start; cursor < source.length; cursor++) {
    if (source[cursor] === '\\') {
      cursor++
      continue
    }
    if (angle && source[cursor] === '>' && source[cursor + 1] === ')') {
      return { targetEnd: cursor + 1, end: cursor + 2 }
    }
    if (!angle && source[cursor] === '(') depth++
    else if (!angle && source[cursor] === ')' && depth === 0) {
      return { targetEnd: cursor, end: cursor + 1 }
    } else if (!angle && source[cursor] === ')') depth--
  }
  return null
}

function unescapeMarkdown(value: string): string {
  let output = ''
  for (let cursor = 0; cursor < value.length; cursor++) {
    if (value[cursor] === '\\') {
      if (cursor + 1 < value.length) output += value[++cursor]
    } else {
      output += value[cursor]
    }
  }
  return output
}

function percentDecode(value: string): string {
  const bytes: number[] = []
  const encoder = new TextEncoder()
  for (let cursor = 0; cursor < value.length;) {
    const escape = value.slice(cursor, cursor + 3)
    if (/^%[0-9a-f]{2}$/iu.test(escape)) {
      bytes.push(Number.parseInt(escape.slice(1), 16))
      cursor += 3
      continue
    }
    const character = String.fromCodePoint(value.codePointAt(cursor) ?? 0)
    bytes.push(...encoder.encode(character))
    cursor += character.length
  }
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(Uint8Array.from(bytes))
  } catch {
    return value
  }
}

function stripLineLocation(target: string): string {
  const fragment = target.match(/^(.*)#(?:L?\d+)(?:-L\d+)*$/u)
  if (fragment) return fragment[1] ?? target
  const line = target.match(/^(.*):\d+$/u)
  return line?.[1] ?? target
}

const LEGACY_MENTION = /(?:^|\s)@([a-zA-Z0-9_.\-\\/]+)/gu

export function tokenizeInlineComposerText(text: string): InlineComposerToken[] {
  const files = parseInlineFileReferences(text)
  const tokens: InlineComposerToken[] = []
  let cursor = 0

  const appendTextWithMentions = (start: number, end: number) => {
    const value = text.slice(start, end)
    let localCursor = 0
    LEGACY_MENTION.lastIndex = 0
    let match: RegExpExecArray | null
    while ((match = LEGACY_MENTION.exec(value)) !== null) {
      const atOffset = match[0].indexOf('@')
      const mentionStart = start + match.index + atOffset
      const rawEnd = start + match.index + match[0].length
      const mentionEnd = text.slice(mentionStart, rawEnd).replace(/[,;!?:)\]}"']+$/u, '').length + mentionStart
      if (mentionEnd <= mentionStart + 1) continue
      const relativeStart = mentionStart - start
      if (relativeStart > localCursor) {
        tokens.push({ kind: 'text', start: start + localCursor, end: mentionStart, value: value.slice(localCursor, relativeStart) })
      }
      const canonical = text.slice(mentionStart, mentionEnd)
      const target = canonical.slice(1)
      tokens.push({
        kind: 'mention',
        start: mentionStart,
        end: mentionEnd,
        canonical,
        target,
        isDirectory: /[/\\]$/u.test(target),
      })
      localCursor = mentionEnd - start
      LEGACY_MENTION.lastIndex = localCursor
    }
    if (localCursor < value.length) {
      tokens.push({ kind: 'text', start: start + localCursor, end, value: value.slice(localCursor) })
    }
  }

  for (const reference of files) {
    appendTextWithMentions(cursor, reference.start)
    tokens.push({ kind: 'file', ...reference })
    cursor = reference.end
  }
  appendTextWithMentions(cursor, text.length)
  return tokens
}

export function insertInlineFileReference(
  text: string,
  start: number,
  end: number,
  target: string,
): { text: string; cursor: number } {
  const before = text.slice(0, start)
  const after = text.slice(end)
  const prefix = before.length && !/\s$/u.test(before) ? ' ' : ''
  const suffix = after.length && !/^\s/u.test(after) ? ' ' : ''
  const insert = `${prefix}${markdownFileReference(target)}${suffix}`
  const existingSeparator = suffix ? 0 : (after.match(/^\s/u)?.[0].length ?? 0)
  return {
    text: `${before}${insert}${after}`,
    cursor: before.length + insert.length + existingSeparator,
  }
}

export function atomicReferenceDeletion(
  text: string,
  start: number,
  end: number,
  direction: 'backward' | 'forward',
): { text: string; cursor: number } | null {
  if (start !== end) return null
  const references = [
    ...parseInlineFileReferences(text),
    ...tokenizeInlineComposerText(text)
      .filter((token): token is Extract<InlineComposerToken, { kind: 'mention' }> => token.kind === 'mention'),
  ]
  const reference = references.find((item) => direction === 'backward' ? item.end === start : item.start === start)
  if (!reference) return null
  let removeStart = reference.start
  let removeEnd = reference.end
  if (direction === 'backward' && removeStart > 0 && /\s/u.test(text[removeStart - 1] ?? '')) removeStart--
  if (direction === 'forward' && /\s/u.test(text[removeEnd] ?? '')) removeEnd++
  return { text: text.slice(0, removeStart) + text.slice(removeEnd), cursor: removeStart }
}
