import type {
  ActivityItem,
  ActivityKind,
  AgentSession,
  ProviderResumeCursor,
  ReportedCommand,
  SequencedEvent,
  ThreadGoal,
  TranscriptBlock,
  TurnStatus,
} from './generated'

export interface PendingPermission {
  requestId: string
  title: string
  detail: string
  options: Array<{ id: string; label: string; allow: boolean }>
}

export interface PendingUserInput {
  requestId: string
  questions: Array<{
    id: string
    header: string
    question: string
    options: Array<{ label: string; description?: string }>
    multiSelect: boolean
  }>
}

export interface RuntimeEventResult {
  session: AgentSession
  permission?: PendingPermission | null
  userInput?: PendingUserInput | null
  settled: boolean
  removeRuntime: boolean
  error?: string
}

export interface ReducerClock {
  nowSeconds: () => number
  nowMillis: () => number
  randomUUID: () => string
}

const defaultClock: ReducerClock = {
  nowSeconds: () => Math.floor(Date.now() / 1_000),
  nowMillis: () => Date.now(),
  randomUUID: () => crypto.randomUUID(),
}

export function reduceRuntimeEvent(
  current: AgentSession,
  wire: SequencedEvent,
  clock: ReducerClock = defaultClock,
  processExitError: string | null = null,
): RuntimeEventResult {
  const session = clone(current)
  const { kind, payload } = wire.event
  const result: RuntimeEventResult = {
    session,
    settled: false,
    removeRuntime: false,
  }

  session.runtime_event_cursor = {
    runtime_id: wire.runtimeId,
    epoch: wire.epoch,
    sequence: wire.sequence,
  }

  switch (kind) {
    case 'connected':
      session.provider_cursor = (payload as ProviderResumeCursor | null) ?? null
      if (
        session.provider_cursor?.provider === 'claude'
          && session.provider_cursor.resumeAt
      ) {
        const turn = activeTurn(session)
        if (turn) turn.provider_resume_at = session.provider_cursor.resumeAt
      }
      if (session.status === 'connecting') session.status = 'working'
      break
    case 'agentPresetSelected':
      session.agent_preset = typeof payload === 'string' ? payload : null
      break
    case 'autoTitleUpdated':
      session.auto_title = typeof payload === 'string' ? normalizeSessionTitle(payload) : null
      break
    case 'availableCommands':
      if (Array.isArray(payload)) session.available_commands = payload as ReportedCommand[]
      break
    case 'turnStarted': {
      const turn = activeTurn(session)
      if (turn) {
        turn.provider_turn_started = true
        session.status = 'working'
      } else if (
        session.provider === 'codex'
        && !['connecting', 'working', 'waiting'].includes(session.status)
      ) {
        // Codex starts turns on its own: goal continuation pursues an active
        // goal whenever the thread is idle. Give the turn a transcript home —
        // there is no user message for it — so its work streams in instead of
        // being dropped.
        session.turns.push({
          id: clock.randomUUID(),
          turn_count: session.turns.length + 1,
          status: 'running',
          provider_turn_started: true,
          provider_resume_at: null,
          started_at: clock.nowSeconds(),
          completed_at: null,
          checkpoint: null,
        })
        session.status = 'working'
      }
      break
    }
    case 'textDelta':
      if (typeof payload === 'string' && acceptsTurnOutput(session)) {
        appendText(session, payload, clock)
      }
      break
    case 'reasoningDelta':
      if (typeof payload === 'string' && acceptsTurnOutput(session)) {
        appendReasoning(session, payload, clock)
      }
      break
    case 'activity': {
      const value = asRecord(payload)
      if (!acceptsTurnOutput(session) || !value || typeof value.title !== 'string') break
      upsertActivity(
        session,
        {
          id: clock.randomUUID(),
          source_id: typeof value.id === 'string' ? value.id : null,
          kind: isActivityKind(value.kind) ? value.kind : 'tool',
          title: value.title,
          detail: typeof value.detail === 'string' ? value.detail : null,
          arguments: null,
          output: null,
          image_urls: [],
          failed: false,
          complete: value.complete === true,
          file_changes: [],
          display_target: null,
          display_description: null,
          reasoning: null,
        },
        clock,
      )
      break
    }
    case 'richActivity':
      if (acceptsTurnOutput(session) && asRecord(payload)) {
        upsertActivity(session, payload as ActivityItem, clock)
      }
      break
    case 'permission': {
      const value = asRecord(payload)
      if (!acceptsTurnOutput(session) || !value || typeof value.requestId !== 'string') break
      result.permission = {
        requestId: value.requestId,
        title: typeof value.title === 'string' ? value.title : 'Permission required',
        detail: typeof value.detail === 'string' ? value.detail : '',
        options: Array.isArray(value.options)
          ? value.options.filter(isPermissionOption)
          : [],
      }
      session.status = 'waiting'
      break
    }
    case 'userInputRequested': {
      const value = asRecord(payload)
      if (!acceptsTurnOutput(session) || !value || typeof value.requestId !== 'string') break
      const questions = Array.isArray(value.questions)
        ? value.questions.map(asUserInputQuestion).filter((question) => question !== null)
        : []
      if (!questions.length) break
      result.userInput = { requestId: value.requestId, questions }
      session.status = 'waiting'
      break
    }
    case 'usageUpdated': {
      const value = asRecord(payload)
      if (!value) break
      const previous = session.context_usage ?? { tokens: 0, window: null }
      session.context_usage = {
        tokens:
          typeof value.contextTokens === 'number' ? value.contextTokens : previous.tokens,
        window:
          typeof value.contextWindow === 'number'
            ? value.contextWindow
            : previous.window,
      }
      break
    }
    case 'goalUpdated': {
      // Conversation meta like usage: it applies regardless of turn state,
      // and `null` means the provider cleared the goal.
      const goal = asThreadGoal(payload)
      if (goal && session.messages.length === 0) {
        // A goal-first task is named after its objective until the provider
        // reports a better title.
        setTitleFromPrompt(session, goal.objective)
      }
      session.thread_goal = goal
      break
    }
    case 'turnFinished': {
      const value = asRecord(payload)
      const success = value?.success === true
      result.settled = settleTurn(
        session,
        success ? 'completed' : 'failed',
        typeof value?.summary === 'string' ? value.summary : null,
        clock,
      )
      result.permission = null
      result.userInput = null
      break
    }
    case 'error': {
      if (typeof payload !== 'string') break
      result.error = payload
      // An optimistic pursuit turn has no submission to fail with. Unwind it
      // so the error cannot strand a spinner; if the pursuit does start
      // later, its own start report recreates the turn.
      const pursuit = session.turns.at(-1)
      if (
        pursuit && pursuit.status === 'running'
        && !pursuit.provider_turn_started
        && !session.messages.some((message) => message.turn_id === pursuit.id)
      ) {
        session.turns.pop()
        if (['connecting', 'working', 'waiting'].includes(session.status)) {
          session.status = 'idle'
        }
        break
      }
      const turn = activeTurn(session)
      if (!turn || session.status === 'working') break
      const hasAssistant = session.messages.some(
        (message) => message.turn_id === turn.id && message.role === 'assistant',
      )
      session.status = 'failed'
      if (!hasAssistant) {
        session.messages.push({
          id: clock.randomUUID(),
          turn_id: turn.id,
          role: 'assistant',
          content: payload,
          created_at: clock.nowSeconds(),
          streaming: false,
        })
      }
      break
    }
    case 'processExited':
      result.settled = settleTurn(
        session,
        'failed',
        processExitError ?? 'The agent exited before responding.',
        clock,
      )
      result.permission = null
      result.userInput = null
      result.removeRuntime = true
      break
    default:
      break
  }

  session.updated_at = clock.nowSeconds()
  return result
}

