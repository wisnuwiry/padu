import { useQueryClient } from '@tanstack/react-query'
import { PaduClient, type HostKind, type HostProfile } from '@padu/client'
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import {
  clearStoredConnection,
  displayHost,
  loadActiveHostId,
  loadStoredConnection,
  loadStoredHosts,
  normalizeDaemonAddress,
  storeActiveHostId,
  storeConnection,
  storeHosts,
  validateConnectionConfig,
  type ConnectionConfig,
} from './connection'
import { translate, useI18n } from './i18n'

export type ConnectionPhase =
  | 'booting'
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'error'

interface DaemonContextValue {
  client: PaduClient | null
  config: ConnectionConfig | null
  hosts: HostProfile[]
  activeHostId: string | null
  activeHost: HostProfile | null
  phase: ConnectionPhase
  error: string | null
  connect: (config: ConnectionConfig, hostId?: string) => Promise<void>
  reconnect: () => Promise<void>
  disconnect: () => void
  forget: () => void
  addHost: (input: {
    name: string
    address: string
    token?: string
    kind?: HostKind
  }) => Promise<HostProfile>
  updateHost: (id: string, updates: Partial<HostProfile>) => Promise<void>
  removeHost: (id: string) => Promise<void>
  switchHost: (id: string | null) => Promise<void>
}

const DaemonContext = createContext<DaemonContextValue | null>(null)

