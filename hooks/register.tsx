import { atom, read, update } from 'claude-code'
import type { AgentInfo, EngineInterface, PluginOptions, Register, SessionUsage } from 'claude-code'

import {
  PROBE_TIMEOUT_MS,
  candidates,
  formatVersion,
  isNewEnough,
  missingMessage,
  parseVersion,
  tooOldMessage,
} from './binary'
import type { Version } from './binary'
import { buildPayload } from './payload'
import type { ActiveTool } from './payload'
import { shouldRefresh } from './throttle'
import type { BinaryChoice, DrawnOutput, Span, SpanStyle, TaskItem, Tasks, Todo } from '../types'

/**
 * cc-pulseline as a mod, stage 0c.
 *
 * What the first two runs established:
 *   - a user-tier hooks module loads, and `$.process.run` with stdin is
 *     allowed there, so the Rust binary can stay the renderer;
 *   - the `AbovePrompt` band refuses a whole tree if any text child holds a
 *     control character, so the binary's ANSI has to be parsed into Text
 *     props (see `spansOf`), never passed through;
 *   - the band is small outside fullscreen: `maxRows` came back as 2, and a
 *     taller tree scrolls behind a `n more` row.
 *
 * So this revision draws the statusline rows and nothing else, clipped to
 * `maxRows`, and refreshes them when a turn ends rather than only at
 * session start.
 *
 * Every `$` call is written literally: the loader statically scans this
 * source for them, so aliasing one would make it invisible to the scan.
 *
 * Every gating hook carries `.catch(($, e, next) => next(e))`. This mod
 * only observes — it never denies and never rewrites what it is handed — so
 * the right answer when one of its hooks throws or outruns its budget is
 * whatever the chain beneath says. `next` is replay-safe inside a handler:
 * where the hook had already called it, `next(e)` resolves to what that call
 * settled to and nothing beneath runs twice, so a `tool.call` that failed
 * after the tool ran still answers with the tool's real result and never a
 * fabricated one.
 *
 * The budget those handlers guard is the hook's own code: 10s
 * (`HookBudget.ms`), with waits on `next` and on `$` not counted. So the
 * binary's `timeoutMs: 5000` sits outside it — it is a `$.process.run` wait
 * — and only the work around the spawn, chiefly `spansOf` over a few
 * hundred bytes, is ever on the clock.
 */

const PANE_ID = 'pulseline'

/**
 * The layouts the pane's switcher row offers, in order.
 *
 * `console` is left out on purpose: inside a pane it reads as `ledger` with
 * a label column, and four visibly different views are worth more than five
 * where two look alike.
 */
/**
 * Calls that are not tool activity, mirrored from `providers/transcript.rs`.
 *
 * Two groups, excluded there in two different ways. `Agent`, `TaskCreate`,
 * `TaskUpdate` and `TodoWrite` are dispatched and then returned from before
 * the tool tracking, because each belongs somewhere else — the agent list, or
 * the todo row. The rest are that file's `NOISE_TOOLS`, Claude Code's own
 * control calls.
 *
 * Without this the band inflated its completed totals with its own
 * bookkeeping, and an `Agent` call was drawn twice: once as a running tool
 * and once as an agent.
 */
const NOT_TOOL_ACTIVITY = new Set([
  'Agent',
  'TaskCreate',
  'TaskUpdate',
  'TodoWrite',
  'EnterPlanMode',
  'ExitPlanMode',
  'EnterWorktree',
  'ExitWorktree',
  'TaskGet',
  'TaskList',
  'TaskOutput',
  'TaskStop',
  'ToolSearch',
])

const LAYOUTS = ['ledger', 'budgets', 'compact', 'rail']

/** Button key prefix, so a press can name the layout it asks for. */
const LAYOUT_KEY = 'layout:'

/** Set true to draw the band/run diagnostics above the statusline rows. */
const DEBUG = false

/**
 * The session's values, held by the host so a hot reload does not zero them.
 *
 * `plugin` and `key` are source literals on purpose: `claude plugin validate`
 * lists every `$.state` key and holds it to the contract in `../types`, and
 * only a family member's `id` may be computed.
 */
const sessionFacts = atom({ plugin: 'pulseline', key: 'session' } as const, {
  cwd: '',
  version: null,
  effort: null,
  tokens: null,
  compactCount: 0,
  outputStyle: null,
  lastApiErrorMs: null,
  cacheReadTotal: 0,
})

const toolTotals = atom({ plugin: 'pulseline', key: 'tools' } as const, {
  counts: {},
  completedTotal: 0,
  failedTotal: 0,
})

const agentTimings = atom({ plugin: 'pulseline', key: 'agents' } as const, {
  startedAt: {},
  finishedAt: {},
  agentType: {},
})

const instructions = atom({ plugin: 'pulseline', key: 'instructions' } as const, {
  claudeMd: [],
  agentsMd: [],
  rules: [],
  memories: [],
})

const todoList = atom({ plugin: 'pulseline', key: 'todo' } as const, null)

/**
 * The Task API's items, which the todo row is rebuilt from.
 *
 * An atom rather than a closure field for the same reason as the rest: an
 * edit to this module reloads it, and a `TaskUpdate` naming an item created
 * before the reload has to find it.
 */
const taskItems = atom({ plugin: 'pulseline', key: 'tasks' } as const, {
  items: {} as Record<string, TaskItem>,
})

/** The person's own pane choices: a reload must not close it or reset it. */
const paneWanted = atom({ plugin: 'pulseline', key: 'pane' } as const, {
  open: false,
  layout: 'ledger',
})

