// Source of truth for per-agent marketing landing pages.
// To add a new agent, append an entry here and create a route file at
// `src/routes/<slug>.tsx`. The sitemap (vite.config) reads `AGENT_PAGE_SLUGS`.

export interface AgentPage {
  slug: string;
  name: string;
  category?: "native" | "acp";
  badge?: string;
  title: string;
  subtitle: string;
  metaTitle: string;
  metaDescription: string;
}

export const AGENT_PAGES = [
  {
    slug: "claude-code",
    name: "Claude Code",
    category: "native",
    badge: "Native",
    title: "Open source desktop app for Claude Code",
    subtitle:
      "Run Anthropic's Claude Code on your machine. Stream transcripts, switch models, review diffs, and queue prompts natively.",
    metaTitle: "Claude Code Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Claude Code. Run agents on your machine, monitor progress, review diffs, and merge from anywhere. Self-hosted, your code stays local.",
  },
  {
    slug: "codex",
    name: "Codex CLI",
    category: "native",
    badge: "Native",
    title: "Open source desktop app for Codex",
    subtitle:
      "Run OpenAI's Codex CLI on your machine. Sandbox controls, workspace isolation, and local checkpoint history.",
    metaTitle: "Codex CLI Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for OpenAI Codex. Launch agents on your machine, monitor progress, and ship code from anywhere. Self-hosted.",
  },
  {
    slug: "opencode",
    name: "OpenCode",
    category: "native",
    badge: "Native",
    title: "Open source desktop app for OpenCode",
    subtitle:
      "Run OpenCode on your machine with multi-provider model support, local transcript storage, and Git worktrees.",
    metaTitle: "OpenCode Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for OpenCode. Launch agents on your machine, watch them work, ship code from anywhere. Self-hosted.",
  },
  {
    slug: "pi",
    name: "Pi Agent",
    category: "native",
    badge: "Native",
    title: "Open source desktop app for the Pi coding agent",
    subtitle:
      "Run the minimal Pi coding agent on your machine. Lightweight, multi-provider LLM support, and fast terminal transcripts.",
    metaTitle: "Pi Agent Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for the Pi coding agent. Launch sessions on your machine, monitor progress, merge from anywhere. Self-hosted.",
  },
  {
    slug: "amp",
    name: "Amp",
    category: "native",
    badge: "Native",
    title: "Open source desktop app for Amp",
    subtitle:
      "Run Amp on your machine. Native protocol integration, deep reasoning tokens, and structured session continuity.",
    metaTitle: "Amp Coding Agent Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Amp, the frontier coding agent. Launch tasks on your machine, monitor progress, ship from anywhere.",
  },
  {
    slug: "cursor",
    name: "Cursor CLI",
    category: "acp",
    badge: "ACP",
    title: "Open source desktop app for Cursor CLI",
    subtitle:
      "Send tasks to Cursor on your machine. Monitor agent turns, inspect unified diffs, and work in isolated worktrees.",
    metaTitle: "Cursor CLI Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Cursor CLI. Launch tasks on your machine, monitor output, review diffs, and merge from anywhere. Self-hosted.",
  },
  {
    slug: "fx",
    name: "Fx",
    category: "acp",
    badge: "ACP",
    title: "Open source desktop app for Fx",
    subtitle:
      "Fast terminal-based coding assistant integrated directly into Padu's unified workspace and review diffs.",
    metaTitle: "Fx Coding Agent Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Fx. Launch coding sessions on your workstation, stream live transcripts, and inspect diffs.",
  },
  {
    slug: "grok",
    name: "Grok Build",
    category: "acp",
    badge: "ACP",
    title: "Open source desktop app for Grok Build",
    subtitle:
      "Run xAI's Grok Build agentic coding CLI on your machine with live output streaming and diff reviews.",
    metaTitle: "Grok Build Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for xAI's Grok Build coding agent. Launch sessions on your machine, monitor progress, and ship code.",
  },
  {
    slug: "elph",
    name: "Elph",
    category: "acp",
    badge: "ACP",
    title: "Open source desktop app for Elph",
    subtitle:
      "Run Elph's ACP coding agent on your machine with local session management, model discovery, and streamed tool activity.",
    metaTitle: "Elph Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Elph. Launch ACP coding sessions on your machine, monitor progress, and review changes locally.",
  },
  {
    slug: "kimi",
    name: "Kimi Code CLI",
    category: "acp",
    badge: "ACP",
    title: "Open source desktop app for Kimi Code CLI",
    subtitle:
      "Run Moonshot AI's Kimi Code CLI on your machine with local session management and Git checkpoints.",
    metaTitle: "Kimi Code CLI Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Moonshot AI's Kimi Code CLI. Launch sessions on your machine, monitor progress, ship from anywhere.",
  },
  {
    slug: "deepseek-tui",
    name: "DeepSeek (CodeWhale)",
    category: "native",
    badge: "Native",
    title: "Open source desktop app for DeepSeek",
    subtitle:
      "Run DeepSeek coding models and CodeWhale CLI with native token streaming and thought fold controls.",
    metaTitle: "DeepSeek Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for DeepSeek and CodeWhale. Launch coding sessions on your machine, monitor progress, ship from anywhere.",
  },
  {
    slug: "ohmypi",
    name: "Oh My Pi",
    category: "native",
    badge: "Native",
    title: "Open source desktop app for Oh My Pi",
    subtitle:
      "Run Oh My Pi agent with native permission approvals, multi-turn reasoning, and local checkpoints.",
    metaTitle: "Oh My Pi Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Oh My Pi coding agent. Launch sessions on your machine, monitor progress, ship from anywhere. Self-hosted.",
  },
  {
    slug: "antigravity",
    name: "Antigravity",
    category: "acp",
    badge: "ACP",
    title: "Open source desktop app for Google Antigravity",
    subtitle:
      "Run Google Antigravity on your machine with native ACP integration, reasoning effort controls, and local checkpoint history.",
    metaTitle: "Antigravity Desktop & Web App, Open Source",
    metaDescription:
      "Open source app for Google Antigravity. Launch tasks on your machine, monitor output, review diffs, and merge from anywhere. Self-hosted.",
  },
] as const satisfies readonly AgentPage[];

export const AGENT_PAGE_SLUGS: readonly string[] = AGENT_PAGES.map((p) => p.slug);

const AGENT_PAGE_MAP_INTERNAL: Record<string, AgentPage> = Object.fromEntries(
  AGENT_PAGES.map((p) => [p.slug, p]),
);

export function getAgentPage(slug: string): AgentPage {
  const page = AGENT_PAGE_MAP_INTERNAL[slug];
  if (!page) throw new Error(`Unknown agent page slug: ${slug}`);
  return page;
}
