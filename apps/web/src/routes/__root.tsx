/// <reference types="vite/client" />
import type { QueryClient } from '@tanstack/react-query'
import {
  HeadContent,
  Outlet,
  Scripts,
  createRootRouteWithContext,
} from '@tanstack/react-router'
import type { ReactNode } from 'react'
import { Toaster } from 'sonner'
import { DaemonProvider } from '@/lib/daemon-context'
import { RuntimeProvider } from '@/lib/runtime-context'
import appCss from '@/styles.css?url'

const TITLE = 'Padu Web'
const DESCRIPTION =
  'Connect securely to a Padu daemon and continue your coding-agent tasks from the browser.'

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient
}>()({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      {
        name: 'viewport',
        content: 'width=device-width, initial-scale=1, viewport-fit=cover',
      },
      { title: TITLE },
      { name: 'description', content: DESCRIPTION },
      { name: 'robots', content: 'noindex, nofollow' },
      {
        name: 'theme-color',
        media: '(prefers-color-scheme: light)',
        content: '#f7f7f6',
      },
      {
        name: 'theme-color',
        media: '(prefers-color-scheme: dark)',
        content: '#191a1a',
      },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: "icon", href: "/favicon.ico", sizes: "48x48" },
      { rel: "icon", href: "/padu.svg", type: "image/svg+xml" },
      { rel: "apple-touch-icon", href: "/apple-touch-icon.png" },
    ],
    scripts: [
      {
        children: `try{var d=document.documentElement,p=localStorage.getItem('padu.theme'),s=matchMedia('(prefers-color-scheme: dark)').matches,x=p==='dark'||p!=='light'&&s,l=localStorage.getItem('padu.language'),n=(navigator.languages&&navigator.languages[0]||navigator.language||'en').replaceAll('_','-').toLowerCase(),r=l==='zh-CN'||l==='ja'||l==='id'||l==='en'?l:n==='zh-cn'||n==='zh-sg'||n.startsWith('zh-hans')?'zh-CN':n==='ja'||n.startsWith('ja-')?'ja':n==='id'||n.startsWith('id-')?'id':'en';d.classList.toggle('dark',x);d.classList.toggle('light',!x);d.lang=r}catch(e){}`,
      },
    ],
  }),
  component: RootComponent,
})

function RootComponent() {
  return (
    <RootDocument>
      <Outlet />
    </RootDocument>
  )
}

function RootDocument({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <HeadContent />
      </head>
      <body>
        <DaemonProvider>
          <RuntimeProvider>{children}</RuntimeProvider>
        </DaemonProvider>
        <Toaster
          className="padu-toaster"
          closeButton
          gap={8}
          position="top-center"
          toastOptions={{
            className: 'padu-toast',
            style: {
              background: 'var(--popover)',
              border: '1px solid var(--input)',
              color: 'var(--popover-foreground)',
            },
          }}
        />
        <Scripts />
      </body>
    </html>
  )
}
