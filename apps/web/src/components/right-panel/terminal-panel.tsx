import { GhosttyCore } from '@wterm/ghostty'
import { Terminal, useTerminal } from '@wterm/react'
import { useEffect, useRef, useState } from 'react'
import type { AgentSession, Project } from '@padu/client'
import { PaduIcon } from '@/components/padu-icon'
import { useDaemon } from '@/lib/daemon-context'
import { useI18n } from '@/lib/i18n'
import { sessionCwd } from '@/lib/daemon-api'
import { errorMessage, encodeBase64, decodeBase64, clampU16, PanelMessage } from './shared'

export function TerminalPanel({
  terminalId,
  session,
  project,
  onTitle,
}: {
  terminalId: string
  session: AgentSession | null
  project?: Project
  onTitle: (title: string) => void
}) {
  const { t } = useI18n()
  const { client, phase } = useDaemon()
  const { ref, write, focus } = useTerminal()
  const [core, setCore] = useState<GhosttyCore | null>(null)
  const ready = useRef(false)
  const queuedOutput = useRef<Uint8Array[]>([])
  const titleScanner = useRef(new TerminalTitleScanner())
  const [error, setError] = useState<string | null>(null)
  const [exited, setExited] = useState(false)
  const cwd = session && project ? sessionCwd(session, project) : undefined

  useEffect(() => {
    let disposed = false
    void GhosttyCore.load()
      .then((loaded) => {
        if (!disposed) setCore(loaded)
      })
      .catch((cause) => {
        if (!disposed) setError(errorMessage(cause))
      })
    return () => {
      disposed = true
    }
  }, [])

  useEffect(() => {
    if (!client || phase !== 'connected' || !cwd) return
    let disposed = false
    titleScanner.current = new TerminalTitleScanner()
    const unsubscribe = client.subscribe(terminalId, terminalId, (event) => {
      if (event.event.kind === 'terminalOutput') {
        const payload = event.event.payload as { data?: string }
        if (!payload.data) return
        const bytes = decodeBase64(payload.data)
        const title = titleScanner.current.feed(bytes)
        if (title) onTitle(title)
        if (ready.current) write(bytes)
        else queuedOutput.current.push(bytes)
      } else if (event.event.kind === 'terminalExited') {
        setExited(true)
      } else if (event.event.kind === 'terminalError') {
        setError(typeof event.event.payload === 'string' ? event.event.payload : t('terminal.transport_failed'))
      }
    })

    void client.request(
      { type: 'openTerminal', cwd, cols: 80, rows: 24 },
      terminalId,
      terminalId,
    ).catch((cause) => {
      if (!disposed) setError(errorMessage(cause))
    })

    return () => {
      disposed = true
      ready.current = false
      queuedOutput.current = []
      unsubscribe()
      void client.notify({ type: 'closeTerminal' }, terminalId, terminalId).catch(() => {})
    }
  }, [client, cwd, phase, terminalId, write])

  if (!cwd) return <PanelMessage title={t('files.no_project_open')} detail={t('terminal.no_workspace_description')} />
  return (
    <div className="relative flex min-h-0 flex-1 bg-background">
      {core && (
        <Terminal
          autoResize
          className="padu-terminal size-full"
          core={core}
          cursorBlink
          ref={ref}
          onData={(data) => {
            if (!client || phase !== 'connected' || exited) return
            void client.notify(
              { type: 'writeTerminal', data: encodeBase64(new TextEncoder().encode(data)) },
              terminalId,
              terminalId,
            ).catch((cause) => setError(errorMessage(cause)))
          }}
          onError={(cause) => setError(errorMessage(cause))}
          onReady={() => {
            ready.current = true
            for (const bytes of queuedOutput.current) write(bytes)
            queuedOutput.current = []
            focus()
          }}
          onResize={(cols, rows) => {
            if (!client || phase !== 'connected') return
            void client.notify(
              { type: 'resizeTerminal', cols: clampU16(cols), rows: clampU16(rows) },
              terminalId,
              terminalId,
            ).catch(() => {})
          }}
        />
      )}
      {(error || exited) && (
        <div className="pointer-events-none absolute bottom-2 right-2 max-w-[calc(100%-16px)] rounded-md border bg-popover px-2 py-1 text-[10.5px] text-[var(--text-secondary)] shadow-sm">
          {error ?? t('terminal.process_exited')}
        </div>
      )}
    </div>
  )
}

class TerminalTitleScanner {
  private readonly decoder = new TextDecoder()
  private pending = ''

  feed(bytes: Uint8Array): string | null {
    const text = this.pending + this.decoder.decode(bytes, { stream: true })
    const pattern = /\x1b\](?:0|2);([^\x07\x1b]*)(?:\x07|\x1b\\)/g
    let title: string | null = null
    let consumed = 0
    for (const match of text.matchAll(pattern)) {
      title = match[1]?.replace(/[\x00-\x1f\x7f]/g, '').trim().slice(0, 80) || null
      consumed = (match.index ?? 0) + match[0].length
    }

    const incompleteOsc = text.lastIndexOf('\x1b]')
    if (incompleteOsc >= consumed) this.pending = text.slice(incompleteOsc, incompleteOsc + 1_024)
    else this.pending = text.endsWith('\x1b') ? '\x1b' : ''
    return title
  }
}
