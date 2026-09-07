import * as React from "react";

export const GITHUB_REPO = "wisnuwiry/padu";
export const GITHUB_API_URL = `https://api.github.com/repos/${GITHUB_REPO}`;
export const SHIELDS_STARS_URL = `https://img.shields.io/github/stars/${GITHUB_REPO}.json`;

export const STARS_CLIENT_CACHE_KEY = "padu:github-stars:v1";
export const STARS_CACHE_TTL_MS = 60 * 1000; // 60 seconds

export function formatStars(count: number): string {
  if (count < 0) return "0";
  if (count < 1000) return String(count);
  const k = count / 1000;
  return `${k % 1 === 0 ? k.toFixed(0) : k.toFixed(1)}k`;
}

export function parseStarValue(raw: unknown): string | null {
  if (typeof raw === "number" && !Number.isNaN(raw)) {
    return formatStars(raw);
  }
  if (typeof raw === "string") {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    const num = Number(trimmed);
    if (!Number.isNaN(num) && num >= 0) {
      return formatStars(num);
    }
    if (/^\d+(\.\d+)?k$/i.test(trimmed)) {
      return trimmed.toLowerCase();
    }
  }
  return null;
}

export interface StarCacheEntry {
  stars: string;
  timestamp: number;
}

let memoryCache: StarCacheEntry | null = null;
let inFlightRequest: Promise<string | null> | null = null;

export function getClientCachedStars(): string | null {
  const now = Date.now();
  if (memoryCache && now - memoryCache.timestamp < STARS_CACHE_TTL_MS) {
    return memoryCache.stars;
  }
  if (typeof window !== "undefined" && window.sessionStorage) {
    try {
      const stored = window.sessionStorage.getItem(STARS_CLIENT_CACHE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored) as StarCacheEntry;
        if (typeof parsed?.stars === "string" && typeof parsed?.timestamp === "number") {
          if (now - parsed.timestamp < STARS_CACHE_TTL_MS) {
            memoryCache = parsed;
            return parsed.stars;
          }
        }
      }
    } catch {
      // Ignore storage errors
    }
  }
  return null;
}

export function setClientCachedStars(stars: string): void {
  const entry: StarCacheEntry = { stars, timestamp: Date.now() };
  memoryCache = entry;
  if (typeof window !== "undefined" && window.sessionStorage) {
    try {
      window.sessionStorage.setItem(STARS_CLIENT_CACHE_KEY, JSON.stringify(entry));
    } catch {
      // Ignore storage errors
    }
  }
}

export function resetClientStarsCache(): void {
  memoryCache = null;
  inFlightRequest = null;
  if (typeof window !== "undefined" && window.sessionStorage) {
    try {
      window.sessionStorage.removeItem(STARS_CLIENT_CACHE_KEY);
    } catch {
      // Ignore
    }
  }
}

export async function fetchFromGitHub(signal?: AbortSignal): Promise<string | null> {
  const res = await fetch(GITHUB_API_URL, {
    signal,
    headers: {
      Accept: "application/vnd.github+json",
    },
  });
  if (!res.ok) {
    throw new Error(`GitHub API returned HTTP ${res.status}`);
  }
  const data = (await res.json()) as { stargazers_count?: number };
  if (typeof data.stargazers_count === "number") {
    return formatStars(data.stargazers_count);
  }
  return null;
}

export async function fetchFromShieldsCdn(signal?: AbortSignal): Promise<string | null> {
  const res = await fetch(SHIELDS_STARS_URL, {
    signal,
    headers: {
      Accept: "application/json",
    },
  });
  if (!res.ok) {
    throw new Error(`Shields CDN returned HTTP ${res.status}`);
  }
  const data = (await res.json()) as { message?: unknown; value?: unknown };
  const starValue = parseStarValue(data.message ?? data.value);
  if (starValue) {
    return starValue;
  }
  throw new Error("Invalid star value from Shields CDN");
}

export async function fetchRealtimeStars(options?: {
  force?: boolean;
  signal?: AbortSignal;
}): Promise<string | null> {
  if (!options?.force) {
    const cached = getClientCachedStars();
    if (cached) return cached;
  }

  if (inFlightRequest) {
    return inFlightRequest;
  }

  inFlightRequest = (async () => {
    // 1. Try GitHub REST API first for immediate real-time accuracy
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 3500);
      const onParentAbort = () => controller.abort();
      options?.signal?.addEventListener("abort", onParentAbort);

      try {
        const stars = await fetchFromGitHub(controller.signal);
        if (stars) {
          setClientCachedStars(stars);
          return stars;
        }
      } finally {
        clearTimeout(timeoutId);
        options?.signal?.removeEventListener("abort", onParentAbort);
      }
    } catch {
      // Failed, timed out, or rate-limited (HTTP 403/429) -> proceed to CDN
    }

    // 2. Fallback to Shields.io CDN proxy (Cloudflare-backed, bypasses rate limits)
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 4000);
      const onParentAbort = () => controller.abort();
      options?.signal?.addEventListener("abort", onParentAbort);

      try {
        const stars = await fetchFromShieldsCdn(controller.signal);
        if (stars) {
          setClientCachedStars(stars);
          return stars;
        }
      } finally {
        clearTimeout(timeoutId);
        options?.signal?.removeEventListener("abort", onParentAbort);
      }
    } catch {
      // Both sources failed
    }

    return memoryCache?.stars ?? null;
  })().finally(() => {
    inFlightRequest = null;
  });

  return inFlightRequest;
}

/**
 * Hook to provide real-time GitHub stars count for static landing pages.
 * Initializes with the static/SSR count for instant display with zero layout shift,
 * then validates/updates in the background via CDN/GitHub API.
 */
export function useRealtimeStars(initialStars: string = ""): string {
  const [stars, setStars] = React.useState<string>(() => {
    if (typeof window !== "undefined") {
      const cached = getClientCachedStars();
      if (cached) return cached;
    }
    return initialStars;
  });

  // Sync if initialStars changes (e.g. from route loader)
  React.useEffect(() => {
    if (initialStars && !stars) {
      setStars(initialStars);
    }
  }, [initialStars, stars]);

  React.useEffect(() => {
    let isMounted = true;

    const syncStars = (force: boolean = false) => {
      fetchRealtimeStars({ force })
        .then((fresh) => {
          if (isMounted && fresh) {
            setStars((prev) => (prev !== fresh ? fresh : prev));
          }
        })
        .catch(() => {});
    };

    // Client-side fetch on mount
    syncStars(false);

    // Refresh when user returns to tab (e.g. after starring on GitHub)
    const handleVisibility = () => {
      if (typeof document !== "undefined" && document.visibilityState === "visible") {
        syncStars(false);
      }
    };

    const handleFocus = () => {
      syncStars(false);
    };

    window.addEventListener("visibilitychange", handleVisibility);
    window.addEventListener("focus", handleFocus);

    // Periodic sync every 60s
    const intervalId = window.setInterval(() => {
      syncStars(true);
    }, STARS_CACHE_TTL_MS);

    return () => {
      isMounted = false;
      window.removeEventListener("visibilitychange", handleVisibility);
      window.removeEventListener("focus", handleFocus);
      window.clearInterval(intervalId);
    };
  }, []);

  return stars;
}
