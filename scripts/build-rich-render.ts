// Build the rich-render JS bundle (MathJax SVG) that the desktop markdown
// renderer embeds via `include_str!`. Mermaid renders natively through the
// `padu-mermaid` crate and needs no bundle.
//
// The output is committed so the desktop crate compiles offline: re-run this
// script (`bun ./scripts/build-rich-render.ts`) whenever the sources in
// `apps/desktop/src/md/rich/js/` or the mathjax-full dependency change.
// QuickJS (the isolate that evals this bundle) is ES2020-ish, so everything
// is transpiled to `es2020`.

import { build } from 'bun'
import { cp, mkdir, rm } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(fileURLToPath(import.meta.url))
const sourceDir = join(root, '../apps/desktop/src/md/rich/js')
const outDir = sourceDir

const common = {
  format: 'iife' as const,
  target: 'browser' as const,
  minify: true,
  legalComments: 'none' as const,
}

async function bundle(entry: string, outfile: string) {
  await build({
    ...common,
    entrypoints: [join(sourceDir, entry)],
    outdir: tmpDir,
  })
  const [name] = entry.split('.')
  await cp(join(tmpDir, `${name}.js`), join(outDir, outfile))
}

const tmpDir = join(root, '../temp/rich-render-build')
await mkdir(tmpDir, { recursive: true })

await bundle('math-entry.js', 'math.bundle.js')

await rm(tmpDir, { recursive: true, force: true })

const sizes = {
  math: (await Bun.file(join(outDir, 'math.bundle.js')).size) / 1024 / 1024,
}
console.log(`built rich-render bundles: math ${sizes.math.toFixed(2)} MB`)
