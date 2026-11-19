import { describe, expect, test } from "bun:test";

import {
  SKILL_CAPABILITY_PROBE_VERSION,
  evaluateSkillCapabilityProbe,
  readSkillCapabilityEvidence,
  writeSkillCapabilityEvidence,
} from "./provider-skill-capability";

const COMPLETE = {
  leadingInvocation: true,
  inlineMention: true,
  repeatedSameMention: true,
  repeatedDifferentMentions: true,
} as const;

const FIRST_IDENTITY = {
  canonicalPath: "/opt/codex/bin/codex",
  contentSha256: "a".repeat(64),
} as const;

const SECOND_IDENTITY = {
  canonicalPath: "/opt/other/codex",
  contentSha256: "a".repeat(64),
} as const;

describe("provider skill capability evidence", () => {
  test("enables native syntax only after every probe observation passes", () => {
    const evidence = evaluateSkillCapabilityProbe(
      "codex",
      "1.2.3",
      FIRST_IDENTITY,
      COMPLETE,
    );
    expect(evidence?.capability).toEqual({
      nativeSyntax: "dollar",
      leadingInvocation: true,
      inlineMentions: true,
      repeatedMentions: true,
    });

    const incomplete = evaluateSkillCapabilityProbe(
      "codex",
      "1.2.3",
      FIRST_IDENTITY,
      {
        leadingInvocation: true,
        inlineMention: true,
        repeatedSameMention: true,
      },
    );
    expect(incomplete?.capability.repeatedMentions).toBe(false);
    expect(incomplete?.capability.inlineMentions).toBe(true);
    expect(evaluateSkillCapabilityProbe(
      "codex",
      null,
      FIRST_IDENTITY,
      COMPLETE,
    )).toBeNull();
    expect(evaluateSkillCapabilityProbe("codex", "1.2.3", null, COMPLETE))
      .toBeNull();
  });

  test("keeps evidence separate for each exact binary identity", () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    };
    const first = evaluateSkillCapabilityProbe(
      "codex",
      "1.2.3",
      FIRST_IDENTITY,
      COMPLETE,
    )!;
    writeSkillCapabilityEvidence(storage, first);

    expect(readSkillCapabilityEvidence(
      storage,
      "codex",
      "1.2.3",
      FIRST_IDENTITY,
    )).toEqual(first);
    expect(readSkillCapabilityEvidence(
      storage,
      "codex",
      "1.2.3",
      SECOND_IDENTITY,
    )).toBeUndefined();
    expect(readSkillCapabilityEvidence(
      storage,
      "codex",
      "1.2.3",
      { ...FIRST_IDENTITY, contentSha256: "b".repeat(64) },
    )).toBeUndefined();
    expect(readSkillCapabilityEvidence(
      storage,
      "codex",
      "1.2.4",
      FIRST_IDENTITY,
    )).toBeUndefined();
    expect(readSkillCapabilityEvidence(
      storage,
      "fx",
      "1.2.3",
      FIRST_IDENTITY,
    )).toBeUndefined();
  });

  test("ignores stale probe-version records", () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    };
    const evidence = evaluateSkillCapabilityProbe(
      "pi",
      "4.0.0",
      FIRST_IDENTITY,
      COMPLETE,
    )!;
    writeSkillCapabilityEvidence(storage, {
      ...evidence,
      probeVersion: SKILL_CAPABILITY_PROBE_VERSION - 1,
    } as unknown as typeof evidence);
    expect(readSkillCapabilityEvidence(
      storage,
      "pi",
      "4.0.0",
      FIRST_IDENTITY,
    )).toBeUndefined();
  });

  test("rejects capability flags that disagree with observations or provider syntax", () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    };
    const evidence = evaluateSkillCapabilityProbe(
      "codex",
      "1.2.3",
      FIRST_IDENTITY,
      COMPLETE,
    )!;
    const key = [
      "codex",
      "1.2.3",
      FIRST_IDENTITY.canonicalPath,
      FIRST_IDENTITY.contentSha256,
    ].join("\0");
    for (const inconsistent of [
      {
        ...evidence,
        capability: {
          ...evidence.capability,
          leadingInvocation: false,
        },
      },
      {
        ...evidence,
        capability: {
          ...evidence.capability,
          inlineMentions: false,
        },
      },
      {
        ...evidence,
        capability: {
          ...evidence.capability,
          repeatedMentions: false,
        },
      },
      {
        ...evidence,
        capability: {
          ...evidence.capability,
          nativeSyntax: "skillSlash",
        },
      },
    ]) {
      values.set(
        "padu.provider-skill-capabilities.v2",
        JSON.stringify({
          version: 2,
          entries: { [key]: inconsistent },
        }),
      );
      expect(readSkillCapabilityEvidence(
        storage,
        "codex",
        "1.2.3",
        FIRST_IDENTITY,
      )).toBeUndefined();
    }
  });

  test("rejects legacy evidence without a binary identity", () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    };
    const { binaryIdentity: _, ...legacy } = evaluateSkillCapabilityProbe(
      "codex",
      "1.2.3",
      FIRST_IDENTITY,
      COMPLETE,
    )!;
    values.set(
      "padu.provider-skill-capabilities.v2",
      JSON.stringify({
        version: 2,
        entries: {
          legacy,
        },
      }),
    );
    expect(readSkillCapabilityEvidence(
      storage,
      "codex",
      "1.2.3",
      FIRST_IDENTITY,
    )).toBeUndefined();
  });
});
