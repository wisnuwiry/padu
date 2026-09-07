import { describe, expect, it, beforeEach, afterEach } from "vitest";
import {
  formatStars,
  parseStarValue,
  getClientCachedStars,
  setClientCachedStars,
  resetClientStarsCache,
  fetchRealtimeStars,
  GITHUB_API_URL,
  SHIELDS_STARS_URL,
} from "./realtime-stars";

describe("realtime-stars", () => {
  beforeEach(() => {
    resetClientStarsCache();
  });

  afterEach(() => {
    resetClientStarsCache();
  });

  describe("formatStars", () => {
    it("formats counts under 1000 verbatim", () => {
      expect(formatStars(0)).toBe("0");
      expect(formatStars(5)).toBe("5");
      expect(formatStars(999)).toBe("999");
    });

    it("formats thousands with k suffix without trailing zero", () => {
      expect(formatStars(1000)).toBe("1k");
      expect(formatStars(2000)).toBe("2k");
    });

    it("formats thousands with single decimal when needed", () => {
      expect(formatStars(1200)).toBe("1.2k");
      expect(formatStars(1250)).toBe("1.3k");
      expect(formatStars(10500)).toBe("10.5k");
    });

    it("handles negative values defensively", () => {
      expect(formatStars(-1)).toBe("0");
    });
  });

  describe("parseStarValue", () => {
    it("parses numbers directly", () => {
      expect(parseStarValue(5)).toBe("5");
      expect(parseStarValue(1500)).toBe("1.5k");
    });

    it("parses numeric strings", () => {
      expect(parseStarValue("5")).toBe("5");
      expect(parseStarValue(" 42 ")).toBe("42");
      expect(parseStarValue("1200")).toBe("1.2k");
    });

    it("parses already formatted badge strings", () => {
      expect(parseStarValue("1.2k")).toBe("1.2k");
      expect(parseStarValue("90k")).toBe("90k");
      expect(parseStarValue("15.4K")).toBe("15.4k");
    });

    it("returns null on invalid or error strings", () => {
      expect(parseStarValue("repo not found")).toBeNull();
      expect(parseStarValue("")).toBeNull();
      expect(parseStarValue("   ")).toBeNull();
      expect(parseStarValue(null)).toBeNull();
      expect(parseStarValue(undefined)).toBeNull();
      expect(parseStarValue({})).toBeNull();
    });
  });

  describe("caching", () => {
    it("reads and writes to memory cache", () => {
      expect(getClientCachedStars()).toBeNull();
      setClientCachedStars("42");
      expect(getClientCachedStars()).toBe("42");
    });
  });

  describe("fetchRealtimeStars", () => {
    it("fetches directly from GitHub API when available", async () => {
      const originalFetch = globalThis.fetch;
      globalThis.fetch = (async (url: string | URL | Request) => {
        const urlStr = url.toString();
        if (urlStr === GITHUB_API_URL) {
          return new Response(JSON.stringify({ stargazers_count: 15 }));
        }
        return new Response("Not found", { status: 404 });
      }) as typeof fetch;

      try {
        const stars = await fetchRealtimeStars({ force: true });
        expect(stars).toBe("15");
        expect(getClientCachedStars()).toBe("15");
      } finally {
        globalThis.fetch = originalFetch;
      }
    });

    it("falls back to Shields.io CDN when GitHub API returns 403 rate limit", async () => {
      const originalFetch = globalThis.fetch;
      globalThis.fetch = (async (url: string | URL | Request) => {
        const urlStr = url.toString();
        if (urlStr === GITHUB_API_URL) {
          return new Response(JSON.stringify({ message: "API rate limit exceeded" }), {
            status: 403,
          });
        }
        if (urlStr === SHIELDS_STARS_URL) {
          return new Response(JSON.stringify({ message: "25" }));
        }
        return new Response("Not found", { status: 404 });
      }) as typeof fetch;

      try {
        const stars = await fetchRealtimeStars({ force: true });
        expect(stars).toBe("25");
        expect(getClientCachedStars()).toBe("25");
      } finally {
        globalThis.fetch = originalFetch;
      }
    });

    it("deduplicates simultaneous in-flight requests", async () => {
      let callCount = 0;
      const originalFetch = globalThis.fetch;
      globalThis.fetch = (async (url: string | URL | Request) => {
        const urlStr = url.toString();
        if (urlStr === GITHUB_API_URL) {
          callCount++;
          await new Promise((resolve) => setTimeout(resolve, 10));
          return new Response(JSON.stringify({ stargazers_count: 50 }));
        }
        return new Response("Not found", { status: 404 });
      }) as typeof fetch;

      try {
        const [res1, res2, res3] = await Promise.all([
          fetchRealtimeStars({ force: true }),
          fetchRealtimeStars({ force: true }),
          fetchRealtimeStars({ force: true }),
        ]);
        expect(res1).toBe("50");
        expect(res2).toBe("50");
        expect(res3).toBe("50");
        expect(callCount).toBe(1);
      } finally {
        globalThis.fetch = originalFetch;
      }
    });
  });
});
