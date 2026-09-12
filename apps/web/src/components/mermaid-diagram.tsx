import type { MermaidConfig } from 'mermaid'
import { useEffect, useState } from 'react'
import { PaduIcon } from '@/components/padu-icon'

const SANS_FAMILY = "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"

/// Renders a ` ```mermaid ` code block client-side. Mermaid is ~MBs, so it is
/// loaded lazily on first use and the library instance is shared. The SVG it
/// produces is kept in state and applied through `dangerouslySetInnerHTML`, so
/// React's virtual DOM never loses sync with the painted diagram (which a
/// direct `innerHTML` write on a React-owned node causes, surfacing as a
/// `removeChild` crash on the next render).
export function MermaidDiagram({ code }: { code: string }) {
  const [theme, setTheme] = useState<'light' | 'dark'>(() => (isDarkTheme() ? 'dark' : 'light'))
  const [status, setStatus] = useState<'rendering' | 'ready' | 'error'>('rendering')
  const [svg, setSvg] = useState('')
  const [message, setMessage] = useState('')

  useEffect(() => {
    const observer = new MutationObserver(() => setTheme(isDarkTheme() ? 'dark' : 'light'))
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    let cancelled = false
    const render = async () => {
      setStatus('rendering')
      setSvg('')
      setMessage('')
      try {
        const { default: mermaid } = await import('mermaid')
        if (cancelled) return
        mermaid.initialize(mermaidConfig(theme))
        const id = `padu-mermaid-${mermaidInstance++}`
        const rendered = await mermaid.render(id, code)
        if (cancelled) return
        setSvg(rendered.svg)
        setStatus('ready')
      } catch (error) {
        if (cancelled) return
        setStatus('error')
        setMessage(error instanceof Error ? error.message : String(error))
      }
    }
    void render()
    return () => {
      cancelled = true
    }
  }, [code, theme])

  return (
    <div className="max-w-full overflow-x-auto rounded-xl border bg-muted/45 p-4">
      {status === 'error' ? (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-1.5 text-[11.5px] text-[var(--text-tertiary)]">
            <PaduIcon className="size-3.5" name="alert" />
            <span className="truncate">{message || 'Diagram failed to render'}</span>
          </div>
          <pre className="overflow-x-auto whitespace-pre-wrap font-mono text-[12px] leading-5 text-[var(--text-secondary)]">
            {code}
          </pre>
        </div>
      ) : status === 'ready' ? (
        <div className="mermaid-diagram" dangerouslySetInnerHTML={{ __html: svg }} />
      ) : (
        <div className="mermaid-diagram">
          <span className="flex items-center gap-1.5 text-[11.5px] text-[var(--text-tertiary)]">
            <PaduIcon className="size-3.5 motion-safe:animate-spin" name="loaderCircle" />
            Rendering diagram…
          </span>
        </div>
      )}
    </div>
  )
}

function isDarkTheme(): boolean {
  return typeof document !== 'undefined'
    && document.documentElement.classList.contains('dark')
}

let mermaidInstance = 0

