import { useSyncExternalStore } from 'react'

export type ConversationBackground = {
  imageUrl: string | null
  fileName: string | null
  opacity: number
  heightPercent: number
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
  loading: true,
}

let snapshot = readSettings()
const listeners = new Set<() => void>()
let objectUrl: string | null = null
let generation = 0
let mutationQueue: Promise<void> = Promise.resolve()

function readSettings(): ConversationBackground {
  if (typeof window === 'undefined') return defaults
  try {
    const raw = window.localStorage.getItem(SETTINGS_KEY)
    if (!raw) return { ...defaults, loading: false }
    const parsed = JSON.parse(raw) as Partial<ConversationBackground>
    return {
      ...defaults,
      ...parsed,
      opacity: clamp(Number(parsed.opacity ?? defaults.opacity), 0, 1, defaults.opacity),
      heightPercent: clamp(Number(parsed.heightPercent ?? defaults.heightPercent), 20, 100, defaults.heightPercent),
      loading: true,
    }
  } catch {
    return { ...defaults, loading: false }
  }
}

function clamp(value: number, min: number, max: number, fallback: number) {
  return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback
}

function replaceObjectUrl(next: string | null) {
  const previous = objectUrl
  objectUrl = next
  if (previous && previous !== next) URL.revokeObjectURL(previous)
}

function enqueueMutation(operation: () => Promise<void>) {
  const pending = mutationQueue.then(operation).catch((error: unknown) => {
    snapshot = { ...snapshot, loading: false }
    emit()
    throw error
  })
  mutationQueue = pending.catch(() => {})
  return pending
}

function emit() {
  for (const listener of listeners) listener()
}

function saveSettings(next: Partial<ConversationBackground>) {  snapshot = {
    ...snapshot,
    ...next,
    opacity: clamp(next.opacity ?? snapshot.opacity, 0, 1, defaults.opacity),
    heightPercent: clamp(next.heightPercent ?? snapshot.heightPercent, 20, 100, defaults.heightPercent),
    loading: false,
  }
  try {
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(SETTINGS_KEY, JSON.stringify({
        fileName: snapshot.fileName,
        opacity: snapshot.opacity,
        heightPercent: snapshot.heightPercent,
      }))
    }
  } catch {
  } finally {
    emit()
  }
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
  try {
    return await new Promise((resolve, reject) => {
      const transaction = db.transaction(STORE_NAME, 'readonly')
      const request = transaction.objectStore(STORE_NAME).get(IMAGE_KEY)
      request.onsuccess = () => resolve((request.result as Blob | undefined) ?? null)
      request.onerror = () => reject(request.error)
      transaction.onabort = () => reject(transaction.error ?? new Error('Background image transaction aborted'))
    })
  } finally {
    db.close()
  }
}

async function writeImage(file: Blob | null) {
  const db = await openDatabase()
  try {
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(STORE_NAME, 'readwrite')
      transaction.oncomplete = () => resolve()
      transaction.onabort = () => reject(transaction.error ?? new Error('Background image transaction aborted'))
      transaction.onerror = () => reject(transaction.error)
      transaction.objectStore(STORE_NAME).put(file, IMAGE_KEY)
    })
  } finally {
    db.close()
  }
}

export async function loadConversationBackground() {
  if (typeof window === 'undefined') return
  const currentGeneration = ++generation
  await mutationQueue
  if (currentGeneration !== generation) return
  try {
    const blob = await readImage()
    if (currentGeneration !== generation) return
    replaceObjectUrl(blob ? URL.createObjectURL(blob) : null)
    snapshot = { ...snapshot, imageUrl: objectUrl, loading: false }
  } catch {
    if (currentGeneration !== generation) return
    replaceObjectUrl(null)
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

export class UnsupportedConversationBackgroundError extends Error {
  constructor() {
    super('Unsupported image format')
    this.name = 'UnsupportedConversationBackgroundError'
  }
}

export class OversizedConversationBackgroundError extends Error {
  constructor() {
    super('Image exceeds the 2 MB background limit')
    this.name = 'OversizedConversationBackgroundError'
  }
}

export const MAX_BACKGROUND_IMAGE_BYTES = 2 * 1024 * 1024

export async function chooseConversationBackground(file: File) {
  if (!SUPPORTED_TYPES.has(file.type)) throw new UnsupportedConversationBackgroundError()
  if (file.size > MAX_BACKGROUND_IMAGE_BYTES) throw new OversizedConversationBackgroundError()
  generation += 1
  await enqueueMutation(async () => {
    const nextUrl = URL.createObjectURL(file)
    try {
      await writeImage(file)
    } catch (error) {
      URL.revokeObjectURL(nextUrl)
      throw error
    }
    replaceObjectUrl(nextUrl)
    saveSettings({ imageUrl: objectUrl, fileName: file.name })
  })
}

export async function clearConversationBackground() {
  generation += 1
  await enqueueMutation(async () => {
    await writeImage(null)
    replaceObjectUrl(null)
    saveSettings({ imageUrl: null, fileName: null })
  })
}

export function updateConversationBackground(settings: Partial<Pick<ConversationBackground, 'opacity' | 'heightPercent'>>) {
  saveSettings(settings)
}

export function subscribeConversationBackground(listener: () => void) {
  listeners.add(listener)
  return () => { listeners.delete(listener) }
}

export function getConversationBackgroundSnapshot() {
  return snapshot
}

export function useConversationBackground() {
  return useSyncExternalStore(
    subscribeConversationBackground,
    getConversationBackgroundSnapshot,
    () => defaults,
  )
}

if (typeof window !== 'undefined') {
  void loadConversationBackground()
}
