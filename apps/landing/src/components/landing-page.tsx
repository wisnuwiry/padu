import * as React from "react";
import {
  ArrowRight,
  Check,
  ChevronLeft,
  ChevronRight,
  Cpu,
  GitFork,
  Globe,
  RotateCcw,
  ShieldCheck,
  Smartphone,
  Tablet,
} from "lucide-react";
import {
  motion,
  AnimatePresence,
  type Transition,
  type Variants,
} from "framer-motion";

// Apple-style quintic easing curves and refined motion parameters
const APPLE_SMOOTH = [0.19, 1, 0.22, 1] as const;
const TAB_SPRING: Transition = { type: "spring", stiffness: 420, damping: 32 };
const SLIDE_TRANSITION: Transition = { duration: 0.4, ease: APPLE_SMOOTH };

const VIEWPORT_CONFIG = { once: true, margin: "-50px" };

const HERO_CONTAINER_VARIANTS: Variants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: {
      staggerChildren: 0.1,
      delayChildren: 0.06,
    },
  },
};

const HERO_ITEM_VARIANTS: Variants = {
  hidden: { opacity: 0, y: 12, filter: "blur(3px)" },
  visible: {
    opacity: 1,
    y: 0,
    filter: "blur(0px)",
    transition: { duration: 0.75, ease: APPLE_SMOOTH },
  },
};

const SECTION_CONTAINER_VARIANTS: Variants = {
  hidden: { opacity: 0, y: 18, filter: "blur(3px)" },
  visible: {
    opacity: 1,
    y: 0,
    filter: "blur(0px)",
    transition: { duration: 0.7, ease: APPLE_SMOOTH },
  },
};

const STAGGER_CONTAINER_VARIANTS: Variants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: {
      staggerChildren: 0.08,
      delayChildren: 0.05,
    },
  },
};

const CARD_ITEM_VARIANTS: Variants = {
  hidden: { opacity: 0, y: 14, filter: "blur(2px)" },
  visible: {
    opacity: 1,
    y: 0,
    filter: "blur(0px)",
    transition: { duration: 0.6, ease: APPLE_SMOOTH },
  },
};

import { CursorFieldProvider } from "~/components/butterfly";
import { AGENT_PAGES } from "~/data/agent-pages";
import {
  getDownloadOptions,
  trackDownload,
  useDetectedPlatform,
} from "~/downloads";
import { useRelease } from "~/routes/__root";
import { HeroMockup } from "~/components/hero-mockup";
import {
  AntigravityIcon,
  ClaudeCodeIcon,
  CodexIcon,
  CursorIcon,
  OpenCodeIcon,
} from "~/components/agent-icons";
import { ClaudeIcon } from "~/components/mockup";
import { FAQItem } from "~/components/faq-item";
import { SiteFooter } from "~/components/site-footer";
import { SiteHeader } from "~/components/site-header";
import "~/styles.css";

interface LandingPageProps {
  eyebrow?: React.ReactNode;
  title: React.ReactNode;
  subtitle: React.ReactNode;
}

export function LandingPage({ eyebrow, title, subtitle }: LandingPageProps) {
  return (
    <CursorFieldProvider>
      <div className="relative bg-background">
        {/* Hero header & content */}
        <div className="relative px-6 pt-4 pb-10 md:px-32 md:pt-6 md:pb-12 max-w-7xl mx-auto">
          <Nav />
          <motion.div
            variants={HERO_CONTAINER_VARIANTS}
            initial="hidden"
            animate="visible"
            className="flex flex-col items-center transform-gpu"
          >
            <Hero eyebrow={eyebrow} title={title} subtitle={subtitle} />
            <GetStarted />
          </motion.div>
        </div>

        {/* Mockup Frame */}
        <motion.div
          initial={{ opacity: 0, y: 24, scale: 0.985, filter: "blur(4px)" }}
          animate={{ opacity: 1, y: 0, scale: 1, filter: "blur(0px)" }}
          transition={{ duration: 0.9, delay: 0.25, ease: APPLE_SMOOTH }}
          className="relative px-6 md:px-8 pt-4 md:pt-8 pb-12 md:pb-20 transform-gpu"
        >
          <div className="max-w-7xl mx-auto">
            <HeroMockup />
          </div>
        </motion.div>
      </div>

      {/* Content section */}
      <div className="landing-content bg-background border-t border-white/[0.06]">
        <main className="p-6 md:p-20 md:pt-32 max-w-5xl mx-auto">
          <div className="space-y-32">
            <ArchitectureCarousel />
            <EcosystemBentoSection />
            <MultiProviderSection />
            <FAQ />
          </div>
        </main>
        <SiteFooter />
      </div>
    </CursorFieldProvider>
  );
}

