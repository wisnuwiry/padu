import type {
  ProviderKind,
  SkillBinaryIdentity,
  SkillCapabilityMetadata,
  SkillNativeSyntax,
  SlashCommand,
} from "./generated";

export const SKILL_AUTOCOMPLETE_CAP = 64;
export const SKILL_CAPABILITY_PROBE_VERSION = 4;
export const SKILL_FALLBACK_PREFIX = "[mandatory! open skill: ";
export const SKILL_FALLBACK_SUFFIX = "]";
export const SKILL_CATALOG_UNAVAILABLE_WARNING =
  "Skill catalog is unavailable; $skill mentions were left unchanged.";

export type SkillCommand = Pick<
  SlashCommand,
  "name" | "description" | "scope" | "argument_hint"
> & {
  /** Absolute SKILL.md path on the active daemon host. */
  skill_file?: string | null;
};

export interface SkillCommandCatalog {
  commands: SlashCommand[];
  skill_capability?: SkillCapabilityMetadata | null;
}

export interface SkillTrigger {
  kind: "skill";
  query: string;
  start: number;
  end: number;
}

export interface SkillMention {
  name: string;
  start: number;
  end: number;
}

export interface ProviderSkillCapability {
  nativeSyntax: SkillNativeSyntax | null;
  leadingInvocation: boolean;
  inlineMentions: boolean;
  repeatedMentions: boolean;
}

/**
 * Native placement/count support stays disabled until a disposable provider
 * session verifies the exact behavior. The syntax names document the
 * provider's leading-invocation form without enabling it for `$` mentions.
 */
export const DEFAULT_PROVIDER_SKILL_CAPABILITIES = {
  agy: capability(null),
  amp: capability(null),
  claude: capability(null),
  codex: capability("dollar"),
  cursor: capability(null),
  deepSeek: capability(null),
  fx: capability("dollar"),
  openCode: capability(null),
  openCodeV2: capability(null),
  grok: capability(null),
  kimi: capability(null),
  commandCode: capability(null),
  ohMyPi: capability("skillSlash"),
  pi: capability("skillSlash"),
  qoder: capability(null),
} satisfies Record<ProviderKind, ProviderSkillCapability>;

export interface SkillRenderResult {
  providerText: string;
  mentions: SkillMention[];
  warnings: string[];
}

export interface SkillRenderOptions {
  /** A verified capability override for the active provider. */
  capability?: Partial<ProviderSkillCapability>;
}

export interface FreshSkillCatalog<T> {
  catalog: T | undefined;
  binaryVersion: string | null;
  binaryIdentity: SkillBinaryIdentity | null;
  catalogUnavailable: boolean;
}

export interface SkillBinarySelection {
  binaryVersion: string | null;
  binaryIdentity: SkillBinaryIdentity;
}

export type SkillBinaryRefresh = SkillBinarySelection | string | null;

export function isValidSkillBinaryIdentity(
  value: unknown,
): value is SkillBinaryIdentity {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }
  const identity = value as Record<string, unknown>;
  return typeof identity.canonicalPath === "string"
    && isAbsoluteHostPath(identity.canonicalPath)
    && typeof identity.contentSha256 === "string"
    && /^[0-9a-f]{64}$/.test(identity.contentSha256);
}

export function sameSkillBinaryIdentity(
  left: SkillBinaryIdentity | null | undefined,
  right: SkillBinaryIdentity | null | undefined,
): boolean {
  if (left == null || right == null) return left == null && right == null;
  return left.canonicalPath === right.canonicalPath
    && left.contentSha256 === right.contentSha256;
}

/**
 * The structural subset shared by TanStack Query refetch results. A failed
 * refetch can still carry the previous cached data, so callers must not treat
 * `data` as proof that the refresh succeeded.
 */
export interface SkillRefetchResult<T> {
  isError: boolean;
  data: T | undefined;
}

export function refetchedBinaryVersion(
  result: SkillRefetchResult<{ version?: string | null } | null>,
): string | null {
  return result.isError ? null : result.data?.version ?? null;
}

export function refetchedBinaryIdentity(
  result: SkillRefetchResult<{
    version?: string | null;
    binaryIdentity?: SkillBinaryIdentity | null;
  } | null>,
): SkillBinarySelection | null {
  if (result.isError) return null;
  const identity = result.data?.binaryIdentity;
  if (!isValidSkillBinaryIdentity(identity)) return null;
  return {
    binaryVersion: result.data?.version ?? null,
    binaryIdentity: identity,
  };
}

