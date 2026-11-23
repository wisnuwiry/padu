import type {
  ProviderKind,
  SkillBinaryIdentity,
  SkillNativeSyntax,
} from "./generated";
import {
  SKILL_CAPABILITY_PROBE_VERSION,
  isValidSkillBinaryIdentity,
  type ProviderSkillCapability,
} from "./composer-skills";
export { SKILL_CAPABILITY_PROBE_VERSION } from "./composer-skills";

const CACHE_KEY = "padu.provider-skill-capabilities.v2";
const CACHE_VERSION = 2;
const PROVIDER_KINDS: ReadonlySet<string> = new Set([
  "agy",
  "amp",
  "claude",
  "codex",
  "cursor",
  "deepSeek",
  "fx",
  "openCode",
  "openCodeV2",
  "grok",
  "kimi",
  "commandCode",
  "ohMyPi",
  "pi",
  "qoder",
]);

export interface SkillCapabilityProbeObservations {
  leadingInvocation: boolean;
  inlineMention: boolean;
  repeatedSameMention: boolean;
  repeatedDifferentMentions: boolean;
}

export interface SkillCapabilityEvidence {
  provider: ProviderKind;
  binaryVersion: string;
  binaryIdentity: SkillBinaryIdentity;
  probeVersion: typeof SKILL_CAPABILITY_PROBE_VERSION;
  observations: SkillCapabilityProbeObservations;
  capability: ProviderSkillCapability;
}

type StorageLike = Pick<Storage, "getItem" | "setItem">;

interface CacheState {
  version: typeof CACHE_VERSION;
  entries: Record<string, SkillCapabilityEvidence>;
}

/**
 * Convert four explicit observations into renderer capability evidence.
 * Missing or incomplete evidence never enables native syntax.
 */
export function evaluateSkillCapabilityProbe(
  provider: ProviderKind,
  binaryVersion: string | null | undefined,
  binaryIdentity: SkillBinaryIdentity | null | undefined,
  observations: Partial<SkillCapabilityProbeObservations>,
): SkillCapabilityEvidence | null {
  const version = binaryVersion?.trim();
  if (!version || !isValidSkillBinaryIdentity(binaryIdentity)) return null;
  const complete: SkillCapabilityProbeObservations = {
    leadingInvocation: observations.leadingInvocation === true,
    inlineMention: observations.inlineMention === true,
    repeatedSameMention: observations.repeatedSameMention === true,
    repeatedDifferentMentions:
      observations.repeatedDifferentMentions === true,
  };
  return {
    provider,
    binaryVersion: version,
    binaryIdentity,
    probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
    observations: complete,
    capability: {
      nativeSyntax: nativeSkillSyntax(provider),
      leadingInvocation: complete.leadingInvocation,
      inlineMentions: complete.inlineMention,
      repeatedMentions:
        complete.repeatedSameMention && complete.repeatedDifferentMentions,
    },
  };
}

export function readSkillCapabilityEvidence(
  storage: StorageLike | null,
  provider: ProviderKind,
  binaryVersion: string | null | undefined,
  binaryIdentity: SkillBinaryIdentity | null | undefined,
): SkillCapabilityEvidence | undefined {
  const version = binaryVersion?.trim();
  if (!storage || !version || !isValidSkillBinaryIdentity(binaryIdentity)) {
    return undefined;
  }
  const state = readState(storage);
  const evidence = state?.entries[entryKey(provider, version, binaryIdentity)];
  if (
    !evidence
    || evidence.provider !== provider
    || evidence.binaryVersion !== version
    || evidence.binaryIdentity.canonicalPath !== binaryIdentity.canonicalPath
    || evidence.binaryIdentity.contentSha256 !== binaryIdentity.contentSha256
    || evidence.probeVersion !== SKILL_CAPABILITY_PROBE_VERSION
  ) return undefined;
  return evidence;
}

export function writeSkillCapabilityEvidence(
  storage: StorageLike | null,
  evidence: SkillCapabilityEvidence,
): void {
  if (
    !storage
    || !isEvidence(evidence)
    || !evidence.binaryVersion.trim()
    || !isValidSkillBinaryIdentity(evidence.binaryIdentity)
  ) return;
  const state = readState(storage) ?? { version: CACHE_VERSION, entries: {} };
  state.entries[entryKey(
    evidence.provider,
    evidence.binaryVersion,
    evidence.binaryIdentity,
  )] = evidence;
  try {
    storage.setItem(CACHE_KEY, JSON.stringify(state));
  } catch {
    // Capability evidence is disposable; the safe fallback still works.
  }
}

export function nativeSkillSyntax(
  provider: ProviderKind,
): SkillNativeSyntax | null {
  if (provider === "codex" || provider === "fx") return "dollar";
  if (provider === "pi" || provider === "ohMyPi") return "skillSlash";
  return null;
}

function readState(storage: StorageLike): CacheState | undefined {
  try {
    const parsed = JSON.parse(storage.getItem(CACHE_KEY) ?? "null") as unknown;
    if (!isRecord(parsed)
      || parsed.version !== CACHE_VERSION
      || !isRecord(parsed.entries)) {
      return undefined;
    }
    const entries: Record<string, SkillCapabilityEvidence> = {};
    for (const [key, value] of Object.entries(parsed.entries)) {
      if (isEvidence(value)) entries[key] = value;
    }
    return { version: CACHE_VERSION, entries };
  } catch {
    return undefined;
  }
}

function isEvidence(value: unknown): value is SkillCapabilityEvidence {
  if (!isRecord(value)
    || !isProviderKind(value.provider)
    || typeof value.binaryVersion !== "string"
    || !isValidSkillBinaryIdentity(value.binaryIdentity)
    || value.probeVersion !== SKILL_CAPABILITY_PROBE_VERSION
    || !isRecord(value.observations)
    || !isRecord(value.capability)) {
    return false;
  }
  const observations = value.observations;
  const capability = value.capability;
  return [
    observations.leadingInvocation,
    observations.inlineMention,
    observations.repeatedSameMention,
    observations.repeatedDifferentMentions,
    capability.leadingInvocation,
    capability.inlineMentions,
    capability.repeatedMentions,
  ].every((item) => typeof item === "boolean")
    && capability.nativeSyntax === nativeSkillSyntax(value.provider)
    && capability.leadingInvocation === observations.leadingInvocation
    && capability.inlineMentions === observations.inlineMention
    && capability.repeatedMentions === (
      observations.repeatedSameMention
      && observations.repeatedDifferentMentions
    );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isProviderKind(value: unknown): value is ProviderKind {
  return typeof value === "string" && PROVIDER_KINDS.has(value);
}

function entryKey(
  provider: ProviderKind,
  binaryVersion: string,
  binaryIdentity: SkillBinaryIdentity,
): string {
  return [
    provider,
    binaryVersion,
    binaryIdentity.canonicalPath,
    binaryIdentity.contentSha256,
  ].join("\u0000");
}