function Nav() {
  return (
    <nav className="mb-10 sm:mb-14 md:mb-20">
      <SiteHeader />
    </nav>
  );
}

function Hero({
  eyebrow,
  title,
  subtitle,
}: {
  eyebrow?: React.ReactNode;
  title: React.ReactNode;
  subtitle: React.ReactNode;
}) {
  return (
    <div className="space-y-4 text-center max-w-2xl mx-auto">
      {eyebrow && (
        <motion.p
          variants={HERO_ITEM_VARIANTS}
          className="font-mono text-xs font-medium tracking-wider uppercase text-zinc-400"
        >
          {eyebrow}
        </motion.p>
      )}

      <motion.h1
        variants={HERO_ITEM_VARIANTS}
        className="text-3xl sm:text-5xl md:text-6xl font-semibold tracking-tight leading-[1.08] text-white"
      >
        {title}
      </motion.h1>
      <motion.p
        variants={HERO_ITEM_VARIANTS}
        className="text-sm sm:text-base md:text-lg leading-relaxed text-zinc-400 max-w-xl mx-auto font-normal"
      >
        {subtitle}
      </motion.p>
    </div>
  );
}

const FEATURED_AGENT_COUNT = 5;
const ADDITIONAL_AGENT_COUNT = AGENT_PAGES.length - FEATURED_AGENT_COUNT;

