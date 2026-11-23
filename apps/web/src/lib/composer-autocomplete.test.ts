import { describe, expect, test } from 'bun:test'
import type { FileEntry, ReportedCommand, SlashCommand } from '@padu/client'
import {
  SKILL_CATALOG_UNAVAILABLE_WARNING,
  skillSubmissionWarnings,
  type SkillCommand,
} from '@padu/client/composer-skills'
import {
  composerAutocompleteRows,
  detectComposerTrigger,
  expandCommandTemplate,
  expandedComposerSubmission,
  filterComposerCommandsForProvider,
  isFastModeToggleSubmission,
  isResumeSubmission,
  mergeComposerCommands,
  parseGoalSubmission,
  renderedComposerSubmission,
  replaceComposerTrigger,
  toggledFastServiceTier,
} from './composer-autocomplete'

describe('composer autocomplete', () => {
  test('detects slash commands only at the start of the current line', () => {
    expect(detectComposerTrigger('intro\n/rev', 10)).toEqual({
      kind: 'command',
      query: 'rev',
      start: 6,
      end: 10,
    })
    expect(detectComposerTrigger('intro /rev', 10)).toBeNull()
    expect(detectComposerTrigger('/review now', 11)).toBeNull()
    expect(detectComposerTrigger('/review $bra', 12)).toEqual({
      kind: 'skill',
      query: 'bra',
      start: 8,
      end: 12,
    })
  })

  test('hides compact from providers outside the supported harness set', () => {
    const commands = [
      command('compact', 'Builtin', 'Compact context', null),
      command('review', 'Builtin', 'Review', null),
    ]
    expect(
      filterComposerCommandsForProvider('claude', commands).map((item) => item.name),
    ).toEqual(['compact', 'review'])
    expect(
      filterComposerCommandsForProvider('openCode', commands).map((item) => item.name),
    ).toEqual(['review'])
  })

  test('detects the current whitespace-delimited file mention', () => {
    expect(detectComposerTrigger('look at @src/app', 16)).toEqual({
      kind: 'file',
      query: 'src/app',
      start: 8,
      end: 16,
    })
    expect(detectComposerTrigger('email@example.com', 17)).toBeNull()
  })

  test('detects $ skill mentions at whitespace boundaries only', () => {
    expect(detectComposerTrigger('use $bra', 8)).toEqual({
      kind: 'skill',
      query: 'bra',
      start: 4,
      end: 8,
    })
    expect(detectComposerTrigger('email$bra', 9)).toBeNull()
    expect(detectComposerTrigger('`$bra`', 5)).toBeNull()
  })

  test('replaces only the active token and keeps one separator before following text', () => {
    const trigger = detectComposerTrigger('read @app then', 9)!
    expect(replaceComposerTrigger('read @app then', trigger, {
      kind: 'file',
      file: { path: 'src/app.ts', is_dir: false },
    })).toEqual({ text: 'read [app.ts](src/app.ts) then', cursor: 26 })
  })

  test('replaces a $ trigger with the exact skill name', () => {
    const text = 'please $bra then'
    const trigger = detectComposerTrigger(text, 11)!
    expect(replaceComposerTrigger(text, trigger, {
      kind: 'skill',
      command: skill('brainstroming', '/skills/brainstroming/SKILL.md'),
    })).toEqual({
      text: 'please $brainstroming then',
      cursor: 22,
    })
  })

  test('merges live provider commands without losing discovered templates', () => {
    const discovered = [
      command('review', 'Project', 'Review changes', 'Review $ARGUMENTS'),
      command('deploy', 'Skill', 'Deploy the app', null),
    ]
    const reported: ReportedCommand[] = [
      { name: 'review', description: 'Provider review' },
      { name: 'compact', description: 'Compact context' },
    ]
    expect(mergeComposerCommands(discovered, reported)).toEqual([
      command('compact', 'Builtin', 'Compact context', null),
      { ...command('note', 'Builtin', 'Search and embed a note in chat', null), argument_hint: '<text>' },
      discovered[0],
      discovered[1],
    ])
  })

  test('keeps a provider or project note command without adding a duplicate', () => {
    const existing = command('note', 'Project', 'Project note command', 'use note')
    const merged = mergeComposerCommands([existing], [])
    expect(merged.filter((item) => item.name === 'note')).toEqual([existing])
  })

  test('recognizes only the resolved Codex fast-mode command', () => {
    const builtin = command('fast', 'Builtin', 'Toggle fast mode', null)
    expect(isFastModeToggleSubmission('codex', '/fast ', [builtin])).toBe(true)
    expect(isFastModeToggleSubmission('claude', '/fast', [builtin])).toBe(false)
    expect(isFastModeToggleSubmission('codex', '/fast now', [builtin])).toBe(false)
    expect(isFastModeToggleSubmission('codex', '/fast', [
      command('fast', 'Project', 'Project fast command', 'Run fast'),
    ])).toBe(false)
  })

  test('recognizes Resume as an exact provider-neutral local command', () => {
    expect(isResumeSubmission('/resume')).toBe(true)
    expect(isResumeSubmission('  /resume  ')).toBe(true)
    expect(isResumeSubmission('/resume latest')).toBe(false)
    expect(isResumeSubmission('please /resume')).toBe(false)
  })

  test('toggles the concrete Fast service-tier ID reported by the model', () => {
    const tiers = [{ id: 'priority', label: 'Fast' }]
    expect(toggledFastServiceTier('default', tiers)).toBe('priority')
    expect(toggledFastServiceTier('priority', tiers)).toBe('default')
    expect(toggledFastServiceTier(null, [])).toBeNull()
  })

  test('filters by fuzzy path and caps the result count', () => {
    const files: FileEntry[] = Array.from({ length: 100 }, (_, index) => ({
      path: `src/component-${index}.tsx`,
      is_dir: false,
    }))
    const trigger = { kind: 'file' as const, query: 'cmp1', start: 0, end: 5 }
    const rows = composerAutocompleteRows(trigger, [], files, 3)
    expect(rows).toHaveLength(3)
    expect(rows.every((row) => row.kind === 'file')).toBe(true)
  })

  test('keeps slash commands complete while $ opens only skills', () => {
    const commands = [
      skill('brainstroming', '/skills/brainstroming/SKILL.md'),
      command('review', 'Project', 'Review changes', 'Review $ARGUMENTS'),
    ]
    const slashRows = composerAutocompleteRows(
      { kind: 'command', query: '', start: 0, end: 1 },
      commands,
      [],
    )
    const skillRows = composerAutocompleteRows(
      { kind: 'skill', query: '', start: 0, end: 1 },
      commands,
      [],
    )
    expect(slashRows).toHaveLength(2)
    expect(slashRows[0]).toMatchObject({
      kind: 'command',
      command: { name: 'brainstroming', scope: 'Skill' },
    })
    expect(slashRows[1]).toMatchObject({
      kind: 'command',
      command: { name: 'review' },
    })
    expect(skillRows).toEqual([{ kind: 'skill', command: commands[0] }])
  })
})

