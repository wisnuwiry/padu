import { describe, expect, test } from 'bun:test';
import type { DaemonSettings } from '@padu/client';

import {
  CONVERSATION_BACKGROUND_DEFAULTS,
  daemonBackgroundImageUrl,
  parseDaemonBackground,
} from './conversation-background';

function settingsWith(extra: unknown): DaemonSettings {
  return {
    computer_use_enabled: false,
    computer_use_allowed_apps: [],
    disabled_providers: [],
    provider_binary_overrides: {},
    conversation_background: extra,
  } as unknown as DaemonSettings;
}

describe('daemon conversation background', () => {
  test('fixed defaults match web settings defaults', () => {
    expect(CONVERSATION_BACKGROUND_DEFAULTS).toEqual({
      opacity: 0.18,
      heightPercent: 50,
    });
  });

  test('parses a configured daemon background', () => {
    expect(parseDaemonBackground(settingsWith({
      image_blob: 'padu-blob:abc123.png',
      image_mime_type: 'image/png',
      opacity: 0.3,
      height_percent: 60,
    }))).toEqual({
      reference: 'padu-blob:abc123.png',
      mimeType: 'image/png',
      opacity: 0.3,
      heightPercent: 60,
    });
  });

  test('infers mime from the blob reference and clamps ranges', () => {
    expect(parseDaemonBackground(settingsWith({
      image_blob: 'padu-blob:abc123.webp',
      opacity: 9,
      height_percent: 5,
    }))).toEqual({
      reference: 'padu-blob:abc123.webp',
      mimeType: 'image/webp',
      opacity: 1,
      heightPercent: 20,
    });
  });

  test('returns null when unconfigured or malformed', () => {
    expect(parseDaemonBackground(undefined)).toBeNull();
    expect(parseDaemonBackground(settingsWith(null))).toBeNull();
    expect(parseDaemonBackground(settingsWith({ opacity: 0.5 }))).toBeNull();
    expect(parseDaemonBackground(settingsWith({ image_blob: '' }))).toBeNull();
  });

  test('builds a data URL from blob bytes', () => {
    expect(daemonBackgroundImageUrl('image/png', 'iVBOR')).toBe(
      'data:image/png;base64,iVBOR',
    );
  });
});
