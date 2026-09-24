#!/usr/bin/env bun

import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const iconDirectory = join(root, "assets/icons");
const targetPath = join(root, "apps/mobile/src/components/padu-icons.generated.ts");

/** Placeholder the mobile icon component swaps for the runtime tint color. */
const TINT = "%%TINT%%";

/** `provider-*.svg` asset backing each agent provider. `codex` reuses the
 * OpenAI mark — the same mapping as web's `PROVIDER_ICONS`. */
const PROVIDER_FILES: Array<readonly [file: string, kind: string]> = [
  ["provider-agy.svg", "agy"],
  ["provider-amp.svg", "amp"],
  ["provider-claude.svg", "claude"],
  ["provider-openai.svg", "codex"],
  ["provider-command-code.svg", "commandCode"],
  ["provider-cursor.svg", "cursor"],
  ["provider-deepseek.svg", "deepSeek"],
  ["provider-fx.svg", "fx"],
  ["provider-opencode.svg", "openCode"],
  ["provider-grok.svg", "grok"],
  ["provider-kimi.svg", "kimi"],
  ["provider-ohmypi.svg", "ohMyPi"],
  ["provider-pi.svg", "pi"],
  ["provider-qoder.svg", "qoder"],
];

function toCamelCase(kebab: string): string {
  return kebab.replace(/-([a-z0-9])/g, (_, char: string) => char.toUpperCase());
}

function normalizeIcon(file: string): { key: string; viewBox: string; body: string } {
  const source = readFileSync(join(iconDirectory, file), "utf8");
  const svg = source.match(/<svg\b([^>]*)>([\s\S]*)<\/svg>/i);
  if (!svg) throw new Error(`Invalid Padu icon: ${file}`);

  const viewBox = svg[1]!.match(/viewBox="([^"]+)"/i)?.[1]?.trim() ?? "0 0 24 24";
  // Hoist the root presentation attributes onto a wrapping group (same as
  // web's tailwind icon transform), then swap every paint token for the
  // runtime tint placeholder. `fill="none"` strokes survive untouched, brand
  // gradients (Oh My Pi) keep their own stops, and `fill="black"` is left
  // alone: provider-command-code uses black inside its cutout mask, where
  // the value is luminance data rather than paint.
  const inherited = [...svg[1]!.matchAll(/\s(fill|stroke|stroke-width|stroke-linecap|stroke-linejoin|fill-rule|clip-rule)="([^"]+)"/gi)]
    .map((match) => `${match[1]}="${match[2]}"`)
    .join(" ");
  const tintify = (value: string): string => value
    .replace(/currentColor/gi, TINT)
    .replace(/#000(?:000)?\b/gi, TINT);
  const body = tintify(svg[2]!
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/<title>[\s\S]*?<\/title>/gi, "")
    .trim());
  const wrapped = inherited ? `<g ${tintify(inherited)}>${body}</g>` : body;
  return { key: toCamelCase(file.slice(0, -".svg".length)), viewBox, body: wrapped };
}

function generate(): string {
  const files = readdirSync(iconDirectory)
    .filter((file) => file.endsWith(".svg"))
    .sort();
  const icons = files.map(normalizeIcon);
  const byFile = new Map(icons.map((icon) => [icon.key, icon]));
  const providerEntries = PROVIDER_FILES.map(([file, kind]) => {
    const icon = byFile.get(toCamelCase(file.slice(0, -".svg".length)));
    if (!icon) throw new Error(`Missing provider icon asset: ${file}`);
    return `  ${kind}: "${icon.key}",`;
  });
  const iconEntries = icons.map(
    (icon) =>
      `  ${icon.key}: { viewBox: ${JSON.stringify(icon.viewBox)}, body: ${JSON.stringify(icon.body)} },`,
  );
  return `// Generated from assets/icons by scripts/generate-mobile-icons.ts. Do not edit directly.

import type { ProviderKind } from "@padu/client";

/** Every design-system mark, keyed like web's \`PADU_ICONS\`. The tint
 * placeholder is swapped for the runtime color by \`PaduIcon\`. */
export const PADU_ICON_SVGS = {
${iconEntries.join("\n")}
} as const;

export type PaduIconName = keyof typeof PADU_ICON_SVGS;

/** Design-system mark backing each agent provider. */
export const PROVIDER_ICON_NAME: Record<ProviderKind, PaduIconName> = {
${providerEntries.join("\n")}
};
`;
}

function main() {
  const isCheck = process.argv.includes("--check");
  const content = generate();
  const existing = existsSync(targetPath) ? readFileSync(targetPath, "utf-8") : null;
  if (existing !== content) {
    if (isCheck) {
      console.error("[mobile-icons:check] Generated icons are out of sync! Run `bun run mobile:icons:generate`.");
      process.exit(1);
    }
    writeFileSync(targetPath, content, "utf-8");
    console.log("[mobile-icons:generate] Updated Mobile generated icons (apps/mobile/src/components/padu-icons.generated.ts)");
  } else if (!isCheck) {
    console.log("[mobile-icons:generate] Up to date: Mobile generated icons (apps/mobile/src/components/padu-icons.generated.ts)");
  } else {
    console.log("[mobile-icons:check] Generated icons are current.");
  }
}

main();
