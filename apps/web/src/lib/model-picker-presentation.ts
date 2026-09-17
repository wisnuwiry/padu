import type { ProviderKind, ProviderModel } from '@padu/client'

export interface ModelPickerRow {
  provider: ProviderKind
  model: ProviderModel
}

export function selectedModelPickerIndex(
  rows: readonly ModelPickerRow[],
  provider: ProviderKind,
  modelId: string | undefined,
) {
  if (!modelId) return -1
  return rows.findIndex((row) => row.provider === provider && row.model.id === modelId)
}

export function nextModelPickerHighlight(
  current: number | null,
  length: number,
  direction: 'next' | 'previous',
) {
  if (!length) return null
  if (current === null) return direction === 'next' ? 0 : length - 1
  return direction === 'next'
    ? (current + 1) % length
    : (current - 1 + length) % length
}

export function modelPickerSubtitle(
  providerShortName: string,
  subProvider?: string | null,
): string {
  const trimmed = subProvider?.trim()
  if (!trimmed || trimmed.toLowerCase() === providerShortName.toLowerCase()) {
    return providerShortName
  }
  return `${trimmed} · ${providerShortName}`
}
