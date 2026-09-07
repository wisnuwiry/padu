import type { ReactNode } from "react";
import { createContext, useContext } from "react";
import { Outlet, createRootRoute, HeadContent, Scripts } from "@tanstack/react-router";
import type { ReleaseChannels, ReleaseInfo } from "~/latest-release";
import { getLatestRelease } from "~/release";
import { getStarCount } from "~/stars";
import { useRealtimeStars } from "~/realtime-stars";

interface StarsContext {
  stars: string;
}

const ReleaseCtx = createContext<ReleaseChannels>({
  stable: {
    version: "",
    macDmgAsset: "",
    linuxX64Tarball: "",
    linuxArm64Tarball: null,
    windowsX64Asset: null,
    windowsArm64Asset: null,
  },
  beta: null,
});
const StarsCtx = createContext<StarsContext>({ stars: "" });

/** The latest stable release. Everything on the site points here by default. */
export function useRelease(): ReleaseInfo {
  return useContext(ReleaseCtx).stable;
}

/** The current beta, or null when there is no beta ahead of stable. */
export function useBetaRelease(): ReleaseInfo | null {
  return useContext(ReleaseCtx).beta;
}

export function useStars(): StarsContext {
  return useContext(StarsCtx);
}

export const Route = createRootRoute({
  loader: async () => {
    const [release, stars] = await Promise.all([getLatestRelease(), getStarCount()]);
    return { release, ...stars };
  },
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { name: "theme-color", content: "#0d0f14" },
      { property: "og:site_name", content: "Padu" },
      { property: "og:type", content: "website" },
      { property: "og:image", content: "https://padu.dev/preview-dark.webp" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:image", content: "https://padu.dev/preview-dark.webp" },
    ],
    links: [
      { rel: "icon", href: "/favicon.ico", sizes: "48x48" },
      { rel: "icon", href: "/padu.svg", type: "image/svg+xml" },
      { rel: "apple-touch-icon", href: "/apple-touch-icon.png" },
    ],
  }),
  component: RootComponent,
});

function RootComponent() {
  const data = Route.useLoaderData();
  const stars = useRealtimeStars(data.stars);
  return (
    <ReleaseCtx value={data.release}>
      <StarsCtx value={{ stars }}>
        <RootDocument>
          <Outlet />
        </RootDocument>
      </StarsCtx>
    </ReleaseCtx>
  );
}

function RootDocument({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body className="antialiased bg-background text-foreground">
        {children}
        <Scripts />
      </body>
    </html>
  );
}