function asUserInputQuestion(value: unknown): PendingUserInput['questions'][number] | null {
  const question = asRecord(value)
  if (!question || typeof question.id !== 'string' || typeof question.question !== 'string') {
    return null
  }
  return {
    id: question.id,
    header: typeof question.header === 'string' ? question.header : 'Question',
    question: question.question,
    options: Array.isArray(question.options)
      ? question.options.flatMap((value) => {
          const option = asRecord(value)
          return option && typeof option.label === 'string'
            ? [{
                label: option.label,
                ...(typeof option.description === 'string'
                  ? { description: option.description }
                  : {}),
              }]
            : []
        })
      : [],
    multiSelect: question.multiSelect === true,
  }
}

function appendText(session: AgentSession, delta: string, clock: ReducerClock) {
  if (!delta) return
  completeReasoning(session)
  const previous = session.messages.at(-1)
  if (previous?.role === 'assistant' && previous.streaming) {
    previous.content += delta
  } else {
    session.messages.push({
      id: clock.randomUUID(),
      turn_id: activeTurn(session)?.id ?? null,
      role: 'assistant',
      content: delta,
      created_at: clock.nowSeconds(),
      streaming: true,
    })
  }
}

function appendReasoning(session: AgentSession, delta: string, clock: ReducerClock) {
  if (!delta.trim() && !lastReasoning(session)) return
  finishStreamingMessages(session)
  const existing = lastReasoning(session)
  if (existing && !existing.activity.complete) {
    existing.activity.reasoning!.content += delta
    existing.activity.reasoning!.finished_at_ms = clock.nowMillis()
    return
  }
  const now = clock.nowMillis()
  pushActivity(session, {
    id: clock.randomUUID(),
    source_id: null,
    kind: 'reasoning',
    title: 'Reasoning',
    detail: null,
    arguments: null,
    output: null,
    image_urls: [],
    failed: false,
    complete: false,
    file_changes: [],
    display_target: null,
    display_description: null,
    reasoning: { content: delta, started_at_ms: now, finished_at_ms: now },
  })
}

