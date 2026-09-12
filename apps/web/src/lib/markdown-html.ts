import { defaultSchema } from 'rehype-sanitize'
import type { Options as SanitizeSchema } from 'rehype-sanitize'

/// Raw-HTML policy for markdown surfaces, modeled on GitHub's markdown
/// sanitizer. `rehype-raw` parses embedded HTML, then `rehype-sanitize` runs
/// this schema to strip anything that does not belong in a rendered note or
/// transcript (scripts, event handlers, `javascript:` URLs, unknown tags).
///
/// The schema is the library's GitHub-style default with two app-specific
/// extensions, both deliberate:
///
/// - Math markers: `remark-math` emits inline `$…$` as
///   `<span class="math math-inline">` and display `$$…$$` as
///   `<pre><code class="language-math math-display">`. Sanitize runs *before*
///   `rehype-katex`, so those class markers must survive or KaTeX never sees
///   the math. The rendered KaTeX output is added afterwards and is not
///   sanitized.
/// - `data:` image sources: agent transcripts and notes routinely embed
///   inline images as data URIs, which GitHub's stricter `src` policy would
///   drop. `data:` in an `<img>` is inert (never executes scripts).
export const markdownHtmlSchema: SanitizeSchema = {
  ...defaultSchema,
  attributes: {
    ...(defaultSchema.attributes ?? {}),
    span: [
      ...(defaultSchema.attributes?.span ?? []),
      ['className', 'math', 'math-inline'],
    ],
    div: [
      ...(defaultSchema.attributes?.div ?? []),
      ['className', 'math', 'math-display'],
    ],
  },
  protocols: {
    ...(defaultSchema.protocols ?? {}),
    src: [...(defaultSchema.protocols?.src ?? []), 'data'],
  },
}