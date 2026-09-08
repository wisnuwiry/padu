import fs from "node:fs";
import path from "node:path";
import { defineConfig, type UserConfig } from "vite";
import tsConfigPaths from "vite-tsconfig-paths";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

import { AGENT_PAGES } from "./src/data/agent-pages";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const siteHost = "https://padu.dev";

function discoverDocsRoutes(): string[] {
  const docsDir = path.join(repoRoot, "public-docs");
  if (!fs.existsSync(docsDir)) return ["/docs"];
  const routes = new Set<string>(["/docs"]);
  const walk = (dir: string) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      if (!entry.name.endsWith(".md")) continue;
      const rel = path
        .relative(docsDir, full)
        .replace(/\.md$/, "")
        .replace(/\/index$/, "");
      if (rel === "index" || rel === "") continue;
      routes.add(`/docs/${rel.split(path.sep).join("/")}`);
    }
  };
  walk(docsDir);
  return [...routes].sort();
}

function discoverAgentRoutes(): string[] {
  const routesDir = path.join(import.meta.dirname, "src/routes");
  if (!fs.existsSync(routesDir)) return [];
  const reserved = new Set([
    "__root",
    "agents",
    "changelog",
    "docs",
    "download",
    "index",
    "privacy",
    "terms",
  ]);
  return fs
    .readdirSync(routesDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".tsx"))
    .map((entry) => entry.name.replace(/\.tsx$/, ""))
    .filter((name) => !reserved.has(name))
    .sort()
    .map((slug) => `/${slug}`);
}

const sitemapPages = [
  "/",
  "/agents",
  "/changelog",
  "/download",
  "/privacy",
  "/terms",
  ...discoverAgentRoutes(),
  ...discoverDocsRoutes(),
].map((routePath) => ({
  path: routePath,
}));

function parseDocFrontmatter(raw: string): { title?: string; description?: string } {
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) return {};
  const data: Record<string, string> = {};
  for (const line of match[1].split("\n")) {
    const colonIdx = line.indexOf(":");
    if (colonIdx === -1) continue;
    const key = line.slice(0, colonIdx).trim();
    let value = line.slice(colonIdx + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    data[key] = value;
  }
  return data;
}

function syncMarkdownDocs(): void {
  const docsDir = path.join(repoRoot, "public-docs");
  if (!fs.existsSync(docsDir)) return;
  const publicDir = path.join(import.meta.dirname, "public");
  const targetDocsDir = path.join(publicDir, "docs");
  fs.mkdirSync(targetDocsDir, { recursive: true });
  for (const entry of fs.readdirSync(docsDir, { withFileTypes: true })) {
    if (!entry.name.endsWith(".md")) continue;
    const src = path.join(docsDir, entry.name);
    if (entry.name === "index.md") {
      fs.copyFileSync(src, path.join(publicDir, "docs.md"));
    } else {
      fs.copyFileSync(src, path.join(targetDocsDir, entry.name));
    }
  }
}

function syncLlmsTxt(): void {
  const docsDir = path.join(repoRoot, "public-docs");
  if (!fs.existsSync(docsDir)) return;
  const docLines: string[] = [];
  for (const entry of fs.readdirSync(docsDir, { withFileTypes: true })) {
    if (!entry.isFile() || !entry.name.endsWith(".md") || entry.name === "index.md") continue;
    const slug = entry.name.replace(/\.md$/, "");
    const raw = fs.readFileSync(path.join(docsDir, entry.name), "utf-8");
    const { title = slug, description } = parseDocFrontmatter(raw);
    const suffix = description ? `: ${description}` : "";
    docLines.push(`- [${title}](https://padu.dev/docs/${slug}.md)${suffix}`);
  }
  docLines.sort();

  const agentLines = AGENT_PAGES.map(
    (agent) => `- [${agent.name}](https://padu.dev/${agent.slug}): ${agent.subtitle}`,
  );

  const content = `# Padu

> Native, local-first desktop and web workspace for orchestrating AI coding agents. Built in Rust with GPUI.

Padu is an open-source native application that lets you run and orchestrate AI coding agents on your own machine. Your code stays strictly local — Padu connects directly to your real development environment, local Git repositories, and installed CLI agents instead of running in someone else's cloud.

Engineered with Rust and GPUI (the GPU-accelerated UI engine behind Zed), Padu delivers instant startup, minimal memory consumption, and smooth 120fps transcript streaming. A self-hosted daemon manages subprocess lifecycles, structured event streaming, Git worktree isolation, and turn-by-turn checkpoint rewinds.

Padu provides native direct drivers and ACP integrations for leading coding agents: Claude Code, OpenAI Codex CLI, OpenCode, Antigravity, Pi Agent, Oh My Pi, Amp, DeepSeek, Cursor CLI, Fx, Grok Build, and Kimi Code.

Distribution: Native desktop apps for macOS, Windows, Linux; web application; companion mobile clients. License: GPL-3.0 at https://github.com/wisnuwiry/padu. Marketing site: https://padu.dev.

## Docs

${docLines.join("\n")}

## Supported agents

${agentLines.join("\n")}

## Optional

- [Changelog](https://padu.dev/changelog): Release notes for the Padu daemon, CLI, desktop, and mobile apps.
- [Download](https://padu.dev/download): Install Padu on Mac, Windows, Linux, iOS, Android, or run the web app.
- [Privacy](https://padu.dev/privacy): Privacy policy.
- [Terms](https://padu.dev/terms): Terms of service for Padu open-source software.
- [GitHub](https://github.com/wisnuwiry/padu): Source code, issues, and releases.
`;

  const publicDir = path.join(import.meta.dirname, "public");
  fs.writeFileSync(path.join(publicDir, "llms.txt"), content, "utf-8");
}

syncMarkdownDocs();
syncLlmsTxt();

export default defineConfig((): UserConfig => {
  return {
    server: {
      host: "0.0.0.0",
      port: 3000,
      strictPort: false,
      fs: {
        allow: [repoRoot],
      },
      watch: {
        ignored: ["**/.tanstack/**"],
      },
    },
    plugins: [
      tsConfigPaths(),
      tanstackStart({
        router: {
          quoteStyle: "double",
          semicolons: true,
        },
        prerender: {
          enabled: true,
        },
        pages: sitemapPages,
        sitemap: {
          host: siteHost,
        },
      }),
      react(),
      tailwindcss(),
    ],
  };
});