function upsertActivity(
  session: AgentSession,
  incoming: ActivityItem,
  _clock: ReducerClock,
) {
  finishStreamingMessages(session)
  completeReasoning(session)
  for (const block of [...session.transcript_blocks].reverse()) {
    const activities = ensureActivities(block)
    const matching = [...activities].reverse().find((activity) =>
      incoming.source_id
        ? activity.source_id === incoming.source_id
        : activity.title === incoming.title && !activity.complete,
    )
    if (!matching) continue
    Object.assign(matching, {
      ...incoming,
      id: matching.id,
      detail: incoming.detail ?? matching.detail,
      arguments: incoming.arguments ?? matching.arguments,
      output: incoming.output ?? matching.output,
      image_urls: incoming.image_urls?.length ? incoming.image_urls : matching.image_urls,
      file_changes: incoming.file_changes?.length
        ? incoming.file_changes
        : matching.file_changes,
      display_target: incoming.display_target ?? matching.display_target,
      display_description: incoming.display_description ?? matching.display_description,
      reasoning: incoming.reasoning ?? matching.reasoning,
    })
    return
  }
  pushActivity(session, incoming)
}

function pushActivity(session: AgentSession, activity: ActivityItem) {
  const afterMessage = session.messages.length
  const turnId = activeTurn(session)?.id ?? null
  const last = session.transcript_blocks.at(-1)
  if (last && last.after_message === afterMessage && last.turn_id === turnId) {
    ensureActivities(last).push(activity)
    return
  }
  session.transcript_blocks.push({
    after_message: afterMessage,
    turn_id: turnId,
    content: { kind: 'activities', data: [activity] },
  })
}

function settleTurn(
  session: AgentSession,
  status: TurnStatus,
  fallback: string | null,
  clock: ReducerClock,
): boolean {
  finishStreamingMessages(session)
  completeActivities(session)
  const turn = activeTurn(session)
  if (!turn) return false
  const hasAssistant = session.messages.some(
    (message) => message.turn_id === turn.id && message.role === 'assistant',
  )
  if (!hasAssistant) {
    session.messages.push({
      id: clock.randomUUID(),
      turn_id: turn.id,
      role: 'assistant',
      content:
        fallback ??
        (status === 'completed'
          ? 'The turn completed without a text response.'
          : 'The turn stopped before a response.'),
      created_at: clock.nowSeconds(),
      streaming: false,
    })
  }
  turn.status = status
  turn.completed_at = clock.nowSeconds()
  session.last_reply_at = turn.completed_at
  session.status = status === 'completed' ? 'idle' : 'failed'
  return true
}

function finishStreamingMessages(session: AgentSession) {
  for (const message of session.messages) {
    if (message.role === 'assistant') message.streaming = false
  }
}

