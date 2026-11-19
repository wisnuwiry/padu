import { describe, expect, test } from 'bun:test';
import type {
  AgentSession,
  EmbeddedNote,
  MessageAttachment,
  Project,
  SequencedEvent,
} from '@padu/client';
import { renderSkillMentions } from '@padu/client/composer-skills';

import {
  applyAcceptedSteer,
  applySessionOptions,
  attemptQueuedSubmission,
  beginTurn,
  composerDraftSnapshot,
  composeMobileProviderPrompt,
  createSession,
  createTaskWithRecovery,
  drainQueuedSubmission,
  queueSubmission,
  queuedMessageMetadata,
  queuedProviderPrompt,
  queuedVisiblePrompt,
  restoreRejectedSteer,
  runtimeEventAlreadyApplied,
  sessionBusy,
  sessionCwd,
  shouldApplyRuntimeEvent,
} from './mobile-runtime';

describe('mobile runtime projection', () => {
  test('begins a turn with a user message and prompt-derived title', () => {
    let id = 0;
    const started = beginTurn(session(), '  Fix the mobile reconnect race  ', {
      nowSeconds: () => 42,
      randomUUID: () => `id-${++id}`,
    });
    expect(started.status).toBe('connecting');
    expect(started.auto_title).toBe('Fix the mobile reconnect race');
    expect(started.messages.at(-1)).toMatchObject({
      id: 'id-2',
      turn_id: 'id-1',
      content: 'Fix the mobile reconnect race',
      created_at: 42,
    });
    expect(started.turns.at(-1)).toMatchObject({ id: 'id-1', status: 'running' });
  });

  test('uses a worktree path and rejects replayed runtime events', () => {
    const project: Project = { id: 'p', name: 'Padu', path: '/padu', created_at: 1 };
    const current = session({
      workspace: { kind: 'worktree', path: '/padu-worktree', branch: 'mobile' },
      runtime_event_cursor: { runtime_id: 'runtime', epoch: 'epoch', sequence: 4 },
    });
    const event = {
      sessionId: current.id,
      runtimeId: 'runtime',
      epoch: 'epoch',
      sequence: 4,
      event: { kind: 'textDelta', payload: 'duplicate' },
    } satisfies SequencedEvent;
    expect(sessionCwd(current, project)).toBe('/padu-worktree');
    expect(runtimeEventAlreadyApplied(current, event)).toBe(true);
  });

  test('replays a pending control request after an app restart', () => {
    const current = session({
      status: 'waiting',
      runtime_event_cursor: { runtime_id: 'runtime', epoch: 'epoch', sequence: 4 },
    });
    const permission = {
      sessionId: current.id,
      runtimeId: 'runtime',
      epoch: 'epoch',
      sequence: 4,
      event: { kind: 'permission', payload: {} },
    } satisfies SequencedEvent;
    const text = {
      ...permission,
      event: { kind: 'textDelta', payload: 'duplicate' },
    } satisfies SequencedEvent;
    expect(shouldApplyRuntimeEvent(current, permission)).toBe(true);
    expect(shouldApplyRuntimeEvent(current, text)).toBe(false);
    expect(shouldApplyRuntimeEvent({ ...current, status: 'working' }, permission)).toBe(false);
  });

  test('creates a provider-neutral isolated draft', () => {
    const created = createSession('project', 'claude', true, {
      nowSeconds: () => 50,
      randomUUID: () => 'new-session',
    });
    expect(created).toMatchObject({
      id: 'new-session',
      project_id: 'project',
      provider: 'claude',
      workspace: { kind: 'newWorktree' },
      runtime_mode: 'fullAccess',
      status: 'idle',
    });
  });

  test('creates a draft carrying the chosen model and access mode', () => {
    const created = createSession('project', 'codex', false, {
      nowSeconds: () => 50,
      randomUUID: () => 'new-session',
    }, {
      model: 'gpt-5-codex',
      reasoningEffort: 'high',
      runtimeMode: 'ask',
    });
    expect(created).toMatchObject({
      model: 'gpt-5-codex',
      reasoning_effort: 'high',
      runtime_mode: 'ask',
    });
  });

  test('queues a submission while a turn is live', () => {
    let id = 0;
    const busy = session({ status: 'working' });
    expect(sessionBusy(busy)).toBe(true);
    const queued = queueSubmission(busy, 'follow up', {
      nowSeconds: () => 99,
      randomUUID: () => `queued-${++id}`,
    });
    expect(queued.queued_messages).toEqual([{
      id: 'queued-1',
      content: 'follow up',
      display_content: null,
      attachments: [],
      embedded_notes: [],
      created_at: 99,
    }]);
    expect(queued.updated_at).toBe(99);
    expect(busy.queued_messages ?? []).toEqual([]);
  });

  test('creates a recoverable draft snapshot with attachments and notes', () => {
    expect(composerDraftSnapshot('$deploy keep this', ATTACHMENTS, NOTES)).toEqual({
      text: '$deploy keep this',
      attachments: ATTACHMENTS,
      embedded_notes: NOTES,
    });
    expect(composerDraftSnapshot('   ', [], [])).toBeNull();
  });

  test('a provider-start failure keeps the task but marks the prompt unaccepted', async () => {
    const draft = session({ status: 'idle', messages: [], turns: [] });
    const failed = { ...draft, status: 'failed' as const };
    const reported: Array<[string, string]> = [];

    const result = await createTaskWithRecovery({
      createDraft: () => draft,
      cache: () => {},
      persist: async (value) => value,
      send: async () => {
        throw new Error('codex is not installed on the daemon host');
      },
      latest: () => failed,
      reportError: (sessionId, error) => reported.push([sessionId, error]),
    });

    expect(result.promptAccepted).toBe(false);
    expect(result.session).toBe(failed);
    expect(result.error).toBe('codex is not installed on the daemon host');
    expect(reported).toEqual([[
      draft.id,
      'codex is not installed on the daemon host',
    ]]);
  });

  test('removes only the accepted queued row and preserves later metadata', async () => {
    const first = queuedMessage('first');
    const later = queuedMessage('later');
    const current = session({ queued_messages: [first, later] });
    const attempt = await attemptQueuedSubmission(current, first, async (_queued, metadata) => {
      expect(metadata).toEqual({
        visiblePrompt: '$deploy explain this',
        providerPrompt: '[mandatory! open skill: /remote/SKILL.md] explain this',
        attachments: ATTACHMENTS,
        embeddedNotes: NOTES,
      });
    });

    expect(attempt.accepted).toBe(true);
    if (!attempt.accepted) throw new Error('expected acceptance');
    expect(attempt.session.queued_messages?.[0]).toBe(later);
    expect(attempt.session.queued_messages).toHaveLength(1);
  });

  for (const [phase, error] of [
    ['disconnect', new Error('daemon disconnected')],
    ['startup', new Error('runtime failed to start')],
    ['preparation', new Error('provider preparation failed')],
  ] as const) {
    test(`restores the exact row at the front after ${phase} failure`, async () => {
      const first = queuedMessage('first');
      const later = queuedMessage('later');
      const current = session({ queued_messages: [first, later] });
      const attempt = await attemptQueuedSubmission(current, first, async () => {
        throw error;
      });

      expect(attempt.accepted).toBe(false);
      expect(attempt.accepted ? undefined : attempt.error).toBe(error);
      expect(attempt.session.queued_messages?.[0]).toBe(first);
      expect(attempt.session.queued_messages?.[1]).toBe(later);
      expect(queuedMessageMetadata(attempt.session.queued_messages![0]!)).toEqual({
        visiblePrompt: '$deploy explain this',
        providerPrompt: '[mandatory! open skill: /remote/SKILL.md] explain this',
        attachments: ATTACHMENTS,
        embeddedNotes: NOTES,
      });
    });
  }

  test('production drain removes only the exact accepted ID after queue updates', async () => {
    const first = queuedMessage('first');
    const later = queuedMessage('later', {
      content: 'later provider text',
      display_content: '$later visible prompt',
      created_at: 20,
    });
    const addedDuringSend = queuedMessage('added-during-send', {
      content: 'newer provider text',
      display_content: '$newer visible prompt',
      created_at: 30,
    });
    const queuedSession = session({ queued_messages: [first, later] });
    const current = session({
      status: 'connecting',
      queued_messages: [later, first, addedDuringSend],
    });
    let cached: AgentSession | undefined;
    const persisted: AgentSession[] = [];

    const drained = await drainQueuedSubmission(queuedSession, first, {
      readCurrent: () => current,
      send: async (sessionAtDrain, metadata) => {
        expect(sessionAtDrain).toBe(queuedSession);
        expect(metadata).toEqual(queuedMessageMetadata(first));
      },
      cache: (sessionToCache) => {
        cached = sessionToCache;
      },
      persist: async (sessionToPersist) => {
        persisted.push(sessionToPersist);
        return sessionToPersist;
      },
    });

    expect(drained.queued_messages?.map((message) => message.id)).toEqual([
      'later',
      'added-during-send',
    ]);
    expect(drained.queued_messages?.[0]).toBe(later);
    expect(drained.queued_messages?.[1]).toBe(addedDuringSend);
    expect(cached).toBe(drained);
    expect(persisted).toEqual([drained]);
  });

  for (const [phase, error] of [
    ['disconnect', new Error('daemon disconnected')],
    ['startup', new Error('runtime failed to start')],
    ['preparation', new Error('provider preparation failed')],
  ] as const) {
    test(`production drain restores the exact row after ${phase} failure`, async () => {
      const first = queuedMessage('first');
      const later = queuedMessage('later', {
        content: 'later provider text',
        display_content: '$later visible prompt',
        created_at: 20,
      });
      const staleFirst = queuedMessage('first', {
        content: 'stale provider text',
        display_content: '$stale visible prompt',
        created_at: 99,
      });
      const queuedSession = session({ queued_messages: [first, later] });
      const current = session({
        updated_at: 200,
        queued_messages: [later, staleFirst],
      });
      let cached: AgentSession | undefined;
      const persisted: AgentSession[] = [];

      await expect(drainQueuedSubmission(queuedSession, first, {
        readCurrent: () => current,
        send: async (_sessionAtDrain, metadata) => {
          expect(metadata).toEqual(queuedMessageMetadata(first));
          throw error;
        },
        cache: (sessionToCache) => {
          cached = sessionToCache;
        },
        persist: async (sessionToPersist) => {
          persisted.push(sessionToPersist);
          return sessionToPersist;
        },
      })).rejects.toBe(error);

      expect(cached?.queued_messages?.map((message) => message.id)).toEqual([
        'first',
        'later',
      ]);
      expect(cached?.queued_messages?.[0]).toBe(first);
      expect(cached?.queued_messages?.[1]).toBe(later);
      expect(cached?.queued_messages?.[0]).toMatchObject({
        id: first.id,
        content: first.content,
        display_content: first.display_content,
        attachments: ATTACHMENTS,
        embedded_notes: NOTES,
        created_at: first.created_at,
      });
      if (!cached) throw new Error('expected the restored session to be cached');
      expect(persisted).toEqual([cached]);
    });
  }

  test('keeps visible and provider text distinct on send and queue', () => {
    const visible = '$brainstroming explain this';
    const rendered = renderSkillMentions(visible, 'codex', [{
      name: 'brainstroming',
      description: '',
      scope: 'Skill',
      argument_hint: null,
      skill_file: '/skills/brainstroming/SKILL.md',
    }]);
    const started = beginTurn(session(), visible, {
      nowSeconds: () => 100,
      randomUUID: (() => {
        let id = 0;
        return () => `turn-${++id}`;
      })(),
    }, {
      providerPrompt: rendered.providerText,
    });
    expect(started.messages.at(-1)).toMatchObject({
      content: '[mandatory! open skill: /skills/brainstroming/SKILL.md] explain this',
      display_content: visible,
    });

    const queued = queueSubmission(session({ status: 'working' }), visible, {
      nowSeconds: () => 101,
      randomUUID: () => 'queue-1',
    }, rendered.providerText);
    const message = queued.queued_messages?.[0]!;
    expect(queuedVisiblePrompt(message)).toBe(visible);
    expect(queuedProviderPrompt(message)).toBe(rendered.providerText);
    expect(message.display_content).toBe(visible);
  });

  test('preserves cross-device attachments and notes through edit, steer, accept, and rejection', () => {
    const attachments: MessageAttachment[] = [{
      path: '/workspace/src/mobile.tsx',
      mention: 'src/mobile.tsx',
      name: 'mobile.tsx',
      is_dir: false,
      is_image: false,
    }];
    const notes: EmbeddedNote[] = [{
      id: 'note-1',
      title: 'Review context',
      content: 'Keep the phone and tablet layouts aligned.',
      revision: 2,
    }];
    const visible = '$deploy keep this context';
    const provider = composeMobileProviderPrompt(
      '[mandatory! open skill: /home/remote/.agents/skills/deploy/SKILL.md] keep this context',
      attachments,
      notes,
    );
    const clock = {
      nowSeconds: () => 120,
      randomUUID: (() => {
        let id = 0;
        return () => `cross-${++id}`;
      })(),
    };
    const queued = queueSubmission(
      session({ status: 'working' }),
      visible,
      clock,
      provider,
      attachments,
      notes,
    );
    const metadata = queuedMessageMetadata(queued.queued_messages![0]!);

    expect(metadata).toMatchObject({
      visiblePrompt: visible,
      providerPrompt: provider,
      attachments,
      embeddedNotes: notes,
    });
    expect(provider).toContain('@src/mobile.tsx');
    expect(provider).toContain(notes[0]!.content);

    const pending = {
      providerPrompt: provider,
      value: {
        displayContent: visible,
        attachments,
        embeddedNotes: notes,
      },
    };
    const accepted = applyAcceptedSteer(
      session({ status: 'working' }),
      pending,
      provider,
      clock,
    );
    expect(accepted.messages.at(-1)).toMatchObject({
      content: provider,
      display_content: visible,
      attachments,
      embedded_notes: notes,
    });

    const rejected = restoreRejectedSteer(
      session({ status: 'working' }),
      pending,
      clock,
    );
    expect(rejected.queued_messages?.at(-1)).toMatchObject({
      content: provider,
      display_content: visible,
      attachments,
      embedded_notes: notes,
    });

    const sent = beginTurn(
      session(),
      visible,
      clock,
      { providerPrompt: provider, attachments, embeddedNotes: notes },
    );
    expect(sent.messages.at(-1)).toMatchObject({
      content: provider,
      display_content: visible,
      attachments,
      embedded_notes: notes,
    });
  });

  test('leaves slash commands and Markdown code samples unchanged', () => {
    const commands = [{
      name: 'brainstroming',
      description: '',
      scope: 'Skill' as const,
      argument_hint: null,
      skill_file: '/skills/brainstroming/SKILL.md',
    }];
    expect(renderSkillMentions('/review $ARGUMENTS', 'codex', commands).providerText)
      .toBe('/review $ARGUMENTS');
    expect(renderSkillMentions('`$brainstroming`', 'codex', commands).providerText)
      .toBe('`$brainstroming`');
  });

  test('applies option changes without clobbering unrelated fields', () => {
    const current = session({ model: 'old', reasoning_effort: 'low' });
    const next = applySessionOptions(current, { model: 'new-model' }, {
      nowSeconds: () => 77,
      randomUUID: () => 'unused',
    });
    expect(next.model).toBe('new-model');
    expect(next.reasoning_effort).toBe('low');
    expect(next.runtime_mode).toBe(current.runtime_mode);
    expect(next.updated_at).toBe(77);
    const cleared = applySessionOptions(current, { model: null, reasoningEffort: null }, {
      nowSeconds: () => 78,
      randomUUID: () => 'unused',
    });
    expect(cleared.model).toBeNull();
    expect(cleared.reasoning_effort).toBeNull();
  });
});

