import { describe, expect, test } from 'bun:test';

import { messageCopyText } from './message-presentation';

describe('mobile message presentation', () => {
  test('copies visible skill text instead of provider fallback text', () => {
    expect(messageCopyText({
      content: '[mandatory! open skill: /home/remote/.agents/skills/deploy/SKILL.md]',
      display_content: '$deploy fix this',
    })).toBe('$deploy fix this');
  });

  test('falls back to content when no separate visible text exists', () => {
    expect(messageCopyText({ content: 'plain prompt', display_content: null }))
      .toBe('plain prompt');
  });
});
