// Build the rich-render JS bundles (KaTeX→MathJax SVG and Mermaid + DOM shim)
// that the desktop markdown renderer embeds via `include_str!`.
//
// The outputs are committed so the desktop crate compiles offline: re-run this
// script (`bun ./scripts/build-rich-render.ts`) whenever the sources in
// `apps/desktop/src/md/rich/js/` or the mermaid/mathjax-full dependencies
// change. QuickJS (the isolate that evals these bundles) is ES2020-ish, so
// everything is transpiled to `es2020` and the DOM shim is emitted as its own
// IIFE that runs before the mermaid bundle — ESM imports are hoisted above
// module-body setup, so static imports in the same file could not install the
// DOM before mermaid initializes.

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

// Mermaid: shim first (installs the DOM), then the mermaid entry.
await bundle('shim.js', 'shim.bundle.js')
await bundle('mermaid-entry.js', 'mermaid-entry.bundle.js')
const shim = await Bun.file(join(outDir, 'shim.bundle.js')).text()
const mermaid = await Bun.file(join(outDir, 'mermaid-entry.bundle.js')).text()
await Bun.write(join(outDir, 'mermaid.bundle.js'), shim + '\n' + mermaid)

await bundle('math-entry.js', 'math.bundle.js')

// Keep the intermediate pieces out of the repo; only the concatenated bundles
// and the sources are committed.
await rm(join(outDir, 'shim.bundle.js'))
await rm(join(outDir, 'mermaid-entry.bundle.js'))
await rm(tmpDir, { recursive: true, force: true })

const sizes = {
  mermaid: (await Bun.file(join(outDir, 'mermaid.bundle.js')).size) / 1024 / 1024,
  math: (await Bun.file(join(outDir, 'math.bundle.js')).size) / 1024 / 1024,
}
console.log(`built rich-render bundles: mermaid ${sizes.mermaid.toFixed(2)} MB, math ${sizes.math.toFixed(2)} MB`)