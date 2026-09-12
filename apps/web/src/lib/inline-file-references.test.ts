import { describe, expect, test } from 'bun:test'
import {
  atomicReferenceDeletion,
  fileReferenceBasename,
  insertInlineFileReference,
  markdownFileReference,
  parseInlineFileReferences,
  tokenizeInlineComposerText,
} from './inline-file-references'

describe('inline file references', () => {
  test('formats escaped labels and normalized Markdown destinations like desktop', () => {
    expect(markdownFileReference('docs/a [file] (old).md')).toBe(
      '[a \\[file\\] (old).md](docs/a%20[file]%20\\(old\\).md)',
    )
    expect(markdownFileReference('src\\widgets\\')).toBe('[widgets](src/widgets/)')
  })

  test('parses escaped labels plus parenthesized and angle destinations', () => {
    const parsed = parseInlineFileReferences(
      String.raw`[ignored\] label](docs/a%20file\(old\).md#L12) [nested](src/a(b).ts) [angle](<src/a%20b.ts>)`,
    )
    expect(parsed.map(({ target, basename }) => ({ target, basename }))).toEqual([
      { target: 'docs/a file(old).md#L12', basename: 'a file(old).md' },
      { target: 'src/a(b).ts', basename: 'a(b).ts' },
      { target: 'src/a b.ts', basename: 'a b.ts' },
    ])
  })

  test('uses decoded target basenames and rejects non-file Markdown links', () => {
    expect(fileReferenceBasename('deep/My%20File.rs:42')).toBe('My File.rs')
    const parsed = parseInlineFileReferences(
      '![img](shot.png) [web](https://padu.dev) [mail](MAILTO:a@b.co) [app](PaDu:settings) [js](javascript:alert(1)) [data](data:text/plain,x) [vb](VBScript:msgbox(1)) [win](C:/src/app.ts) [ok](src/app.ts)',
    )
    expect(parsed.map((reference) => reference.target)).toEqual(['C:/src/app.ts', 'src/app.ts'])
  })

  test('projects pasted canonical references while preserving legacy mentions', () => {
    const tokens = tokenizeInlineComposerText('Open [app.ts](src/app.ts) and @legacy/file.rs')
    expect(tokens.filter((token) => token.kind === 'file')).toHaveLength(1)
    expect(tokens.filter((token) => token.kind === 'mention')).toHaveLength(1)
  })

  test('inserts with stable separators and deletes adjacent chips atomically', () => {
    expect(insertInlineFileReference('read old next', 5, 8, 'src/app.ts')).toEqual({
      text: 'read [app.ts](src/app.ts) next',
      cursor: 26,
    })
    expect(atomicReferenceDeletion('read [app.ts](src/app.ts) next', 25, 25, 'backward')).toEqual({
      text: 'read next',
      cursor: 4,
    })
  })
})
