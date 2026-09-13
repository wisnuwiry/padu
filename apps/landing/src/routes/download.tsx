import type { ReactNode } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { changelogLink } from "~/changelog";
import { CodeBlock } from "~/components/code-block";
import { SiteShell } from "~/components/site-shell";
import { pageMeta } from "~/meta";
import {
  downloadUrls,
  trackDownload,
  webAppUrl,
  AppleIcon,
  AndroidIcon,
  WindowsIcon,
  LinuxIcon,
  TerminalIcon,
  GlobeIcon,
} from "~/downloads";
import { useBetaRelease, useRelease } from "~/routes/__root";
import { Monitor, Smartphone, Globe, Terminal } from "lucide-react";
import "~/styles.css";

interface DownloadSearch {
  channel?: "beta";
}

const STABLE_SEARCH: DownloadSearch = {};
const BETA_SEARCH: DownloadSearch = { channel: "beta" };

export const Route = createFileRoute("/download")({
  validateSearch: (search: Record<string, unknown>): DownloadSearch =>
    search.channel === "beta" ? { channel: "beta" } : {},
  head: () =>
    pageMeta(
      "Download Padu for macOS, Windows, Linux, Web, and Mobile",
      "Install Padu on every platform. Native desktop apps for macOS, Windows, and Linux. Remote CLI daemon for headless servers. Open source and free.",
      "/download",
    ),
  component: Download,
});

function Download() {
  const stable = useRelease();
  const beta = useBetaRelease();
  const { channel } = Route.useSearch();

  const activeBeta = channel === "beta" ? beta : null;
  const onBeta = activeBeta !== null;
  const release = activeBeta ?? stable;
  const { version } = release;
  const urls = downloadUrls(release);

  return (
    <SiteShell width="default">
      <div className="mb-10 flex flex-wrap items-start justify-between gap-x-6 gap-y-4">
        <div>
          <h1 className="text-3xl md:text-4xl font-medium tracking-tight mb-2 text-white">
            Download Padu
          </h1>
          <p className="text-white/60">
            v{version}
            <span className="mx-2 text-white/30">·</span>
            <Link
              {...changelogLink(version)}
              className="underline underline-offset-4 decoration-white/20 hover:text-white hover:decoration-current transition-colors"
            >
              What&apos;s new
            </Link>
          </p>
        </div>
        {beta && <ChannelSwitch onBeta={onBeta} />}
      </div>

      {onBeta && <BetaNotice />}

      {/* Desktop */}
      <section className="rounded-xl border border-white/10 bg-white/[0.02] p-6 md:p-8 mb-6">
        <div className="flex items-start justify-between mb-8">
          <div>
            <h2 className="text-2xl font-medium text-white">Desktop</h2>
            <p className="text-sm text-white/60 mt-1">
              Recommended native build with built-in loopback daemon and GPU acceleration
            </p>
          </div>
          <Monitor className="h-5 w-5 text-white/40 mt-1.5" strokeWidth={1.5} />
        </div>

        <div className="divide-y divide-white/10">
          <PlatformRow icon={AppleIcon} label="macOS">
            <PillGroup>
              <DownloadPill href={urls.macDmg} label="Download DMG" platform="macos" />
            </PillGroup>
          </PlatformRow>

          <PlatformRow icon={WindowsIcon} label="Windows">
            <PillGroup>
              <DownloadPill
                href={urls.windowsExeX64}
                label={urls.windowsExeArm64 ? "Intel / x64 (.exe)" : "Download (.exe)"}
                platform="windows-x64"
              />
              {urls.windowsExeArm64 && (
                <DownloadPill href={urls.windowsExeArm64} label="ARM64 (.exe)" platform="windows-arm64" />
              )}
            </PillGroup>
          </PlatformRow>

          <PlatformRow icon={LinuxIcon} label="Linux">
            <div className="flex flex-col gap-3 sm:items-end">
              <CodeBlock size="sm">curl -fsSL https://padu.dev/install.sh | sh</CodeBlock>
              <PillGroup>
                <DownloadPill href={urls.linuxTarballX64} label="x86_64 (.tar.gz)" platform="linux-x64" />
                {urls.linuxTarballArm64 && (
                  <DownloadPill href={urls.linuxTarballArm64} label="ARM64 (.tar.gz)" platform="linux-arm64" />
                )}
              </PillGroup>
            </div>
          </PlatformRow>
        </div>
      </section>

      {/* Server / Remote CLI */}
      {/*<section className="rounded-xl border border-white/10 bg-white/[0.02] p-6 md:p-8 mb-6">
        <div className="flex items-start justify-between mb-8">
          <div>
            <h2 className="text-2xl font-medium text-white">Remote Daemon / CLI</h2>
            <p className="text-sm text-white/60 mt-1">
              Run the headless daemon on remote devboxes or cloud VMs and connect from any client
            </p>
          </div>
          <Terminal className="h-5 w-5 text-white/40 mt-1.5" strokeWidth={1.5} />
        </div>

        <div className="divide-y divide-white/10">
          <PlatformRow icon={TerminalIcon} label="npm">
            <CodeBlock size="sm">
              {onBeta
                ? "npm install -g @padu/cli@beta && padu"
                : "npm install -g @padu/cli && padu"}
            </CodeBlock>
          </PlatformRow>
        </div>
      </section>*/}

      {/* Web */}
      {!onBeta && (
        <section className="rounded-xl border border-white/10 bg-white/[0.02] p-6 md:p-8 mb-6">
          <div className="flex items-start justify-between mb-8">
            <div>
              <h2 className="text-2xl font-medium text-white">Web Client</h2>
              <p className="text-sm text-white/60 mt-1">
                Connect to a running Padu daemon from any modern web browser
              </p>
            </div>
            <Globe className="h-5 w-5 text-white/40 mt-1.5" strokeWidth={1.5} />
          </div>

          <div className="divide-y divide-white/10">
            <PlatformRow icon={GlobeIcon} label="Web App">
              <PillGroup>
                <DownloadPill href={webAppUrl} label="Open Web App" external platform="web" />
              </PillGroup>
            </PlatformRow>
          </div>
        </section>
      )}

      {/* Mobile */}
      <section className="rounded-xl border border-white/10 bg-white/[0.02] p-6 md:p-8 mb-6">
        <div className="flex items-center justify-between mb-8">
          <div>
            <h2 className="text-2xl font-medium text-white">Mobile Companion</h2>
            <p className="text-sm text-white/60 mt-1">
              Drive your agents and review diffs from mobile devices
            </p>
          </div>
          <Smartphone className="h-5 w-5 text-white/40" strokeWidth={1.5} />
        </div>

        <div className="divide-y divide-white/10">
          <PlatformRow icon={AndroidIcon} label="Android">
            <PillGroup>
              {/*<DownloadPill href={urls.androidApk} label="Download APK" />*/}
              <span className="text-xs text-white/50 px-3 py-1.5 rounded-md bg-white/5 border border-white/10">
                Coming Soon
              </span>
            </PillGroup>
          </PlatformRow>

          {!onBeta && (
            <PlatformRow icon={AppleIcon} label="iOS">
              <PillGroup>
                <span className="text-xs text-white/50 px-3 py-1.5 rounded-md bg-white/5 border border-white/10">
                  Coming Soon
                </span>
              </PillGroup>
            </PlatformRow>
          )}
        </div>
      </section>

      <p className="text-center text-xs text-white/40 mt-8">
        All release packages, checksums, and source archives are available on{" "}
        <a
          href="https://github.com/wisnuwiry/padu/releases"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-white transition-colors"
        >
          GitHub Releases
        </a>
        .
      </p>
    </SiteShell>
  );
}

