import type { AgentInfo, On, PromptOrigin, ToolInfo } from 'claude-code'

/**
 * Shared scaffolding for the mod's tests, and the three things about the
 * test kit that are not obvious from the contract.
 *
 * **1. `$.state` cannot be read from a test.** The host refuses a `$` call
 * the caller's own source scan does not list, so `$.state.get` fails even
 * from a bottom hook registered by the test — the test is its own tiny
 * plugin and its scan lists nothing. So nothing here asserts on an atom
 * directly. Anything held in `$.state` is read where it is load-bearing:
 * in the JSON the mod hands the binary. `lastPayload` is that reader, and
 * `bottom` is what makes it possible, by standing in as the binary and
 * echoing its stdin straight back as its stdout.
 *
 * **2. Bottom-hook results are wrapped.** A `tool.call` answers
 * `{ result }`; every other noun call answers `{ value }`, which is what
 * its `next(e)` resolves to. Returning the bare value is reported as
 * "returned something that is not a result object".
 *
 * **3. `process.run`'s stdin is at `e.init.stdin`**, the event's input
 * being `{ argv, init? }`, and a `ProcessRunResult` must carry
 * `isStdoutTruncated` and `isStderrTruncated` as well as the three obvious
 * fields. Note also that `Glob` is not a built-in tool name on this build,
 * so a test that wants a second tool should reach for `Bash`.
 *
 * (A fourth, for whoever edits `../types/index.d.ts`: a state contract must
 * be self-contained. `claude plugin validate` refuses one that imports, so
 * `SpanStyle` is spelled out there rather than taken from `TextProps`.)
 */

/** What the bottom's own band draws, so a test can tell the mod yielded. */
export const BENEATH = 'drawn-beneath-the-mod'

/** One `$.process.run` the mod made: what it asked for, and on what stdin. */
export type Spawn = { argv: readonly string[]; stdin: string }

/** What `bottom` hands back, for reading the mod's side of the exchange. */
export type Recorder = {
  /** Every spawn the mod has made, in order. */
  spawns: Spawn[]
  /** The payload of the most recent spawn, parsed. */
  lastPayload: () => PayloadShape
  /** The payload of the most recent spawn carrying `--layout`, parsed. */
  lastPanePayload: () => PayloadShape
  /** Every band payload, parsed, in order — for asserting across refreshes. */
  payloads: () => PayloadShape[]
  /** Moves the fake clock on, so the 400ms refresh gate opens. */
  advance: (ms: number) => void
}

/**
 * The payload as the tests read it: the fields they assert on, loosely
 * typed, since the thing under test is `buildPayload`'s output rather than
 * this declaration.
 */
export type PayloadShape = {
  session_id?: string
  model?: { display_name?: string }
  output_style?: { name?: string }
  version?: string
  effort?: { level?: string }
  cwd?: string
  workspace?: { current_dir?: string }
  context_window?: {
    context_window_size?: number
    used_percentage?: number
    current_usage?: Record<string, number>
  }
  cost?: { total_cost_usd?: number }
  rate_limits?: Record<string, { used_percentage?: number; resets_at?: number }>
  env?: {
    mcp_count?: number
    claude_md_count?: number
    rules_count?: number
    memory_count?: number
  }
  activity?: {
    tools?: { id: string; name: string; target: string | null }[]
    completed_counts?: { name: string; count: number; failed: number }[]
    completed_total?: number
    failed_total?: number
    agents?: {
      id: string
      description?: string
      agent_type?: string
      started_at?: number
      completed_at?: number | null
    }[]
    compact_count?: number
    last_api_error_ms?: number
    todo?: {
      text?: string
      pending?: number
      completed?: number
      total?: number
      in_progress_items?: { text: string }[]
      all_done?: boolean
      is_task_api?: boolean
    } | null
  }
}

/** What the fake engine answers for `$.session.usage()`. */
export type UsageOptions = {
  window?: number
  percent?: number
  rateLimits?: { kind: string; percentUsed: number; resetsAt?: string }[]
  cost?: { usd: number } | undefined
}

export type BottomOptions = {
  /** Tools `$.tool.list()` answers; `mcp: true` ones feed the MCP count. */
  tools?: ToolInfo[]
  /** Agents `$.agent.list()` answers. */
  agents?: AgentInfo[]
  usage?: UsageOptions
  model?: string
  sessionId?: string
  /** Milliseconds the clock moves on each read; 1000 opens the 400ms gate. */
  tick?: number
  /** Where the clock starts. */
  startMs?: number
  /** Rows the stand-in binary answers with, instead of echoing the payload. */
  stdout?: string
  /**
   * What the tool beneath answers. `on` throws on a repeat registration, so
   * a test that wants a failing tool asks for it here rather than
   * registering `tool.call` a second time of its own.
   */
  toolResult?: { result?: unknown; isError?: true; isReadOnly?: true }
}

/**
 * Registers everything the mod asks the engine for, beneath the mod.
 *
 * The stand-in binary echoes the payload it was handed, so a test can read
 * the mod's whole accumulated state off `lastPayload()` — see note 1 above.
 */