describe('slash command templates', () => {
  test('expands all-argument and positional placeholders', () => {
    expect(expandCommandTemplate('All: $ARGUMENTS / $@ / $1 / $3', 'one two three'))
      .toBe('All: one two three / one two three / one / three')
  })

  test('appends arguments when a template has no placeholder', () => {
    expect(expandCommandTemplate('Review this project', 'carefully'))
      .toBe('Review this project\n\ncarefully')
  })

  test('expands only a known command with a template', () => {
    const commands = [command('review', 'Project', '', 'Review $ARGUMENTS')]
    expect(expandedComposerSubmission('openCode', '/review src', commands)).toBe('Review src')
    expect(expandedComposerSubmission('openCode', '/unknown src', commands)).toBeNull()
    expect(expandedComposerSubmission('openCode', 'please /review src', commands)).toBeNull()
  })

  test('uses each provider native skill invocation', () => {
    const commands = [command('deploy', 'Skill', '', null)]
    expect(expandedComposerSubmission('fx', '/deploy production', commands))
      .toBe('$deploy production')
    expect(expandedComposerSubmission('pi', '/deploy production', commands))
      .toBe('/skill:deploy production')
  })

  test('renders $ mentions with the shared fallback and preserves slash commands', () => {
    const commands = [
      skill('brainstroming', '/skills/Brain Stroming/SKILL.md'),
      command('review', 'Project', '', 'Review $ARGUMENTS'),
    ]
    expect(renderedComposerSubmission('claude', '$brainstroming explain', commands)).toEqual({
      text: '[mandatory! open skill: /skills/Brain Stroming/SKILL.md] explain',
      warnings: [],
    })
    expect(renderedComposerSubmission('openCode', '/review $brainstroming', commands)).toEqual({
      text: 'Review [mandatory! open skill: /skills/Brain Stroming/SKILL.md]',
      warnings: [],
    })
  })

  test('warns when a failed catalog refresh leaves a stale mention unresolved', () => {
    const rendered = renderedComposerSubmission(
      'codex',
      '$missing',
      [{
        name: 'deploy',
        description: '',
        scope: 'Skill',
        argument_hint: null,
        template: null,
        skill_file: '/remote/.agents/skills/deploy/SKILL.md',
      }],
    )
    expect(
      skillSubmissionWarnings('$missing', rendered.text, rendered.warnings, true),
    ).toEqual([SKILL_CATALOG_UNAVAILABLE_WARNING])
  })

  test('returns invalid-path warnings without changing visible text', () => {
    const commands = [skill('brainstroming', '/skills/bad]/SKILL.md')]
    const rendered = renderedComposerSubmission('codex', '$brainstroming', commands)
    expect(rendered.text).toBe('$brainstroming')
    expect(rendered.warnings).toHaveLength(1)
  })
})