/**
 * The binary's last output.
 *
 * Held rather than rebuilt for two reasons: a reload redraws the rows it
 * already had instead of flashing the not-run-yet note, and a render hook
 * that reads this subscribes its drawing, so writing it here draws the band
 * again with nobody calling `$.ui.invalidate`.
 */
/**
 * Which binary was resolved, `null` until the ladder has been walked.
 *
 * Cleared on `classic.ConfigChange`, so configuring the path or installing
 * the binary mid-session takes effect without restarting.
 */
const binaryChoice = atom({ plugin: 'pulseline', key: 'binary' } as const, null)

const drawnOutput = atom({ plugin: 'pulseline', key: 'output' } as const, {
  rows: [],
  paneRows: [],
  runNote: 'binary: not run yet',
  paneNote: 'pane: not run yet',
})

// Built from a char code on purpose: a literal escape in this source
// does not survive every editor round-trip.
const ESC = String.fromCharCode(27)
const ANSI_PATTERN = new RegExp(ESC + '\\[([0-9;]*)m', 'g')

/** xterm-256 index to `#rrggbb`: 16 base colours, a 6^3 cube, a 24-step ramp. */
export function ansi256Hex(index: number): string {
  const base = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0, 0x808080,
    0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
  ]
  const hex = (value: number) => `#${value.toString(16).padStart(6, '0')}`
  if (index < 16) return hex(base[index] ?? 0)
  if (index < 232) {
    const step = [0, 95, 135, 175, 215, 255]
    const offset = index - 16
    const r = step[Math.floor(offset / 36)] ?? 0
    const g = step[Math.floor((offset % 36) / 6)] ?? 0
    const b = step[offset % 6] ?? 0
    return hex((r << 16) | (g << 8) | b)
  }
  const level = 8 + (index - 232) * 10
  return hex((level << 16) | (level << 8) | level)
}

/** Drops the control characters the band refuses to draw. */
function stripControls(text: string): string {
  let clean = ''
  for (const char of text) {
    const code = char.codePointAt(0) ?? 32
    if (code >= 32 && code !== 127) clean += char
  }
  return clean
}

/** Splits one line of the binary's output into styled spans. */
export function spansOf(line: string): Span[] {
  const spans: Span[] = []
  let style: SpanStyle = {}
  let cursor = 0

  const push = (text: string) => {
    const clean = stripControls(text)
    if (clean.length > 0) spans.push({ ...style, children: clean })
  }

  for (const match of line.matchAll(ANSI_PATTERN)) {
    push(line.slice(cursor, match.index))
    cursor = match.index + match[0].length

    const codes = (match[1] ?? '').split(';').filter(code => code.length > 0)
    for (let at = 0; at < codes.length; at += 1) {
      const code = Number(codes[at])
      if (code === 0) style = {}
      else if (code === 1) style = { ...style, bold: true }
      else if (code === 2) style = { ...style, dimColor: true }
      else if (code === 7) style = { ...style, inverse: true }
      else if (code === 49) {
        // The one reset code the renderer emits besides 0: `powerline.rs`'s
        // DEFAULT_BG, written at the pointed outer edge of a rail or anchor
        // cluster to hand the terminal's own background back. Dropping the
        // property is the whole fix — carried over, the previous segment's
        // background stays on the seam span and the point paints as a filled
        // rectangle. 39 is not handled because nothing emits it.
        const cleared = { ...style }
        delete cleared.backgroundColor
        style = cleared
      } else if ((code === 38 || code === 48) && codes[at + 1] === '5') {
        const hex = ansi256Hex(Number(codes[at + 2]))
        style = code === 38 ? { ...style, color: hex } : { ...style, backgroundColor: hex }
        at += 2
      }
    }
  }
  push(line.slice(cursor))
  return spans
}

/** What one refresh's engine calls answered, kept so a failed one can reuse it. */
type Collected = {
  usage: SessionUsage
  modelId: string
  sessionId: string
  agents: readonly AgentInfo[]
  now: number
}

/**
 * What stays in the module's closure, and so starts over on a hot reload.
 *
 * Everything the person would notice losing is in `$.state` instead (see
 * `../types`). What is left is of two kinds:
 *
 *   - geometry, which a render hook writes as it measures the box it is
 *     given. It cannot be an atom: the host refuses a state write while a
 *     `ui.render` hook draws. It is re-measured on the next draw anyway.
 *   - values a reload cannot be caught in the middle of: `active` is the
 *     tools in flight, and nothing is in flight at the instant the module
 *     is rebuilt; `lastRunMs` only gates the next 400ms; `lastCollected`
 *     is a cache of the engine's own answers, re-asked on the next refresh.
 */
type State = {
  active: Map<string, ActiveTool>
  lastRunMs: number
  bandRows: number
  bandCols: number
  paneHeight: number
  paneCols: number
  /** MCP servers that are serving tools right now, by `$.tool.list()`. */
  mcpServers: Set<string>
  /** The last reading that came back whole, drawn again when one fails. */
  lastCollected: Collected | null
  /** Which refresh is newest; only that one may publish. See `refresh`. */
  runSeq: number
  /**
   * The manifest's `userConfig` values, as `register`'s second parameter
   * handed them over. Fixed for this activation: a change to them reloads
   * the module, so a closure is the right place for them.
   */
  options: PluginOptions
}

/**
 * The argument a tool row shows beside the tool name.
 *
 * `tool.call` carries the tool's own arguments on `e`, so this is a lookup,
 * not the guess-by-shape that the transcript parser has to make.
 */
export function targetOf(event: Record<string, unknown>): string | undefined {
  const keys = ['file_path', 'command', 'path', 'pattern', 'url', 'prompt', 'description']
  for (const key of keys) {
    const value = event[key]
    if (typeof value === 'string' && value.length > 0) return value.slice(0, 120)
  }
  return undefined
}