function completeReasoning(session: AgentSession) {
  const reasoning = lastReasoning(session)
  if (reasoning) reasoning.activity.complete = true
}

function completeActivities(session: AgentSession) {
  for (const block of session.transcript_blocks) {
    for (const activity of ensureActivities(block)) activity.complete = true
  }
}

function lastReasoning(session: AgentSession) {
  const block = session.transcript_blocks.at(-1)
  const activity = block ? ensureActivities(block).at(-1) : undefined
  return activity?.reasoning ? { activity } : null
}

export function activitiesForBlock(block: TranscriptBlock): ActivityItem[] {
  if (block.content.kind === 'activities') return block.content.data
  const reasoning = block.content.data
  return [
    {
      id: `legacy-reasoning-${block.after_message}`,
      source_id: null,
      kind: 'reasoning',
      title: 'Reasoning',
      detail: null,
      arguments: null,
      output: null,
      image_urls: [],
      failed: false,
      complete: true,
      file_changes: [],
      display_target: null,
      display_description: null,
      reasoning,
    },
  ]
}

function ensureActivities(block: TranscriptBlock): ActivityItem[] {
  if (block.content.kind === 'activities') return block.content.data
  const activities = activitiesForBlock(block)
  block.content = { kind: 'activities', data: activities }
  return activities
}

function asThreadGoal(payload: unknown): ThreadGoal | null {
  const value = asRecord(payload)
  if (!value || typeof value.objective !== 'string' || typeof value.status !== 'string') {
    return null
  }
  return value as unknown as ThreadGoal
}

/** Mirror of the desktop's prompt-derived title fallback: normalized and
 * humanized first words, ellipsized at 54 characters, applied only while
 * the task is unnamed. */
function setTitleFromPrompt(session: AgentSession, prompt: string) {
  if (session.messages.length > 0 || session.title !== 'New task' || session.auto_title) return
  const title = promptFallbackTitle(prompt)
  if (!title) return
  session.auto_title = title
}

/** Maximum characters kept for a provider-supplied automatic title. */
export const AUTO_TITLE_MAX_CHARS = 80
/** Words kept for the local prompt-derived fallback title. */
export const PROMPT_TITLE_WORDS = 7
/** Characters kept for the local prompt-derived fallback title. */
export const PROMPT_TITLE_MAX_CHARS = 54

/**
 * Normalizes an automatically generated session title: trims, decodes a JSON
 * `{title}` envelope, keeps the first meaningful line, strips markdown
 * (fences, inline code, emphasis, links) and a `Title:` prefix, humanizes
 * code-like tokens (`snake_case`, `kebab-case`, `camelCase`, file paths),
 * and caps the result at 80 characters. Returns `null` when nothing
 * title-worthy remains, including provider placeholders such as
 * `New session - …`. Mirrors `normalize_session_title` in
 * `crates/padu-protocol/src/model.rs`.
 */
export function normalizeSessionTitle(raw: string): string | null {
  const trimmed = raw.trim()
  if (!trimmed) return null
  let candidate = trimmed
  if (candidate.startsWith('{') || candidate.startsWith('"')) {
    try {
      const value: unknown = JSON.parse(candidate)
      if (typeof value === 'string') {
        candidate = value
      } else if (value && typeof value === 'object' && typeof (value as Record<string, unknown>).title === 'string') {
        candidate = (value as Record<string, unknown>).title as string
      }
    } catch {
      // Not JSON — treat the raw text as the title.
    }
  }
  let line: string | null = null
  for (const entry of candidate.split('\n')) {
    const text = entry.trim()
    if (!text || text.startsWith('```') || text.toLowerCase() === 'json') continue
    line = text
    break
  }
  if (line === null) return null
  // Provider placeholders must not replace the local prompt fallback. Check
  // before humanization turns `New session - …` into `New session …`.
  const loweredLine = line.toLowerCase()
  if (loweredLine === 'new task' || loweredLine === 'new session') return null
  for (const prefix of ['new session -', 'new session:']) {
    if (loweredLine.startsWith(prefix)) {
      const rest = line
        .slice(prefix.length)
        .trim()
        .replace(/^[-:]+/, '')
        .trim()
      if (!rest) return null
      line = rest
      break
    }
  }
  const title = cleanAndHumanizeTitle(line)
  if (!title) return null
  const lowered = title.toLowerCase()
  if (lowered === 'new task' || lowered === 'new session') return null
  if (![...title].some((character) => /[\p{L}\p{N}]/u.test(character))) return null
  return truncateTitle(title, AUTO_TITLE_MAX_CHARS)
}

