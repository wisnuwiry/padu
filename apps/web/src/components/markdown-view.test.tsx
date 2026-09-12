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