/**
 * Folds the Task API's items into the todo row's shape.
 *
 * A mirror of `state/mod.rs`'s `build_todo_from_tasks`, down to the summary
 * wording and the sort, so the band draws the same row whichever path fed it.
 * Note `pending` is everything not completed, the in-progress items included —
 * that is the transcript path's arithmetic, not an oversight.
 */
export function todoFromTasks(items: Record<string, TaskItem>): Todo | null {
  const all = Object.values(items)
  if (all.length === 0) return null

  const total = all.length
  const completed = all.filter(item => item.status === 'completed').length
  const pending = total - completed
  const running = all
    .filter(item => item.status === 'in_progress')
    .map(item => ({ text: item.activeForm ?? item.subject, started_at: item.startedAt }))
    .sort((a, b) => (a.started_at ?? Infinity) - (b.started_at ?? Infinity))

  return {
    text: `${completed}/${total} done, ${pending} pending`,
    pending,
    completed,
    total,
    in_progress_items: running,
    all_done: pending === 0 && completed > 0,
    is_task_api: true,
  }
}

/**
 * Folds one answered `TodoWrite`, `TaskCreate` or `TaskUpdate` into the todo
 * row.
 *
 * All three run after `next(e)` and only when the call did not fail, because
 * a list or a task the engine refused must not be drawn as current. Both ids
 * come from the engine: the created task's from the call's own result, an
 * update's from the event. Nothing is counted here, so a refused create
 * cannot leave a phantom that a later update would move instead of the real
 * task.
 */
async function rememberTodoCall(
  $: EngineInterface,
  name: string,
  seen: Record<string, unknown>,
  result: { result?: unknown },
): Promise<void> {
  if (name === 'TodoWrite' && Array.isArray(seen.todos)) {
    const todo = todoOf(seen.todos as { content?: string; status?: string }[])
    await update($, todoList, () => todo)
    return
  }

  if (name === 'TaskCreate' && typeof seen.subject === 'string') {
    const subject = seen.subject
    const answered = (result.result as { task?: { id?: unknown } } | undefined)?.task?.id
    if (typeof answered !== 'string' || answered.length === 0) return
    const item: TaskItem = {
      subject,
      status: 'pending',
      ...(typeof seen.activeForm === 'string' ? { activeForm: seen.activeForm } : {}),
    }
    await rebuildTodo($, held => ({ items: { ...held.items, [answered]: item } }))
    return
  }

  if (name === 'TaskUpdate' && typeof seen.taskId === 'string') {
    const taskId = seen.taskId
    // `TaskUpdate` answers `{ success }`; a call that reported failure moved
    // nothing, so neither does this.
    const answered = result.result as { success?: unknown } | undefined
    if (answered !== undefined && answered.success === false) return
    const asked = typeof seen.status === 'string' ? seen.status : undefined
    const subject = typeof seen.subject === 'string' ? seen.subject : undefined
    const activeForm = typeof seen.activeForm === 'string' ? seen.activeForm : undefined
    const at = await $.clock.now()
    await rebuildTodo($, held => {
      if (asked === 'deleted') {
        const items = { ...held.items }
        delete items[taskId]
        return { items }
      }
      const was = held.items[taskId]
      if (was === undefined) return held
      // `status` is optional on `TaskUpdate`: the call also carries a new
      // subject, active form or dependency set on its own. Defaulting it to
      // `pending` reopened a completed task whenever the model only renamed
      // one.
      const status = asked ?? was.status
      // The start is stamped once, so the row's elapsed time does not restart
      // every time the same task is touched again.
      const startedAt =
        status === 'in_progress' && was.status !== 'in_progress' ? at : was.startedAt
      return {
        items: {
          ...held.items,
          [taskId]: {
            ...was,
            status,
            startedAt,
            ...(subject !== undefined ? { subject } : {}),
            ...(activeForm !== undefined ? { activeForm } : {}),
          },
        },
      }
    })
  }
}

/**
 * Applies one change to the task items and rewrites the todo row from the
 * result.
 *
 * Two writes rather than one because the row is a fold over every item while
 * an update names a single id — and they are two atoms because `todo` is the
 * payload's own shape, which the items are not. Task events are a handful per
 * turn, so the second round trip is not on any hot path.
 */
async function rebuildTodo($: EngineInterface, change: (held: Tasks) => Tasks): Promise<void> {
  const next = await update($, taskItems, change)
  await update($, todoList, () => todoFromTasks(next.items))
}

/** Folds one TodoWrite call's arguments into the todo row's shape. */
function todoOf(items: { content?: string; status?: string; activeForm?: string }[]): Todo {
  const running = items.filter(item => item.status === 'in_progress')
  const completed = items.filter(item => item.status === 'completed').length
  // Everything not completed, the running item included — which is what the
  // transcript path's `extract_todo_summary` does and what `todoFromTasks`
  // above does. Counting only `pending` left the ledger's
  // `done = completed.max(total - pending)` reading a running item as done,
  // so a lone running task drew `1/1 done · 0 pending`.
  const pending = items.length - completed
  const first = running[0]
  return {
    text: first?.activeForm ?? first?.content ?? '',
    pending,
    completed,
    total: items.length,
    in_progress_items: running.map(item => ({ text: item.activeForm ?? item.content ?? '' })),
    all_done: items.length > 0 && completed === items.length,
    is_task_api: true,
  }
}

