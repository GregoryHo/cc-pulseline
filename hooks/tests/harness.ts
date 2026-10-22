import type { AgentInfo, On, PromptOrigin, ToolInfo } from 'claude-code'
import { mock } from 'claude-code/testing'

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

/**
 * Where `bottom`'s clock sits, and so the `last_completed_at` every tool
 * count carries: the clock stands still unless a case moves it.
 */
export const STAMP = 1_700_000_000_000

/** What the bottom's own band draws, so a test can tell the mod yielded. */
export const BENEATH = 'drawn-beneath-the-mod'

/** One `$.process.run` the mod made: what it asked for, and on what stdin. */
export type Spawn = { argv: readonly string[]; stdin: string }

/**
 * Whether a spawn was the pane's rather than the band's.
 *
 * Both carry `--layout` now — the band always asks for `activity`, the one
 * layout that draws no identity and no budget row — so presence alone no
 * longer tells them apart and the value has to be read. The pane's switcher
 * offers `LAYOUTS` and `activity` is not among them, which is what keeps this
 * unambiguous.
 */
export const isPaneSpawn = (run: Spawn): boolean => {
  const at = run.argv.indexOf('--layout')
  return at >= 0 && run.argv[at + 1] !== 'activity' && !isDesktopBandSpawn(run)
}

/** The layouts the `desktopLayout` setting offers, mirrored from the mod. */
const DESKTOP_LAYOUTS = ['compact', 'rail', 'anchor']

/**
 * Whether a spawn was the desktop band's: a desktop layout without Nerd Font
 * glyphs.
 *
 * The pane offers `compact` and `rail` too, and adds `--no-icons` whenever the
 * desktop is attached, so a desktop case that opens the pane must not switch
 * it to the layout the desktop band draws — the two would read alike here.
 */
export const isDesktopBandSpawn = (run: Spawn): boolean => {
  const at = run.argv.indexOf('--layout')
  return (
    at >= 0 &&
    DESKTOP_LAYOUTS.includes(run.argv[at + 1] ?? '') &&
    run.argv.includes('--no-icons')
  )
}

