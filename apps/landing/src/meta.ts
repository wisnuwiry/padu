const SITE_ORIGIN = "https://padu.dev";

export interface PageMetaOptions {
  keywords?: string[];
  structuredData?: Record<string, unknown> | Array<Record<string, unknown>>;
  type?: "website" | "article";
  image?: string;
  imageAlt?: string;
  noindex?: boolean;
}

export function pageMeta(
  title: string,
  description: string,
  path: string,
  options?: PageMetaOptions,
) {
  const url = `${SITE_ORIGIN}${path}`;
  const meta: Array<Record<string, any>> = [
    { title },
    { name: "description", content: description },
    { property: "og:title", content: title },
    { property: "og:description", content: description },
    { property: "og:url", content: url },
    { property: "og:type", content: options?.type ?? "website" },
    { name: "twitter:title", content: title },
    { name: "twitter:description", content: description },
  ];

  if (options?.keywords && options.keywords.length > 0) {
    meta.push({ name: "keywords", content: options.keywords.join(", ") });
  }

  if (options?.image) {
    meta.push(
      { property: "og:image", content: options.image },
      { name: "twitter:image", content: options.image },
    );
  }

  if (options?.imageAlt) {
    meta.push(
      { property: "og:image:alt", content: options.imageAlt },
      { name: "twitter:image:alt", content: options.imageAlt },
    );
  }

  if (options?.noindex) {
    meta.push({ name: "robots", content: "noindex, nofollow" });
  }

  if (options?.structuredData) {
    if (Array.isArray(options.structuredData)) {
      for (const item of options.structuredData) {
        meta.push({ "script:ld+json": item });
      }
    } else {
      meta.push({ "script:ld+json": options.structuredData });
    }
  }

  return {
    meta,
    links: [{ rel: "canonical", href: url }],
  };
}

