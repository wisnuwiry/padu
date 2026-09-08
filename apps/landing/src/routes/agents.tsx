import { createFileRoute, Link } from "@tanstack/react-router";
import { CursorFieldProvider } from "~/components/butterfly";
import { SiteShell } from "~/components/site-shell";
import { AGENT_PAGES } from "~/data/agent-pages";
import { pageMeta } from "~/meta";
import "~/styles.css";

export const Route = createFileRoute("/agents")({
  head: () =>
    pageMeta(
      "Supported agents – Every coding agent Padu runs",
      "Run Claude Code, Codex, OpenCode, Antigravity, Pi, Oh My Pi, Cursor, DeepSeek, and more coding agents from your phone or desktop. Self-hosted, your code stays on your machine.",
      "/agents",
    ),
  component: AgentsPage,
});

function AgentsPage() {
  const nativeAgents = AGENT_PAGES.filter((a) => a.category === "native");
  const acpAgents = AGENT_PAGES.filter((a) => a.category === "acp");

  return (
    <CursorFieldProvider>
      <SiteShell width="default">
        <header className="space-y-4 max-w-2xl mb-12">
          <h1 className="text-3xl md:text-5xl font-medium tracking-tight">
            Supported coding agents
          </h1>
          <p className="text-white/70 text-lg leading-relaxed">
            Padu detects and orchestrates your locally installed AI coding agent CLIs. Bring your
            existing subscriptions, configs, MCP servers, and skills.
          </p>
        </header>

        {/* Native Support */}
        <section className="mb-12">
          <div className="flex items-center gap-3 mb-6">
            <h2 className="text-xl font-medium text-white">Native drivers</h2>
            <span className="rounded-full bg-purple-500/10 border border-purple-500/20 px-2.5 py-0.5 text-xs text-purple-300 font-medium">
              Native
            </span>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {nativeAgents.map((agent) => (
              <Link
                key={agent.slug}
                to={`/${agent.slug}`}
                className="group flex flex-col justify-between rounded-xl border border-white/10 bg-white/[0.02] p-5 hover:border-purple-500/40 hover:bg-purple-500/[0.04] transition-colors"
              >
                <div>
                  <div className="flex items-center justify-between">
                    <h3 className="font-medium text-white group-hover:text-purple-300 transition-colors">
                      {agent.name}
                    </h3>
                    <span className="text-[10px] font-mono uppercase tracking-wider text-purple-400/80 bg-purple-400/10 px-2 py-0.5 rounded">
                      Native
                    </span>
                  </div>
                  <p className="mt-2 text-sm text-white/60 leading-relaxed">{agent.subtitle}</p>
                </div>
              </Link>
            ))}
          </div>
        </section>

        {/* ACP Support */}
        <section className="mb-12">
          <div className="flex items-center gap-3 mb-6">
            <h2 className="text-xl font-medium text-white">ACP integrations</h2>
            <span className="rounded-full bg-sky-500/10 border border-sky-500/20 px-2.5 py-0.5 text-xs text-sky-300 font-medium">
              ACP
            </span>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {acpAgents.map((agent) => (
              <Link
                key={agent.slug}
                to={`/${agent.slug}`}
                className="group flex flex-col justify-between rounded-xl border border-white/10 bg-white/[0.02] p-5 hover:border-sky-500/40 hover:bg-sky-500/[0.04] transition-colors"
              >
                <div>
                  <div className="flex items-center justify-between">
                    <h3 className="font-medium text-white group-hover:text-sky-300 transition-colors">
                      {agent.name}
                    </h3>
                    <span className="text-[10px] font-mono uppercase tracking-wider text-sky-400/80 bg-sky-400/10 px-2 py-0.5 rounded">
                      ACP
                    </span>
                  </div>
                  <p className="mt-2 text-sm text-white/60 leading-relaxed">{agent.subtitle}</p>
                </div>
              </Link>
            ))}
          </div>
        </section>
      </SiteShell>
    </CursorFieldProvider>
  );
}