/**
 * Resolves Claude Code's own version, once per session.
 *
 * No call on `$` answers it, and `CLAUDE_CODE_EXECPATH` is set on the
 * children Claude Code spawns rather than on its own environment, so this
 * asks the CLI. Left null when that fails: the statusline then draws its
 * unknown marker, which is the truth.
 */
async function probeVersion(engine: EngineInterface): Promise<void> {
  const held = await read(engine, sessionFacts)
  if (held.version !== null) return
  const fromEnv = await engine.env.get('CLAUDE_CODE_EXECPATH')
  const guess = fromEnv?.split('/').pop()
  if (guess && /^\d+\.\d+\.\d+$/.test(guess)) {
    await update(engine, sessionFacts, facts => ({ ...facts, version: guess }))
    return
  }
  try {
    const run = await engine.process.run(['claude', '--version'], { timeoutMs: 10000 })
    const found = run.stdout.match(/\d+\.\d+\.\d+/)
    if (found) {
      const version = found[0]
      await update(engine, sessionFacts, facts => ({ ...facts, version }))
    }
  } catch {
    // Left as it was: the statusline draws its unknown marker, which is true.
  }
}

/**
 * Asks the engine everything one draw needs, in one round trip.
 *
 * These used to be six sequential awaits in `refresh`, outside any `try`, so
 * a single engine call failing took the whole draw down to a note. They run
 * together now and the caller owns the failure.
 *
 * It also stamps `lastRunMs` first thing, before anything is awaited on the
 * engine: the stamp used to come after all six, so two callers could both
 * clear the refresh gate and spawn the binary twice. Only
 * `classic.ConfigChange` is still gated, and that is the stamp it reads.
 */
async function collect(engine: EngineInterface, state: State): Promise<Collected> {
  const now = await engine.clock.now()
  state.lastRunMs = now

  const [usage, modelId, sessionId, agents, tools] = await Promise.all([
    engine.session.usage(),
    engine.session.model(),
    engine.session.id(),
    engine.agent.list(),
    engine.tool.list(),
  ])

  // Which MCP servers are actually serving tools, as against how many are
  // written down in the settings files.
  state.mcpServers = new Set(
    tools
      .filter(tool => tool.mcp)
      .map(tool => tool.name.split('__')[1])
      .filter((server): server is string => server !== undefined && server.length > 0),
  )

  // An agent the engine lists that no SubagentStart announced starts now,
  // and stays started then: the stamp has to outlive this draw, or the
  // agent's age would reset to zero on every row the band draws. It is a
  // write, not a default, which is why it cannot live in `buildPayload`.
  await update(engine, agentTimings, timings => {
    const startedAt = { ...timings.startedAt }
    const finishedAt = { ...timings.finishedAt }
    let added = false
    for (const agent of agents) {
      if (startedAt[agent.id] === undefined) {
        startedAt[agent.id] = now
        added = true
      }
      // The same write at the other end. An agent the engine no longer lists
      // as running that no `SubagentStop` stamped finished *now*, and stays
      // finished: the renderer shows `completed_at - started_at`, so without
      // the stamp a completed agent's elapsed time grows by one refresh
      // interval on every draw instead of standing still.
      if (agent.status !== 'running' && finishedAt[agent.id] === undefined) {
        finishedAt[agent.id] = now
        added = true
      }
    }
    return added ? { ...timings, startedAt, finishedAt } : timings
  })

  return { usage, modelId, sessionId, agents, now }
}

/**
 * Walks the ladder until something answers `--version` and is new enough.
 *
 * Each rung is probed by running it, never by `$.fs.stat`: on a machine
 * that has used the statusline, `~/.claude/pulseline/` exists and holds
 * `config.toml` and no binary, and a stat of that directory answers yes.
 * Running the candidate tells a binary from a folder and checks the version
 * in the same call.
 *
 * `stdin: ''` is passed on purpose. `ProcessRunInit.stdin` is documented as
 * "written to the child's standard input, then closed"; what an *absent*
 * `stdin` does is not stated for `$.process.run` (its sibling
 * `ProcessSpawnRequest.input` does say "absent, standard input is closed
 * from the start", but `run` does not repeat it). Passing the empty string
 * puts it on the documented path, so a same-named executable on PATH that
 * reads standard input cannot sit there until the timeout. The timeout is
 * the second guard, at 400ms a rung.
 *
 * The answer is cached in `$.state`, so this runs once a session rather
 * than once a refresh.
 */
async function resolveBinary(engine: EngineInterface, state: State): Promise<BinaryChoice> {
  const held = await read(engine, binaryChoice)
  if (held !== null) return held

  const configured = state.options.binaryPath
  const home = (await engine.env.get('HOME')) ?? (await engine.env.get('USERPROFILE')) ?? ''
  const facts = await read(engine, sessionFacts)

  const ladder = candidates({
    configured: typeof configured === 'string' ? configured : '',
    pluginRoot: engine.plugin.root,
    home,
    cwd: facts.cwd,
  })

  // The *newest* thing that answered but was too old, so the note names the
  // version the person actually has. Keeping the first one found would name
  // an older copy further up the ladder and send them updating to a version
  // they are already past.
  let outdated: { path: string; version: Version; rung: string } | null = null

  for (const candidate of ladder) {
    try {
      const run = await engine.process.run([candidate.path, '--version'], {
        stdin: '',
        timeoutMs: PROBE_TIMEOUT_MS,
      })
      if (run.exitCode !== 0) continue
      const version = parseVersion(run.stdout)
      if (version === null) continue
      if (isNewEnough(version)) {
        const choice: BinaryChoice = {
          kind: 'ok',
          path: candidate.path,
          version: formatVersion(version),
          rung: candidate.rung,
        }
        await update(engine, binaryChoice, () => choice)
        return choice
      }
      if (outdated === null || isNewEnough(version, outdated.version)) {
        outdated = { path: candidate.path, version, rung: candidate.rung }
      }
    } catch {
      // Not there, not runnable, or it outran the probe. Next rung.
    }
  }

  const choice: BinaryChoice =
    outdated === null
      ? { kind: 'missing' }
      : {
          kind: 'too-old',
          path: outdated.path,
          version: formatVersion(outdated.version),
          rung: outdated.rung,
        }
  await update(engine, binaryChoice, () => choice)
  return choice
}

