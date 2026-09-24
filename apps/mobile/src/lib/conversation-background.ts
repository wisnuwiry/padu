import type { DaemonSettings } from '@padu/client';

/**
 * Render-only conversation background. Mobile never picks or uploads an
 * image — the daemon owns the background and this only renders what the
 * daemon serves: a `conversation_background` object inside daemon settings
 * (`{ image_blob, image_mime_type, opacity, height_percent }`, preserved
 * through the settings `extra` map without a protocol change) plus the
 * image bytes behind the `padu-blob:` reference via `ReadBlob`.
 */
export interface ConversationBackground {
  imageUrl: string | null;
  opacity: number;
  heightPercent: number;
  loading: boolean;
}

export interface DaemonBackgroundRef {
  reference: string;
  mimeType: string;
  opacity: number;
  heightPercent: number;
}

export const CONVERSATION_BACKGROUND_DEFAULTS = {
  opacity: 0.18,
  heightPercent: 50,
} as const;

const MIME_BY_EXTENSION: Record<string, string> = {
  avif: 'image/avif',
  gif: 'image/gif',
  heic: 'image/heic',
  jpeg: 'image/jpeg',
  jpg: 'image/jpeg',
  png: 'image/png',
  svg: 'image/svg+xml',
  webp: 'image/webp',
};

function clampNumber(value: unknown, min: number, max: number, fallback: number): number {
  const parsed = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(parsed) ? Math.min(max, Math.max(min, parsed)) : fallback;
}

function mimeTypeForReference(reference: string, declared: unknown): string {
  if (typeof declared === 'string' && declared.includes('/')) return declared;
  const extension = reference.split('.').at(-1)?.toLowerCase();
  return (extension && MIME_BY_EXTENSION[extension]) || 'image/png';
}

/**
 * Reads the daemon-owned background out of daemon settings. Unknown keys
 * ride inside the settings `extra` map, so no protocol change is needed.
 * Returns null when nothing (valid) is configured.
 */
export function parseDaemonBackground(settings: DaemonSettings | undefined): DaemonBackgroundRef | null {
  if (!settings) return null;
  const raw = (settings as Record<string, unknown>)['conversation_background'];
  if (!raw || typeof raw !== 'object') return null;
  const record = raw as Record<string, unknown>;
  const reference = record['image_blob'];
  if (typeof reference !== 'string' || !reference) return null;
  return {
    reference,
    mimeType: mimeTypeForReference(reference, record['image_mime_type']),
    opacity: clampNumber(
      record['opacity'],
      0,
      1,
      CONVERSATION_BACKGROUND_DEFAULTS.opacity,
    ),
    heightPercent: clampNumber(
      record['height_percent'],
      20,
      100,
      CONVERSATION_BACKGROUND_DEFAULTS.heightPercent,
    ),
  };
}

/** Base64 blob bytes become a renderable image URL (RN Image takes data URIs). */
export function daemonBackgroundImageUrl(mimeType: string, base64: string): string {
  return `data:${mimeType};base64,${base64}`;
}
