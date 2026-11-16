import { useSyncExternalStore } from 'react'
import effectConfig from '../../../../background-effects.json'

export type ConversationBackgroundEffect = 'none' | 'dither' | 'ascii'

export type ConversationBackground = {
  imageUrl: string | null
  processedImageUrl: string | null
  fileName: string | null
  opacity: number
  heightPercent: number
  effect: ConversationBackgroundEffect
  loading: boolean
}

const SETTINGS_KEY = 'padu:conversation-background'
const DB_NAME = 'padu-preferences'
const STORE_NAME = 'conversation-background'
const IMAGE_KEY = 'image'
const defaults: ConversationBackground = {
  imageUrl: null,
  processedImageUrl: null,
  fileName: null,
  opacity: 0.18,
  heightPercent: 50,
  effect: 'none',
  loading: true,
}

let snapshot = readSettings()
const listeners = new Set<() => void>()
let objectUrl: string | null = null
let generation = 0
let mutationQueue: Promise<void> = Promise.resolve()

function sanitizeEffect(effect: unknown): ConversationBackgroundEffect {
  return effect === 'dither' || effect === 'ascii' ? effect : 'none'
}

function readSettings(): ConversationBackground {
  if (typeof window === 'undefined') return defaults
  try {
    const raw = window.localStorage.getItem(SETTINGS_KEY)
    if (!raw) return { ...defaults, loading: false }
    const parsed = JSON.parse(raw) as Partial<ConversationBackground>
    return {
      ...defaults,
      ...parsed,
      effect: sanitizeEffect(parsed.effect),
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

function saveSettings(next: Partial<ConversationBackground>) {
  snapshot = {
    ...snapshot,
    ...next,
    effect: next.effect ?? snapshot.effect,
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
        effect: snapshot.effect,
      }))
    }
  } catch {
  } finally {
    emit()
  }
}

const BAYER_8X8 = [
  [0 / 64, 32 / 64, 8 / 64, 40 / 64, 2 / 64, 34 / 64, 10 / 64, 42 / 64],
  [48 / 64, 16 / 64, 56 / 64, 24 / 64, 50 / 64, 18 / 64, 58 / 64, 26 / 64],
  [12 / 64, 44 / 64, 4 / 64, 36 / 64, 14 / 64, 46 / 64, 6 / 64, 38 / 64],
  [60 / 64, 28 / 64, 52 / 64, 20 / 64, 62 / 64, 30 / 64, 54 / 64, 22 / 64],
  [3 / 64, 35 / 64, 11 / 64, 43 / 64, 1 / 64, 33 / 64, 9 / 64, 41 / 64],
  [51 / 64, 19 / 64, 59 / 64, 27 / 64, 49 / 64, 17 / 64, 57 / 64, 25 / 64],
  [15 / 64, 47 / 64, 7 / 64, 39 / 64, 13 / 64, 45 / 64, 5 / 64, 37 / 64],
  [63 / 64, 31 / 64, 55 / 64, 23 / 64, 61 / 64, 29 / 64, 53 / 64, 21 / 64],
]

const DITHER_LUT = new Uint8Array(64 * 256)
for (let b = 0; b < 64; b++) {
  const y = (b >> 3) & 7
  const x = b & 7
  const bayerVal = BAYER_8X8[y][x] - 0.5
  const spread = bayerVal * effectConfig.dither.spread
  const step = effectConfig.dither.colorStep
  const offset = b * 256
  for (let c = 0; c < 256; c++) {
    DITHER_LUT[offset + c] = Math.min(255, Math.max(0, Math.round((c + spread) / step) * step))
  }
}

const ASCII_DITHER_LUT = new Uint8Array(64 * 256)
for (let b = 0; b < 64; b++) {
  const y = (b >> 3) & 7
  const x = b & 7
  const bayerVal = BAYER_8X8[y][x] - 0.5
  const spread = bayerVal * effectConfig.ascii.spread
  const step = effectConfig.ascii.colorStep
  const offset = b * 256
  for (let c = 0; c < 256; c++) {
    ASCII_DITHER_LUT[offset + c] = Math.min(255, Math.max(0, Math.round((c + spread) / step) * step))
  }
}

function processDither(img: HTMLImageElement): string {
  const naturalW = img.naturalWidth || 1920
  const naturalH = img.naturalHeight || 1080
  const targetW = Math.min(effectConfig.dither.maxWidth, naturalW)
  const targetH = Math.max(1, Math.round((naturalH / naturalW) * targetW))
  const canvas = document.createElement('canvas')
  canvas.width = targetW
  canvas.height = targetH
  const ctx = canvas.getContext('2d')
  if (!ctx) return ''
  ctx.drawImage(img, 0, 0, targetW, targetH)
  const imgData = ctx.getImageData(0, 0, targetW, targetH)
  const data = imgData.data

  for (let y = 0; y < targetH; y++) {
    const yShift = (y & 7) << 3
    const rowOffset = y * targetW * 4
    for (let x = 0; x < targetW; x++) {
      const idx = rowOffset + x * 4
      if (data[idx + 3] < 10) continue
      const lutOffset = (yShift | (x & 7)) * 256
      data[idx] = DITHER_LUT[lutOffset + data[idx]]
      data[idx + 1] = DITHER_LUT[lutOffset + data[idx + 1]]
      data[idx + 2] = DITHER_LUT[lutOffset + data[idx + 2]]
    }
  }
  ctx.putImageData(imgData, 0, 0)
  return canvas.toDataURL('image/png')
}