function SectionHeader({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string;
  title: string;
  description: string;
}) {
  return (
    <div className="mb-10 space-y-2.5">
      <p className="font-mono text-xs font-medium tracking-wider uppercase text-zinc-400">
        {eyebrow}
      </p>
      <h2 className="text-2xl sm:text-3xl font-semibold tracking-tight text-white">{title}</h2>
      <p className="text-sm sm:text-base text-zinc-400 max-w-xl leading-relaxed">{description}</p>
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Clean Gray Placeholder Component                                          */
/* -------------------------------------------------------------------------- */

function GrayPlaceholder({
  label,
  aspect = "aspect-[16/10]",
}: {
  label: string;
  aspect?: string;
}) {
  return (
    <div
      className={`w-full ${aspect} rounded-xl border border-white/10 bg-white/[0.02] flex flex-col items-center justify-center gap-2.5 p-6 text-center select-none`}
    >
      <div className="w-9 h-9 rounded-lg border border-white/[0.08] bg-white/[0.03] flex items-center justify-center text-zinc-500">
        <svg
          className="w-4.5 h-4.5 text-zinc-600"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        >
          <rect width="18" height="18" x="3" y="3" rx="2" ry="2" />
          <circle cx="9" cy="9" r="2" />
          <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" />
        </svg>
      </div>
      <span className="font-mono text-xs text-zinc-500">{label}</span>
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Architecture Tab Carousel (Apple Style)                                    */
/* -------------------------------------------------------------------------- */

const ARCHITECTURE_TABS = [
  {
    id: "gpui",
    name: "GPUI Engine",
    tag: "gpui / rust",
    icon: Cpu,
    title: "GPU-accelerated native rendering",
    description:
      "Built on GPUI—the high-performance GPU UI framework developed for Zed. No Electron, no DOM overhead, and direct rendering to Metal, Vulkan, and DirectX at native refresh rates.",
    points: [
      "Sub-millisecond turn updates under massive streaming token volume",
      "Direct Metal/Vulkan draw calls without browser engine reflow",
      "Virtualized list rendering handling 100k+ line session transcripts",
    ],
    docHref: "/docs",
    docLabel: "Read architecture docs",
    preview: <GrayPlaceholder label="GPUI Rendering Architecture" />,
  },
  {
    id: "worktrees",
    name: "Git Worktrees",
    tag: "git worktree",
    icon: GitFork,
    title: "Parallel worktree branch isolation",
    description:
      "Spawn concurrent agent sessions in dedicated worktree directories. Agents write code, execute commands, and run tests on independent branches without mutating your active staging or unstaged files.",
    points: [
      "Launch multiple agent tasks in parallel on separate branches",
      "Active working tree stays clean and untouched",
      "Automated lifecycle cleanup when tasks settle",
    ],
    docHref: "/docs/worktrees",
    docLabel: "Read worktrees docs",
    preview: <GrayPlaceholder label="Git Worktree Isolation Diagram" />,
  },
  {
    id: "checkpoints",
    name: "Checkpoints",
    tag: "checkpoints / time-travel",
    icon: RotateCcw,
    title: "Turn-by-turn checkpoint rewind",
    description:
      "Padu captures automated Git snapshots before and after every turn. Review structured file diffs in real time, or rewind code, prompts, and conversation state back to any historical point.",
    points: [
      "Automated Git commit snapshot on every agent turn",
      "Unified diff review with side-by-side hunk inspection",
      "1-click rollback of code and conversation context",
    ],
    preview: <GrayPlaceholder label="Turn Checkpoints & Rewind Timeline" />,
  },
  {
    id: "daemon",
    name: "Local Daemon",
    tag: "local-first / stdio",
    icon: ShieldCheck,
    title: "Supervised local processes & zero cloud intermediary",
    description:
      "The lightweight background daemon manages agent CLIs via stdio/PTY and structured RPC. Credentials stay in your local OS keychain, and source code never leaves your computer.",
    points: [
      "Loopback RPC communication over local Unix sockets/TCP",
      "100% local data storage with zero analytics or telemetry",
      "Run headless on remote devboxes and connect over private network",
    ],
    docHref: "/docs/cli",
    docLabel: "Read CLI docs",
    preview: <GrayPlaceholder label="Local Daemon Supervision Architecture" />,
  },
];

function ArchitectureCarousel() {
  const [activeTab, setActiveTab] = React.useState(0);
  const current = ARCHITECTURE_TABS[activeTab];

  const handlePrev = React.useCallback(() => {
    setActiveTab((i) => (i === 0 ? ARCHITECTURE_TABS.length - 1 : i - 1));
  }, []);

  const handleNext = React.useCallback(() => {
    setActiveTab((i) => (i === ARCHITECTURE_TABS.length - 1 ? 0 : i + 1));
  }, []);

  return (
    <motion.section
      variants={SECTION_CONTAINER_VARIANTS}
      initial="hidden"
      whileInView="visible"
      viewport={VIEWPORT_CONFIG}
      className="transform-gpu"
    >
      <SectionHeader
        eyebrow="Architecture"
        title="A native runtime built for developer control."
        description="Padu runs directly on your hardware. The desktop client renders natively with GPUI, communicating with a lightweight background daemon that supervises your local agent CLIs."
      />

      {/* Apple-style Tab Bar */}
      <div className="mb-6 flex flex-wrap items-center gap-1 sm:gap-2 bg-white/[0.03] p-1.5 rounded-2xl border border-white/[0.08] backdrop-blur-xl w-full">
        {ARCHITECTURE_TABS.map((tab, idx) => {
          const selected = activeTab === idx;
          const Icon = tab.icon;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(idx)}
              aria-selected={selected}
              role="tab"
              className={`relative flex-1 min-w-[120px] cursor-pointer rounded-xl px-3.5 py-2.5 text-xs sm:text-sm font-medium transition-all flex items-center justify-center gap-2 ${
                selected ? "text-white" : "text-zinc-400 hover:text-white hover:bg-white/[0.02]"
              }`}
            >
              {selected && (
                <motion.span
                  layoutId="arch-tab-pill"
                  transition={TAB_SPRING}
                  className="absolute inset-0 rounded-xl bg-white/10 border border-white/15 shadow-sm"
                />
              )}
              <Icon className="h-4 w-4 shrink-0 relative z-10" />
              <span className="relative z-10 truncate">{tab.name}</span>
            </button>
          );
        })}
      </div>

      {/* Active Tab Slide Content Container */}
      <div className="rounded-2xl border border-white/10 bg-white/[0.02] p-6 sm:p-8 md:p-10 relative overflow-hidden min-h-[380px]">
        <AnimatePresence mode="wait">
          <motion.div
            key={current.id}
            initial={{ opacity: 0, y: 8, filter: "blur(4px)" }}
            animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            exit={{ opacity: 0, y: -8, filter: "blur(4px)" }}
            transition={SLIDE_TRANSITION}
            className="grid grid-cols-1 md:grid-cols-12 gap-8 items-center transform-gpu"
          >
            {/* Left explanation side */}
            <div className="md:col-span-6 space-y-4">
              <div className="flex items-center gap-2">
                <span className="font-mono text-xs text-zinc-400 bg-white/[0.04] border border-white/[0.08] px-2.5 py-0.5 rounded-md">
                  {current.tag}
                </span>
              </div>
              <h3 className="text-xl sm:text-2xl font-semibold tracking-tight text-white">
                {current.title}
              </h3>
              <p className="text-sm text-zinc-400 leading-relaxed">
                {current.description}
              </p>

              <ul className="space-y-2 pt-2 text-xs sm:text-sm text-zinc-300">
                {current.points.map((point) => (
                  <li key={point} className="flex items-start gap-2">
                    <Check className="h-4 w-4 text-emerald-400 shrink-0 mt-0.5" />
                    <span>{point}</span>
                  </li>
                ))}
              </ul>

              {current.docHref && (
                <div className="pt-3">
                  <a
                    href={current.docHref}
                    className="inline-flex items-center gap-1 text-xs font-medium text-zinc-300 hover:text-white transition-colors"
                  >
                    {current.docLabel ?? "Read documentation"} <ArrowRight className="h-3.5 w-3.5" />
                  </a>
                </div>
              )}
            </div>

            {/* Right placeholder preview */}
            <div className="md:col-span-6 w-full">
              {current.preview}
            </div>
          </motion.div>
        </AnimatePresence>
      </div>

      {/* Carousel controls located underneath the content */}
      <div className="mt-5 flex items-center justify-between px-2">
        <div className="flex items-center gap-2">
          {ARCHITECTURE_TABS.map((tab, idx) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(idx)}
              aria-label={`Go to ${tab.name}`}
              className={`h-1.5 rounded-full transition-all cursor-pointer ${
                activeTab === idx ? "w-8 bg-white" : "w-2 bg-white/20 hover:bg-white/40"
              }`}
            />
          ))}
        </div>

        <div className="flex items-center gap-3">
          <span className="font-mono text-xs text-zinc-500">
            0{activeTab + 1} / 0{ARCHITECTURE_TABS.length}
          </span>
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              onClick={handlePrev}
              aria-label="Previous architecture feature"
              className="w-8 h-8 rounded-full border border-white/10 bg-white/[0.03] flex items-center justify-center text-zinc-400 hover:text-white hover:border-white/20 active:scale-95 transition-all cursor-pointer"
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={handleNext}
              aria-label="Next architecture feature"
              className="w-8 h-8 rounded-full border border-white/10 bg-white/[0.03] flex items-center justify-center text-zinc-400 hover:text-white hover:border-white/20 active:scale-95 transition-all cursor-pointer"
            >
              <ChevronRight className="h-4 w-4" />
            </button>
          </div>
        </div>
      </div>
    </motion.section>
  );
}

