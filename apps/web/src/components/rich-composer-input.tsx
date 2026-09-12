import {
  forwardRef,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  type ClipboardEvent,
  type CompositionEvent,
  type FormEvent,
  type KeyboardEvent,
} from 'react'
import { fileTypeIconClass } from '@/components/padu-icon'
import {
  fileReferenceBasename,
  tokenizeInlineComposerText,
} from '@/lib/inline-file-references'
import { cn } from '@/lib/utils'

export type ComposerSelection = { start: number; end: number }

function canonicalLength(node: Node): number {
  if (node instanceof HTMLElement && node.dataset.canonical !== undefined) {
    return node.dataset.canonical.length
  }
  if (node.nodeType === Node.TEXT_NODE) return node.textContent?.length ?? 0
  if (node instanceof HTMLBRElement) return 1
  return [...node.childNodes].reduce((length, child) => length + canonicalLength(child), 0)
}

function pointOffset(root: HTMLElement, target: Node, targetOffset: number): number {
  let result = 0
  let found = false
  const visit = (node: Node): void => {
    if (found) return
    if (node === target) {
      if (node.nodeType === Node.TEXT_NODE) {
        result += Math.min(targetOffset, node.textContent?.length ?? 0)
      } else {
        const children = [...node.childNodes]
        for (let index = 0; index < Math.min(targetOffset, children.length); index++) {
          result += canonicalLength(children[index])
        }
      }
      found = true
      return
    }
    if (node instanceof HTMLElement && node.dataset.canonical !== undefined) {
      result += node.dataset.canonical.length
      return
    }
    if (node.nodeType === Node.TEXT_NODE) {
      result += node.textContent?.length ?? 0
      return
    }
    if (node instanceof HTMLBRElement) {
      result++
      return
    }
    for (const child of node.childNodes) visit(child)
  }
  visit(root)
  return result
}

export function composerEditorSelection(root: HTMLElement | null): ComposerSelection {
  if (!root) return { start: 0, end: 0 }
  const selection = window.getSelection()
  if (!selection?.rangeCount) return { start: canonicalLength(root), end: canonicalLength(root) }
  const range = selection.getRangeAt(0)
  if (!root.contains(range.startContainer) || !root.contains(range.endContainer)) {
    return { start: canonicalLength(root), end: canonicalLength(root) }
  }
  return {
    start: pointOffset(root, range.startContainer, range.startOffset),
    end: pointOffset(root, range.endContainer, range.endOffset),
  }
}

function setPoint(root: HTMLElement, range: Range, offset: number, start: boolean) {
  let remaining = Math.max(0, offset)
  const visit = (node: Node): boolean => {
    if (node instanceof HTMLElement && node.dataset.canonical !== undefined) {
      const length = node.dataset.canonical.length
      if (remaining <= length) {
        const parent = node.parentNode!
        const index = [...parent.childNodes].indexOf(node)
        const after = remaining > length / 2
        if (start) range.setStart(parent, index + Number(after))
        else range.setEnd(parent, index + Number(after))
        return true
      }
      remaining -= length
      return false
    }
    if (node.nodeType === Node.TEXT_NODE) {
      const length = node.textContent?.length ?? 0
      if (remaining <= length) {
        if (start) range.setStart(node, remaining)
        else range.setEnd(node, remaining)
        return true
      }
      remaining -= length
      return false
    }
    for (const child of node.childNodes) {
      if (visit(child)) return true
    }
    return false
  }
  if (!visit(root)) {
    if (start) range.setStart(root, root.childNodes.length)
    else range.setEnd(root, root.childNodes.length)
  }
}

export function setComposerEditorSelection(root: HTMLElement | null, start: number, end = start) {
  if (!root) return
  const range = document.createRange()
  setPoint(root, range, start, true)
  setPoint(root, range, end, false)
  const selection = window.getSelection()
  selection?.removeAllRanges()
  selection?.addRange(range)
}

function serializeEditor(node: Node): string {
  if (node instanceof HTMLElement && node.dataset.canonical !== undefined) {
    return node.dataset.canonical
  }
  if (node.nodeType === Node.TEXT_NODE) return node.textContent ?? ''
  if (node instanceof HTMLBRElement) return '\n'
  return [...node.childNodes].map(serializeEditor).join('')
}

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

function renderComposerProjection(value: string): string {
  return tokenizeInlineComposerText(value).map((token) => {
    if (token.kind === 'text') return escapeHtml(token.value)

    const label = token.kind === 'mention'
      ? token.target.replace(/[/\\]$/u, '')
      : fileReferenceBasename(token.target)
    const icon = token.isDirectory
      ? 'i-padu-folder'
      : fileTypeIconClass(token.target)
    const canonical = escapeHtml(token.canonical)
    const target = escapeHtml(token.target)
    return `<span contenteditable="false" class="composer-inline-file-chip" data-canonical="${canonical}" title="${target}" aria-label="${target}"><span class="inline-grid size-4 shrink-0 place-items-center"><span class="${icon}" style="width:100%;height:100%"></span></span><span class="min-w-0 truncate">${escapeHtml(label)}</span></span>`
  }).join('')
}