export function refetchedSkillCatalog<T>(
  result: SkillRefetchResult<T>,
): T | undefined {
  return result.isError ? undefined : result.data;
}

export function skillSubmissionWarnings(
  visiblePrompt: string,
  renderedText: string,
  renderedWarnings: readonly string[],
  catalogUnavailable: boolean,
): string[] {
  const warnings = new Set(renderedWarnings);
  if (catalogUnavailable && findSkillMentions(visiblePrompt).length > 0) {
    warnings.add(SKILL_CATALOG_UNAVAILABLE_WARNING);
  }
  return [...warnings];
}

/**
 * Submission must never authorize native syntax from a cached
 * (catalog, binary-version) pair. Resolve the executable identity and catalog
 * in order immediately before rendering. A failed refresh degrades to the
 * portable fallback by clearing the identity, so stale evidence cannot remain
 * active. A version-only refresh is retained for compatibility but never
 * authorizes native syntax.
 */
export async function resolveFreshSkillCatalog<T>(
  current: Pick<FreshSkillCatalog<T>, "catalog" | "binaryVersion">
    & Partial<Pick<FreshSkillCatalog<T>, "binaryIdentity">>,
  hasSkillMentions: boolean,
  refetchBinary: () => Promise<SkillBinaryRefresh>,
  refetchCatalog: () => Promise<T | undefined>,
): Promise<FreshSkillCatalog<T>> {
  if (!hasSkillMentions) {
    return {
      ...current,
      binaryIdentity: current.binaryIdentity ?? null,
      catalogUnavailable: false,
    };
  }

  let binaryVersion: string | null = null;
  let binaryIdentity: SkillBinaryIdentity | null = null;
  try {
    const refreshed = await refetchBinary();
    if (typeof refreshed === "string") {
      binaryVersion = refreshed;
    } else if (refreshed) {
      binaryVersion = refreshed.binaryVersion;
      binaryIdentity = refreshed.binaryIdentity;
    }
  } catch {
    // A provider whose exact executable cannot be identified has no verified
    // native capability.
  }

  let catalog: T | undefined;
  let catalogUnavailable = false;
  try {
    const refreshed = await refetchCatalog();
    if (refreshed === undefined) {
      catalogUnavailable = true;
    } else {
      catalog = refreshed;
    }
  } catch {
    // A failed refresh cannot prove that a cached absolute path belongs to the
    // active host. Drop it so every mention remains visible and receives the
    // catalog-unavailable warning.
    catalogUnavailable = true;
  }
  return { catalog, binaryVersion, binaryIdentity, catalogUnavailable };
}

export function catalogSkillCapability(
  catalog: SkillCommandCatalog | undefined,
  provider: ProviderKind,
  currentBinaryVersion: string | null | undefined,
  currentBinaryIdentity: SkillBinaryIdentity | string | null | undefined,
): ProviderSkillCapability {
  const fallback = DEFAULT_PROVIDER_SKILL_CAPABILITIES[provider];
  const metadata = catalog?.skill_capability;
  // Legacy callers may only have version text. It can no longer authorize
  // evidence because two different executables can report the same version.
  if (typeof currentBinaryIdentity === "string") return fallback;
  const version = currentBinaryVersion?.trim();
  if (!version) return fallback;
  if (
    !metadata
    || metadata.probeVersion !== SKILL_CAPABILITY_PROBE_VERSION
    || metadata.binaryVersion.trim() !== version
    || !isValidSkillBinaryIdentity(currentBinaryIdentity)
    || !isValidSkillBinaryIdentity(metadata.binaryIdentity)
    || !sameSkillBinaryIdentity(metadata.binaryIdentity, currentBinaryIdentity)
  ) return fallback;
  const capability = metadata.capability;
  if (capability.nativeSyntax !== fallback.nativeSyntax) return fallback;
  return {
    nativeSyntax: capability.nativeSyntax,
    leadingInvocation: capability.leadingInvocation,
    inlineMentions: capability.inlineMentions,
    repeatedMentions: capability.repeatedMentions,
  };
}

interface ProtectedRange {
  start: number;
  end: number;
  kind: "span" | "fence";
}

interface Fence {
  character: "`" | "~";
  length: number;
  start: number;
}

/**
 * Detect a `$` trigger at the current cursor. The current token must start at
 * a whitespace boundary or immediately after a closed Markdown code range.
 */