/* -------------------------------------------------------------------------- */
/* Ecosystem Bento Section with Staggered Entrance Animations                */
/* -------------------------------------------------------------------------- */

function EcosystemBentoSection() {
  return (
    <motion.section
      variants={SECTION_CONTAINER_VARIANTS}
      initial="hidden"
      whileInView="visible"
      viewport={VIEWPORT_CONFIG}
      className="transform-gpu"
    >
      <SectionHeader
        eyebrow="Ecosystem"
        title="Every device. Every screen."
        description="Connect seamlessly to your active workspace from your iPad, iPhone, Android, or modern web browser with zero feature loss."
      />

      <motion.div
        variants={STAGGER_CONTAINER_VARIANTS}
        className="grid grid-cols-1 md:grid-cols-3 gap-4"
      >
        {/* iPad / Tablet - 2 columns */}
        <motion.div
          variants={CARD_ITEM_VARIANTS}
          whileHover={{ y: -3, transition: { duration: 0.25, ease: APPLE_SMOOTH } }}
          className="md:col-span-2 rounded-2xl border border-white/10 bg-white/[0.02] p-6 sm:p-7 flex flex-col justify-between hover:border-white/20 transition-colors transform-gpu"
        >
          <div className="space-y-2.5 mb-5">
            <div className="flex items-center justify-between">
              <span className="font-mono text-xs text-zinc-400 bg-white/[0.04] border border-white/[0.08] px-2.5 py-0.5 rounded-md">
                iPad &amp; Tablets
              </span>
              <Tablet className="h-4 w-4 text-zinc-400" strokeWidth={1.5} />
            </div>
            <h3 className="text-lg font-semibold text-white tracking-tight">
              Split-screen workspace &amp; diff review
            </h3>
            <p className="text-sm text-zinc-400 leading-relaxed max-w-lg">
              Expansive multi-column view with live transcript streaming, file tree navigation, and
              touch-optimized side-by-side diff inspection.
            </p>
          </div>
          <GrayPlaceholder label="iPad & Tablet Split-screen Preview" aspect="aspect-[16/9]" />
        </motion.div>

        {/* iPhone (iOS) - 1 column */}
        <motion.div
          variants={CARD_ITEM_VARIANTS}
          whileHover={{ y: -3, transition: { duration: 0.25, ease: APPLE_SMOOTH } }}
          className="md:col-span-1 rounded-2xl border border-white/10 bg-white/[0.02] p-6 sm:p-7 flex flex-col justify-between hover:border-white/20 transition-colors transform-gpu"
        >
          <div className="space-y-2.5 mb-5">
            <div className="flex items-center justify-between">
              <span className="font-mono text-xs text-zinc-400 bg-white/[0.04] border border-white/[0.08] px-2.5 py-0.5 rounded-md">
                iPhone &amp; iOS
              </span>
              <Smartphone className="h-4 w-4 text-zinc-400" strokeWidth={1.5} />
            </div>
            <h3 className="text-lg font-semibold text-white tracking-tight">
              Prompt steering in your pocket
            </h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Steer running agents, queue follow-ups, and review changes on the go.
            </p>
          </div>
          <GrayPlaceholder label="iPhone Companion App Preview" aspect="aspect-[16/10] sm:aspect-[9/12]" />
        </motion.div>

        {/* Android - 1 column */}
        <motion.div
          variants={CARD_ITEM_VARIANTS}
          whileHover={{ y: -3, transition: { duration: 0.25, ease: APPLE_SMOOTH } }}
          className="md:col-span-1 rounded-2xl border border-white/10 bg-white/[0.02] p-6 sm:p-7 flex flex-col justify-between hover:border-white/20 transition-colors transform-gpu"
        >
          <div className="space-y-2.5 mb-5">
            <div className="flex items-center justify-between">
              <span className="font-mono text-xs text-zinc-400 bg-white/[0.04] border border-white/[0.08] px-2.5 py-0.5 rounded-md">
                Android
              </span>
              <Smartphone className="h-4 w-4 text-zinc-400" strokeWidth={1.5} />
            </div>
            <h3 className="text-lg font-semibold text-white tracking-tight">
              Native companion app
            </h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Direct daemon connectivity over local Wi-Fi, VPN, or private tailnets.
            </p>
          </div>
          <GrayPlaceholder label="Android Companion App Preview" aspect="aspect-[16/10] sm:aspect-[9/12]" />
        </motion.div>

        {/* PWA & Web - 2 columns */}
        <motion.div
          variants={CARD_ITEM_VARIANTS}
          whileHover={{ y: -3, transition: { duration: 0.25, ease: APPLE_SMOOTH } }}
          className="md:col-span-2 rounded-2xl border border-white/10 bg-white/[0.02] p-6 sm:p-7 flex flex-col justify-between hover:border-white/20 transition-colors transform-gpu"
        >
          <div className="space-y-2.5 mb-5">
            <div className="flex items-center justify-between">
              <span className="font-mono text-xs text-zinc-400 bg-white/[0.04] border border-white/[0.08] px-2.5 py-0.5 rounded-md">
                Web &amp; PWA
              </span>
              <Globe className="h-4 w-4 text-zinc-400" strokeWidth={1.5} />
            </div>
            <h3 className="text-lg font-semibold text-white tracking-tight">
              Progressive Web App at app.padu.dev
            </h3>
            <p className="text-sm text-zinc-400 leading-relaxed max-w-lg">
              Zero install required. Connect to any local or remote daemon from any modern browser
              with full real-time WebSocket communication.
            </p>
          </div>
          <GrayPlaceholder label="Web App & PWA Interface Preview" aspect="aspect-[16/9]" />
        </motion.div>
      </motion.div>
    </motion.section>
  );
}

