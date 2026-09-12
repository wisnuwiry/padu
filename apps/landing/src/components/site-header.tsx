import * as React from "react";
import type { MouseEvent } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import { motion, AnimatePresence } from "framer-motion";
import { Menu, X } from "lucide-react";
import "~/styles.css";
import { GitHubIcon } from "~/components/brand-icons";
import { useStars } from "~/routes/__root";
import { useRealtimeStars } from "~/realtime-stars";

export function SiteHeader() {
  const context = useStars();
  const stars = useRealtimeStars(context.stars);
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const [mobileMenuOpen, setMobileMenuOpen] = React.useState(false);

  // Auto-close mobile menu on route change
  React.useEffect(() => {
    setMobileMenuOpen(false);
  }, [pathname]);

  const toggleMobileMenu = React.useCallback(() => {
    setMobileMenuOpen((open) => !open);
  }, []);

  // Clicking a link to the current route makes the router re-run the root
  // loader, which re-invokes the release/stars server functions. On the static
  // (prerendered) host those endpoints return the SPA HTML, so the loader data
  // degrades and components that destructure `release.version` crash. Skip the
  // navigation entirely when the target route is already active.
  const skipSameRoute = React.useCallback(
    (to: string) => (event: MouseEvent<HTMLAnchorElement>) => {
      if (
        pathname === to &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        !event.shiftKey &&
        event.button === 0
      ) {
        event.preventDefault();
      }
    },
    [pathname],
  );

  return (
    <header className="relative z-50 flex items-center justify-between w-full py-1">
      {/* Brand logo & name */}
      <Link
        to="/"
        onClick={skipSameRoute("/")}
        className="flex items-center gap-2.5 group"
      >
        <div className="flex items-center justify-center w-8 h-8 rounded-xl bg-white/[0.06] border border-white/10 group-hover:border-white/20 transition-all shadow-sm">
          <img
            src="/padu.svg"
            alt="Padu"
            className="w-4.5 h-4.5 transition-transform duration-200 group-hover:scale-110"
          />
        </div>
        <span className="text-base font-semibold tracking-tight text-white">Padu</span>
      </Link>

      {/* Desktop Navigation Pill (hidden on mobile, visible on sm+) */}
      <nav
        aria-label="Main Navigation"
        className="hidden sm:flex items-center gap-1 sm:gap-1.5 bg-white/[0.03] p-1.5 rounded-full border border-white/[0.08] backdrop-blur-xl shadow-lg shadow-black/20"
      >
        <Link
          to="/docs"
          onClick={skipSameRoute("/docs")}
          className={`px-3.5 py-1.5 rounded-full text-xs font-medium transition-all ${
            pathname.startsWith("/docs")
              ? "bg-white/10 text-white shadow-sm"
              : "text-zinc-400 hover:text-white hover:bg-white/[0.05]"
          }`}
        >
          Docs
        </Link>
        <Link
          to="/agents"
          onClick={skipSameRoute("/agents")}
          className={`px-3.5 py-1.5 rounded-full text-xs font-medium transition-all ${
            pathname === "/agents"
              ? "bg-white/10 text-white shadow-sm"
              : "text-zinc-400 hover:text-white hover:bg-white/[0.05]"
          }`}
        >
          Agents
        </Link>
        <Link
          to="/changelog"
          onClick={skipSameRoute("/changelog")}
          className={`px-3.5 py-1.5 rounded-full text-xs font-medium transition-all ${
            pathname === "/changelog"
              ? "bg-white/10 text-white shadow-sm"
              : "text-zinc-400 hover:text-white hover:bg-white/[0.05]"
          }`}
        >
          Changelog
        </Link>
        <a
          href="https://github.com/wisnuwiry/padu"
          target="_blank"
          rel="noopener noreferrer"
          aria-label={stars ? `GitHub, ${stars} stars` : "GitHub"}
          className="px-3 py-1.5 rounded-full text-xs font-medium text-zinc-400 hover:text-white hover:bg-white/[0.05] transition-all inline-flex items-center gap-1.5"
        >
          <GitHubIcon width="14" height="14" />
          {stars && <span className="tabular-nums text-[11px] font-mono opacity-80">{stars}</span>}
        </a>
        <Link
          to="/download"
          onClick={skipSameRoute("/download")}
          className={`ml-1 inline-flex items-center justify-center rounded-full px-4 py-1.5 text-xs font-medium transition-all ${
            pathname === "/download"
              ? "bg-white text-black font-semibold"
              : "bg-white text-black font-semibold hover:bg-zinc-200 active:scale-95"
          }`}
        >
          Download
        </Link>
      </nav>

      {/* Mobile action buttons (visible on mobile only) */}
      <div className="flex sm:hidden items-center gap-2">
        <Link
          to="/download"
          onClick={skipSameRoute("/download")}
          className="inline-flex items-center justify-center rounded-full px-3.5 py-1.5 text-xs font-semibold bg-white text-black hover:bg-zinc-200 active:scale-95 transition-all"
        >
          Download
        </Link>
        <button
          type="button"
          onClick={toggleMobileMenu}
          aria-label={mobileMenuOpen ? "Close navigation menu" : "Open navigation menu"}
          aria-expanded={mobileMenuOpen}
          className="w-8 h-8 rounded-full border border-white/10 bg-white/[0.04] flex items-center justify-center text-zinc-300 hover:text-white active:scale-95 transition-all cursor-pointer"
        >
          {mobileMenuOpen ? <X className="h-4 w-4" /> : <Menu className="h-4 w-4" />}
        </button>
      </div>

      {/* Mobile dropdown menu */}
      <AnimatePresence>
        {mobileMenuOpen && (
          <motion.div
            initial={{ opacity: 0, y: -8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -8, scale: 0.98 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            className="absolute top-full left-0 right-0 mt-3 p-3 rounded-2xl border border-white/10 bg-[#0c0c0e]/95 backdrop-blur-2xl shadow-2xl shadow-black/80 flex flex-col gap-1 z-50 sm:hidden"
          >
            <Link
              to="/docs"
              onClick={skipSameRoute("/docs")}
              className={`px-4 py-2.5 rounded-xl text-sm font-medium transition-all ${
                pathname.startsWith("/docs")
                  ? "bg-white/10 text-white"
                  : "text-zinc-400 hover:text-white hover:bg-white/[0.04]"
              }`}
            >
              Docs
            </Link>
            <Link
              to="/agents"
              onClick={skipSameRoute("/agents")}
              className={`px-4 py-2.5 rounded-xl text-sm font-medium transition-all ${
                pathname === "/agents"
                  ? "bg-white/10 text-white"
                  : "text-zinc-400 hover:text-white hover:bg-white/[0.04]"
              }`}
            >
              Supported Agents
            </Link>
            <Link
              to="/changelog"
              onClick={skipSameRoute("/changelog")}
              className={`px-4 py-2.5 rounded-xl text-sm font-medium transition-all ${
                pathname === "/changelog"
                  ? "bg-white/10 text-white"
                  : "text-zinc-400 hover:text-white hover:bg-white/[0.04]"
              }`}
            >
              Changelog
            </Link>
            <div className="h-px bg-white/[0.08] my-1" />
            <a
              href="https://github.com/wisnuwiry/padu"
              target="_blank"
              rel="noopener noreferrer"
              className="px-4 py-2.5 rounded-xl text-sm font-medium text-zinc-400 hover:text-white hover:bg-white/[0.04] transition-all flex items-center justify-between"
            >
              <span className="flex items-center gap-2">
                <GitHubIcon width="16" height="16" />
                GitHub Repository
              </span>
              {stars && (
                <span className="tabular-nums text-xs font-mono text-zinc-400 bg-white/[0.04] px-2 py-0.5 rounded border border-white/[0.08]">
                  {stars} ★
                </span>
              )}
            </a>
          </motion.div>
        )}
      </AnimatePresence>
    </header>
  );
}
