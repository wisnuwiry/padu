import { describe, expect, test } from 'bun:test'
import type { ProviderProbeResult } from './provider-probe-cache'
import {
  readProviderProbeCache,
  writeProviderProbeCache,
} from './provider-probe-cache'
import {
  catalogSkillCapability,
  DEFAULT_PROVIDER_SKILL_CAPABILITIES,
  refetchedBinaryIdentity,
  SKILL_CAPABILITY_PROBE_VERSION,
  type SkillCommandCatalog,
} from '@padu/client/composer-skills'

const IDENTITY = {
  canonicalPath: '/usr/local/bin/codex',
  contentSha256: 'a'.repeat(64),
}

class MemoryStorage {
  private readonly values = new Map<string, string>()

  getItem(key: string) {
    return this.values.get(key) ?? null
  }

  setItem(key: string, value: string) {
    this.values.set(key, value)
  }
}

const probe: ProviderProbeResult = {
  provider: 'codex',
  installed: true,
  path: '/usr/local/bin/codex',
  unavailable_reason: null,
  storage_paths: [],
  models: [{
    id: 'gpt-5',
    name: 'GPT-5',
    is_default: true,
    reasoning_efforts: [],
    service_tiers: [],
    context_windows: [],
  }],
  agent_presets: [],
  version: '1.2.3',
  binaryIdentity: IDENTITY,
}

describe('provider probe cache', () => {
  test('restores a daemon-scoped model catalog and timestamp', () => {
    const storage = new MemoryStorage()
    writeProviderProbeCache(storage, 'ws://daemon-a', 'codex', null, probe, 10_000)

    expect(readProviderProbeCache(storage, 'ws://daemon-a', 'codex', null, 20_000)).toEqual({
      binaryOverride: null,
      data: probe,
      updatedAt: 10_000,
    })
    expect(readProviderProbeCache(storage, 'ws://daemon-b', 'codex', null, 20_000)).toBeUndefined()
  })

  test('does not reuse a catalog after its binary override changes', () => {
    const storage = new MemoryStorage()
    writeProviderProbeCache(storage, 'ws://daemon-a', 'codex', '/opt/codex', probe, 10_000)

    expect(readProviderProbeCache(storage, 'ws://daemon-a', 'codex', '/other/codex', 20_000)).toBeUndefined()
    expect(readProviderProbeCache(storage, 'ws://daemon-a', 'codex', undefined, 20_000)?.data).toEqual(probe)
  })

  test('ignores expired and malformed cache entries', () => {
    const storage = new MemoryStorage()
    writeProviderProbeCache(storage, 'ws://daemon-a', 'codex', null, probe, 10_000)
    expect(readProviderProbeCache(storage, 'ws://daemon-a', 'codex', null, 31 * 24 * 60 * 60 * 1_000)).toBeUndefined()

    storage.setItem('padu.provider-probes.v3', '{bad json')
    expect(readProviderProbeCache(storage, 'ws://daemon-a', 'codex')).toBeUndefined()
  })

  test('a version-only cached probe cannot authorize native capability', () => {
    const storage = new MemoryStorage()
    writeProviderProbeCache(storage, 'ws://daemon-a', 'codex', null, {
      ...probe,
      binaryIdentity: null,
    }, 10_000)
    const cached = readProviderProbeCache(
      storage,
      'ws://daemon-a',
      'codex',
      null,
      20_000,
    )
    const catalog: SkillCommandCatalog = {
      commands: [],
      skill_capability: {
        binaryVersion: '1.2.3',
        binaryIdentity: IDENTITY,
        probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
        capability: {
          nativeSyntax: 'dollar',
          leadingInvocation: true,
          inlineMentions: true,
          repeatedMentions: true,
        },
      },
    }
    const refreshed = refetchedBinaryIdentity({
      isError: false,
      data: cached?.data ?? null,
    })

    expect(refreshed).toBeNull()
    expect(catalogSkillCapability(
      catalog,
      'codex',
      cached?.data.version,
      refreshed?.binaryIdentity ?? null,
    )).toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex)
  })
})