/**
 * Derives the local prompt fallback title: the normalized, humanized prompt
 * capped at 7 words and 54 characters. Mirrors `prompt_fallback_title` in
 * `crates/padu-protocol/src/model.rs`.
 */
export function promptFallbackTitle(prompt: string): string | null {
  const cleaned = cleanAndHumanizeTitle(prompt)
  if (!cleaned) return null
  const words = cleaned.split(/\s+/u).filter(Boolean).slice(0, PROMPT_TITLE_WORDS).join(' ')
  if (!words) return null
  if ([...words].length > PROMPT_TITLE_MAX_CHARS) {
    return `${[...words].slice(0, PROMPT_TITLE_MAX_CHARS - 1).join('')}…`
  }
  return words
}

function cleanAndHumanizeTitle(text: string): string | null {
  const withoutFences = text.replaceAll('```', ' ')
  const withoutLinks = stripMarkdownLinks(withoutFences)
  // Drop inline-code, emphasis, and strikethrough markers but keep the
  // words they wrap; underscores and dashes are humanized per token below.
  const withoutMarkers = [...withoutLinks].filter((character) => character !== '`' && character !== '*' && character !== '~').join('')
  let cleaned = withoutMarkers.trim()
  for (const prefix of ['title:', 'session title:']) {
    if (cleaned.length >= prefix.length && cleaned.slice(0, prefix.length).toLowerCase() === prefix) {
      cleaned = cleaned.slice(prefix.length).trim()
      break
    }
  }
  cleaned = stripLeadingListMarkers(cleaned)
  // A leading slash-command (`/fix …`) or mention is an instruction, not
  // part of the name.
  const parts = cleaned.split(/\s+/u).filter(Boolean)
  if (parts.length > 0 && parts[0]!.length > 1 && (parts[0]!.startsWith('/') || parts[0]!.startsWith('@'))) {
    const stripped = parts[0]!.slice(1).trim()
    const rest = parts.slice(1).join(' ')
    cleaned = rest ? `${stripped} ${rest}` : stripped
  }
  const words: string[] = []
  for (const token of cleaned.split(/\s+/u)) {
    humanizeTitleToken(token, words)
  }
  const title = words
    .join(' ')
    .trim()
    .replace(/^["'#_`]+|["'#_`]+$/g, '')
    .replace(/[.,:;]+$/u, '')
    .trim()
  return title || null
}

/** Turns one whitespace-separated token into title words: unwraps quotes and
 * brackets, keeps the last path segment, drops call parens and file
 * extensions, then splits `snake_case`, `kebab-case`, and `camelCase`. */
function humanizeTitleToken(token: string, words: string[]): void {
  let current = token.trim().replace(/^["'`()[\]{}<>]+|["'`()[\]{}<>]+$/g, '').trim()
  if (!current) return
  if ((current.startsWith('/') || current.startsWith('@')) && current.length > 1) {
    current = current.slice(1).trim()
    if (!current) return
  }
  if (current.includes('/') || current.includes('\\')) {
    const segments = current.split(/[/\\]/u).map((segment) => segment.trim()).filter(Boolean)
    current = segments.at(-1) ?? current
  }
  while (current.endsWith('()') && current.length > 2) {
    current = current.slice(0, -2).trim()
  }
  current = current.replace(/^[()[\]{}<>"'`]+|[()[\]{}<>"'`]+$/g, '').trim()
  if (!current) return
  // Drop a trailing file extension (`auth.rs` → `auth`) when the suffix
  // looks like one: short and alphabetic.
  const dot = current.lastIndexOf('.')
  if (dot > 0) {
    const stem = current.slice(0, dot)
    const suffix = current.slice(dot + 1)
    if (
      stem && suffix.length >= 1 && suffix.length <= 4 && /^[A-Za-z]+$/.test(suffix)
      && [...stem].some((character) => /[\p{L}\p{N}]/u.test(character))
    ) {
      current = stem.trim()
    }
  }
  if (!current) return
  const spaced = [...current]
    .map((character) =>
      character === '_' || character === '-' || character === '.' || character === '/' || character === '\\' || character === ':'
        ? ' '
        : character,
    )
    .join('')
  for (const word of splitCamelCase(spaced).split(/\s+/u)) {
    const cleaned = word.trim().replace(/^["'()[\]{}:;,.]+|["'()[\]{}:;,.]+$/g, '').trim()
    if (cleaned) words.push(cleaned)
  }
}

function splitCamelCase(text: string): string {
  const chars = [...text]
  let output = ''
  chars.forEach((character, index) => {
    if (index > 0 && /\p{Lu}/u.test(character)) {
      const previous = chars[index - 1]!
      const next = chars[index + 1]
      const lowerToUpper = (/[\p{Ll}]/u.test(previous) || /\p{N}/u.test(previous))
      const acronymBoundary = /[\p{Lu}]/u.test(previous) && next !== undefined && /[\p{Ll}]/u.test(next)
      if (lowerToUpper || acronymBoundary) output += ' '
    }
    output += character
  })
  return output
}

/** Replaces markdown links and images with their visible text:
 * `[label](url)` → `label`, `![alt](url)` → `alt`. */
function stripMarkdownLinks(text: string): string {
  let output = ''
  let index = 0
  while (index < text.length) {
    const char = text[index]!
    if (char === '[') {
      const close = text.indexOf(']', index)
      if (close !== -1 && text[close + 1] === '(') {
        const end = text.indexOf(')', close + 1)
        if (end !== -1) {
          output += text.slice(index + 1, close)
          index = end + 1
          continue
        }
      }
    }
    if (char === '!' && text[index + 1] === '[') {
      // Handled above through the `[` branch on the next pass; keep
      // the `!` out of the output.
      index += 1
      continue
    }
    output += char
    index += 1
  }
  return output
}

function stripLeadingListMarkers(text: string): string {
  let cleaned = text.trim()
  for (;;) {
    const trimmed = cleaned.trimStart()
    const markerOnly = trimmed.replace(/^[#>»•*\-+❯»]+/u, '')
    if (markerOnly !== trimmed) {
      cleaned = markerOnly.trimStart()
      continue
    }
    const ordered = trimmed.match(/^(\d+)[.)]\s+/u)
    if (ordered) {
      cleaned = trimmed.slice(ordered[0].length)
      continue
    }
    cleaned = trimmed
    break
  }
  return cleaned
}

function truncateTitle(title: string, maxChars: number): string {
  if ([...title].length <= maxChars) return title
  let truncated = [...title].slice(0, maxChars).join('')
  // Avoid leaving a half-word: backtrack to the last space when the cut
  // lands inside one.
  const next = [...title][maxChars]
  const last = [...truncated].at(-1)
  if (next !== undefined && last !== undefined && /[\p{L}\p{N}]/u.test(next) && /[\p{L}\p{N}]/u.test(last)) {
    const space = truncated.lastIndexOf(' ')
    if (space !== -1) truncated = truncated.slice(0, space)
  }
  return truncated.trimEnd()
}

function activeTurn(session: AgentSession) {
  const turn = session.turns.at(-1)
  return turn?.status === 'running' ? turn : undefined
}

function acceptsTurnOutput(session: AgentSession) {
  return Boolean(
    activeTurn(session)
      && ['connecting', 'working', 'waiting'].includes(session.status),
  )
}

function isActivityKind(value: unknown): value is ActivityKind {
  return (
    typeof value === 'string' &&
    [
      'reasoning',
      'command',
      'fileChange',
      'fileRead',
      'fileSearch',
      'fileList',
      'search',
      'plan',
      'tool',
    ].includes(value)
  )
}

function isPermissionOption(
  value: unknown,
): value is { id: string; label: string; allow: boolean } {
  const option = asRecord(value)
  return Boolean(
    option &&
      typeof option.id === 'string' &&
      typeof option.label === 'string' &&
      typeof option.allow === 'boolean',
  )
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}