/// Mermaid config derived from the app's design tokens. Colors are read from
/// the CSS custom properties at render time and resolved to solid hex (any
/// translucency is composited over the surface), so a diagram always matches
/// the active theme. `theme: 'base'` + `themeVariables` replaces mermaid's
/// cartoonish defaults with the same surface, border, and text tokens the rest
/// of the app uses.
function mermaidConfig(theme: 'light' | 'dark'): MermaidConfig {
  const css = (name: string, fallback: string) => readCssColor(name, fallback)
  const foreground = css('--foreground', theme === 'dark' ? '#e2e2e2' : '#242424')
  const background = css('--background', theme === 'dark' ? '#1a1a1a' : '#f6f5f6')
  const card = css('--card', theme === 'dark' ? '#212121' : '#ffffff')
  const muted = css('--muted', theme === 'dark' ? '#232323' : '#ececec')
  const secondary = css('--text-secondary', theme === 'dark' ? '#a3a3a3' : '#666666')
  const tertiary = css('--text-tertiary', theme === 'dark' ? '#7d7d7d' : '#858585')
  const border = css('--border', theme === 'dark' ? '#2c2c2c' : '#e4e4e4')
  const ring = css('--ring', theme === 'dark' ? '#8b5cf6' : '#7c3aed')
  const success = css('--success', theme === 'dark' ? '#62c987' : '#2f8f52')
  const warning = css('--warning', theme === 'dark' ? '#e0b36a' : '#a66b20')
  const danger = css('--destructive', theme === 'dark' ? '#e2726a' : '#c64a42')

  return {
    startOnLoad: false,
    securityLevel: 'strict',
    fontFamily: SANS_FAMILY,
    theme: 'base',
    themeVariables: {
      fontFamily: SANS_FAMILY,
      fontSize: '14px',
      background,
      // Flat, shadow-free nodes: mermaid's injected styles gate both the
      // drop-shadow filters and gradient fills on these flags, so disabling
      // them yields the clean card look instead of the cartoonish default.
      dropShadow: 'none',
      useGradient: false,
      primaryColor: card,
      primaryBorderColor: border,
      primaryTextColor: foreground,
      secondaryColor: muted,
      secondaryBorderColor: border,
      secondaryTextColor: foreground,
      tertiaryColor: muted,
      tertiaryBorderColor: border,
      tertiaryTextColor: foreground,
      lineColor: secondary,
      textColor: foreground,
      mainBkg: card,
      nodeBorder: border,
      nodeTextColor: foreground,
      clusterBkg: background,
      clusterBorder: border,
      titleColor: secondary,
      edgeLabelBackground: card,
      edgeLabelColor: foreground,
      // Sequence diagram
      actorBkg: card,
      actorBorder: border,
      actorTextColor: foreground,
      actorLineColor: tertiary,
      signalColor: secondary,
      signalTextColor: foreground,
      labelBoxBkgColor: card,
      labelBoxBorderColor: border,
      labelTextColor: foreground,
      loopTextColor: secondary,
      activationBkgColor: muted,
      activationBorderColor: border,
      sequenceNumberColor: tertiary,
      noteBkgColor: muted,
      noteBorderColor: border,
      noteTextColor: foreground,
      // Class diagram
      classText: foreground,
      classBg: card,
      classBorder: border,
      classArrow: secondary,
      // ER diagram
      entityBkg: card,
      entityBorder: border,
      attributeBkg: muted,
      attributeBorder: border,
      // Pie chart
      pie1: ring,
      pie2: success,
      pie3: warning,
      pie4: secondary,
      pie5: tertiary,
      pie6: danger,
      pieTitleTextColor: foreground,
      pieSectionTextColor: foreground,
      pieLegendTextColor: secondary,
      // Gantt
      taskBkgColor: muted,
      taskBorderColor: border,
      sectionBkgColor: background,
      altSectionBkgColor: background,
      gridColor: border,
      todayLineColor: ring,
    },
    flowchart: {
      htmlLabels: true,
      curve: 'linear',
      padding: 12,
      nodeSpacing: 40,
      rankSpacing: 50,
      useMaxWidth: true,
    },
    sequence: {
      useMaxWidth: true,
      actorMargin: 40,
      messageMargin: 35,
      boxMargin: 8,
    },
    pie: { useMaxWidth: true },
    er: { useMaxWidth: true },
    // The design system's card radius (--radius is 0.625rem = 10px). Nodes are
    // drawn as `<rect>`s with an `rx` attribute; CSS `rx`/`ry` override that
    // attribute, so every rectangular node picks up the app's rounded corners.
    themeCSS: `
      .node rect {
        rx: 10px;
        ry: 10px;
      }
    `,
  }
}

export type RGBA = { r: number; g: number; b: number; a: number }

/// Resolve a CSS custom property to a solid hex color, compositing any
/// translucency over the app surface so mermaid's color math always sees a
/// plain opaque value.
function readCssColor(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback
  const style = getComputedStyle(document.documentElement)
  const surface = parseColor(style.getPropertyValue('--background').trim())
    ?? { r: 255, g: 255, b: 255, a: 1 }
  const value = style.getPropertyValue(name).trim()
  const parsed = value ? parseColor(value) : null
  return parsed ? hexColor(composite(parsed, surface)) : fallback
}

