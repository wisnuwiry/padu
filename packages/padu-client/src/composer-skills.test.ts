import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

import {
  DEFAULT_PROVIDER_SKILL_CAPABILITIES,
  SKILL_FALLBACK_PREFIX,
  SKILL_CAPABILITY_PROBE_VERSION,
  catalogSkillCapability,
  detectSkillTrigger,
  filterSkillCommands,
  findSkillMentions,
  isAbsoluteSkillPath,
  isValidSkillBinaryIdentity,
  refetchedBinaryIdentity,
  refetchedBinaryVersion,
  refetchedSkillCatalog,
  renderSkillMentions,
  resolveFreshSkillCatalog,
  replaceSkillTrigger,
  skillSubmissionWarnings,
  type SkillCommand,
  type SkillCommandCatalog,
  type ProviderSkillCapability,
} from "./composer-skills";
import type { SkillBinaryIdentity, SlashCommand } from "./generated";

const PATH = "/home/alice/.agents/skills/brainstroming/SKILL.md";
const IDENTITY: SkillBinaryIdentity = {
  canonicalPath: "/opt/codex/bin/codex",
  contentSha256: "a".repeat(64),
};
const OTHER_IDENTITY: SkillBinaryIdentity = {
  canonicalPath: "/opt/override/codex",
  contentSha256: "a".repeat(64),
};

describe("skill trigger detection", () => {
  test("detects $ only at a whitespace token boundary", () => {
    expect(detectSkillTrigger("$bra", 4)).toEqual({
      kind: "skill",
      query: "bra",
      start: 0,
      end: 4,
    });
    expect(detectSkillTrigger("use $bra", 8)).toEqual({
      kind: "skill",
      query: "bra",
      start: 4,
      end: 8,
    });
    expect(detectSkillTrigger("email$bra", 9)).toBeNull();
    expect(detectSkillTrigger("$bra more", 9)).toBeNull();
  });

  test("does not trigger inside code spans or fences", () => {
    expect(detectSkillTrigger("`$bra`", 5)).toBeNull();
    expect(detectSkillTrigger("```\n$bra\n```", 8)).toBeNull();
  });

  test("processes a mention immediately after a matching closing run", () => {
    const text = "`$inside`$bra";
    expect(detectSkillTrigger(text, text.length)).toEqual({
      kind: "skill",
      query: "bra",
      start: 9,
      end: 13,
    });
  });

  test("replaces only the active skill token", () => {
    const text = "please $bra then";
    const trigger = detectSkillTrigger(text, 11)!;
    expect(replaceSkillTrigger(text, trigger, { name: "brainstroming" })).toEqual({
      text: "please $brainstroming then",
      cursor: 22,
    });
  });
});

describe("skill catalog filtering", () => {
  test("keeps skills separate from slash commands", () => {
    const commands: SlashCommand[] = [
      command("brainstroming", "Skill", PATH),
      command("review", "Project", null),
      command("deploy", "Skill", "/skills/deploy/SKILL.md"),
    ];
    expect(filterSkillCommands(commands, "").map((item) => item.name)).toEqual([
      "brainstroming",
      "deploy",
    ]);
    expect(filterSkillCommands(commands, "brain").map((item) => item.name)).toEqual([
      "brainstroming",
    ]);
  });
});

describe("skill binary identity", () => {
  test("accepts absolute host paths without depending on the client OS", () => {
    for (const canonicalPath of [
      "/home/alice/.local/bin/codex",
      "/opt/codex/bin/codex",
      "C:\\Tools\\codex.exe",
      "D:/Tools/codex.exe",
      "\\\\server\\share\\codex.exe",
    ]) {
      expect(isValidSkillBinaryIdentity({
        canonicalPath,
        contentSha256: "a".repeat(64),
      })).toBe(true);
    }
    for (const canonicalPath of [
      "",
      "relative/codex",
      "/opt/codex/",
      "C:\\Tools\\",
      "C:relative\\codex.exe",
      "\\\\server\\share\\",
      "/opt/\0codex",
    ]) {
      expect(isValidSkillBinaryIdentity({
        canonicalPath,
        contentSha256: "a".repeat(64),
      })).toBe(false);
    }
    expect(isValidSkillBinaryIdentity({
      canonicalPath: "/opt/codex/bin/codex",
      contentSha256: "A".repeat(64),
    })).toBe(false);
  });
});

