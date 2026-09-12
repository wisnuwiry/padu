import type { HastNode } from './markdown-veil'

/**
 * Rehype plugin that wraps `@mention` text runs in `mention-chip` spans.
 *
 * Unified plugin shape matters here: the entry in `rehypePlugins` is the
 * attacher itself, so calling it must return the transformer directly. An
 * extra arrow level (`() => (tree) => …`) makes unified treat the returned
 * function as the new syntax tree, which silently renders every Markdown
 * body as an empty string with no error.
 */
export function rehypeMentionChips() {
  return (tree: HastNode) => {
    const pattern = /(?:^|\s)@([a-zA-Z0-9_.\-\\/]+)/g
    const visit = (node: HastNode) => {
      if (!node.children || node.tagName === 'code' || node.tagName === 'pre' || node.tagName === 'a') return
      node.children = node.children.flatMap((child) => {
        if (child.type === 'text') {
          const value: string = child.value || ''
          if (!value.includes('@')) return [child]
          const result: HastNode[] = []
          let lastIndex = 0
          let match: RegExpExecArray | null
          pattern.lastIndex = 0
          while ((match = pattern.exec(value)) !== null) {
            const fullMatch = match[0]
            const atOffset = fullMatch.indexOf('@')
            const start = match.index + atOffset
            const end = match.index + fullMatch.length
            const trimmedEnd = value.slice(start, end).replace(/[,;!?:)\]}"']+$/, '').length + start
            if (trimmedEnd <= start + 1) continue

            if (start > lastIndex) {
              result.push({ type: 'text', value: value.slice(lastIndex, start) })
            }
            result.push({
              type: 'element',
              tagName: 'span',
              properties: {
                className: ['mention-chip'],
                'data-mention': value.slice(start, trimmedEnd).replace(/^@/, ''),
              },
              children: [{ type: 'text', value: value.slice(start, trimmedEnd) }],
            })
            lastIndex = trimmedEnd
            pattern.lastIndex = trimmedEnd
          }
          if (lastIndex < value.length) {
            result.push({ type: 'text', value: value.slice(lastIndex) })
          }
          return result.length ? result : [child]
        }
        visit(child)
        return [child]
      })
    }
    visit(tree)
  }
}