export function DaemonProvider({ children }: { children: ReactNode }) {
  const { locale } = useI18n()
  const queryClient = useQueryClient()
  const [client, setClient] = useState<PaduClient | null>(null)
  const [config, setConfig] = useState<ConnectionConfig | null>(null)
  const [hosts, setHosts] = useState<HostProfile[]>(() => loadStoredHosts())
  const [activeHostId, setActiveHostId] = useState<string | null>(() => loadActiveHostId())
  const [phase, setPhase] = useState<ConnectionPhase>('booting')
  const [error, setError] = useState<string | null>(null)
  const generation = useRef(0)
  const bootstrapped = useRef(false)

  const activeHost = hosts.find((h) => h.id === activeHostId) ?? null

  const open = useCallback(
    async (candidate: ConnectionConfig, hostId?: string) => {
      const normalized = validateConnectionConfig(
        candidate,
        (key) => translate(locale, key),
      )
      const attempt = ++generation.current
      client?.disconnect()
      queryClient.clear()
      setPhase('connecting')
      setError(null)
      setConfig(normalized)

      const next = new PaduClient({
        address: normalized.address,
        token: normalized.token,
      })
      setClient(next)
      try {
        await next.connect()
        if (generation.current !== attempt) {
          next.disconnect()
          return
        }
        storeConnection(normalized)
        if (hostId !== undefined) {
          setActiveHostId(hostId)
          storeActiveHostId(hostId)
          const now = Math.floor(Date.now() / 1_000)
          setHosts((current) => {
            const next = current.map((h) =>
              h.id === hostId ? { ...h, lastConnectedAt: now, updatedAt: now } : h,
            )
            storeHosts(next)
            return next
          })
        }
        setPhase('connected')
      } catch (cause) {
        if (generation.current !== attempt) return
        const message = cause instanceof Error ? cause.message : String(cause)
        setError(message)
        setPhase('error')
        throw cause
      }
    },
    [client, locale, queryClient],
  )

  const addHost = useCallback(
    async (input: {
      name: string
      address: string
      token?: string
      kind?: HostKind
    }) => {
      const now = Math.floor(Date.now() / 1_000)
      const id = typeof crypto !== 'undefined' && crypto.randomUUID
        ? crypto.randomUUID()
        : Math.random().toString(36).slice(2)
      const normalizedAddress = normalizeDaemonAddress(
        input.address,
        (key) => translate(locale, key),
      )
      const newProfile: HostProfile = {
        id,
        name: input.name.trim() || displayHost(normalizedAddress),
        kind: input.kind ?? 'direct',
        address: normalizedAddress,
        token: input.token?.trim() || undefined,
        createdAt: now,
        updatedAt: now,
        lastConnectedAt: undefined,
      }
      const nextHosts = [...hosts, newProfile]
      setHosts(nextHosts)
      storeHosts(nextHosts)
      return newProfile
    },
    [hosts, locale],
  )

  const updateHost = useCallback(
    async (id: string, updates: Partial<HostProfile>) => {
      const now = Math.floor(Date.now() / 1_000)
      const nextHosts = hosts.map((h) =>
        h.id === id ? { ...h, ...updates, updatedAt: now } : h,
      )
      setHosts(nextHosts)
      storeHosts(nextHosts)
    },
    [hosts],
  )

  const removeHost = useCallback(
    async (id: string) => {
      const nextHosts = hosts.filter((h) => h.id !== id)
      setHosts(nextHosts)
      storeHosts(nextHosts)
      if (activeHostId === id) {
        setActiveHostId(null)
        storeActiveHostId(null)
      }
    },
    [activeHostId, hosts],
  )

  const switchHost = useCallback(
    async (id: string | null) => {
      if (id === null) {
        setActiveHostId(null)
        storeActiveHostId(null)
        const stored = loadStoredConnection()
        if (stored) {
          await open(stored)
        }
        return
      }
      const target = hosts.find((h) => h.id === id)
      if (!target) return
      await open(
        {
          address: target.address,
          token: target.token ?? '',
          remember: true,
        },
        id,
      )
    },
    [hosts, open],
  )

  useEffect(() => {
    if (bootstrapped.current) return
    bootstrapped.current = true
    const stored = loadStoredConnection()
    if (!stored) {
      setPhase('disconnected')
      return
    }
    const currentActiveId = loadActiveHostId()
    void open(stored, currentActiveId ?? undefined).catch(() => {})
    // Storage is read once at browser startup.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    if (!client || phase !== 'connected') return
    const unsubscribeTaskState = client.subscribeTaskState(() => {
      if (!config) return
      void queryClient.invalidateQueries({
        queryKey: ['daemon', config.address, 'task-state'],
      })
    })
    const timer = window.setInterval(() => {
      if (!client.connected) {
        setError(translate(locale, 'web.daemon_connection_closed'))
        setPhase('error')
      }
    }, 1_000)
    return () => {
      unsubscribeTaskState()
      window.clearInterval(timer)
    }
  }, [client, config, locale, phase, queryClient])

  const reconnect = useCallback(async () => {
    if (!client || !config) throw new Error(translate(locale, 'web.daemon_not_configured'))
    setPhase('connecting')
    setError(null)
    try {
      await client.connect()
      setPhase('connected')
      void queryClient.invalidateQueries()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
      setPhase('error')
      throw cause
    }
  }, [client, config, locale, queryClient])

  const disconnect = useCallback(() => {
    ++generation.current
    client?.disconnect()
    setPhase('disconnected')
    setError(null)
    queryClient.clear()
  }, [client, queryClient])

  const forget = useCallback(() => {
    disconnect()
    clearStoredConnection()
    setActiveHostId(null)
    storeActiveHostId(null)
    setClient(null)
    setConfig(null)
  }, [disconnect])

  useEffect(() => () => client?.disconnect(), [client])

  const value: DaemonContextValue = {
    client,
    config,
    hosts,
    activeHostId,
    activeHost,
    phase,
    error,
    connect: open,
    reconnect,
    disconnect,
    forget,
    addHost,
    updateHost,
    removeHost,
    switchHost,
  }

  return <DaemonContext.Provider value={value}>{children}</DaemonContext.Provider>
}

export function useDaemon() {
  const context = useContext(DaemonContext)
  if (!context) throw new Error('useDaemon must be used inside DaemonProvider')
  return context
}