export function detectSkillTrigger(text: string, cursor: number): SkillTrigger | null {
  const end = Math.max(0, Math.min(cursor, text.length));
  const protectedRanges = markdownProtectedRanges(text);
  if (rangeContaining(protectedRanges, end) !== null) return null;

  let tokenStart = end;
  while (tokenStart > 0 && !isWhitespace(text[tokenStart - 1]!)) tokenStart -= 1;

  for (const range of protectedRanges) {
    if (range.end <= tokenStart || range.end > end) continue;
    tokenStart = range.end;
  }

  const token = text.slice(tokenStart, end);
  if (!token.startsWith("$")) return null;
  if (!isSkillBoundary(text, tokenStart, protectedRanges)) return null;
  return {
    kind: "skill",
    query: token.slice(1),
    start: tokenStart,
    end,
  };
}

/**
 * Return every eligible `$name` token outside inline code spans and fenced
 * code blocks. Unknown names are included; catalog resolution happens later.
 */
export function findSkillMentions(text: string): SkillMention[] {
  const protectedRanges = markdownProtectedRanges(text);
  const mentions: SkillMention[] = [];
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] !== "$") continue;
    if (rangeContaining(protectedRanges, index) !== null) continue;
    if (!isSkillBoundary(text, index, protectedRanges)) continue;
    let end = index + 1;
    while (end < text.length && !isWhitespace(text[end]!)) end += 1;
    if (end === index + 1) continue;
    mentions.push({ name: text.slice(index + 1, end), start: index, end });
    index = end - 1;
  }
  return mentions;
}

export function filterSkillCommands<T extends SkillCommand>(
  commands: readonly T[],
  query: string,
  cap = SKILL_AUTOCOMPLETE_CAP,
): T[] {
  const skills = commands.filter((command) => command.scope === "Skill");
  const normalizedQuery = query.toLowerCase();
  if (!normalizedQuery) return skills.slice(0, cap);

  return skills
    .map((command, index) => {
      const name = command.name.toLowerCase();
      const rank = name.startsWith(normalizedQuery)
        ? 0
        : name.includes(normalizedQuery)
          ? 1
          : isSubsequence(normalizedQuery, name)
            ? 2
            : null;
      return { command, index, rank };
    })
    .filter(
      (item): item is { command: T; index: number; rank: number } =>
        item.rank !== null,
    )
    .sort((left, right) => left.rank - right.rank || left.index - right.index)
    .slice(0, cap)
    .map(({ command }) => command);
}

export function replaceSkillTrigger(
  text: string,
  trigger: SkillTrigger,
  skill: Pick<SkillCommand, "name">,
): { text: string; cursor: number } {
  const before = text.slice(0, trigger.start);
  const after = text.slice(trigger.end);
  const prefix = before.length && !isWhitespace(before.at(-1)!) ? " " : "";
  const suffix = after.length && !isWhitespace(after[0]!) ? " " : "";
  const insert = `${prefix}$${skill.name}${suffix}`;
  const next = `${before}${insert}${after}`;
  const existingSeparator = suffix ? 0 : (after.match(/^\s/u)?.[0].length ?? 0);
  return {
    text: next,
    cursor: before.length + insert.length + existingSeparator,
  };
}

export function skillFallbackMarker(path: string): string | null {
  if (!isAbsoluteSkillPath(path) || /[\]\r\n]/u.test(path)) return null;
  return `${SKILL_FALLBACK_PREFIX}${path}${SKILL_FALLBACK_SUFFIX}`;
}

/**
 * Paths cross client platforms: a Windows desktop may render a Linux
 * daemon's `/home/.../SKILL.md`, so this deliberately does not use the host's
 * path parser.
 */
export function isAbsoluteSkillPath(path: string): boolean {
  if (!isWellFormedUnicode(path) || !path || /[/\\]$/u.test(path)) return false;

  if (path.startsWith("\\\\")) {
    const components = path.slice(2).split(/[\\/]/u).filter(Boolean);
    return components.length >= 3 && components.at(-1) === "SKILL.md";
  }
  if (path.startsWith("/")) {
    // Backslash is a literal filename character on a POSIX daemon host.
    return path.slice(path.lastIndexOf("/") + 1) === "SKILL.md";
  }
  if (/^[A-Za-z]:[\\/]/u.test(path)) {
    return path.split(/[\\/]/u).at(-1) === "SKILL.md";
  }
  return false;
}

