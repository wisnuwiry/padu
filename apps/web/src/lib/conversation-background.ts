import { useSyncExternalStore } from 'react'

export type ConversationBackgroundFit = 'cover' | 'contain'

export type ConversationBackground = {
  imageUrl: string | null
  fileName: string | null
  opacity: number
  heightPercent: number
  fit: ConversationBackgroundFit
  loading: boolean
}

const SETTINGS_KEY = 'padu:conversation-background'
const DB_NAME = 'padu-preferences'
const STORE_NAME = 'conversation-background'
const IMAGE_KEY = 'image'
const defaults: ConversationBackground = {
  imageUrl: null,
  fileName: null,
  opacity: 0.18,
  heightPercent: 50,
  fit: 'cover',
  loading: true,
}

let snapshot = readSettings()
const listeners = new Set<() => void>()
let objectUrl: string | null = null

function readSettings(): ConversationBackground {
  if (typeof window === 'undefined') return defaults
  try {
    const raw = window.localStorage.getItem(SETTINGS_KEY)
    if (!raw) return { ...defaults, loading: false }
    const parsed = JSON.parse(raw) as Partial<ConversationBackground>
    return {
      ...defaults,
      ...parsed,
      opacity: clamp(Number(parsed.opacity ?? defaults.opacity), 0, 1),
      heightPercent: clamp(Number(parsed.heightPercent ?? defaults.heightPercent), 20, 100),
      loading: true,
    }
  } catch {
    return { ...defaults, loading: false }
  }
}

function clamp(value: number, min: number, max: number) {
  return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : min
}

function emit() {
  for (const listener of listeners) listener()
}

function saveSettings(next: Partial<ConversationBackground>) {
  snapshot = {
    ...snapshot,
    ...next,
    opacity: clamp(next.opacity ?? snapshot.opacity, 0, 1),
    heightPercent: clamp(next.heightPercent ?? snapshot.heightPercent, 20, 100),
    loading: false,
  }
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(SETTINGS_KEY, JSON.stringify({
      fileName: snapshot.fileName,
      opacity: snapshot.opacity,
      heightPercent: snapshot.heightPercent,
      fit: snapshot.fit,
    }))
  }
  emit()
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1)
    request.onupgradeneeded = () => request.result.createObjectStore(STORE_NAME)
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error)
  })
}

async function readImage(): Promise<Blob | null> {
  const db = await openDatabase()
  return new Promise((resolve, reject) => {
    const request = db.transaction(STORE_NAME, 'readonly').objectStore(STORE_NAME).get(IMAGE_KEY)
    request.onsuccess = () => resolve((request.result as Blob | undefined) ?? null)
    request.onerror = () => reject(request.error)
  })
}

async function writeImage(file: Blob | null) {
  const db = await openDatabase()
  return new Promise<void>((resolve, reject) => {
    const request = db.transaction(STORE_NAME, 'readwrite').objectStore(STORE_NAME).put(file, IMAGE_KEY)
    request.onsuccess = () => resolve()
    request.onerror = () => reject(request.error)
  })
}

export async function loadConversationBackground() {
  if (typeof window === 'undefined') return
  try {
    const blob = await readImage()
    if (blob) {
      objectUrl = URL.createObjectURL(blob)
      snapshot = { ...snapshot, imageUrl: objectUrl, loading: false }
    } else {
      snapshot = { ...snapshot, imageUrl: null, loading: false }
    }
  } catch {
    snapshot = { ...snapshot, imageUrl: null, loading: false }
  }
  emit()
}

const SUPPORTED_TYPES = new Set([
  'image/png',
  'image/jpeg',
  'image/webp',
  'image/gif',
  'image/svg+xml',
  'image/bmp',
  'image/tiff',
  'image/x-icon',
  'image/x-portable-anymap',
])

export async function chooseConversationBackground(file: File) {
  if (!SUPPORTED_TYPES.has(file.type)) throw new Error('Unsupported image format')
  await writeImage(file)
  if (objectUrl) URL.revokeObjectURL(objectUrl)
  objectUrl = URL.createObjectURL(file)
  saveSettings({ imageUrl: objectUrl, fileName: file.name })
}

export async function clearConversationBackground() {
  await writeImage(null)
  if (objectUrl) URL.revokeObjectURL(objectUrl)
  objectUrl = null
  saveSettings({ imageUrl: null, fileName: null })
}

export function updateConversationBackground(settings: Partial<Pick<ConversationBackground, 'opacity' | 'heightPercent' | 'fit'>>) {
  saveSettings(settings)
}

export function useConversationBackground() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => snapshot,
    () => defaults,
  )
}

if (typeof window !== 'undefined') {
  void loadConversationBackground()
}
