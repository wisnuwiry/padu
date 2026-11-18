import { describe, expect, test } from 'bun:test';
import type {
  ProviderSkillCapability,
  SkillCommandCatalog,
} from '@padu/client/composer-skills';
import { SKILL_CAPABILITY_PROBE_VERSION } from '@padu/client/composer-skills';
import type { SkillBinaryIdentity } from '@padu/client';

import {
  MobilePromptContextChangedError,
  renderSkillSubmissionPrompt,
  resolveMobilePromptInStableContext,
  SKILL_CATALOG_UNAVAILABLE_WARNING,
  type MobilePromptResolutionContext,
} from './skill-submission';

const PATH = '/home/remote/.agents/skills/deploy/SKILL.md';
const IDENTITY: SkillBinaryIdentity = {
  canonicalPath: '/opt/codex/bin/codex',
  contentSha256: 'a'.repeat(64),
};
const OTHER_IDENTITY: SkillBinaryIdentity = {
  canonicalPath: '/opt/codex/bin/codex',
  contentSha256: 'b'.repeat(64),
};
const capability: ProviderSkillCapability = {
  nativeSyntax: 'dollar',
  leadingInvocation: true,
  inlineMentions: true,
  repeatedMentions: true,
};

describe('mobile skill submission', () => {
  test('refreshes version before catalog and ignores stale native evidence', async () => {
    const oldCatalog: SkillCommandCatalog = {
      commands: [{
        name: 'deploy',
        description: '',
        scope: 'Skill',
        argument_hint: null,
        template: null,
        skill_file: PATH,
      }],
      skill_capability: {
        binaryVersion: '1.2.3',
        binaryIdentity: IDENTITY,
        probeVersion: 2,
        capability,
      },
    };
    const calls: string[] = [];
    const rendered = await renderSkillSubmissionPrompt({
      visiblePrompt: '$deploy',
      provider: 'codex',
      catalog: oldCatalog,
      binaryVersion: '1.2.3',
      binaryIdentity: IDENTITY,
      refetchBinary: async () => {
        calls.push('version');
        return {
          isError: false,
          data: { version: '1.2.4', binaryIdentity: OTHER_IDENTITY },
        };
      },
      refetchCatalog: async () => {
        calls.push('catalog');
        return {
          isError: false,
          data: { ...oldCatalog, skill_capability: undefined },
        };
      },
    });

    expect(calls).toEqual(['version', 'catalog']);
    expect(rendered.providerText).toBe(`[mandatory! open skill: ${PATH}]`);
    expect(rendered.warnings).toEqual([]);
  });

  test('surfaces an unavailable catalog instead of silently sending a mention', async () => {
    const rendered = await renderSkillSubmissionPrompt({
      visiblePrompt: '$deploy',
      provider: 'codex',
      catalog: undefined,
      binaryVersion: null,
      binaryIdentity: null,
      refetchBinary: async () => ({ isError: false, data: null }),
      refetchCatalog: async () => ({ isError: false, data: undefined }),
    });

    expect(rendered.providerText).toBe('$deploy');
    expect(rendered.warnings).toEqual([SKILL_CATALOG_UNAVAILABLE_WARNING]);
  });

  test('warns when a failed refresh leaves a known cached mention unresolved', async () => {
    const staleCatalog: SkillCommandCatalog = {
      commands: [{
        name: 'deploy',
        description: '',
        scope: 'Skill',
        argument_hint: null,
        template: null,
        skill_file: PATH,
      }],
    };
    const rendered = await renderSkillSubmissionPrompt({
      visiblePrompt: '$deploy',
      provider: 'codex',
      catalog: staleCatalog,
      binaryVersion: '1.2.3',
      binaryIdentity: IDENTITY,
      refetchBinary: async () => ({
        isError: true,
        data: { version: '1.2.3', binaryIdentity: IDENTITY },
      }),
      refetchCatalog: async () => ({
        isError: true,
        data: staleCatalog,
      }),
    });

    expect(rendered.providerText).toBe('$deploy');
    expect(rendered.warnings).toEqual([SKILL_CATALOG_UNAVAILABLE_WARNING]);
  });

  test('native evidence requires the exact current binary identity', async () => {
    const rendered = await renderSkillSubmissionPrompt({
      visiblePrompt: '$deploy',
      provider: 'codex',
      catalog: {
        commands: [{
          name: 'deploy',
          description: '',
          scope: 'Skill',
          argument_hint: null,
          template: null,
          skill_file: PATH,
        }],
        skill_capability: {
          binaryVersion: '1.2.3',
          binaryIdentity: IDENTITY,
          probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
          capability,
        },
      },
      binaryVersion: '1.2.3',
      binaryIdentity: OTHER_IDENTITY,
      refetchBinary: async () => ({
        isError: false,
        data: { version: '1.2.3', binaryIdentity: OTHER_IDENTITY },
      }),
      refetchCatalog: async () => ({
        isError: false,
        data: {
          commands: [{
            name: 'deploy',
            description: '',
            scope: 'Skill',
            argument_hint: null,
            template: null,
            skill_file: PATH,
          }],
          skill_capability: {
            binaryVersion: '1.2.3',
            binaryIdentity: IDENTITY,
            probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
            capability,
          },
        },
      }),
    });

    expect(rendered.providerText).toBe(`[mandatory! open skill: ${PATH}]`);
  });

  for (const [name, change] of [
    ['host', (context: MobilePromptResolutionContext) => ({
      ...context,
      hostId: 'other-host',
      hostAddress: 'wss://other.example',
    })],
    ['provider', (context: MobilePromptResolutionContext) => ({
      ...context,
      provider: 'claude' as const,
    })],
    ['project', (context: MobilePromptResolutionContext) => ({
      ...context,
      projectId: 'other-project',
      projectRoot: '/workspace/other',
    })],
    ['binary version', (context: MobilePromptResolutionContext) => ({
      ...context,
      binaryVersion: '1.2.4',
    })],
    ['binary identity', (context: MobilePromptResolutionContext) => ({
      ...context,
      binaryIdentity: OTHER_IDENTITY,
    })],
    ['binary override', (context: MobilePromptResolutionContext) => ({
      ...context,
      binaryOverride: '/opt/codex-next',
    })],
    ['model selection', (context: MobilePromptResolutionContext) => ({
      ...context,
      selection: { ...context.selection, model: 'other-model' },
    })],
  ] as const) {
    test(`rejects ${name} changes during version and catalog resolution`, async () => {
      const initial = promptContext();
      let current = initial;

      await expect(resolveMobilePromptInStableContext(
        initial,
        () => current,
        () => true,
        async () => {
          current = change(initial);
          return { providerText: '$deploy', warnings: [] };
        },
      )).rejects.toBeInstanceOf(MobilePromptContextChangedError);
    });
  }

  test('rejects a resolution that completes after the composer unmounts', async () => {
    let mounted = true;
    await expect(resolveMobilePromptInStableContext(
      promptContext(),
      () => promptContext(),
      () => mounted,
      async () => {
        mounted = false;
        return { providerText: '$deploy', warnings: [] };
      },
    )).rejects.toBeInstanceOf(MobilePromptContextChangedError);
  });

  test('keeps the snapshot final-check available until immediately before send', async () => {
    let current = promptContext();
    const resolution = await resolveMobilePromptInStableContext(
      current,
      () => current,
      () => true,
      async () => ({ providerText: '$deploy', warnings: [] }),
    );

    resolution.assertCurrent();
    current = { ...current, binaryVersion: '1.2.4' };
    expect(() => resolution.assertCurrent()).toThrow(MobilePromptContextChangedError);
  });
});

function promptContext(): MobilePromptResolutionContext {
  return {
    hostId: 'host',
    hostAddress: 'wss://daemon.example',
    sessionId: 'session',
    projectId: 'project',
    projectRoot: '/workspace/padu',
    provider: 'codex',
    binaryOverride: null,
    binaryVersion: '1.2.3',
    binaryIdentity: IDENTITY,
    selection: {
      model: 'gpt-5-codex',
      reasoningEffort: 'high',
      runtimeMode: 'fullAccess',
      interactionMode: 'build',
      agentPreset: null,
      serviceTier: 'priority',
      isolated: false,
      baseBranch: null,
      workspaceKind: 'local',
      workspacePath: null,
      workspaceBranch: null,
    },
  };
}