/**
 * Resolve all eligible catalog mentions for the provider-facing prompt.
 * Every recognized occurrence is replaced independently so repetition is
 * preserved. Unknown names and invalid paths stay byte-for-byte unchanged.
 */
export function renderSkillMentions(
  text: string,
  provider: ProviderKind,
  commands: readonly SkillCommand[],
  options: SkillRenderOptions = {},
): SkillRenderResult {
  const mentions = findSkillMentions(text);
  if (!mentions.length) {
    return { providerText: text, mentions, warnings: [] };
  }

  const commandByMention = new Map<string, SkillCommand>();
  for (const command of commands) {
    if (command.scope === "Skill" && !commandByMention.has(command.name)) {
      commandByMention.set(command.name, command);
    }
  }
  const recognized = mentions.filter((mention) =>
    commandByMention.has(mention.name)
  );
  if (!recognized.length) {
    return { providerText: text, mentions, warnings: [] };
  }

  const capability = {
    ...DEFAULT_PROVIDER_SKILL_CAPABILITIES[provider],
    ...options.capability,
  };
  const positionsSupported = recognized.every((mention) =>
    text.slice(0, mention.start).trim().length === 0
      ? capability.leadingInvocation
      : capability.inlineMentions
  );
  const countSupported = recognized.length === 1 || capability.repeatedMentions;
  const useNative = capability.nativeSyntax !== null
    && positionsSupported
    && countSupported;

  const warnings: string[] = [];
  let cursor = 0;
  let providerText = "";
  for (const mention of mentions) {
    providerText += text.slice(cursor, mention.start);
    const command = commandByMention.get(mention.name);
    if (!command) {
      providerText += text.slice(mention.start, mention.end);
      cursor = mention.end;
      continue;
    }

    if (useNative) {
      providerText += nativeSkillSyntax(capability.nativeSyntax!, mention.name);
    } else {
      const marker = command.skill_file
        ? skillFallbackMarker(command.skill_file)
        : null;
      if (marker) {
        providerText += marker;
      } else {
        providerText += text.slice(mention.start, mention.end);
        const warning = skillWarning(mention.name, command.skill_file);
        if (!warnings.includes(warning)) warnings.push(warning);
      }
    }
    cursor = mention.end;
  }
  providerText += text.slice(cursor);
  return { providerText, mentions, warnings };
}

/**
 * Markdown protected ranges for inline code spans and fenced code blocks.
 * Fences use three or more matching backticks/tildes, may be indented up to
 * three spaces, understand CRLF, and remain protected through EOF when open.
 */
export function markdownProtectedRanges(text: string): ProtectedRange[] {
  const ranges: ProtectedRange[] = [];
  let index = 0;
  let inlineStart: number | null = null;
  let inlineDelimiterLength = 0;
  let fence: Fence | null = null;

  while (index < text.length) {
    if (fence) {
      const closing = closingFenceAt(text, index, fence);
      if (closing !== null) {
        const end = nextLineStart(text, closing);
        ranges.push({ start: fence.start, end, kind: "fence" });
        fence = null;
        index = end;
      } else {
        index = nextLineStart(text, index);
      }
      continue;
    }

    if (inlineStart !== null) {
      const closing = closingBacktickRun(
        text,
        index,
        inlineDelimiterLength,
      );
      if (closing === null) {
        ranges.push({ start: inlineStart, end: text.length, kind: "span" });
        index = text.length;
        break;
      }
      const end = closing + inlineDelimiterLength;
      ranges.push({ start: inlineStart, end, kind: "span" });
      inlineStart = null;
      inlineDelimiterLength = 0;
      index = end;
      continue;
    }

    if (isLineStart(text, index)) {
      const opening = openingFenceAt(text, index);
      if (opening) {
        fence = opening;
        index = nextLineStart(text, index);
        continue;
      }
    }

    if (text[index] === "`") {
      const length = runLength(text, index, "`");
      if (isEscaped(text, index)) {
        index += length;
      } else {
        inlineStart = index;
        inlineDelimiterLength = length;
        index += length;
      }
      continue;
    }
    index += 1;
  }

  if (inlineStart !== null) {
    ranges.push({ start: inlineStart, end: text.length, kind: "span" });
  }
  if (fence) {
    ranges.push({ start: fence.start, end: text.length, kind: "fence" });
  }
  return ranges;
}