function ChannelSwitch({ onBeta }: { onBeta: boolean }) {
  return (
    <div
      aria-label="Release channel"
      className="inline-flex items-center gap-1 rounded-full border border-white/10 bg-white/[0.03] p-1"
    >
      <ChannelOption label="Stable" active={!onBeta} search={STABLE_SEARCH} />
      <ChannelOption label="Beta" active={onBeta} search={BETA_SEARCH} />
    </div>
  );
}

function ChannelOption({
  label,
  active,
  search,
}: {
  label: string;
  active: boolean;
  search: DownloadSearch;
}) {
  return (
    <Link
      to="/download"
      search={search}
      replace
      resetScroll={false}
      aria-current={active ? "true" : undefined}
      className={`rounded-full px-4 py-1.5 text-sm font-medium transition-colors ${
        active
          ? "bg-foreground text-background"
          : "text-white/60 hover:bg-white/5 hover:text-white"
      }`}
    >
      {label}
    </Link>
  );
}

function BetaNotice() {
  return (
    <div className="mb-6 rounded-xl border border-purple-500/20 bg-purple-500/5 p-5 md:px-8 md:py-6">
      <p className="text-sm text-purple-300 font-medium">Beta Channel</p>
      <p className="mt-1 text-sm text-white/60">
        Beta builds ship ahead of stable and may include experimental features. Already running the
        desktop app? Set <span className="text-white">Settings → Release channel → Beta</span> to
        auto-update.
      </p>
    </div>
  );
}

function PlatformRow({
  icon: Icon,
  label,
  children,
}: {
  icon: (props: React.SVGProps<SVGSVGElement>) => ReactNode;
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-3 py-5 first:pt-0 last:pb-0 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex items-center gap-3">
        <Icon className="h-5 w-5 text-white/80" />
        <span className="font-medium text-white/90">{label}</span>
      </div>
      {children}
    </div>
  );
}

function PillGroup({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-2">{children}</div>;
}

function DownloadPill({
  href,
  label,
  external,
  platform,
}: {
  href: string;
  label: string;
  external?: boolean;
  platform?: string;
}) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      onClick={() => platform && trackDownload(platform)}
      className="inline-flex items-center justify-center rounded-full bg-foreground px-4 py-1.5 text-sm font-medium text-background hover:bg-foreground/85 transition-colors"
    >
      {label}
      {external && (
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="ml-1.5 h-3 w-3"
          aria-hidden="true"
        >
          <path d="M7 17L17 7" />
          <path d="M7 7h10v10" />
        </svg>
      )}
    </a>
  );
}
