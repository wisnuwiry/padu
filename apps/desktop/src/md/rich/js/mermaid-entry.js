// Mermaid render entry for the rich-render QuickJS isolate.
//
// Bundled as a standalone IIFE that runs after shim.js has installed the DOM.
// Exposes `__paduRenderMermaid(id, code, themeJson)` which resolves to
// `{ svg: string }` or rejects with a render error. Theme colors are derived
// from the app palette (mirroring the web client's mermaid config) so a
// diagram always matches the active theme. `flowchart.htmlLabels: false` is
// mandatory: htmlLabels emit `<foreignObject>` HTML that resvg cannot raster.

import mermaid from 'mermaid'

globalThis.__paduRenderMermaid = function (id, code, themeJson) {
  const theme = JSON.parse(themeJson)
  const config = {
    startOnLoad: false,
    securityLevel: 'strict',
    fontFamily: theme.fontFamily,
    theme: 'base',
    themeVariables: {
      fontFamily: theme.fontFamily,
      fontSize: `${theme.fontPx}px`,
      background: theme.surface,
      // Flat, shadow-free nodes: mermaid's injected styles gate both the
      // drop-shadow filters and gradient fills on these flags, so disabling
      // them yields the clean card look instead of the cartoonish default.
      dropShadow: 'none',
      useGradient: false,
      primaryColor: theme.card,
      primaryBorderColor: theme.border,
      primaryTextColor: theme.text,
      secondaryColor: theme.card,
      secondaryBorderColor: theme.border,
      secondaryTextColor: theme.text,
      tertiaryColor: theme.card,
      tertiaryBorderColor: theme.border,
      tertiaryTextColor: theme.text,
      lineColor: theme.secondary,
      textColor: theme.text,
      mainBkg: theme.card,
      nodeBorder: theme.border,
      nodeTextColor: theme.text,
      clusterBkg: theme.surface,
      clusterBorder: theme.border,
      titleColor: theme.secondary,
      edgeLabelBackground: theme.card,
      edgeLabelColor: theme.text,
      // Sequence diagram
      actorBkg: theme.card,
      actorBorder: theme.border,
      actorTextColor: theme.text,
      actorLineColor: theme.tertiary,
      signalColor: theme.secondary,
      signalTextColor: theme.text,
      labelBoxBkgColor: theme.card,
      labelBoxBorderColor: theme.border,
      labelTextColor: theme.text,
      loopTextColor: theme.secondary,
      activationBkgColor: theme.card,
      activationBorderColor: theme.border,
      sequenceNumberColor: theme.tertiary,
      noteBkgColor: theme.card,
      noteBorderColor: theme.border,
      noteTextColor: theme.text,
      // Class diagram
      classText: theme.text,
      classBg: theme.card,
      classBorder: theme.border,
      classArrow: theme.secondary,
      // ER diagram
      entityBkg: theme.card,
      entityBorder: theme.border,
      attributeBkg: theme.card,
      attributeBorder: theme.border,
      // Pie chart
      pie1: theme.accent,
      pie2: theme.success,
      pie3: theme.warning,
      pie4: theme.secondary,
      pie5: theme.tertiary,
      pie6: theme.danger,
      pieTitleTextColor: theme.text,
      pieSectionTextColor: theme.text,
      pieLegendTextColor: theme.secondary,
      // Gantt
      taskBkgColor: theme.card,
      taskBorderColor: theme.border,
      sectionBkgColor: theme.surface,
      altSectionBkgColor: theme.surface,
      gridColor: theme.border,
      todayLineColor: theme.accent,
    },
    flowchart: {
      htmlLabels: false,
      curve: 'linear',
      padding: 12,
      nodeSpacing: 40,
      rankSpacing: 50,
      useMaxWidth: true,
    },
    sequence: { useMaxWidth: true, actorMargin: 40, messageMargin: 35, boxMargin: 8 },
    pie: { useMaxWidth: true },
    er: { useMaxWidth: true },
    themeCSS: '.node rect { rx: 10px; ry: 10px; }',
  }
  mermaid.initialize(config)
  return mermaid.render(id, code).then((out) => ({ svg: out.svg }))
}