function capability(
  nativeSyntax: SkillNativeSyntax | null,
): ProviderSkillCapability {
  return {
    nativeSyntax,
    leadingInvocation: false,
    inlineMentions: false,
    repeatedMentions: false,
  };
}

function isAbsoluteHostPath(path: string): boolean {
  if (!path || path.includes("\0") || /[\\/]$/u.test(path)) return false;
  if (path.startsWith("\\\\")) {
    return path.slice(2).split(/[\\/]/u).filter(Boolean).length >= 3;
  }
  if (path.startsWith("/")) return true;
  return /^[A-Za-z]:[\\/]/u.test(path);
}

function nativeSkillSyntax(
  syntax: SkillNativeSyntax,
  name: string,
): string {
  return syntax === "dollar" ? `$${name}` : `/skill:${name}`;
}

function skillWarning(
  name: string,
  path: string | null | undefined,
): string {
  if (!path) {
    return `Skill "${name}" was left unchanged because it has no SKILL.md path.`;
  }
  return `Skill "${name}" was left unchanged because its SKILL.md path cannot be represented safely.`;
}

function isSkillBoundary(
  text: string,
  index: number,
  ranges: readonly ProtectedRange[],
): boolean {
  if (index === 0 || isWhitespace(text[index - 1]!)) return true;
  return ranges.some((range) => range.end === index);
}

function rangeContaining(
  ranges: readonly ProtectedRange[],
  index: number,
): ProtectedRange | null {
  return ranges.find((range) => index >= range.start && index < range.end) ?? null;
}

function isWhitespace(character: string): boolean {
  return /\s/u.test(character);
}

function isSubsequence(needle: string, haystack: string): boolean {
  let matched = 0;
  for (const character of haystack) {
    if (character === needle[matched]) matched += 1;
    if (matched === needle.length) return true;
  }
  return false;
}

function openingFenceAt(text: string, lineStart: number): Fence | null {
  const contentEnd = lineContentEnd(text, lineStart);
  let index = lineStart;
  while (index < contentEnd && text[index] === " " && index - lineStart < 4) {
    index += 1;
  }
  if (index - lineStart > 3) return null;
  const character = text[index];
  if (character !== "`" && character !== "~") return null;
  const length = runLength(text, index, character);
  if (length < 3) return null;
  if (
    character === "`"
    && text.slice(index + length, contentEnd).includes("`")
  ) {
    return null;
  }
  return { character, length, start: lineStart };
}

function closingFenceAt(
  text: string,
  lineStart: number,
  fence: Fence,
): number | null {
  const contentEnd = lineContentEnd(text, lineStart);
  let index = lineStart;
  while (index < contentEnd && text[index] === " " && index - lineStart < 4) {
    index += 1;
  }
  if (index - lineStart > 3 || text[index] !== fence.character) return null;
  const length = runLength(text, index, fence.character);
  if (length < fence.length) return null;
  const rest = text.slice(index + length, contentEnd);
  return /^[ \t]*$/u.test(rest) ? index : null;
}

function closingBacktickRun(
  text: string,
  start: number,
  delimiterLength: number,
): number | null {
  let index = start;
  while (index < text.length) {
    const next = text.indexOf("`", index);
    if (next === -1) return null;
    const length = runLength(text, next, "`");
    if (length === delimiterLength) return next;
    index = next + length;
  }
  return null;
}

function runLength(
  text: string,
  start: number,
  character: string,
): number {
  let end = start;
  while (text[end] === character) end += 1;
  return end - start;
}

function isEscaped(text: string, index: number): boolean {
  let backslashes = 0;
  for (let cursor = index - 1; cursor >= 0 && text[cursor] === "\\"; cursor -= 1) {
    backslashes += 1;
  }
  return backslashes % 2 === 1;
}

function isWellFormedUnicode(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function isLineStart(text: string, index: number): boolean {
  return index === 0 || text[index - 1] === "\n" || text[index - 1] === "\r";
}

function lineContentEnd(text: string, lineStart: number): number {
  let index = lineStart;
  while (index < text.length && text[index] !== "\r" && text[index] !== "\n") {
    index += 1;
  }
  return index;
}

function nextLineStart(text: string, lineStart: number): number {
  const contentEnd = lineContentEnd(text, lineStart);
  if (contentEnd >= text.length) return text.length;
  if (text[contentEnd] === "\r" && text[contentEnd + 1] === "\n") {
    return contentEnd + 2;
  }
  return contentEnd + 1;
}
