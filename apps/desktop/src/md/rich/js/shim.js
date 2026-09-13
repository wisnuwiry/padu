// Synchronous DOM shim for the rich-render QuickJS isolate.
//
// Mermaid lays out its diagrams against a real browser DOM (d3-selection
// drives attribute/element work and reads geometry through `getBBox` /
// `getComputedTextLength` / `getBoundingClientRect`). QuickJS has no DOM, so
// this file installs a minimal one built on `linkedom` plus a geometric
// measurement layer. This file is bundled as its own IIFE that runs BEFORE the
// mermaid bundle, so `window`/`document` exist by the time mermaid
// initializes. Text measurement is a glyph-width heuristic; final raster
// sizing is done by GPUI's resvg pass, which measures real glyphs.

import { parseHTML } from 'linkedom'

const { window, document, HTMLElement, SVGElement, Node, DOMParser, MutationObserver } = parseHTML(
  '<!doctype html><html><head></head><body></body></html>',
)

globalThis.window = window
globalThis.document = document
globalThis.HTMLElement = HTMLElement
globalThis.SVGElement = SVGElement
globalThis.Node = Node
globalThis.DOMParser = DOMParser
globalThis.MutationObserver = MutationObserver

try {
  globalThis.navigator = { userAgent: 'padu', language: 'en' }
} catch {
  Object.defineProperty(globalThis, 'navigator', {
    value: { userAgent: 'padu', language: 'en' },
    configurable: true,
  })
}

window.devicePixelRatio = 1
window.matchMedia = () => ({ matches: false, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} })
window.requestAnimationFrame = (fn) => setTimeout(() => fn(Date.now()), 0)
window.cancelAnimationFrame = (id) => clearTimeout(id)
window.scrollTo = () => {}
window.getComputedStyle = () => ({ getPropertyValue: () => '' })

const num = (value) => {
  const n = parseFloat(value)
  return Number.isFinite(n) ? n : 0
}

// Estimated bounding box for the SVG subset mermaid paints. Text is measured
// by a width heuristic (≈0.6em per glyph at the resolved font size), shapes by
// their explicit geometry attributes, groups by the union of their children.
const measureElement = (el) => {
  const tag = (el.tagName || '').toLowerCase()
  const text = (el.textContent || '').replace(/\s+/g, ' ')
  const fontSize = num(el.getAttribute?.('font-size')) || 16
  let x = num(el.getAttribute?.('x'))
  let y = num(el.getAttribute?.('y'))
  if (tag === 'svg') {
    const vb = (el.getAttribute?.('viewBox') || '').split(/[\s,]+/)
    if (vb.length === 4 && vb.every((v) => !Number.isNaN(num(v)))) {
      return { x: num(vb[0]), y: num(vb[1]), width: num(vb[2]), height: num(vb[3]) }
    }
  }
  if (tag === 'text' || tag === 'tspan') {
    return { x, y, width: text.length * fontSize * 0.6, height: fontSize * 1.2 }
  }
  if (tag === 'circle') {
    const r = num(el.getAttribute?.('r'))
    return { x: x - r, y: y - r, width: r * 2, height: r * 2 }
  }
  const box = { x, y, width: num(el.getAttribute?.('width')), height: num(el.getAttribute?.('height')) }
  const children = el.children || []
  for (const child of children) {
    const childBox = measureElement(child)
    if (childBox.width === 0 && childBox.height === 0) continue
    const right = box.x + box.width
    const bottom = box.y + box.height
    const childRight = childBox.x + childBox.width
    const childBottom = childBox.y + childBox.height
    box.x = Math.min(box.x, childBox.x)
    box.y = Math.min(box.y, childBox.y)
    box.width = Math.max(right, childRight) - box.x
    box.height = Math.max(bottom, childBottom) - box.y
  }
  return box
}

const patchBBox = (el) => {
  if (!el || el.__paduPatched) return el
  el.__paduPatched = true
  el.getBBox = () => measureElement(el)
  if (typeof el.getComputedTextLength !== 'function') {
    el.getComputedTextLength = () => {
      const text = el.textContent || ''
      const fontSize = num(el.getAttribute?.('font-size')) || 16
      return text.length * fontSize * 0.6
    }
  }
  if (typeof el.getBoundingClientRect !== 'function') {
    el.getBoundingClientRect = () => {
      const box = measureElement(el)
      return {
        x: box.x, y: box.y, width: box.width, height: box.height,
        top: box.y, left: box.x, right: box.x + box.width, bottom: box.y + box.height,
      }
    }
  }
  return el
}

const origCreateElementNS = document.createElementNS.bind(document)
document.createElementNS = (ns, name) => patchBBox(origCreateElementNS(ns, name))
const origCreateElement = document.createElement.bind(document)
document.createElement = (name) => patchBBox(origCreateElement(name))
const origAppend = Node.prototype.appendChild
Node.prototype.appendChild = function (child) {
  patchBBox(child)
  return origAppend.call(this, child)
}

// Minimal CSSStyleSheet: mermaid assembles its theme CSS through
// `insertRule`/`cssRules` (and optionally `replaceSync` for `themeCSS`).
class CSSStyleSheet {
  constructor() {
    this.cssRules = []
  }
  insertRule(rule, index = 0) {
    this.cssRules.splice(index, 0, { cssText: String(rule) })
    return this.cssRules.length - 1
  }
  deleteRule(index) {
    this.cssRules.splice(index, 1)
  }
  replaceSync(css) {
    this.cssRules = String(css)
      .split(/}\s*/)
      .filter((chunk) => chunk.trim())
      .map((chunk) => ({ cssText: chunk.endsWith('}') ? chunk : chunk + '}' }))
    return this
  }
}
globalThis.CSSStyleSheet = CSSStyleSheet
window.CSSStyleSheet = CSSStyleSheet