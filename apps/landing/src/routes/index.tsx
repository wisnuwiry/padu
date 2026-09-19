import { createFileRoute } from "@tanstack/react-router";
import { LandingPage } from "~/components/landing-page";
import { pageMeta } from "~/meta";

const LANDING_STRUCTURED_DATA = [
  {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "Padu",
    operatingSystem: "macOS, Linux, Windows",
    applicationCategory: "DeveloperApplication",
    description:
      "Native, local-first GUI and workspace for AI coding agents including Claude Code, Codex, and OpenCode. Built in Rust with GPUI.",
    url: "https://padu.dev",
    downloadUrl: "https://padu.dev/download",
    license: "https://www.gnu.org/licenses/gpl-3.0.html",
    screenshot: "https://padu.dev/preview-dark.webp",
    offers: {
      "@type": "Offer",
      price: "0",
      priceCurrency: "USD",
    },
    author: {
      "@type": "Organization",
      name: "Padu",
      url: "https://padu.dev",
    },
  },
  {
    "@context": "https://schema.org",
    "@type": "WebSite",
    name: "Padu",
    url: "https://padu.dev",
    description: "Native, local-first desktop and web workspace for AI coding agents.",
  },
  {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    mainEntity: [
      {
        "@type": "Question",
        name: "What is Padu?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "Padu is a high-performance, native desktop and web workspace for orchestrating local AI coding agents. Built in Rust with GPUI (the GPU-accelerated UI engine behind Zed), Padu keeps all your projects, sessions, transcripts, and credentials strictly on your machine.",
        },
      },
      {
        "@type": "Question",
        name: "Is Padu free and open source?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "Yes. Padu is 100% free and open source, licensed under the GNU General Public License v3.0 (GPL-3.0). You bring your own API credentials or subscriptions for the agent providers you choose to run.",
        },
      },
      {
        "@type": "Question",
        name: "Does my code or data leave my machine?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "No. Padu is strictly local-first. It never transmits your source code, prompts, files, or agent transcripts to external servers, and includes zero telemetry or analytics tracking. Agents communicate directly with their provider APIs using the credentials on your computer.",
        },
      },
      {
        "@type": "Question",
        name: "What AI coding agents does Padu support?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "Padu supports leading coding agents through native direct drivers and ACP (Agent Client Protocol) integrations. Native drivers: Claude Code, OpenAI Codex CLI, OpenCode, Pi, Oh My Pi, Amp, DeepSeek, and Command Code. ACP integrations: Antigravity, Cursor CLI, Fx, Grok Build, Kimi Code, and Qoder CLI.",
        },
      },
      {
        "@type": "Question",
        name: "How does Padu integrate with coding agents?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "Padu communicates directly with your locally installed agent CLIs via native structured protocols and process lifecycles. It does not intercept tokens or alter agent behavior—it provides a unified native UI for streaming live transcripts, switching models, inspecting unified diffs, and queueing follow-up prompts.",
        },
      },
      {
        "@type": "Question",
        name: "How do Git worktrees and checkpoints work?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "When launching a task, Padu can run the agent inside an isolated Git worktree so it operates on a separate branch without modifying your active working directory. Padu also tracks turn-by-turn Git checkpoints, enabling 1-click diff reviews and exact state rewinds.",
        },
      },
      {
        "@type": "Question",
        name: "What platforms and operating systems are supported?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "Padu provides native desktop releases for macOS (Apple Silicon & Intel), Linux (Wayland & X11), and Windows (x86_64), alongside a web client and companion mobile apps.",
        },
      },
      {
        "@type": "Question",
        name: "Can I steer or queue prompts while an agent is actively working?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "Yes. Padu supports live message queueing and steering, allowing you to append new instructions, additional context, or corrections while an agent is executing a turn.",
        },
      },
      {
        "@type": "Question",
        name: "Do I need a separate account or cloud service to use Padu?",
        acceptedAnswer: {
          "@type": "Answer",
          text: "No. Padu requires no cloud accounts, logins, or remote subscription fees. The desktop app manages its local daemon automatically on loopback, and you can run the daemon headless on remote servers or cloud VMs using the CLI.",
        },
      },
    ],
  },
];

export const Route = createFileRoute("/")({
  head: () =>
    pageMeta(
      "Padu – Native, Local-First Desktop & Web Workspace for AI Coding Agents",
      "Native, local-first GUI for Claude Code, Codex, OpenCode, and AI coding agents. Built in Rust with GPUI. Git worktree isolation and zero cloud telemetry.",
      "/",
      {
        keywords: [
          "ai coding agents",
          "claude code gui",
          "codex desktop app",
          "opencode client",
          "antigravity ai",
          "local first developer tools",
          "gpui rust",
          "git worktrees",
          "open source ai workspace",
          "cursor cli",
          "ai pair programming",
        ],
        structuredData: LANDING_STRUCTURED_DATA,
      },
    ),
  component: Home,
});

function Home() {
  return (
    <LandingPage
      title={
        <>
          The native workspace
          <br />
          for AI coding agents
        </>
      }
      subtitle={
        <>
          A GPU-accelerated client for Claude Code, Codex, and local agent CLIs.
          <br className="hidden sm:inline" />
          {" "}Built in Rust with GPUI, Git worktree isolation, and zero cloud telemetry.
        </>
      }
    />
  );
}