function session(overrides: Partial<AgentSession> = {}): AgentSession {
  return {
    id: 'session',
    title: 'New task',
    auto_title: null,
    project_id: 'p',
    workspace: { kind: 'local' },
    provider: 'codex',
    runtime_mode: 'fullAccess',
    interaction_mode: 'build',
    status: 'idle',
    created_at: 1,
    updated_at: 1,
    last_reply_at: null,
    provider_cursor: null,
    messages: [],
    transcript_blocks: [],
    turns: [],
    ...overrides,
  };
}

const ATTACHMENTS: MessageAttachment[] = [{
  path: '/workspace/src/mobile.tsx',
  mention: 'src/mobile.tsx',
  name: 'mobile.tsx',
  is_dir: false,
  is_image: false,
}];

const NOTES: EmbeddedNote[] = [{
  id: 'note-1',
  title: 'Mobile context',
  content: 'Keep the visible and provider text split.',
  revision: 4,
}];

function queuedMessage(
  id: string,
  overrides: Partial<NonNullable<AgentSession['queued_messages']>[number]> = {},
): NonNullable<AgentSession['queued_messages']>[number] {
  return {
    id,
    content: '[mandatory! open skill: /remote/SKILL.md] explain this',
    display_content: '$deploy explain this',
    attachments: ATTACHMENTS,
    embedded_notes: NOTES,
    created_at: 10,
    ...overrides,
  };
}
