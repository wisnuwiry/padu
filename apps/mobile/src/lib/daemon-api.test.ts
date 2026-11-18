import { describe, expect, test } from 'bun:test';
import type { DaemonSettings, PaduClient } from '@padu/client';

import {
  browseDaemonDirectory,
  createProject,
  discoverComposerCommands,
  persistProject,
  probeProvider,
} from './daemon-api';

describe('mobile daemon API', () => {
  test('browses a directory on the remote host', async () => {
    let command: unknown;
    const directory = {
      type: 'directory' as const,
      path: '/Users/me',
      parent: '/Users',
      home: '/Users/me',
      filesystem_root: '/',
      entries: [],
    };
    const client = {
      request: async (next: unknown) => {
        command = next;
        return { type: 'workspace', result: directory };
      },
    } as unknown as PaduClient;

    await expect(browseDaemonDirectory(client, null)).resolves.toEqual(directory);
    expect(command).toEqual({
      type: 'workspace',
      operation: { type: 'browseDirectory', path: null },
    });
  });

  test('normalizes absolute Unix and Windows project paths', () => {
    expect(createProject('/srv/padu/', 'one', 10)).toEqual({
      id: 'one',
      name: 'padu',
      path: '/srv/padu',
      created_at: 10,
    });
    expect(createProject('C:\\dev\\padu\\', 'two', 10).name).toBe('padu');
    expect(() => createProject('dev/padu', 'three')).toThrow('absolute path');
  });

  test('discovers slash commands through the daemon workspace query', async () => {
    let command: unknown;
    const commands = [{
      name: 'brainstroming',
      description: 'Brainstorm',
      scope: 'Skill' as const,
      argument_hint: null,
      template: null,
      skill_file: '/skills/brainstroming/SKILL.md',
    }];
    const client = {
      request: async (next: unknown) => {
        command = next;
        return { type: 'workspace', result: { type: 'slashCommands', commands } };
      },
    } as unknown as PaduClient;

    const catalog = { type: 'slashCommands' as const, commands };
    await expect(discoverComposerCommands(
      client,
      'codex',
      '/srv/padu',
      '/opt/codex',
    )).resolves.toEqual(catalog);
    expect(command).toEqual({
      type: 'workspace',
      operation: {
        type: 'discoverSlashCommands',
        provider: 'codex',
        project_root: '/srv/padu',
        binary_override: '/opt/codex',
      },
    });
  });

  test('keeps the exact binary identity alongside the probed version', async () => {
    const identity = {
      canonicalPath: '/opt/padu/codex',
      contentSha256: 'a'.repeat(64),
    };
    const client = {
      request: async () => ({
        type: 'providerProbe',
        probe: {
          provider: 'codex',
          installed: true,
          path: '/opt/padu/codex',
          models: [],
          agent_presets: [],
        },
        version: '1.2.3',
        binaryIdentity: identity,
      }),
    } as unknown as PaduClient;

    await expect(probeProvider(
      client,
      'codex',
      { provider_binary_overrides: {} } as DaemonSettings,
    )).resolves.toMatchObject({
      version: '1.2.3',
      binaryIdentity: identity,
    });
  });

  test('persists a project without replacing sessions', async () => {
    const commands: unknown[] = [];
    const client = {
      request: async (command: any) => {
        commands.push(command);
        if (command.type === 'loadTaskState') {
          return {
            type: 'taskState',
            revision: 2,
            projects: [],
            sessions: [{ id: 'live' }],
          };
        }
        return { type: 'taskStateSaved', revision: 3, sessions: [] };
      },
    } as unknown as PaduClient;
    const project = createProject('/srv/padu', 'project', 10);

    const saved = await persistProject(client, project);
    expect(saved.project).toEqual(project);
    expect(commands[1]).toEqual({
      type: 'saveTaskState',
      projects: [project],
      liveSessionIds: ['live'],
      sessions: [],
    });
  });
});