/// Pure color math, exported for tests.
export function parseColor(value: string): RGBA | null {
  let match = /^#([0-9a-f]{3,8})$/i.exec(value)
  if (match) {
    let hex = match[1]
    if (hex.length === 3 || hex.length === 4) {
      hex = hex.split('').map((c) => c + c).join('')
    }
    if (hex.length === 6) {
      return {
        r: parseInt(hex.slice(0, 2), 16),
        g: parseInt(hex.slice(2, 4), 16),
        b: parseInt(hex.slice(4, 6), 16),
        a: 1,
      }
    }
    if (hex.length === 8) {
      return {
        r: parseInt(hex.slice(0, 2), 16),
        g: parseInt(hex.slice(2, 4), 16),
        b: parseInt(hex.slice(4, 6), 16),
        a: parseInt(hex.slice(6, 8), 16) / 255,
      }
    }
    return null
  }

  match = /^rgba?\(([^)]+)\)$/i.exec(value)
  if (match) {
    const parts = match[1].split(/[\s,/]+/).filter(Boolean)
    const channel = (raw: string) => raw.endsWith('%')
      ? Math.round(255 * parseFloat(raw) / 100)
      : parseFloat(raw)
    const r = channel(parts[0] ?? '')
    const g = channel(parts[1] ?? '')
    const b = channel(parts[2] ?? '')
    let a = 1
    if (parts[3]) a = parts[3].endsWith('%') ? parseFloat(parts[3]) / 100 : parseFloat(parts[3])
    if (Number.isFinite(r) && Number.isFinite(g) && Number.isFinite(b)) {
      return { r, g, b, a }
    }
    return null
  }

  match = /^hsla?\(([^)]+)\)$/i.exec(value)
  if (match) {
    const parts = match[1].split(/[\s,/]+/).filter(Boolean)
    const h = parseFloat(parts[0] ?? '')
    const s = parseFloat(parts[1] ?? '') / 100
    const l = parseFloat(parts[2] ?? '') / 100
    let a = 1
    if (parts[3]) a = parts[3].endsWith('%') ? parseFloat(parts[3]) / 100 : parseFloat(parts[3])
    if (!Number.isFinite(h) || !Number.isFinite(s) || !Number.isFinite(l)) return null
    const c = (1 - Math.abs(2 * l - 1)) * s
    const x = c * (1 - Math.abs(((h / 60) % 2) - 1))
    const m = l - c / 2
    let r = 0
    let g = 0
    let b = 0
    if (h < 60) [r, g, b] = [c, x, 0]
    else if (h < 120) [r, g, b] = [x, c, 0]
    else if (h < 180) [r, g, b] = [0, c, x]
    else if (h < 240) [r, g, b] = [0, x, c]
    else if (h < 300) [r, g, b] = [x, 0, c]
    else [r, g, b] = [c, 0, x]
    return {
      r: Math.round((r + m) * 255),
      g: Math.round((g + m) * 255),
      b: Math.round((b + m) * 255),
      a,
    }
  }
  return null
}

export function composite(fg: RGBA, bg: RGBA): RGBA {
  const alpha = fg.a + bg.a * (1 - fg.a)
  if (alpha === 0) return { r: 0, g: 0, b: 0, a: 0 }
  return {
    r: Math.round((fg.r * fg.a + bg.r * bg.a * (1 - fg.a)) / alpha),
    g: Math.round((fg.g * fg.a + bg.g * bg.a * (1 - fg.a)) / alpha),
    b: Math.round((fg.b * fg.a + bg.b * bg.a * (1 - fg.a)) / alpha),
    a: alpha,
  }
}

export function hexColor({ r, g, b }: RGBA): string {
  return `#${[r, g, b].map((value) => value.toString(16).padStart(2, '0')).join('')}`
}