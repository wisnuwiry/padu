import { createServerFn } from "@tanstack/react-start";
import { getWebsiteCacheContext } from "./cloudflare-cache";
import { getBlockingColdCache } from "./github-cache";
import {
  formatStars,
  parseStarValue,
  GITHUB_API_URL,
  SHIELDS_STARS_URL,
} from "./realtime-stars";

interface GitHubRepo {
  stargazers_count: number;
}

const STARS_CACHE_KEY = "github-stars:v2";

async function fetchStarCount(): Promise<string> {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "User-Agent": "padu-website",
  };
  const token =
    typeof process !== "undefined"
      ? process.env?.GITHUB_TOKEN || process.env?.GH_TOKEN
      : undefined;
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  // 1. Try GitHub API
  try {
    const res = await fetch(GITHUB_API_URL, {
      headers,
      cf: {
        cacheEverything: true,
        cacheTtl: 60,
        cacheKey: "github-repo-stars",
      },
    } as RequestInit);
    if (res.ok) {
      const repo = (await res.json()) as GitHubRepo;
      if (typeof repo.stargazers_count === "number") {
        return formatStars(repo.stargazers_count);
      }
    }
  } catch {
    // Fallback to CDN
  }

  // 2. Fallback to Shields.io CDN
  try {
    const res = await fetch(SHIELDS_STARS_URL, {
      headers: {
        Accept: "application/json",
        "User-Agent": "padu-website",
      },
      cf: {
        cacheEverything: true,
        cacheTtl: 300,
        cacheKey: "shields-repo-stars",
      },
    } as RequestInit);
    if (res.ok) {
      const data = (await res.json()) as { message?: unknown; value?: unknown };
      const parsed = parseStarValue(data.message ?? data.value);
      if (parsed) return parsed;
    }
  } catch {
    // Both failed
  }

  return "";
}

function isStars(value: unknown): value is string {
  return typeof value === "string" && (value === "" || /^(\d+|\d+\.\d+k|\d+k)$/.test(value));
}

export const getStarCount = createServerFn({ method: "GET" }).handler(async () => {
  const stars = await getBlockingColdCache({
    context: getWebsiteCacheContext(),
    key: STARS_CACHE_KEY,
    isValue: isStars,
    fetchFresh: fetchStarCount,
  });
  return { stars };
});