/** What `bottom` hands back, for reading the mod's side of the exchange. */
export type Recorder = {
  /** Every spawn the mod has made, in order. */
  spawns: Spawn[]
  /** The payload of the most recent spawn, parsed. */
  lastPayload: () => PayloadShape
  /** The payload of the most recent pane spawn, parsed. */
  lastPanePayload: () => PayloadShape
  /** Every band payload, parsed, in order — for asserting across refreshes. */
  payloads: () => PayloadShape[]
  /**
   * Each candidate the binary ladder probed with `--version`, in order.
   * Claude Code's own version probe is not one of these.
   */
  probes: string[]
  /** Moves the fake clock on, so the 400ms refresh gate opens. */
  advance: (ms: number) => Promise<void>
  /** Lets work the mod started unawaited run to its end, the clock where it is. */
  settle: () => Promise<void>
  /** Moves the session's project root, as `/cd` or a worktree move would. */
  moveRoot: (to: string) => void
  /**
   * Every `$.ui.status` and `$.ui.log` the mod made, in order.
   *
   * A plugin a person installs should make none: a status entry sits beside
   * their prompt and a log line lands in their transcript, and what the band
   * cannot draw the band itself says.
   */
  notices: string[]
  /** Every `$.ui.notify` the mod made, by its text. */
  notifications: string[]
  /** Every `$.ui.toast` the mod made, by its text. */
  toasts: string[]
  /** Every `$.prompt.fill` the mod made, by its text. */
  fills: string[]
  /** What `$.store` holds now, by key. */
  store: Map<string, unknown>
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
    agents_md_count?: number
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
    cache_read_total?: number
    todo?: {
      text?: string
      pending?: number
      completed?: number
      total?: number
      in_progress_items?: { text: string; started_at?: number }[]
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
  /**
   * What `$.ui.open` answers: `false` is a surface that would not place the
   * pane, which is the only way a test reaches that branch — the kit's own
   * bottom always places it, and a second `on('ui.open')` is refused.
   */
  panePlaced?: boolean
  /** Tools `$.tool.list()` answers; `mcp: true` ones feed the MCP count. */
  tools?: ToolInfo[]
  /** Agents `$.agent.list()` answers. */
  agents?: AgentInfo[]
  usage?: UsageOptions
  model?: string
  sessionId?: string
  /** The directory the session's project root starts in. */
  root?: string
  /** Where the clock starts. */
  startMs?: number
  /**
   * What `$.session.surfaces()` answers; `['terminal']` when not given. The
   * mod runs the desktop band only when `desktop` is among them.
   */
  surfaces?: readonly ('terminal' | 'desktop' | 'mobile' | 'vscode')[]
  /**
   * What `$.session.version()` answers; `2.1.299` when not given. `null`
   * makes the call throw, so the mod falls back to asking the CLI.
   */
  engineVersion?: string | null
  /** Rows the stand-in binary answers with, instead of echoing the payload. */
  stdout?: string
  /**
   * Milliseconds a render waits on the clock before answering, so a case can
   * hold a draw open and see what the rest of the mod does meanwhile. The
   * spawn is recorded as it is made, before the wait.
   */
  renderDelayMs?: number
  /** Exit code the stand-in binary answers a render with; 0 when not given. */
  exitCode?: number
  /** What the stand-in binary writes to stderr on a render. */
  stderr?: string
  /**
   * What the tool beneath answers. `on` throws on a repeat registration, so
   * a test that wants a failing tool asks for it here rather than
   * registering `tool.call` a second time of its own.
   */
  toolResult?:
    | { result?: unknown; isError?: true; isReadOnly?: true }
    | { deny: string }
  /**
   * Makes the tool beneath reject rather than answer, which is how a call
   * that never completes ends: the person interrupts and `next.signal`
   * aborts, and the hook above goes on running for `lingerMs` with its
   * `next(e)` rejected. The band draws between turns now, so a tool left
   * behind in the running list would be drawn as running for the rest of
   * the session — hence a case that proves it is not.
   */
  toolThrows?: string
  /**
   * What a probed candidate answers `--version` with.
   *
   * A version string means every candidate is a working binary of that
   * version, so the first rung probed wins. `null` means every candidate
   * fails, as a missing file does, and the ladder runs out.
   */
  binaryVersion?: string | null
  /**
   * Candidate paths that answer; every other one fails. Left out, all of
   * them answer. Use it to make a particular rung the one that hits.
   */
  binaryAt?: readonly string[]
  /**
   * A version per candidate path, for a ladder whose rungs disagree — which
   * is the real shape of this machine, where `cargo install` left 1.4.0 on
   * PATH while the dev build is current. Given, a path absent from it fails
   * as a missing file does.
   */
  binaryVersions?: Readonly<Record<string, string | null>>
  /** What `$.env.get('HOME')` answers, for the installed rung's path. */
  home?: string
  /**
   * Whether `$.ui.notify` reports its notification sent; `true` when not
   * given. `false` is a session with no channel, as one the desktop hosts
   * alone can be.
   */
  notifySent?: boolean
  /** The draft `$.prompt.read()` answers; empty when not given. */
  draft?: string
  /**
   * What `$.store` holds at the start: other sessions' spends, say. Empty
   * when not given. A case reads it back off the recorder's `store`.
   */
  store?: Readonly<Record<string, unknown>>
}

/**
 * Registers everything the mod asks the engine for, beneath the mod.
 *
 * The stand-in binary echoes the payload it was handed, so a test can read
 * the mod's whole accumulated state off `lastPayload()` — see note 1 above.
 */
export function bottom(on: On, options: BottomOptions = {}): Recorder {
  const spawns: Spawn[] = []
  const probes: string[] = []
  const notices: string[] = []
  const notifications: string[] = []
  const toasts: string[] = []
  const fills: string[] = []
  const clock = mock.clock(on, { now: options.startMs ?? STAMP })
  // `$.store` in memory. Not `mock.store`, which answers the mod but leaves
  // a case's own `$` with no store to read back from.
  const store = new Map(Object.entries(options.store ?? {}))
  on('store.get', ($, e) => ({ value: store.get(e.key) }))
  on('store.set', ($, e) => {
    store.set(e.key, JSON.parse(JSON.stringify(e.value)))
    return { value: undefined }
  })
  on('store.delete', ($, e) => {
    store.delete(e.key)
    return { value: undefined }
  })
  on('store.keys', () => ({ value: [...store.keys()] }))

  // The Task API's two calls are answered the way the engine answers them,
  // because the mod reads the created task's id out of the *result* rather
  // than counting creations. Ids are `task-1`, `task-2`, … in call order, so
  // a case can name one without reading it back.
  let createdTasks = 0
  on('tool.call', ($, e) => {
    if (options.toolThrows !== undefined) throw new Error(options.toolThrows)
    if (options.toolResult !== undefined) return options.toolResult
    if (e.tool === 'TaskCreate') {
      createdTasks += 1
      return {
        result: { task: { id: `task-${createdTasks}`, subject: String(e.subject ?? '') } },
      }
    }
    if (e.tool === 'TaskUpdate') {
      return {
        result: { success: true, taskId: String(e.taskId ?? ''), updatedFields: [] },
      }
    }
    return { result: 'ok', isReadOnly: true }
  })
  // The classic events the mod observes need a bottom of their own; each
  // answers with an empty classic result, which is "nothing to add".
  on('classic.PostToolUse', () => ({}))
  // A turn's end, where a turn that called no tool carries its effort.
  on('classic.Stop', () => ({}))
  on('classic.InstructionsLoaded', () => ({}))
  on('classic.ConfigChange', () => ({}))
  on('classic.SubagentStart', () => ({}))
  on('classic.SubagentStop', () => ({}))
  // Both ends of a compaction: the mod counts `PostCompact`, and a case
  // drives `PreCompact` to show that asking for one counts nothing.
  on('classic.PreCompact', () => ({}))
  on('classic.PostCompact', () => ({}))
  // A turn's own bottom. Its result is `{ text, usage? }`, not `{ value }`.
  on('turn.complete', () => ({ text: '' }))
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
  // The project root the mod reads before every ladder walk. Mutable, so a
  // case can move it the way `/cd` does.
  let root = options.root ?? '/work'
  on('session.root', () => ({ value: root }))
  on('session.id', () => ({ value: options.sessionId ?? 'session-under-test' }))
  on('session.surfaces', () => ({ value: options.surfaces ?? ['terminal'] }))
  // The engine's own version, deliberately not the CLI probe's `2.1.296`
  // below, so a case can tell which one reached the payload.
  on('session.version', () =>
    options.engineVersion === null
      ? (() => {
          throw new Error('session.version refused')
        })()
      : { value: { version: options.engineVersion ?? '2.1.299', base: '2.1.299' } },
  )
  // Answered as core echoes it, for the case that attaches the desktop late.
  on('session.attach', (_engine, e) => ({ clientId: e.clientId }))
  on('agent.list', () => ({ value: options.agents ?? [] }))
  on('tool.list', () => ({ value: options.tools ?? [] }))
  on('process.run', async ($, e) => {
    const argv = [...e.argv]
    const stdin = String(e.init?.stdin ?? '')
    spawns.push({ argv, stdin })

    const answer = (exitCode: number, stdout: string) => ({
      value: { exitCode, stdout, stderr: '', isStdoutTruncated: false, isStderrTruncated: false },
    })

    // Four different spawns reach here. Claude Code's own version probe,
    // which asks the CLI by name:
    if (argv[0] === 'claude') return answer(0, '2.1.296 (Claude Code)')

    // The binary ladder, probing one candidate:
    if (argv[1] === '--version') {
      const path = argv[0] ?? ''
      probes.push(path)
      const version =
        options.binaryVersions !== undefined
          ? options.binaryVersions[path]
          : 'binaryVersion' in options
            ? options.binaryVersion
            : '1.7.0'
      const present = options.binaryAt === undefined || options.binaryAt.includes(path)
      if (version === null || version === undefined || !present) {
        // As a missing file does: nothing on stdout and a non-zero status.
        return answer(127, '')
      }
      return answer(0, `cc-pulseline ${version}`)
    }

    // And a real render, which is handed the payload on stdin.
    if (options.renderDelayMs !== undefined) await clock.sleep(options.renderDelayMs)
    if (options.exitCode !== undefined && options.exitCode !== 0) {
      return {
        value: {
          exitCode: options.exitCode,
          stdout: '',
          stderr: options.stderr ?? '',
          isStdoutTruncated: false,
          isStderrTruncated: false,
        },
      }
    }
    return answer(0, options.stdout ?? stdin)
  })
  // The mod pins a status line and logs; nothing beneath answers either by
  // default, and an unanswered one is only dropped, but answering keeps the
  // runner's report clean.
  // The ladder asks for HOME to build the installed path; Claude Code's own
  // version probe asks for CLAUDE_CODE_EXECPATH and must not find one, so
  // that path falls through to asking the CLI.
  on('env.get', (_engine, e) => ({
    value: e.name === 'HOME' ? (options.home ?? '/home/under-test') : undefined,
  }))
  on('ui.status', (_engine, e) => {
    notices.push(`status: ${String(e.text ?? '')}`)
    return { value: undefined }
  })
  on('ui.log', (_engine, e) => {
    notices.push(`log: ${String(e.text ?? '')}`)
    return { value: undefined }
  })
  // The quiet desktop band's notifications and its `/compact` button.
  on('ui.notify', (_engine, e) => {
    notifications.push(String(e.text ?? ''))
    return {
      value:
        options.notifySent === false
          ? { isSent: false as const, reason: 'no-surface' as const }
          : { isSent: true as const, channel: 'terminal_bell' as const },
    }
  })
  on('ui.toast', (_engine, e) => {
    toasts.push(String(e.text ?? ''))
    return { value: undefined }
  })
  on('prompt.read', () => {
    const text = options.draft ?? ''
    return { value: { text, cursor: text.length } }
  })
  // An event, like `ui.render`: its result is the answer itself, not `{ value }`.
  on('prompt.fill', (_engine, e) => {
    fills.push(String(e.text ?? ''))
    return { isFilled: true }
  })
  // The pane. Without a `ui.open` beneath, the mod's `command.run` hook
  // throws, is skipped, and the dispatch falls through to a `command.run`
  // nothing answers — which reads as the wrong failure entirely.
  on('ui.open', () =>
    options.panePlaced === false
      ? { value: { isPlaced: false as const, reason: 'the surface is too narrow' } }
      : { value: { isPlaced: true as const } },
  )
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

  const binaryRuns = () =>
    spawns.filter(run => run.argv[1] !== '--version')

  const parse = (run: Spawn | undefined, what: string): PayloadShape => {
    if (run === undefined) throw new Error(`the binary was never run ${what}`)
    return JSON.parse(run.stdin) as PayloadShape
  }

  return {
    spawns,
    probes,
    lastPayload: () => parse(binaryRuns().at(-1), 'for the band'),
    payloads: () =>
      binaryRuns()
        .filter(run => !isPaneSpawn(run) && !isDesktopBandSpawn(run))
        .map(run => JSON.parse(run.stdin) as PayloadShape),
    lastPanePayload: () => parse(binaryRuns().filter(isPaneSpawn).at(-1), 'for the pane'),
    advance: ms => clock.advance(ms),
    moveRoot: to => {
      root = to
    },
    notices,
    notifications,
    toasts,
    fills,
    store,
    settle: () => clock.settle(),
  }
}

/** The `AbovePrompt` band's props, with room for four rows. */
/**
 * The band while a turn is running, which is the only time it draws its rows:
 * idle it yields, because the statusline is already drawing the steady state
 * on its own row and the band would draw that same row again.
 */
export const BAND = {
  plugin: 'pulseline',
  surface: 'terminal',
  component: 'AbovePrompt',
  props: {
    hasSurvey: false,
    isWorking: true,
    maxRows: 4,
    bodyColumns: 120,
    scroll: { offset: 0, bodyRows: 4 },
    view: {},
  },
} as const

/** The band as the desktop app raises it. */
export const DESKTOP_BAND = { ...BAND, surface: 'desktop' } as const

/** The same band with nothing running. */
export const IDLE_BAND = {
  ...BAND,
  props: { ...BAND.props, isWorking: false },
} as const

/** The pane's props, at the twelve rows `$.ui.open` asks for. */
export const PANE = {
  plugin: 'pulseline',
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

/** The pane as the desktop app raises it. */
export const DESKTOP_PANE = { ...PANE, surface: 'desktop' } as const

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