function MultiProviderSection() {
  const providers = [
    { name: "Claude Code", icon: <ClaudeIcon size={24} />, slug: "claude-code" },
    { name: "OpenAI Codex", icon: <CodexIcon className="w-6 h-6" />, slug: "codex" },
    { name: "OpenCode", icon: <OpenCodeIcon className="w-6 h-6" />, slug: "opencode" },
    { name: "Antigravity", icon: <AntigravityIcon className="w-6 h-6" />, slug: "antigravity" },
    { name: "Cursor CLI", icon: <CursorIcon className="w-6 h-6" />, slug: "cursor" },
  ];

  return (
    <motion.section
      variants={SECTION_CONTAINER_VARIANTS}
      initial="hidden"
      whileInView="visible"
      viewport={VIEWPORT_CONFIG}
      className="transform-gpu"
    >
      <SectionHeader
        eyebrow="Integrations"
        title="Direct drivers for your local agents."
        description="Padu communicates with your locally installed CLIs via native structured protocols. Switch between agents while preserving workspace state."
      />

      <motion.div
        variants={STAGGER_CONTAINER_VARIANTS}
        className="grid grid-cols-2 sm:grid-cols-3 gap-3.5"
      >
        {providers.map((p) => (
          <motion.a
            key={p.name}
            href={`/${p.slug}`}
            variants={CARD_ITEM_VARIANTS}
            whileHover={{ y: -3, transition: { duration: 0.25, ease: APPLE_SMOOTH } }}
            className="flex items-center gap-3.5 rounded-2xl border border-white/10 bg-white/[0.02] p-4.5 hover:border-white/20 hover:bg-white/[0.04] transition-colors group transform-gpu"
          >
            <span className="text-white/80 group-hover:text-white transition-colors">{p.icon}</span>
            <span className="font-medium text-sm text-white/90 group-hover:text-white transition-colors">
              {p.name}
            </span>
          </motion.a>
        ))}
        <motion.a
          href="/agents"
          variants={CARD_ITEM_VARIANTS}
          whileHover={{ y: -3, transition: { duration: 0.25, ease: APPLE_SMOOTH } }}
          className="flex items-center justify-center gap-2 rounded-2xl border border-dashed border-white/15 bg-white/[0.01] p-4.5 text-zinc-400 hover:text-white hover:border-white/30 hover:bg-white/[0.03] transition-colors transform-gpu"
        >
          <span className="font-medium text-sm">+{ADDITIONAL_AGENT_COUNT} more agents</span>
          <ArrowRight className="h-3.5 w-3.5" />
        </motion.a>
      </motion.div>
    </motion.section>
  );
}