describe("provider rendering", () => {
  test("warns when stale catalog failure leaves mentions unresolved", () => {
    const rendered = renderSkillMentions(
      "$missing",
      "codex",
      [command("deploy", "Skill", PATH)],
      { capability: undefined },
    );
    expect(rendered.providerText).toBe("$missing");
    expect(rendered.warnings).toEqual([]);
    expect(
      skillSubmissionWarnings(
        "$missing",
        rendered.providerText,
        rendered.warnings,
        true,
      ),
    ).toEqual([
      "Skill catalog is unavailable; $skill mentions were left unchanged.",
    ]);
  });

  test("accepts capability evidence only for the exact current binary identity", () => {
    const capability = {
      nativeSyntax: "dollar" as const,
      leadingInvocation: true,
      inlineMentions: true,
      repeatedMentions: true,
    };
    const catalog = {
      commands: [],
      skill_capability: {
        binaryVersion: "1.2.3",
        binaryIdentity: IDENTITY,
        probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
        capability,
      },
    };
    expect(catalogSkillCapability(catalog, "codex", "1.2.3", IDENTITY))
      .toEqual(capability);
    expect(catalogSkillCapability(catalog, "codex", "1.2.4", IDENTITY))
      .toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(catalogSkillCapability(catalog, "codex", "1.2.3", OTHER_IDENTITY))
      .toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(catalogSkillCapability(catalog, "codex", "1.2.3", "1.2.3"))
      .toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(catalogSkillCapability(catalog, "codex", "1.2.3", null))
      .toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(catalogSkillCapability(catalog, "codex", null, IDENTITY))
      .toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(catalogSkillCapability({
      ...catalog,
      skill_capability: {
        ...catalog.skill_capability,
        probeVersion: SKILL_CAPABILITY_PROBE_VERSION - 1,
      },
    }, "codex", "1.2.3", IDENTITY))
      .toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(catalogSkillCapability({
      ...catalog,
      skill_capability: {
        ...catalog.skill_capability,
        capability: {
          ...capability,
          nativeSyntax: "skillSlash",
        },
      },
    }, "codex", "1.2.3", IDENTITY))
      .toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
  });

  test("submission refreshes executable identity and catalog before using cached evidence", async () => {
    const oldCapability: ProviderSkillCapability = {
      nativeSyntax: "dollar",
      leadingInvocation: true,
      inlineMentions: true,
      repeatedMentions: true,
    };
    const oldCatalog: SkillCommandCatalog = {
      commands: [command("deploy", "Skill", PATH)],
      skill_capability: {
        binaryVersion: "1.2.3",
        binaryIdentity: IDENTITY,
        probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
        capability: oldCapability,
      },
    };
    const calls: string[] = [];
    const fresh = await resolveFreshSkillCatalog<SkillCommandCatalog>(
      {
        catalog: oldCatalog,
        binaryVersion: "1.2.3",
        binaryIdentity: IDENTITY,
      },
      true,
      async () => {
        calls.push("identity");
        return {
          binaryVersion: "1.2.4",
          binaryIdentity: OTHER_IDENTITY,
        };
      },
      async () => {
        calls.push("catalog");
        return { ...oldCatalog, skill_capability: undefined };
      },
    );

    expect(calls).toEqual(["identity", "catalog"]);
    expect(catalogSkillCapability(
      fresh.catalog,
      "codex",
      fresh.binaryVersion,
      fresh.binaryIdentity,
    )).toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(renderSkillMentions("$deploy", "codex", fresh.catalog!.commands, {
      capability: catalogSkillCapability(
        fresh.catalog,
        "codex",
        fresh.binaryVersion,
        fresh.binaryIdentity,
      ),
    }).providerText).toBe(`[mandatory! open skill: ${PATH}]`);
  });

  test("a failed version refresh cannot authorize the cached catalog", async () => {
    let catalogRefetched = false;
    const fresh = await resolveFreshSkillCatalog<SkillCommandCatalog>(
      {
        catalog: {
          commands: [command("deploy", "Skill", PATH)],
          skill_capability: {
            binaryVersion: "1.2.3",
            binaryIdentity: IDENTITY,
            probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
            capability: {
              nativeSyntax: "dollar",
              leadingInvocation: true,
              inlineMentions: true,
              repeatedMentions: true,
            },
          },
        },
        binaryVersion: "1.2.3",
        binaryIdentity: IDENTITY,
      },
      true,
      async () => {
        throw new Error("probe failed");
      },
      async () => {
        catalogRefetched = true;
        return {
          commands: [command("deploy", "Skill", PATH)],
          skill_capability: undefined,
        };
      },
    );

    expect(catalogRefetched).toBe(true);
    expect(fresh.binaryVersion).toBeNull();
    expect(fresh.binaryIdentity).toBeNull();
    expect(fresh.catalog).toBeDefined();
    expect(catalogSkillCapability(
      fresh.catalog,
      "codex",
      fresh.binaryVersion,
      fresh.binaryIdentity,
    )).toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
    expect(renderSkillMentions("$deploy", "codex", fresh.catalog!.commands, {
      capability: catalogSkillCapability(
        fresh.catalog,
        "codex",
        fresh.binaryVersion,
        fresh.binaryIdentity,
      ),
    }).providerText).toBe(`[mandatory! open skill: ${PATH}]`);
  });

  test("a version-only refresh cannot authorize identity-scoped evidence", async () => {
    const catalog: SkillCommandCatalog = {
      commands: [command("deploy", "Skill", PATH)],
      skill_capability: {
        binaryVersion: "1.2.3",
        binaryIdentity: IDENTITY,
        probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
        capability: {
          nativeSyntax: "dollar",
          leadingInvocation: true,
          inlineMentions: true,
          repeatedMentions: true,
        },
      },
    };
    const fresh = await resolveFreshSkillCatalog(
      { catalog, binaryVersion: "1.2.3", binaryIdentity: IDENTITY },
      true,
      async () => refetchedBinaryVersion({
        isError: false,
        data: { version: "1.2.3" },
      }),
      async () => catalog,
    );

    expect(fresh.binaryVersion).toBe("1.2.3");
    expect(fresh.binaryIdentity).toBeNull();
    expect(catalogSkillCapability(
      fresh.catalog,
      "codex",
      fresh.binaryVersion,
      fresh.binaryIdentity,
    )).toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);
  });

  test("cached refetch failures cannot authorize stale native syntax or hide unavailable catalogs", async () => {
    const staleCatalog: SkillCommandCatalog = {
      commands: [command("deploy", "Skill", PATH)],
      skill_capability: {
        binaryVersion: "1.2.3",
        binaryIdentity: IDENTITY,
        probeVersion: SKILL_CAPABILITY_PROBE_VERSION,
        capability: {
          nativeSyntax: "dollar",
          leadingInvocation: true,
          inlineMentions: true,
          repeatedMentions: true,
        },
      },
    };
    const fresh = await resolveFreshSkillCatalog<SkillCommandCatalog>(
      {
        catalog: staleCatalog,
        binaryVersion: "1.2.3",
        binaryIdentity: IDENTITY,
      },
      true,
      async () => refetchedBinaryIdentity({
        isError: true,
        data: { version: "1.2.3", binaryIdentity: IDENTITY },
      }),
      async () => refetchedSkillCatalog({
        isError: true,
        data: staleCatalog,
      }),
    );

    expect(fresh.binaryVersion).toBeNull();
    expect(fresh.binaryIdentity).toBeNull();
    expect(fresh.catalogUnavailable).toBe(true);
    expect(fresh.catalog).toBeUndefined();
    expect(catalogSkillCapability(
      fresh.catalog,
      "codex",
      fresh.binaryVersion,
      fresh.binaryIdentity,
    )).toEqual(DEFAULT_PROVIDER_SKILL_CAPABILITIES.codex);

    const rendered = renderSkillMentions("$deploy", "codex", fresh.catalog?.commands ?? [], {
      capability: catalogSkillCapability(
        fresh.catalog,
        "codex",
        fresh.binaryVersion,
        fresh.binaryIdentity,
      ),
    });
    expect(rendered.providerText).toBe("$deploy");
    expect(skillSubmissionWarnings(
      "$deploy",
      rendered.providerText,
      rendered.warnings,
      fresh.catalogUnavailable,
    )).toEqual([
      "Skill catalog is unavailable; $skill mentions were left unchanged.",
    ]);
  });

  test("matches the shared cross-surface fallback fixture", () => {
    const cases = JSON.parse(readFileSync(
      new URL("../testdata/skill-fallback-parity.json", import.meta.url),
      "utf8",
    )) as Array<{
      provider: Parameters<typeof renderSkillMentions>[1];
      text: string;
      command: string;
      path?: string;
      capability?: ProviderSkillCapability;
      expected: string;
      warnings?: string[];
    }>;
    for (const item of cases) {
      const rendered = renderSkillMentions(item.text, item.provider, [
        command(item.command, "Skill", item.path ?? null),
      ], item.capability ? { capability: item.capability } : {});
      expect(rendered.providerText).toBe(item.expected);
      expect(rendered.warnings).toEqual(item.warnings ?? []);
    }
  });

  test("uses the exact fallback marker for every recognized occurrence", () => {
    const rendered = renderSkillMentions(
      "$brainstroming explain it, then use $brainstroming again",
      "codex",
      [command("brainstroming", "Skill", PATH)],
    );
    const marker = `[mandatory! open skill: ${PATH}]`;
    expect(rendered.providerText).toBe(
      `${marker} explain it, then use ${marker} again`,
    );
    expect(rendered.warnings).toEqual([]);
    expect(rendered.mentions).toHaveLength(2);
  });

  test("leaves unknown and case-mismatched names unchanged", () => {
    const input = "$Unknown and $Brainstroming stay";
    expect(
      renderSkillMentions(input, "claude", [
        command("brainstroming", "Skill", PATH),
      ]).providerText,
    ).toBe(input);
  });

  test("keeps spaces in the path and rejects ambiguous paths with warnings", () => {
    const spaced = renderSkillMentions("$bra", "claude", [
      command("bra", "Skill", "/skills/Brain Stroming/SKILL.md"),
    ]);
    expect(spaced.providerText).toBe(
      "[mandatory! open skill: /skills/Brain Stroming/SKILL.md]",
    );
    expect(renderSkillMentions("$bra", "claude", [
      command("bra", "Skill", "/skills/a\\b/SKILL.md"),
    ]).providerText).toBe("[mandatory! open skill: /skills/a\\b/SKILL.md]");

    for (const path of ["/skills/bad]/SKILL.md", "/skills/a\rb/SKILL.md", "/skills/a\nb/SKILL.md"]) {
      const rendered = renderSkillMentions("$bra", "claude", [
        command("bra", "Skill", path),
      ]);
      expect(rendered.providerText).toBe("$bra");
      expect(rendered.warnings).toHaveLength(1);
      expect(rendered.warnings[0]).toContain("SKILL.md path");
    }
  });

  test("requires an absolute path across Windows and Linux clients", () => {
    expect(isAbsoluteSkillPath("/home/alice/skills/deploy/SKILL.md")).toBe(true);
    expect(isAbsoluteSkillPath("C:\\Users\\alice\\skills\\deploy\\SKILL.md")).toBe(true);
    expect(isAbsoluteSkillPath("\\\\server\\share\\SKILL.md")).toBe(true);
    expect(isAbsoluteSkillPath("relative/skills/deploy/SKILL.md")).toBe(false);
    expect(isAbsoluteSkillPath("/home/alice/skills/deploy/")).toBe(false);
    expect(isAbsoluteSkillPath("/home/alice/skills/deploy/SKILL.md/")).toBe(false);
    expect(isAbsoluteSkillPath("/home/alice/skills/deploy/notes.md")).toBe(false);
    expect(isAbsoluteSkillPath("C:\\Users\\alice\\skills\\deploy\\")).toBe(false);
    expect(isAbsoluteSkillPath("C:\\Users\\alice\\skills\\deploy\\notes.md")).toBe(false);
    expect(isAbsoluteSkillPath("\\\\server\\SKILL.md")).toBe(false);
    expect(isAbsoluteSkillPath("/skills/bad\ud800/SKILL.md")).toBe(false);

    const rendered = renderSkillMentions("$bra", "claude", [
      command("bra", "Skill", "relative/SKILL.md"),
    ]);
    expect(rendered.providerText).toBe("$bra");
    expect(rendered.warnings).toHaveLength(1);
  });

  test("leaves a catalog skill without a path unchanged and warns", () => {
    const rendered = renderSkillMentions("$bra", "claude", [
      command("bra", "Skill", null),
    ]);
    expect(rendered.providerText).toBe("$bra");
    expect(rendered.warnings).toEqual([
      'Skill "bra" was left unchanged because it has no SKILL.md path.',
    ]);
  });

  test("uses the fallback by default even when a native leading form is known", () => {
    const rendered = renderSkillMentions("$bra", "codex", [
      command("bra", "Skill", PATH),
    ]);
    expect(rendered.providerText.startsWith(SKILL_FALLBACK_PREFIX)).toBe(true);

    const verified = renderSkillMentions("$bra $bra", "codex", [
      command("bra", "Skill", PATH),
    ], {
      capability: {
        leadingInvocation: true,
        inlineMentions: true,
        repeatedMentions: true,
      },
    });
    expect(verified.providerText).toBe("$bra $bra");
  });

  test("unknown mentions do not force recognized mentions onto the fallback", () => {
    const rendered = renderSkillMentions("$bra $unknown", "codex", [
      command("bra", "Skill", PATH),
    ], {
      capability: {
        leadingInvocation: true,
        inlineMentions: true,
        repeatedMentions: true,
      },
    });
    expect(rendered.providerText).toBe("$bra $unknown");
  });
});