export const RichComposerInput = forwardRef<HTMLDivElement, {
  value: string
  placeholder: string
  className?: string
  ariaControls?: string
  ariaExpanded: boolean
  ariaActiveDescendant?: string
  ariaLabel: string
  onBlur: () => void
  onChange: (value: string, selection: ComposerSelection) => void
  onFocus: () => void
  onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => void
  onSelectionChange: (selection: ComposerSelection) => void
}>(function RichComposerInput({
  value,
  placeholder,
  className,
  ariaControls,
  ariaExpanded,
  ariaActiveDescendant,
  ariaLabel,
  onBlur,
  onChange,
  onFocus,
  onKeyDown,
  onSelectionChange,
}, forwardedRef) {
  const localRef = useRef<HTMLDivElement | null>(null)
  const composing = useRef(false)
  const pendingDomSelection = useRef<{
    value: string
    selection: ComposerSelection
  } | null>(null)
  const setRef = useCallback((node: HTMLDivElement | null) => {
    localRef.current = node
    if (typeof forwardedRef === 'function') forwardedRef(node)
    else if (forwardedRef) forwardedRef.current = node
  }, [forwardedRef])

  const reportSelection = () => onSelectionChange(composerEditorSelection(localRef.current))
  const reportValue = () => {
    const root = localRef.current
    if (!root) return
    const nextValue = serializeEditor(root)
    const selection = composerEditorSelection(root)
    // React may replace the just-edited text node with one or more atomic chip
    // nodes. Restore this DOM-originated canonical range after that projection;
    // parent-driven value changes never populate this ref, so autocomplete and
    // prefill cursor restoration remain authoritative.
    pendingDomSelection.current = { value: nextValue, selection }
    onChange(nextValue, selection)
  }

  useLayoutEffect(() => {
    const pending = pendingDomSelection.current
    if (!pending) return
    pendingDomSelection.current = null
    if (pending.value !== value) return
    setComposerEditorSelection(localRef.current, pending.selection.start, pending.selection.end)
  }, [value])

  useEffect(() => {
    const update = () => {
      const root = localRef.current
      const selection = document.getSelection()
      if (root && selection?.anchorNode && root.contains(selection.anchorNode)) reportSelection()
    }
    document.addEventListener('selectionchange', update)
    return () => document.removeEventListener('selectionchange', update)
  })

  const copyCanonical = (event: ClipboardEvent<HTMLDivElement>, cut: boolean) => {
    const selection = composerEditorSelection(localRef.current)
    if (selection.start === selection.end) return
    event.preventDefault()
    event.clipboardData.setData('text/plain', value.slice(selection.start, selection.end))
    if (cut) document.execCommand('delete')
  }

  const insertPlainText = (text: string) => {
    if (document.execCommand('insertText', false, text)) return
    const selection = window.getSelection()
    if (!selection?.rangeCount) return
    const range = selection.getRangeAt(0)
    range.deleteContents()
    const node = document.createTextNode(text)
    range.insertNode(node)
    range.setStartAfter(node)
    range.collapse(true)
    selection.removeAllRanges()
    selection.addRange(range)
    reportValue()
  }

  return (
    <div
      aria-activedescendant={ariaActiveDescendant}
      aria-autocomplete="list"
      aria-controls={ariaControls}
      aria-expanded={ariaExpanded}
      aria-label={ariaLabel}
      className={cn('composer-rich-input max-h-48 min-h-[46px] w-full overflow-y-auto whitespace-pre-wrap break-words px-1 pb-1 pt-0 text-[14px] leading-5 text-foreground outline-none', className)}
      contentEditable
      data-placeholder={placeholder}
      ref={setRef}
      role="combobox"
      spellCheck
      suppressContentEditableWarning
      onBlur={onBlur}
      onClick={reportSelection}
      onCompositionEnd={(_event: CompositionEvent<HTMLDivElement>) => {
        composing.current = false
        reportValue()
      }}
      onCompositionStart={() => { composing.current = true }}
      onCopy={(event) => copyCanonical(event, false)}
      onCut={(event) => copyCanonical(event, true)}
      onFocus={onFocus}
      onInput={(_event: FormEvent<HTMLDivElement>) => {
        if (!composing.current) reportValue()
      }}
      onKeyDown={onKeyDown}
      onKeyUp={reportSelection}
      onPaste={(event) => {
        event.preventDefault()
        insertPlainText(event.clipboardData.getData('text/plain'))
      }}
      dangerouslySetInnerHTML={{ __html: renderComposerProjection(value) }}
    />
  )
})
