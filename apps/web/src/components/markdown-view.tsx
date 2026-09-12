import { createContext, isValidElement, useContext, useRef, type CSSProperties } from 'react'
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown'
import rehypeKatex from 'rehype-katex'
import rehypeRaw from 'rehype-raw'
import rehypeSanitize from 'rehype-sanitize'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import 'katex/dist/katex.min.css'
import { PreviewableImage } from '@/components/image-preview'
import { InlineFileChip } from '@/components/inline-file-chip'
import { MarkdownCodeBlock } from '@/components/markdown-code-block'
import { MermaidDiagram } from '@/components/mermaid-diagram'
import { isLocalFileTarget } from '@/lib/inline-file-references'
import { KATEX_MACROS, renderKatex } from '@/lib/katex-math'
import { markdownHtmlSchema } from '@/lib/markdown-html'
import {
  advanceMarkdownVeil,
  createMarkdownVeilState,
  markdownVeilPlugin,
} from '@/lib/markdown-veil'
import { cn } from '@/lib/utils'

/// Shared markdown rendering for every surface: the transcript, file previews,
/// and notes. Centralizing it here keeps a single component map (links, images,
/// math, mermaid, highlighted code) so every client renderer behaves the same
/// and future style customization has one place to live. `className` merges
/// onto the `.markdown` wrapper so callers can shape size, scroll, and surface
/// without reimplementing.
export const TranscriptLinkContext = createContext<(target: string) => boolean>(() => false)

export function MarkdownView({
  text,
  className,
  style,
  compact = false,
  streaming = false,
}: {
  text: string
  className?: string
  style?: CSSProperties
  compact?: boolean
  streaming?: boolean
}) {
  const onOpenLink = useContext(TranscriptLinkContext)
  // Match the painter's attach semantics: text already present when this row
  // mounts is the baseline; only later appends dissolve in.
  const veil = useRef(createMarkdownVeilState(text))
  const now = Date.now()
  const chunks = advanceMarkdownVeil(veil.current, text, streaming, now)
  return (
    <div className={cn('markdown min-w-0', compact && '[&>*:first-child]:mt-0 [&>*:last-child]:mb-0', className)} style={style}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkMath]}
        // react-markdown applies its own URL guard on top of the sanitizer,
        // and its default transform rejects `data:` sources entirely. Data
        // URIs are common in agent transcripts, so keep them for `src` while
        // delegating everything else (including `href`) to the default.
        urlTransform={(url, key) =>
          key === 'src' && url.startsWith('data:') ? url : defaultUrlTransform(url)
        }
        rehypePlugins={[
          // Raw HTML is parsed then constrained to the GitHub-style subset in
          // lib/markdown-html before anything else runs on the tree. The
          // sanitize pass must precede rehype-katex so the remark-math class
          // markers survive to be rendered (see markdownHtmlSchema).
          rehypeRaw,
          [rehypeSanitize, markdownHtmlSchema],
          // `throwOnError: false` keeps a half-typed `$...$` from tearing the
          // whole document down while a response streams; KaTeX renders the
          // invalid source inline instead of throwing.
          [rehypeKatex, { throwOnError: false, macros: KATEX_MACROS }],
          ...(chunks.length ? [markdownVeilPlugin(chunks, now)] : []),
        ]}
        components={{
          pre: ({ children }) => {
            const child = Array.isArray(children) ? children[0] : children
            if (isValidElement(child)) {
              const props = child.props as { className?: unknown; children?: unknown }
              const className = typeof props.className === 'string' ? props.className : ''
              const code = fenceCode(props.children)
              const classes = className.split(' ')
              if (classes.includes('language-mermaid')) {
                return <MermaidDiagram code={code} />
              }
              if (
                classes.includes('language-latex')
                || classes.includes('language-tex')
                || classes.includes('language-math')
              ) {
                return <LatexBlock code={code} />
              }
              const language = classes.find((entry) => entry.startsWith('language-'))
                ?.slice('language-'.length)
              return <MarkdownCodeBlock language={language} code={code} />
            }
            return <pre>{children}</pre>
          },
          a: ({ children, href, ...props }) => {
            if (typeof href === 'string' && isLocalFileTarget(href)) {
              return (
                <InlineFileChip
                  className={props.className}
                  href={href}
                  target={href}
                  onOpen={onOpenLink}
                />
              )
            }
            return (
              <a
                {...props}
                href={href}
                target="_blank"
                rel="noreferrer noopener"
                onClick={(event) => {
                  if (href && onOpenLink(href)) event.preventDefault()
                }}
              >
                {children}
              </a>
            )
          },
          img: ({ alt, src }) => typeof src === 'string' ? (
            <PreviewableImage
              buttonClassName="max-w-full rounded-[9px] border bg-[var(--inset)]"
              imageClassName="max-h-64 max-w-full object-contain"
              name={alt || imageName(src)}
              source={src}
            />
          ) : null,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  )
}

function imageName(source: string) {
  if (source.startsWith('data:')) return 'Image'
  try {
    const path = new URL(source, window.location.href).pathname
    return decodeURIComponent(path.split('/').filter(Boolean).at(-1) ?? 'Image')
  } catch {
    return source.split('/').filter(Boolean).at(-1) ?? 'Image'
  }
}

/// Text content of a fenced code block, joining any split nodes and stripping
/// the fence's trailing newline so the rendered block has no phantom empty
/// final line (mirrors the desktop parser, which pops the trailing `\n`).
export function fenceCode(children: unknown): string {
  const code = Array.isArray(children) ? children.join('') : String(children ?? '')
  return code.replace(/\n$/, '')
}

/// Renders a ` ```latex ` fenced block as display math. KaTeX output is applied
/// via `dangerouslySetInnerHTML`, keeping React's virtual DOM consistent (a
/// direct `innerHTML` write on a React-owned node would desync and crash).
function LatexBlock({ code }: { code: string }) {
  return (
    <div
      className="katex-display overflow-x-auto overflow-y-hidden py-1"
      dangerouslySetInnerHTML={{ __html: renderKatex(code, true) }}
    />
  )
}