function command(
  name: string,
  scope: SlashCommand['scope'],
  description: string,
  template: string | null,
): SlashCommand {
  return { name, scope, description, template, argument_hint: null }
}

function skill(
  name: string,
  skillFile: string,
  description = '',
): SlashCommand & SkillCommand {
  return {
    name,
    scope: 'Skill',
    description,
    argument_hint: null,
    template: null,
    skill_file: skillFile,
  }
}

describe('parseGoalSubmission', () => {
  const builtin: SlashCommand = {
    name: 'goal',
    description: '',
    scope: 'Builtin',
    argument_hint: null,
    template: null,
  }

  test('parses each goal intent', () => {
    const parse = (prompt: string) => parseGoalSubmission('codex', prompt, [builtin])
    expect(parse('/goal')).toEqual({ kind: 'show' })
    expect(parse('/goal ')).toEqual({ kind: 'show' })
    expect(parse('/goal edit')).toEqual({ kind: 'edit' })
    expect(parse('/goal pause')).toEqual({ kind: 'pause' })
    expect(parse('/goal resume')).toEqual({ kind: 'resume' })
    expect(parse('/goal clear')).toEqual({ kind: 'clear' })
    expect(parse('/goal improve benchmark coverage')).toEqual({
      kind: 'set',
      objective: 'improve benchmark coverage',
    })
    expect(parse('/goals')).toBeNull()
    expect(parse('ship /goal')).toBeNull()
  })

  test('is codex-only and respects command overrides', () => {
    expect(parseGoalSubmission('claude', '/goal', [builtin])).toBeNull()
    const projectOwned: SlashCommand = {
      ...builtin,
      scope: 'Project',
      template: 'do project things',
    }
    expect(parseGoalSubmission('codex', '/goal', [projectOwned])).toBeNull()
  })
})