export function bottom(on: On, options: BottomOptions = {}): Recorder {
  const spawns: Spawn[] = []
  const tick = options.tick ?? 1_000
  let nowMs = options.startMs ?? 1_700_000_000_000

  on('tool.call', () => options.toolResult ?? { result: 'ok', isReadOnly: true })
  on('clock.now', () => ({ value: (nowMs += tick) }))
  // The classic events the mod observes need a bottom of their own; each
  // answers with an empty classic result, which is "nothing to add".
  on('classic.PostToolUse', () => ({}))
  on('classic.InstructionsLoaded', () => ({}))
  on('classic.ConfigChange', () => ({}))
  on('classic.SubagentStart', () => ({}))
  on('classic.SubagentStop', () => ({}))
  on('classic.PreCompact', () => ({}))
  on('session.usage', () => ({
    value: {
      startedAt: 1_699_000_000_000,
      context: {
        window: options.usage?.window ?? 1_000_000,
        percent: options.usage?.percent ?? 43,
      },
      rateLimits: options.usage?.rateLimits ?? [],
      cost: 'cost' in (options.usage ?? {}) ? options.usage?.cost : { usd: 1.53 },
    },
  }))
  on('session.model', () => ({ value: options.model ?? 'claude-opus-5[1m]' }))
  on('session.id', () => ({ value: options.sessionId ?? 'session-under-test' }))
  on('agent.list', () => ({ value: options.agents ?? [] }))
  on('tool.list', () => ({ value: options.tools ?? [] }))
  on('process.run', ($, e) => {
    const stdin = String(e.init?.stdin ?? '')
    spawns.push({ argv: [...e.argv], stdin })
    return {
      value: {
        exitCode: 0,
        // The version probe asks the CLI; every other spawn is the binary.
        stdout: e.argv[1] === '--version' ? '2.1.296 (Claude Code)' : (options.stdout ?? stdin),
        stderr: '',
        isStdoutTruncated: false,
        isStderrTruncated: false,
      },
    }
  })
  // The mod pins a status line and logs; nothing beneath answers either by
  // default, and an unanswered one is only dropped, but answering keeps the
  // runner's report clean.
  on('ui.status', () => ({ value: undefined }))
  on('ui.log', () => ({ value: undefined }))
  // The pane. Without a `ui.open` beneath, the mod's `command.run` hook
  // throws, is skipped, and the dispatch falls through to a `command.run`
  // nothing answers — which reads as the wrong failure entirely.
  on('ui.open', () => ({ value: { isPlaced: true } }))
  on('ui.close', () => ({ value: undefined }))
  on('command.register', (_engine, e) => ({ value: { command: e.name } }))
  // Like `ui.render`, a `command.run` answers with its own result rather
  // than a `{ value }` envelope.
  on('command.run', () => ({ text: undefined }))
  // Where the mod yields the band (a survey owns it), nothing beneath
  // draws, and `ui.mount` fails on an unanswered `ui.render`. A tree is
  // plain data, so a bottom can hand one over without `$.ui.resolve` —
  // which a test could not call anyway, the host allowing only the `$`
  // calls the caller's own scan lists.
  // Unlike a noun call, a render hook's result IS the tree — not `{ value }`.
  // And `children` is the element's own field, never a prop: a tree with
  // `props.children` is refused ("Text prop \"children\" is not allowed"),
  // which is the same rule that makes `<Text {...span} />` drop its text.
  on('ui.render', () => ({ type: 'Text', props: {}, children: [BENEATH] }))

  const binaryRuns = () => spawns.filter(run => run.argv[1] !== '--version')

  const parse = (run: Spawn | undefined, what: string): PayloadShape => {
    if (run === undefined) throw new Error(`the binary was never run ${what}`)
    return JSON.parse(run.stdin) as PayloadShape
  }

  return {
    spawns,
    lastPayload: () => parse(binaryRuns().at(-1), 'for the band'),
    payloads: () =>
      binaryRuns()
        .filter(run => !run.argv.includes('--layout'))
        .map(run => JSON.parse(run.stdin) as PayloadShape),
    lastPanePayload: () =>
      parse(
        binaryRuns()
          .filter(run => run.argv.includes('--layout'))
          .at(-1),
        'for the pane',
      ),
    advance: ms => {
      nowMs += ms
    },
  }
}

/** The `AbovePrompt` band's props, with room for four rows. */
export const BAND = {
  plugin: 'pulseline-mod',
  surface: 'terminal',
  component: 'AbovePrompt',
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 4,
    bodyColumns: 120,
    scroll: { offset: 0, bodyRows: 4 },
    view: {},
  },
} as const

/** The pane's props, at the twelve rows `$.ui.open` asks for. */
export const PANE = {
  plugin: 'pulseline-mod',
  surface: 'terminal',
  component: 'Pane',
  requestId: 'pulseline',
  props: {
    title: 'pulseline',
    isFocused: true,
    placement: 'dock',
    bodyColumns: 120,
    scroll: { offset: 0, bodyRows: 12 },
    view: {},
  },
} as const

/** What `$.command.run` takes as the person's own invocation. */
export const USER_COMMAND: { args: string; origin: PromptOrigin; presentation: { isFullscreen: boolean; columns: number } } = {
  args: '',
  origin: { kind: 'composer' },
  presentation: { isFullscreen: false, columns: 120 },
}

/** A line of real statusline output: 256-colour foreground, bold, reset. */
const ESC = String.fromCharCode(27)
export const COLOURED_ROWS = [
  `${ESC}[1m${ESC}[38;5;39mM:Opus 5${ESC}[0m | ${ESC}[38;5;244mCTX:43%${ESC}[0m`,
  `${ESC}[38;5;196m!2${ESC}[0m +3`,
  'T:Read: .../main.rs',
  'A:Explore: mapping',
].join('\n')
