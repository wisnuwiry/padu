import type React from "react";

const ANALYTICS_ENDPOINT = import.meta.env.PADU_ANALYTICS_ENDPOINT?.trim();
const ANALYTICS_WEBSITE_ID = import.meta.env.PADU_ANALYTICS_WEBSITE_ID?.trim();

export const analyticsAvailable = Boolean(ANALYTICS_ENDPOINT && ANALYTICS_WEBSITE_ID);

interface UmamiLike {
  track: (name: string, data?: Record<string, unknown>) => void | Promise<void>;
}

function umami(): UmamiLike | undefined {
  if (typeof window === "undefined") return undefined;
  return (window as unknown as { umami?: UmamiLike }).umami;
}

export function AnalyticsHeadScript() {
  if (!analyticsAvailable) return null;
  const props = {
    src: `${ANALYTICS_ENDPOINT}/script.js`,
    async: true,
    defer: true,
    "data-website-id": ANALYTICS_WEBSITE_ID,
  } as unknown as React.ScriptHTMLAttributes<HTMLScriptElement>;
  return <script {...props} />;
}

export function trackEvent(name: string, data?: Record<string, unknown>) {
  umami()?.track(name, data);
}