import { describe, expect, test } from 'bun:test'
import { createElement } from 'react'
import { renderToString } from 'react-dom/server'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { rehypeMentionChips } from './markdown-mentions'

function render(text: string) {
  return renderToString(
    createElement(
      ReactMarkdown,
      { remarkPlugins: [remarkGfm], rehypePlugins: [rehypeMentionChips] },
      text,
    ),
  )
}

describe('rehypeMentionChips', () => {
  test('plain text renders instead of an empty string', () => {
    // Regression: the plugin previously returned `() => (tree) => …`, which
    // made unified replace the tree with a function and blank every
    // transcript bubble (user prompt and assistant response) with no error.
    const html = render('Hello, this is a plain user prompt.')
    expect(html).toContain('Hello, this is a plain user prompt.')
  })

  test('markdown formatting still renders', () => {
    const html = render('Some **bold** response with `code`.')
    expect(html).toContain('<strong>bold</strong>')
    expect(html).toContain('code')
  })

  test('mention becomes a chip carrying the mention target', () => {
    const html = render('Fix the bug in @src/app.ts please')
    expect(html).toContain('mention-chip')
    expect(html).toContain('data-mention="src/app.ts"')
    expect(html).toContain('Fix the bug in')
  })

  test('code spans are left alone', () => {
    const html = render('Run `@src/app.ts` to check')
    expect(html).not.toContain('mention-chip')
  })
})
