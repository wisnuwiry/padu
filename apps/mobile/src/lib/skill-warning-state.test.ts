import { describe, expect, test } from 'bun:test';

import { reconcileSkillWarningSession } from './skill-warning-state';

describe('mobile skill warning session state', () => {
  test('keeps active warnings across ordinary renders of the same session', () => {
    const current = {
      sessionId: 'session-one',
      warnings: ['Skill warning'],
    };
    expect(reconcileSkillWarningSession(
      current,
      'session-one',
      ['fresh prop reference'],
    )).toBe(current);
  });

  test('adopts initial warnings when the composer moves to a new session', () => {
    expect(reconcileSkillWarningSession(
      { sessionId: 'session-one', warnings: ['old warning'] },
      'session-two',
      ['new warning'],
    )).toEqual({
      sessionId: 'session-two',
      warnings: ['new warning'],
    });
  });
});