/**
 * Builds the payload from the engine, runs the binary, parses its rows.
 *
 * Declared at the top of the file on purpose: the loader only lets `$` be
 * passed to a function declared here, never to a closure inside `register`.
 */
async function refresh(engine: EngineInterface, state: State): Promise<void> {
  // Each refresh claims a number and only the newest may publish. Several can
  // be in flight at once — parallel tool calls, a turn event landing beside
  // them — each with its own reading and its own spawn, and the write is a
  // plain overwrite. Without this the slowest finisher wins, so an older
  // reading can replace a newer one and leave a finished tool drawn as
  // running until the next event.
  const seq = (state.runSeq += 1)
  const publish = async (change: (out: DrawnOutput) => DrawnOutput): Promise<void> => {
    if (seq !== state.runSeq) return
    // Checked again *inside* the change, because `update` is a read-apply-write
    // with retries: it yields, and a newer refresh can claim the number and
    // publish in that window, which the outer check above has already passed.
    // The change runs inside the transaction and on each retry, so this is the
    // one that decides. Returning the value unchanged is the no-op.
    await update(engine, drawnOutput, out => (seq === state.runSeq ? change(out) : out))
  }

  // Nothing to run means nothing to draw but the reason. The band shows
  // `runNote` whenever there are no rows, so this is what reaches it.
  const binary = await resolveBinary(engine, state)
  if (binary.kind !== 'ok') {
    const note = binary.kind === 'missing' ? missingMessage() : tooOldMessage(binary.version)
    await publish(out => ({ ...out, rows: [], paneRows: [], runNote: note }))
    return
  }

  let collected: Collected
  let stale: string | null = null
  try {
    collected = await collect(engine, state)
    state.lastCollected = collected
  } catch (error) {
    // Nothing has ever come back whole: there is no reading to draw, and
    // spawning the binary with fabricated zeros would draw a lie.
    if (state.lastCollected === null) {
      await publish(out => ({
        ...out,
        rows: [],
        paneRows: [],
        runNote: `engine refused — ${String(error).slice(0, 160)}`,
      }))
      return
    }
    collected = state.lastCollected
    stale = String(error).slice(0, 80)
  }

  // One read per atom the payload is built from. `collect` has already made
  // its own writes, so these are the values the payload goes on to describe.
  const [facts, totals, timings, loaded, todo, pane, held] = await Promise.all([
    read(engine, sessionFacts),
    read(engine, toolTotals),
    read(engine, agentTimings),
    read(engine, instructions),
    read(engine, todoList),
    read(engine, paneWanted),
    read(engine, drawnOutput),
  ])

  // Effort rides on every classic hook input (see the `classic.PostToolUse`
  // hook below); `$.env` does not carry it, being Claude Code's own
  // environment rather than the one it hands to the children it spawns.
  const payload = buildPayload({
    sessionId: collected.sessionId,
    modelId: collected.modelId,
    usage: collected.usage,
    agents: collected.agents,
    now: collected.now,
    cwd: facts.cwd,
    version: facts.version,
    effort: facts.effort,
    outputStyle: facts.outputStyle,
    lastApiErrorMs: facts.lastApiErrorMs,
    cacheReadTotal: facts.cacheReadTotal ?? 0,
    tokens: facts.tokens,
    mcpCount: state.mcpServers.size,
    claudeMdCount: loaded.claudeMd.length,
    agentsMdCount: loaded.agentsMd.length,
    rulesCount: loaded.rules.length,
    memoryCount: loaded.memories.length,
    active: state.active,
    counts: totals.counts,
    completedTotal: totals.completedTotal,
    failedTotal: totals.failedTotal,
    startedAt: timings.startedAt,
    finishedAt: timings.finishedAt,
    agentType: timings.agentType,
    compactCount: facts.compactCount,
    todo,
  })

  // The run's results are gathered locally and written once at the end: one
  // write is one redraw, where a write per field would draw the band four
  // times for a single refresh.
  let rows: Span[][] = held.rows
  let runNote = held.runNote
  let paneRows: Span[][] = held.paneRows
  let paneNote = held.paneNote

  try {
    const argv = [
      binary.path,
      '--rows',
      String(state.bandRows),
      '--columns',
      String(state.bandCols),
    ]
    const run = await engine.process.run(argv, { stdin: payload, timeoutMs: 5000 })
    rows = run.stdout
      .split('\n')
      .filter(line => line.length > 0)
      .map(spansOf)
    runNote =
      run.exitCode === 0
        ? `${rows.length}r band ${state.bandRows}x${state.bandCols} · ${state.active.size} running · ${totals.completedTotal} done · ${paneNote}`
        : `exit ${run.exitCode} — ${run.stderr.slice(0, 120)}`
  } catch (error) {
    runNote = `run refused — ${String(error).slice(0, 160)}`
    rows = []
  }

  // A reading that did not come back is worth saying out loud: the rows
  // above are the previous one's, and silence would pass them off as live.
  if (stale !== null) runNote = `${runNote} · stale: ${stale}`

  if (!pane.open) {
    paneRows = []
  } else {
    // The pane is a different box, so it needs its own render: one row count
    // cannot serve a 1-row band and a 12-row pane.
    try {
      const paneArgv = [
        binary.path,
        '--rows',
        String(state.paneHeight),
        '--columns',
        String(state.paneCols),
        '--layout',
        pane.layout,
      ]
      const paneRun = await engine.process.run(paneArgv, { stdin: payload, timeoutMs: 5000 })
      paneRows = paneRun.stdout
        .split('\n')
        .filter(line => line.length > 0)
        .map(spansOf)
      paneNote =
        paneRun.exitCode === 0
          ? `pane: ${paneRows.length}r ${pane.layout} at ${state.paneHeight}x${state.paneCols}`
          : `pane: exit ${paneRun.exitCode} — ${paneRun.stderr.slice(0, 120)}`
    } catch (error) {
      paneRows = []
      paneNote = `pane: run failed — ${String(error).slice(0, 160)}`
    }
  }

  await publish(() => ({ rows, runNote, paneRows, paneNote }))
}