describe("Markdown protection", () => {
  const commands = [command("brainstroming", "Skill", PATH)];

  test("supports longer inline backtick runs and literal backslashes", () => {
    expect(
      renderSkillMentions("```` $brainstroming ````", "claude", commands)
        .providerText,
    ).toBe("```` $brainstroming ````");

    const afterClosingRun = renderSkillMentions(
      "`$inside`$brainstroming",
      "claude",
      commands,
    );
    expect(afterClosingRun.providerText).toBe("`$inside`" + `[mandatory! open skill: ${PATH}]`);

    const literalBackslash = renderSkillMentions(
      "`\\`$brainstroming",
      "claude",
      commands,
    );
    expect(literalBackslash.providerText).toBe("`\\`" + `[mandatory! open skill: ${PATH}]`);
  });

  test("supports backtick and tilde fences of length three or greater", () => {
    const input = [
      "```",
      "$brainstroming",
      "```",
      "~~~~",
      "$brainstroming",
      "~~~",
      "~~~~",
      "$brainstroming",
    ].join("\n");
    expect(renderSkillMentions(input, "claude", commands).providerText).toBe([
      "```",
      "$brainstroming",
      "```",
      "~~~~",
      "$brainstroming",
      "~~~",
      "~~~~",
      `[mandatory! open skill: ${PATH}]`,
    ].join("\n"));
  });

  test("handles CRLF and unclosed fences through end of input", () => {
    const crlf = "before\r\n```\r\n$brainstroming\r\n```\r\nafter $brainstroming";
    expect(renderSkillMentions(crlf, "claude", commands).providerText).toBe(
      "before\r\n```\r\n$brainstroming\r\n```\r\nafter " +
        `[mandatory! open skill: ${PATH}]`,
    );

    const unclosed = "before\n```\n$brainstroming\nafter";
    expect(renderSkillMentions(unclosed, "claude", commands).providerText).toBe(
      unclosed,
    );
  });

  test("does not rewrite mentions inside four-backtick fences", () => {
    const input = "````\n```\n$brainstroming\n```\n````";
    expect(renderSkillMentions(input, "claude", commands).providerText).toBe(input);
  });
});

function command(
  name: string,
  scope: SlashCommand["scope"],
  skillFile: string | null,
): SlashCommand & SkillCommand {
  return {
    name,
    description: "",
    scope,
    argument_hint: null,
    template: null,
    skill_file: skillFile,
  };
}
