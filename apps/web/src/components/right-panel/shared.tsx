import { useEffect, useState } from 'react'
import type { VirtuosoHandle } from 'react-virtuoso'
import { PaduIcon } from '@/components/padu-icon'
import { cn } from '@/lib/utils'

import type { TreeNavigationKey } from '@/lib/right-panel-state'

export function PanelMessage({ title, detail, danger = false }: { title: string; detail: string; danger?: boolean }) {
  return (
    <div className="grid min-h-0 flex-1 place-items-center p-6 text-center">
      <div className="max-w-64">
        <div className={cn('text-[13px] font-medium', danger && 'text-destructive')}>{title}</div>
        <p className="mt-1.5 text-[11px] leading-4 text-[var(--text-tertiary)]">{detail}</p>
      </div>
    </div>
  )
}

export function parseNumstat(numstat: string) {
  let files = 0
  let additions = 0
  let deletions = 0
  for (const line of numstat.trim().split('\n')) {
    if (!line) continue
    const [added, removed] = line.split('\t')
    files += 1
    additions += Number.parseInt(added || '0', 10) || 0
    deletions += Number.parseInt(removed || '0', 10) || 0
  }
  return { files, additions, deletions }
}

export function encodeBase64(bytes: Uint8Array): string {
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000))
  }
  return btoa(binary)
}

export function decodeBase64(value: string): Uint8Array {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}

export function clampU16(value: number): number {
  return Math.max(1, Math.min(65_535, Math.round(value)))
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

export function panelTabId(tabId: string): string {
  return `right-panel-tab-${tabId}`
}

export function panelContentId(tabId: string): string {
  return `right-panel-content-${tabId}`
}

export function isTreeNavigationKey(key: string): key is TreeNavigationKey {
  return ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(key)
}

export function workingTreeRowId(path: string): string {
  return `working-tree-row-${encodeURIComponent(path)}`
}

export function diffTreeRowId(key: string): string {
  return `diff-tree-row-${encodeURIComponent(key)}`
}

export function focusVirtualTreeRow(
  list: { current: VirtuosoHandle | null },
  index: number,
  id: string,
) {
  list.current?.scrollIntoView({ index, behavior: 'auto' })
  let attempts = 0
  const focus = () => {
    const row = document.getElementById(id)
    if (row) {
      row.focus()
      return
    }
    attempts += 1
    if (attempts < 4) window.requestAnimationFrame(focus)
  }
  window.requestAnimationFrame(focus)
}

export function readStoredWidth(key: string, fallback: number, min: number, max: number): number {
  if (typeof window === 'undefined') return fallback
  const raw = window.localStorage.getItem(key)
  if (raw === null) return fallback
  const value = Number(raw)
  return Number.isFinite(value) ? clamp(value, min, max) : fallback
}

export function absoluteParentPaths(root: string | undefined, relativePath: string) {
  if (!root) return []
  const separator = root.includes('\\') ? '\\' : '/'
  const normalizedRoot = root.replace(/[\\/]+$/, '') || separator
  const parts = relativePath.split(/[\\/]/).filter(Boolean)
  const paths: string[] = []
  let current = normalizedRoot
  for (const part of parts.slice(0, -1)) {
    current = current === separator ? `${current}${part}` : `${current}${separator}${part}`
    paths.push(current)
  }
  return paths
}

export function useViewportWidth(): number {
  const [width, setWidth] = useState(() => typeof window === 'undefined' ? 1_440 : window.innerWidth)

  useEffect(() => {
    const update = () => setWidth(window.innerWidth)
    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [])

  return width
}


export function requireClient<T>(client: T | null): T {
  if (!client) throw new Error('Padu daemon is disconnected')
  return client
}

export function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
