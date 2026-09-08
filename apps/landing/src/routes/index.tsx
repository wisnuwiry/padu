import { createFileRoute } from "@tanstack/react-router";
import { LandingPage } from "~/components/landing-page";
import { pageMeta } from "~/meta";

export const Route = createFileRoute("/")({
  head: () =>
    pageMeta(
      "Padu – Native, Local-First Desktop & Web Workspace for AI Coding Agents",
      "High-performance, local-first GUI for Claude Code, Codex, OpenCode, Antigravity, Amp, and Cursor. Built in Rust with GPUI. Multi-agent orchestration, Git worktrees, and instant checkpoint rewind.",
      "/",
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
