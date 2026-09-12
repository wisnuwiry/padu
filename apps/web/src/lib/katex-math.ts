import katex from 'katex'

/// KaTeX ships with no macros. Agents and notes routinely rely on shorthands,
/// so every markdown surface pre-registers a small set — including the `\f`
/// macro from KaTeX's own homepage example, which fails with an "Undefined
/// control sequence" without it.
export const KATEX_MACROS: Record<string, string> = {
  '\\f': '#1f(#2)',
  '\\R': '\\mathbb{R}',
  '\\N': '\\mathbb{N}',
  '\\Z': '\\mathbb{Z}',
  '\\Q': '\\mathbb{Q}',
  '\\C': '\\mathbb{C}',
}

/// Render a LaTeX source to KaTeX HTML with the shared options. Used for
/// fenced ` ```latex ` blocks, which are explicit display-math escapes for
/// source that never carries `$`/`$$` delimiters.
export function renderKatex(source: string, displayMode: boolean): string {
  return katex.renderToString(source, {
    displayMode,
    throwOnError: false,
    macros: KATEX_MACROS,
  })
}