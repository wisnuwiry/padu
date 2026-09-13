// Math (LaTeX → SVG) render entry for the rich-render QuickJS isolate.
//
// KaTeX — the web client's engine — only emits HTML/CSS, never SVG, so the
// desktop renderer uses MathJax v3's SVG output jax instead. It runs entirely
// headless via the `liteAdaptor` bundled inside mathjax-full and produces an
// SVG string directly, with the same `$…$` / `$$…$$` syntax surface and the
// web client's shared macro set.
//
// Exposes `__paduRenderMath(source, display, themeJson)` returning
// `{ svg, verticalAlignEx }` or `{ error }`. The SVG is normalized so resvg
// can raster it: `ex`-unit width/height are converted to absolute pixels and
// the theme text color is injected as the `color` attribute (MathJax paints
// its glyphs with `currentColor`).

import { mathjax } from 'mathjax-full/js/mathjax.js'
import { TeX } from 'mathjax-full/js/input/tex.js'
import { SVG } from 'mathjax-full/js/output/svg.js'
import { liteAdaptor } from 'mathjax-full/js/adaptors/liteAdaptor.js'
import { RegisterHTMLHandler } from 'mathjax-full/js/handlers/html.js'
import { AllPackages } from 'mathjax-full/js/input/tex/AllPackages.js'

const adaptor = liteAdaptor()
RegisterHTMLHandler(adaptor)

const tex = new TeX({
  packages: AllPackages,
  macros: {
    '\\f': '#1f(#2)',
    '\\R': '\\mathbb{R}',
    '\\N': '\\mathbb{N}',
    '\\Z': '\\mathbb{Z}',
    '\\Q': '\\mathbb{Q}',
    '\\C': '\\mathbb{C}',
  },
})
const svg = new SVG({ fontCache: 'none' })
const doc = mathjax.document('', { InputJax: tex, OutputJax: svg })

globalThis.__paduRenderMath = function (source, display, themeJson) {
  try {
    const theme = JSON.parse(themeJson)
    const fontPx = Number(theme.fontPx) || 14
    const em = fontPx
    const exPx = em / 2
    const node = doc.convert(source, { display: Boolean(display), em, ex: exPx, containerWidth: 800 })
    let svgString = adaptor.innerHTML(node)
    const verticalAlign = /vertical-align:\s*(-?[\d.]+)ex/.exec(svgString)
    const verticalAlignEx = verticalAlign ? Number(verticalAlign[1]) : 0
    svgString = svgString
      .replace(/\s(width|height)="([\d.]+)ex"/g, (_m, unit, value) => {
        const px = (Number(value) * exPx).toFixed(2)
        return ` ${unit}="${px}"`
      })
      .replace(/<svg([^>]*)>/, (whole, attrs) => {
        const withColor = attrs.includes(' color=')
          ? attrs
          : ` color="${theme.text}"${attrs}`
        return `<svg${withColor}>`
      })
      .replace(/\sstyle="[^"]*"/, '')
    return { svg: svgString, verticalAlignEx }
  } catch (e) {
    const message = (e && e.message) || String(e)
    return { error: String(message) }
  }
}