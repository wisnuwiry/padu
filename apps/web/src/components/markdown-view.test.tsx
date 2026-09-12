import { describe, expect, test } from 'bun:test'
import { createElement } from 'react'
import { renderToString } from 'react-dom/server'
import { renderKatex } from '@/lib/katex-math'
import { MarkdownView, fenceCode } from './markdown-view'

function render(text: string) {
  return renderToString(createElement(MarkdownView, { text }))
}

describe('MarkdownView math and diagrams', () => {
  test('inline math renders through KaTeX', () => {
    const html = render('The quadratic formula is $x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$.')
    expect(html).toContain('katex')
    expect(html).not.toContain('$x')
  })

  test('display math renders as a KaTeX display block', () => {
    const html = render('$$\n\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}\n$$')
    expect(html).toContain('katex-display')
  })

  test('unclosed math does not tear down the document', () => {
    // KaTeX runs with `throwOnError: false`, so a still-streaming formula must
    // render (as error markup) instead of aborting the whole parse.
    const html = render('The limit is $\\lim_{x \\to')
    expect(html).toContain('The limit is')
  })

  test('a mermaid fence is rendered as a diagram, not a code block', () => {
    const html = render('```mermaid\ngraph TD\nA-->B\n```')
    expect(html).toContain('mermaid-diagram')
    expect(html).not.toContain('language-mermaid')
  })

  test('a code fence renders a highlighted block with a language header', () => {
    const html = render('```js\nconst x = 1;\n```')
    expect(html).toContain('Copy Code')
    expect(html).not.toContain('mermaid-diagram')
  })

  test('a fence without a language falls back to plain text', () => {
    const html = render('```\nplain\n```')
    expect(html).toContain('Copy Code')
    expect(html).toContain('>text<')
  })

  test('fenceCode strips the trailing newline from fenced content', () => {
    expect(fenceCode('const x = 1;\n')).toBe('const x = 1;')
    expect(fenceCode('a\nb')).toBe('a\nb')
    expect(fenceCode(['a', 'b'])).toBe('ab')
    expect(fenceCode(undefined)).toBe('')
    // A genuinely trailing blank line survives (only the fence artifact goes).
    expect(fenceCode('a\n\n')).toBe('a\n')
  })

  test('the KaTeX demo expands its macro instead of erroring', () => {
    // `\f` is registered as a shared macro; without it KaTeX emits the source
    // as an "Undefined control sequence" in red (`#cc0000`).
    const html = renderKatex('\\f\\relax{x} = \\int_{-\\infty}^{\\infty} \\f\\hat\\xi\\,d\\xi', true)
    expect(html).toContain('<mi>f</mi><mo stretchy="false">(</mo><mi>x</mi>')
    expect(html).not.toContain('mathcolor="#cc0000"')
  })

  test('delimited math resolves shared macros like \\R', () => {
    const html = render('The reals are $\\R$ and the naturals $\\N$.')
    expect(html).toContain('katex')
    expect(html).not.toContain('mathcolor="#cc0000"')
  })

  test('a latex fence renders as display math, not a code block', () => {
    const html = render('```latex\n\\f\\relax{x} = x^2\n```')
    expect(html).toContain('katex-display')
    expect(html).not.toContain('language-latex')
  })
})

describe('MarkdownView raw HTML (GitHub-style constraints)', () => {
  test('safe inline HTML renders', () => {
    const html = render('Press <kbd>⌘K</kbd> to save and <b>bold</b> works.')
    expect(html).toContain('<kbd>⌘K</kbd>')
    expect(html).toContain('<b>bold</b>')
  })

  test('details and summary render as a collapsible block', () => {
    const html = render('<details>\n<summary>More</summary>\nHidden body\n</details>')
    expect(html).toContain('<details>')
    expect(html).toContain('<summary>More</summary>')
    expect(html).toContain('Hidden body')
  })

  test('scripts and event handlers are stripped', () => {
    const html = render('Hello <script>alert(1)</script> <img src="x" onerror="alert(2)">')
    expect(html).not.toContain('script')
    expect(html).not.toContain('onerror')
    expect(html).toContain('Hello')
  })

  test('javascript: URLs are dropped from links', () => {
    const html = render('[click me](javascript:alert(1))')
    expect(html).not.toContain('javascript:')
    expect(html).toContain('click me')
  })

  test('data URI images survive the src policy', () => {
    const html = render('![diagram](data:image/png;base64,AAAA)')
    expect(html).toContain('data:image/png')
  })

  test('math still renders alongside raw HTML', () => {
    const html = render('The limit is $\\lim_{x \\to 0} x$ and <kbd>y</kbd>.')
    expect(html).toContain('katex')
    expect(html).toContain('<kbd>y</kbd>')
  })

  test('raw HTML inside a fenced code block stays literal text', () => {
    // The code surface is a client-side web component, so its content is not
    // present in SSR output. What matters is that the fence is not parsed:
    // the raw `<div>` must not leak out as a real element.
    const html = render('```html\n<div>not html</div>\n```')
    expect(html).toContain('padu-code-surface')
    expect(html).not.toContain('<div>not html</div>')
  })

  test('a half-typed raw tag degrades instead of crashing', () => {
    // Streaming responses can cut off inside a tag; parse5 absorbs it and the
    // sanitizer drops whatever remains, so the prose still renders.
    const html = render('Hello <di')
    expect(html).toContain('Hello')
  })
})