function GetStarted() {
  return (
    <motion.div variants={HERO_ITEM_VARIANTS} className="pt-8 w-full">
      <div className="flex flex-row flex-wrap justify-center gap-3">
        <DownloadButton />
        <OtherPlatformsButton />
      </div>
      <div className="flex flex-wrap items-center justify-center gap-x-5 gap-y-2 pt-6 text-xs text-zinc-400">
        <span className="font-mono text-zinc-500 text-[11px]">Supports</span>
        <a
          href="/claude-code"
          className="flex items-center gap-1.5 hover:text-white transition-colors"
        >
          <ClaudeCodeIcon className="h-4 w-4" />
          <span>Claude Code</span>
        </a>
        <a
          href="/codex"
          className="flex items-center gap-1.5 hover:text-white transition-colors"
        >
          <CodexIcon className="h-4 w-4" />
          <span>Codex</span>
        </a>
        <a
          href="/opencode"
          className="flex items-center gap-1.5 hover:text-white transition-colors"
        >
          <OpenCodeIcon className="h-4 w-4" />
          <span>OpenCode</span>
        </a>
        <a
          href="/antigravity"
          className="flex items-center gap-1.5 hover:text-white transition-colors"
        >
          <AntigravityIcon className="h-4 w-4" />
          <span>Antigravity</span>
        </a>
        <a
          href="/cursor"
          className="flex items-center gap-1.5 hover:text-white transition-colors"
        >
          <CursorIcon className="h-4 w-4" />
          <span>Cursor</span>
        </a>
        <a
          href="/agents"
          className="font-mono text-[11px] text-zinc-500 hover:text-zinc-300 transition-colors"
        >
          +{ADDITIONAL_AGENT_COUNT} more →
        </a>
      </div>
    </motion.div>
  );
}