/**
 * Refreshes only if the last run is older than `gapMs`.
 *
 * The mod sits in the tool-call chain, so a spawn on the way in delays the
 * tool itself; this bounds how often that happens while a burst of calls
 * goes through.
 */
async function maybeRefresh(
  engine: EngineInterface,
  state: State,
  gapMs: number,
): Promise<void> {
  const now = await engine.clock.now()
  if (!shouldRefresh(state.lastRunMs, now, gapMs)) return
  // Claim the window before awaiting anything else, or a second caller
  // arriving while this one is in `refresh` reads the old stamp and spawns
  // the binary too. `collect` stamps it again with its own reading.
  state.lastRunMs = now
  await refresh(engine, state)
}

export const register: Register = (on, options) => {
  const state: State = {
    active: new Map(),
    lastRunMs: 0,
    bandRows: 3,
    bandCols: 120,
    paneHeight: 12,
    paneCols: 120,
    mcpServers: new Set(),
    lastCollected: null,
    runSeq: 0,
    options,
  }

  on('session.start', async ($, e, next) => {
    const result = await next(e)
    const cwd = e.cwd
    await update($, sessionFacts, facts => ({ ...facts, cwd }))
    $.ui.status('alive')
    await probeVersion($)
    try {
      await $.command.register({
        name: 'pulseline',
        description: 'Open the statusline in a pane, where it has room for activity rows',
      })
    } catch (error) {
      const note = `command refused — ${String(error).slice(0, 120)}`
      await update($, drawnOutput, out => ({ ...out, runNote: note }))
    }
    await refresh($, state)
    $.ui.log((await read($, drawnOutput)).runNote)
    return result
  })

  on('tool.call', async ($, e, next) => {
    const seen = e as unknown as Record<string, unknown>
    const name = String(seen.tool ?? 'tool')
    const id = String(seen.tool_use_id ?? name)

    // Bookkeeping must never throw into the tool chain: a bad event shape
    // would take the tool call down with it, and the band with that.
    try {
      if (!NOT_TOOL_ACTIVITY.has(name)) state.active.set(id, { id, name, target: targetOf(seen) })
      // The band's reason to exist is the tool running *now*, and the rows it
      // draws are the binary's stdout — so a running tool reaches the band
      // only if a refresh happens while its entry is still in `state.active`.
      // Without this the list could only ever be filled by an overlapping
      // call, and a lone `cargo test` showed nothing for its whole run.
      //
      // Ungated, unlike the other sites. A gated start is not drawn late, it
      // is not drawn at all: the window is closed by the previous tool's own
      // completion refresh, and the next refresh after that is this tool's
      // completion — by which time its entry is gone. A tool that ran for
      // minutes was therefore never shown. So this pays a spawn per call,
      // against a spawn floor under 3ms and a tool call that is tens to
      // thousands of times that.
      await refresh($, state)
    } catch (error) {
      const note = `mark failed: ${String(error).slice(0, 120)}`
      await update($, drawnOutput, out => ({ ...out, runNote: note }))
    }

    let failed = false
    try {
      const result = await next(e)
      failed = result.isError === true
      // The Task API's bookkeeping waits for the result, where a tool's own
      // mark cannot. `TaskCreate` answers with the id the engine assigned
      // (`{ task: { id } }`) and that is the id a later `TaskUpdate` names, so
      // reading it here beats counting creations: a refused or failed call
      // leaves nothing behind, and no local counter can fall out of step.
      // TodoWrite above is the older API; the transcript path dispatches all
      // three, and without these two the band shows no task progress at all
      // on the current one.
      if (!failed) await rememberTodoCall($, name, seen, result)
      return result
    } catch (error) {
      failed = true
      throw error
    } finally {
      try {
        state.active.delete(id)
        // The count lands before the refresh, and in its own write: an
        // overrun inside `maybeRefresh` must not cost this tool its place
        // in the session totals.
        if (!NOT_TOOL_ACTIVITY.has(name)) {
          await update($, toolTotals, totals => ({
            counts: {
              ...totals.counts,
              [name]: {
                count: (totals.counts[name]?.count ?? 0) + 1,
                failed: (totals.counts[name]?.failed ?? 0) + (failed ? 1 : 0),
              },
            },
            completedTotal: totals.completedTotal + 1,
            failedTotal: totals.failedTotal + (failed ? 1 : 0),
          }))
        }
        // Unconditional, where the way in is gated. A gated completion would
        // leave the band drawing a tool that has already finished until some
        // later event reopened the window — and between two bursts inside one
        // turn that is seconds of a phantom running tool. One spawn per
        // completion is the price, and the spawn floor is under 3ms.
        await refresh($, state)
        $.ui.status((await read($, drawnOutput)).runNote)
      } catch (error) {
        const note = `refresh failed: ${String(error).slice(0, 120)}`
        await update($, drawnOutput, out => ({ ...out, runNote: note }))
        $.ui.status(note)
      }
    }
  }).catch(($, e, next) => next(e))

  // Classic hook inputs carry `effort` on every tool-use-context event, so
  // one cheap hook closes the gap a streaming `turn.step` hook would have.
  // `classic.PreToolUse` is the exception: its argument is the tool call
  // envelope, not the hook input, so the effort rides on PostToolUse.
  on('classic.PostToolUse', async ($, e, next) => {
    const level = e.effort?.level
    if (level) await update($, sessionFacts, facts => ({ ...facts, effort: level }))
    return next(e)
  }).catch(($, e, next) => next(e))

  // What actually reached the model's context, as against what exists on
  // disk: the engine names each instruction file as it loads it.
  on('classic.InstructionsLoaded', async ($, e, next) => {
    const result = await next(e)
    const path = e.file_path
    await update($, instructions, loaded => {
      // A file named twice must not count twice, which is what the Sets
      // these arrays replaced were for.
      // AGENTS.md first, and in its own list: the binary counts it in its own
      // column (`env.rs`'s `count_agents_md`), so sending it as part of
      // `claude_md_count` draws the same file twice on the ENV row.
      if (path.endsWith('AGENTS.md')) {
        return loaded.agentsMd.includes(path)
          ? loaded
          : { ...loaded, agentsMd: [...loaded.agentsMd, path] }
      }
      if (path.endsWith('CLAUDE.md')) {
        return loaded.claudeMd.includes(path)
          ? loaded
          : { ...loaded, claudeMd: [...loaded.claudeMd, path] }
      }
      if (path.includes('/rules/')) {
        return loaded.rules.includes(path) ? loaded : { ...loaded, rules: [...loaded.rules, path] }
      }
      if (path.includes('/memory/')) {
        return loaded.memories.includes(path)
          ? loaded
          : { ...loaded, memories: [...loaded.memories, path] }
      }
      return loaded
    })
    return result
  }).catch(($, e, next) => next(e))

  // A settings file changed: re-read rather than wait out the binary's
  // ten-second environment cache. Throttled, unlike the other refresh
  // sites: the rest are once-per-turn or the person's own doing, while one
  // settings write can raise a burst of these.
  on('classic.ConfigChange', async ($, e, next) => {
    const result = await next(e)
    // Forget which binary was resolved: a settings write is how the path
    // gets configured, and a binary installed since can now be found. The
    // next refresh walks the ladder again.
    await update($, binaryChoice, () => null)
    await maybeRefresh($, state, 400)
    return result
  }).catch(($, e, next) => next(e))

  // Real agent timings, instead of inferring a stop from a list status.
  on('classic.SubagentStart', async ($, e, next) => {
    const now = await $.clock.now()
    const id = e.agent_id
    const type = e.agent_type
    await update($, agentTimings, timings => {
      const finishedAt = { ...timings.finishedAt }
      delete finishedAt[id]
      return {
        startedAt: { ...timings.startedAt, [id]: now },
        agentType: { ...timings.agentType, [id]: type },
        finishedAt,
      }
    })
    // Ungated, for the reason the tool path is. The `Agent` call's own
    // pre-call refresh ran before the agent existed for `$.agent.list()`, and
    // this event lands milliseconds later with the window shut — so a gated
    // refresh here is dropped, and the next one is the call's completion. A
    // subagent that ran for minutes was drawn only once it had finished.
    await refresh($, state)
    return next(e)
  }).catch(($, e, next) => next(e))

  on('classic.SubagentStop', async ($, e, next) => {
    const now = await $.clock.now()
    const id = e.agent_id
    await update($, agentTimings, timings => ({
      ...timings,
      finishedAt: { ...timings.finishedAt, [id]: now },
    }))
    // Ungated for the matching reason: a gated stop leaves a finished agent
    // drawn as running. Agents start and stop a handful of times a session,
    // so neither end is a burst to bound.
    await refresh($, state)
    return next(e)
  }).catch(($, e, next) => next(e))

  // The compaction badge: the transcript parser counts `compact_boundary`
  // events; here the engine says so directly.
  on('classic.PreCompact', async ($, e, next) => {
    await update($, sessionFacts, facts => ({
      ...facts,
      compactCount: facts.compactCount + 1,
    }))
    return next(e)
  }).catch(($, e, next) => next(e))

  // What the person chose in place of the default way of answering. Only
  // `prompt.compose` carries it, and this hook reads and passes on: the
  // sections it is really about are returned untouched.
  on('prompt.compose', async ($, e, next) => {
    const name = e.outputStyle?.name ?? null
    // Composed on every system-prompt render, so write only on a change.
    const facts = await read($, sessionFacts)
    if (facts.outputStyle !== name) {
      await update($, sessionFacts, held => ({ ...held, outputStyle: name }))
    }
    return next(e)
  }).catch(($, e, next) => next(e))

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    const usage = e.usage
    // `TurnCompleteReason` includes 'error', which is the one place the mod
    // can see an API error; the binary drops a reading older than 30s, so
    // this is never cleared.
    const erroredAt = e.reason === 'error' ? await $.clock.now() : null
    if (usage || erroredAt !== null) {
      await update($, sessionFacts, facts => ({
        ...facts,
        tokens: usage
          ? {
              input_tokens: usage.input_tokens,
              output_tokens: usage.output_tokens,
              cache_read_input_tokens: usage.cache_read_input_tokens,
              cache_creation_input_tokens: usage.cache_creation_input_tokens,
            }
          : facts.tokens,
        lastApiErrorMs: erroredAt ?? facts.lastApiErrorMs,
        // Summed here, once per turn, because this is the only place that
        // sees each turn's usage exactly once. `?? 0` because a session
        // whose atom the host wrote before this field existed has none.
        cacheReadTotal:
          (facts.cacheReadTotal ?? 0) + (usage ? usage.cache_read_input_tokens : 0),
      }))
    }
    await refresh($, state)
    return result
  })

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    if (e.surface !== 'terminal') return next(e)
    // A survey owns the band while it is up; yield rather than fight it.
    if (e.props.hasSurvey) return next(e)

    // Read before anything can return: a read while a render hook draws
    // subscribes this drawing, so the write at the end of `refresh` draws
    // the band again by itself, with nobody calling `$.ui.invalidate`.
    const held = await read($, drawnOutput)

    // With no rows, still draw the note: a band that silently disappears
    // tells us nothing about why.
    if (held.rows.length === 0 && !DEBUG) {
      const { Text } = $.ui.resolve(e)
      return <Text>{`pulseline: ${held.runNote}`}</Text>
    }

    state.bandRows = e.props.maxRows
    // Two cells go to the band's own collapse marker at the right edge.
    state.bandCols = Math.max(20, e.props.bodyColumns - 2)

    const { Box, Text } = $.ui.resolve(e)
    const notes = DEBUG ? [<Text>{held.runNote}</Text>] : []
    // Clip to what the band shows whole; a taller tree hides behind `n more`.
    const drawn = held.rows.slice(0, Math.max(1, e.props.maxRows - notes.length))

    return (
      <Box flexDirection="column">
        {notes}
        {drawn.map(spans => (
          <Box flexDirection="row">
            {/*
              A span's text is pulled out of the spread on purpose. `h` takes
              children as rest arguments and ignores `props.children`, so
              `<Text {...span} />` compiles and type-checks and then draws the
              style with no text in it — the row keeps its colours and loses
              every character. Splitting them emits `h(Text, style, children)`,
              which is the only form the engine reads.
            */}
            {spans.map(({ children, ...style }) => <Text {...style}>{children}</Text>)}
          </Box>
        ))}
      </Box>
    )
  })

  on('command.run', { command: 'pulseline' }, async ($, e, next) => {
    if ((await read($, paneWanted)).open) {
      await update($, paneWanted, pane => ({ ...pane, open: false }))
      await $.ui.close({ id: PANE_ID })
      return { text: 'pulseline pane closed' }
    }
    // Fill the rows before the pane mounts: a pane that opens empty and
    // fills a beat later depends on the first draw losing a race with the
    // write, which is not worth running.
    await update($, paneWanted, pane => ({ ...pane, open: true }))
    await refresh($, state)
    const opened = await $.ui.open({ id: PANE_ID, title: 'pulseline', rows: 12, closeOnEscape: true })
    // `ui.open` resolves once the pane has been drawn, so by here the render
    // hook has run and the closure holds the box the surface actually gave —
    // rarely the 12x120 the refresh above was sized for. A second run puts
    // rows of the right width and count in before anyone looks.
    //
    // The render hook cannot do this itself: `$.state.set` is refused while a
    // drawing is in flight, which is why the geometry lives in the closure in
    // the first place. Nothing is drawn when the pane was not placed, so the
    // second run would have nothing to measure against.
    if (opened.isPlaced) await refresh($, state)
    return { text: undefined }
  })

  on('ui.render', { component: 'Pane' }, async ($, e, next) => {
    if (e.requestId !== PANE_ID || e.surface !== 'terminal') return next(e)

    // Both reads come before any other work, so this drawing is subscribed
    // to the rows it shows and to the layout its switcher marks.
    const [held, pane] = await Promise.all([read($, drawnOutput), read($, paneWanted)])

    // Two cells go to the pane's own close mark at the right edge.
    state.paneCols = Math.max(20, e.props.bodyColumns - 2)
    state.paneHeight = Math.max(2, e.props.scroll.bodyRows)

    const { Box, Text, Button } = $.ui.resolve(e)
    const drawn: Span[][] =
      held.paneRows.length > 0
        ? held.paneRows
        : [[{ children: held.paneNote }], [{ children: held.runNote }]]

    const switcher = (
      <Box flexDirection="row" gap={2}>
        {LAYOUTS.map((name, index) => (
          <Button
            key={`${LAYOUT_KEY}${name}`}
            label={`${index + 1}: ${name}`}
            plain
            dimColor={name !== pane.layout}
            // The press also raises `ui.press`, where the switch is handled:
            // this handler is its bottom, and it cannot await a re-render.
            onPress={() => {}}
          />
        ))}
      </Box>
    )

    return (
      <Box flexDirection="column">
        {drawn.map(spans => (
          <Box flexDirection="row">
            {/* Split for the same reason as the band's rows above. */}
            {spans.map(({ children, ...style }) => <Text {...style}>{children}</Text>)}
          </Box>
        ))}
        {switcher}
      </Box>
    )
  })

  on('ui.press', { requestId: PANE_ID }, async ($, e, next) => {
    if (!e.element.startsWith(LAYOUT_KEY)) return next(e)
    const layout = e.element.slice(LAYOUT_KEY.length)
    await update($, paneWanted, pane => ({ ...pane, layout }))
    await refresh($, state)
    return next(e)
  })

  on('ui.close', { id: PANE_ID }, async ($, e, next) => {
    await update($, paneWanted, pane => ({ ...pane, open: false }))
    return next(e)
  }).catch(($, e, next) => next(e))
}
