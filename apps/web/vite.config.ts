import babel from '@rolldown/plugin-babel'
import tailwindcss from '@tailwindcss/vite'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import viteReact, { reactCompilerPreset } from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const sitemapPages = [
  '/',
  '/settings',
  '/settings/general',
  '/settings/appearance',
  '/settings/keybindings',
  '/settings/notifications',
  '/settings/providers',
  '/settings/skills',
  '/settings/usage',
  '/settings/daemon',
  '/settings/about',
].map((routePath) => ({ path: routePath }))

export default defineConfig({
  server: {
    port: 3001,
  },
  resolve: {
    tsconfigPaths: true,
  },
  optimizeDeps: {
    // Keep the core, React wrapper, and editor on one resolved package version.
    // Otherwise a running dev server can retain the pre-editor React bundle
    // while discovering the new editor entry after a dependency update.
    include: ['@pierre/diffs', '@pierre/diffs/edit', '@pierre/diffs/react'],
  },
  worker: {
    format: 'es',
  },
  plugins: [
    tailwindcss(),
    tanstackStart({
      prerender: {
        enabled: true,
      },
      pages: sitemapPages,
    }),
    viteReact(),
    babel({ presets: [reactCompilerPreset()] }),
  ],
})