function DownloadButton() {
  const release = useRelease();
  const detectedPlatform = useDetectedPlatform();
  const primary = getDownloadOptions(release).find((o) => o.platform === detectedPlatform)!;
  const PrimaryIcon = primary.icon;

  return (
    <a
      href={primary.href}
      target="_blank"
      rel="noopener noreferrer"
      onClick={() => trackDownload(primary.platform)}
      className="inline-flex items-center gap-2 rounded-lg bg-foreground px-5 py-2.5 text-sm font-medium text-background hover:bg-foreground/90 active:scale-[0.98] transition-all"
    >
      <PrimaryIcon className="h-4 w-4" />
      Download for {primary.label}
    </a>
  );
}

function OtherPlatformsButton() {
  return (
    <a
      href="/download"
      className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.03] px-4.5 py-2.5 text-sm font-medium text-white hover:bg-white/[0.08] active:scale-[0.98] transition-all"
    >
      Other platforms
    </a>
  );
}

function FAQ() {
  return (
    <motion.section
      variants={SECTION_CONTAINER_VARIANTS}
      initial="hidden"
      whileInView="visible"
      viewport={VIEWPORT_CONFIG}
      className="space-y-6 transform-gpu"
    >
      <SectionHeader
        eyebrow="FAQ"
        title="Frequently asked questions."
        description="Everything you need to know about Padu's architecture, privacy, and agent support."
      />
      <div className="divide-y divide-white/[0.08]">
        <FAQItem question="What is Padu?">
          Padu is a high-performance, native desktop and web workspace for orchestrating local AI
          coding agents. Built in Rust with GPUI (the GPU-accelerated UI engine behind Zed), Padu
          keeps all your projects, sessions, transcripts, and credentials strictly on your machine.
        </FAQItem>
        <FAQItem question="Is Padu free and open source?">
          Yes. Padu is 100% free and open source, licensed under the GNU General Public License v3.0
          (GPL-3.0). You bring your own API credentials or subscriptions for the agent providers you
          choose to run.
        </FAQItem>
        <FAQItem question="Does my code or data leave my machine?">
          No. Padu is strictly local-first. It never transmits your source code, prompts, files, or
          agent transcripts to external servers, and includes zero telemetry or analytics tracking.
          Agents communicate directly with their provider APIs using the credentials on your computer.
        </FAQItem>
        <FAQItem question="What AI coding agents does Padu support?">
          Padu supports leading coding agents with native direct drivers and ACP (Agent Client
          Protocol) integrations: Claude Code, OpenAI Codex CLI, OpenCode, Pi Agent, Amp, DeepSeek,
          Cursor CLI, Fx, Grok Build, Kimi Code, GitHub Copilot, Google Gemini CLI, Cline, Goose, and
          Mistral Vibe. See the full catalog on the{" "}
          <a href="/agents" className="underline hover:text-white transition-colors">
            supported agents page
          </a>
          .
        </FAQItem>
        <FAQItem question="How does Padu integrate with coding agents?">
          Padu communicates directly with your locally installed agent CLIs via native structured
          protocols and process lifecycles. It does not intercept tokens or alter agent
          behavior—it provides a unified native UI for streaming live transcripts, switching models,
          inspecting unified diffs, and queueing follow-up prompts.
        </FAQItem>
        <FAQItem question="How do Git worktrees and checkpoints work?">
          When launching a task, Padu can run the agent inside an isolated Git worktree so it operates
          on a separate branch without modifying your active working directory. Padu also tracks
          turn-by-turn Git checkpoints, enabling 1-click diff reviews and exact state rewinds. Read
          the{" "}
          <a href="/docs/worktrees" className="underline hover:text-white transition-colors">
            worktrees documentation
          </a>
          .
        </FAQItem>
        <FAQItem question="What platforms and operating systems are supported?">
          Padu provides native desktop releases for <strong>macOS</strong> (Apple Silicon & Intel),{" "}
          <strong>Linux</strong> (Wayland & X11), and <strong>Windows</strong> (x86_64), alongside
          a web client and companion mobile apps.
        </FAQItem>
        <FAQItem question="Can I steer or queue prompts while an agent is actively working?">
          Yes. Padu supports live message queueing and steering, allowing you to append new
          instructions, additional context, or corrections while an agent is executing a turn.
        </FAQItem>
        <FAQItem question="Do I need a separate account or cloud service to use Padu?">
          No. Padu requires no cloud accounts, logins, or remote subscription fees. The desktop app
          manages its local daemon automatically on loopback, and you can run the daemon headless
          on remote servers or cloud VMs using the CLI.
        </FAQItem>
      </div>
    </motion.section>
  );
}