const ASCII_GLYPHS_8X8 = [
  [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
  [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
  [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00],
  [0x00, 0x00, 0x00, 0x7E, 0x7E, 0x00, 0x00, 0x00],
  [0x00, 0x18, 0x18, 0x7E, 0x7E, 0x18, 0x18, 0x00],
  [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00],
  [0x00, 0x63, 0x66, 0x0C, 0x18, 0x33, 0x63, 0x00],
  [0x24, 0x7E, 0x24, 0x24, 0x7E, 0x24, 0x00, 0x00],
]

function processAscii(img: HTMLImageElement): string {
  const naturalW = img.naturalWidth || 1920
  const naturalH = img.naturalHeight || 1080
  const { minColumns, maxColumns, cellSize, bgDim } = effectConfig.ascii
  const cols = Math.min(maxColumns, Math.max(minColumns, Math.round(naturalW / cellSize)))
  const rows = Math.max(1, Math.round((naturalH / naturalW) * cols))

  const sampleCanvas = document.createElement('canvas')
  sampleCanvas.width = cols
  sampleCanvas.height = rows
  const sampleCtx = sampleCanvas.getContext('2d')
  if (!sampleCtx) return ''
  sampleCtx.drawImage(img, 0, 0, cols, rows)
  const sampleData = sampleCtx.getImageData(0, 0, cols, rows).data

  const outW = cols * cellSize
  const outH = rows * cellSize
  const outCanvas = document.createElement('canvas')
  outCanvas.width = outW
  outCanvas.height = outH
  const outCtx = outCanvas.getContext('2d')
  if (!outCtx) return ''
  const outImgData = outCtx.createImageData(outW, outH)
  const out32 = new Uint32Array(outImgData.data.buffer)
  const bgScale = Math.round(bgDim * 256)

  for (let cy = 0; cy < rows; cy++) {
    const sampleRowOffset = cy * cols * 4
    for (let cx = 0; cx < cols; cx++) {
      const sampleIdx = sampleRowOffset + cx * 4
      const a = sampleData[sampleIdx + 3]
      if (a < 10) continue

      const r = sampleData[sampleIdx]
      const g = sampleData[sampleIdx + 1]
      const b = sampleData[sampleIdx + 2]

      const luma = (r * 77 + g * 150 + b * 29) >> 8
      const glyphIdx = Math.min(7, (luma * 7 + 128) >> 8)
      const glyph = ASCII_GLYPHS_8X8[glyphIdx]

      // Dither-quantized colors for ASCII characters and softly dimmed background
      const bgR = (r * bgScale) >> 8
      const bgG = (g * bgScale) >> 8
      const bgB = (b * bgScale) >> 8

      const baseOutIdx = (cy * cellSize * outW) + (cx * cellSize)

      for (let gy = 0; gy < 8; gy++) {
        const rowBits = glyph[gy]
        const rowOffset = baseOutIdx + (gy * outW)
        const yShift = gy << 3
        for (let gx = 0; gx < 8; gx++) {
          const isFg = ((rowBits >> (7 - gx)) & 1) !== 0
          const bayerIdx = yShift | gx
          const lutOffset = bayerIdx * 256
          const pR = ASCII_DITHER_LUT[lutOffset + (isFg ? r : bgR)]
          const pG = ASCII_DITHER_LUT[lutOffset + (isFg ? g : bgG)]
          const pB = ASCII_DITHER_LUT[lutOffset + (isFg ? b : bgB)]
          out32[rowOffset + gx] = (a << 24) | (pB << 16) | (pG << 8) | pR
        }
      }
    }
  }
  outCtx.putImageData(outImgData, 0, 0)
  return outCanvas.toDataURL('image/png')
}

const effectCache = new Map<ConversationBackgroundEffect, string>()
let cachedImageSourceUrl: string | null = null
let cachedImageElement: HTMLImageElement | null = null

async function processEffects(url: string | null) {
  if (typeof window === 'undefined' || !url) {
    effectCache.clear()
    cachedImageSourceUrl = null
    cachedImageElement = null
    snapshot = { ...snapshot, processedImageUrl: null }
    emit()
    return
  }
  if (snapshot.effect === 'none') {
    snapshot = { ...snapshot, processedImageUrl: null }
    emit()
    return
  }
  if (url !== cachedImageSourceUrl) {
    effectCache.clear()
    cachedImageSourceUrl = url
    cachedImageElement = null
  }
  const cachedUrl = effectCache.get(snapshot.effect)
  if (cachedUrl) {
    snapshot = { ...snapshot, processedImageUrl: cachedUrl }
    emit()
    return
  }

  let img = cachedImageElement
  if (!img) {
    img = new Image()
    img.crossOrigin = 'anonymous'
    await new Promise<void>((resolve, reject) => {
      img!.onload = () => resolve()
      img!.onerror = () => reject()
      img!.src = url
    }).catch(() => {})
    cachedImageElement = img
  }

  const processed = snapshot.effect === 'dither' ? processDither(img) : processAscii(img)
  if (processed) {
    effectCache.set(snapshot.effect, processed)
  }
  snapshot = { ...snapshot, processedImageUrl: processed }
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
    if (objectUrl) void processEffects(objectUrl)
  } catch {
    if (currentGeneration !== generation) return
    replaceObjectUrl(null)
    snapshot = { ...snapshot, imageUrl: null, processedImageUrl: null, loading: false }
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
    void processEffects(nextUrl)
  })
}

export async function clearConversationBackground() {
  generation += 1
  await enqueueMutation(async () => {
    await writeImage(null)
    replaceObjectUrl(null)
    saveSettings({ imageUrl: null, fileName: null })
    void processEffects(null)
  })
}

export function updateConversationBackground(settings: Partial<Pick<ConversationBackground, 'opacity' | 'heightPercent' | 'effect'>>) {
  saveSettings(settings)
  if (settings.effect !== undefined && snapshot.imageUrl) {
    void processEffects(snapshot.imageUrl)
  }
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
