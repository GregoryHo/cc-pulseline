import { atom, read, update } from 'claude-code'
import type { AgentInfo, EngineInterface, PluginOptions, Register, SessionContextUsage, SessionUsage } from 'claude-code'

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
import {
  COMPACT_KEY,
  LONG_TURN_MS,
  QUIET,
  SPEND_EXPIRE_MS,
  SPEND_PREFIX,
  alertRow,
  desktopAlerts,
  longTurnNotice,
  nextDeltas,
  quotaKey,
  quotaNotice,
  quotaPast,
  runwayOf,
  shareLine,
  shareOf,
  shouldWriteSpend,
  spendOf,
  stripCells,
  stripOf,
  stripRows,
} from './desktop'
import type { SessionSpend, StripCell } from './desktop'
import { buildPayload } from './payload'
import type { ActiveTool } from './payload'
import { shouldRefresh } from './throttle'
import type {
  Alert,
  BandRun,
  BinaryChoice,
  DrawnOutput,
  Span,
  SpanStyle,
  TaskItem,
  Tasks,
  Todo,
} from '../types'

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

/**
 * The layouts the desktop band may draw, the first the default.
 *
 * `quiet`, the default, runs no binary for the band: the Code tab already
 * shows the model, the effort, a context ring and every tool, so the band
 * draws only what passes a limit — context, quota, a cold cache — as an
 * `Svg` gauge and a line (see `hooks/desktop.ts`). Empty otherwise.
 *
 * The others are the binary's rows. The desktop's Code tab does not run the
 * `statusLine` command, so the split the terminal band relies on — identity
 * and budget on the statusline, the turn in flight above the prompt — leaves
 * the desktop with no context, cost or quota anywhere. Each of these carries
 * all of them in a few rows:
 * `compact` fuses identity and budget into one head row over the activity;
 * `rail` and `anchor` draw them as a powerline bar, on half-block seams since
 * the desktop has no Nerd Font (`--no-icons`), with `activity` stacked under
 * (see `runDesktopBand`). Mirrored by the `desktopLayout` field's `options`
 * in `plugin.json`.
 */
const DESKTOP_LAYOUTS = [QUIET, 'compact', 'rail', 'anchor']

/**
 * The desktop band's layout from the `desktopLayout` setting.
 *
 * The engine already treats a stored value outside the field's `options` as
 * unset, so this check only matters to a manifest and a module that have
 * drifted apart — and there the default is the safe answer.
 */
export function desktopLayoutOf(options: PluginOptions): string {
  const chosen = options.desktopLayout
  return typeof chosen === 'string' && DESKTOP_LAYOUTS.includes(chosen)
    ? chosen
    : (DESKTOP_LAYOUTS[0] ?? QUIET)
}

/**
 * Asks the binary for no Nerd Font glyphs. The desktop app draws text in its
 * own face, which has none of the Private Use Area icons or powerline seams;
 * braille, gauge marks and half blocks it draws fine, and the binary keeps
 * them.
 */
const NO_ICONS = '--no-icons'

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
  ctxTokens: null,
  ctxDeltas: [],
  autoCompact: null,
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
  desktopRows: [],
  paneRows: [],
  // Not `'failed'`: nothing has gone wrong yet, and the band draws the note
  // only for `'failed'`. See `BandRun` in `../types`.
  bandRun: 'never',
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

/**
 * Flattens text from outside the module before it goes on the band: a
 * stderr dump, a caught error.
 *
 * The band refuses the *whole tree* over one control character — the engine
 * says `ui.render (AbovePrompt) refused: a text child holds a control
 * character (an escape sequence); the engine drew its own` — so an
 * unflattened panic takes down the note that was meant to explain it, and the
 * band goes blank with nothing to read. Runs become one space rather than
 * nothing, where `stripControls` below drops them: a panic's first line is
 * the useful one, and `at src/main.rs:1:1` must not be glued to the end of
 * it. Flatten before slicing, or a cut can land inside an escape sequence.
 */
function noteText(text: string): string {
  // SGR codes first, whole. Dropping the escape alone and keeping the rest
  // left `[31mthread 'main' panicked [0m` on the band: accepted by the
  // engine, and unreadable.
  let flat = ''
  let spaced = false
  for (const char of text.replace(ANSI_PATTERN, '')) {
    const code = char.codePointAt(0) ?? 32
    if (code >= 32 && code !== 127) {
      flat += char
      spaced = false
      continue
    }
    if (!spaced && flat.length > 0) {
      flat += ' '
      spaced = true
    }
  }
  return flat.trimEnd()
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
  /** Where the session draws, which decides which band runs are made. */
  surfaces: readonly string[]
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
  /** The desktop band's box, measured apart: it is not the terminal's. */
  desktopBandRows: number
  desktopBandCols: number
  /**
   * Whether a desktop band has been measured yet. Until then its box is the
   * 3x120 guess above, so the first measurement is a change by definition.
   */
  desktopBandMeasured: boolean
  paneHeight: number
  paneCols: number
  /** MCP servers that are serving tools right now, by `$.tool.list()`. */
  mcpServers: Set<string>
  /** The last reading that came back whole, drawn again when one fails. */
  lastCollected: Collected | null
  /** Which refresh is newest; only that one may publish. See `refresh`. */
  runSeq: number
  /**
   * The project root `syncRoot` last read, so a move can be told from the
   * first read of a session. A closure field on purpose: a reload is not a
   * move, and this starting over is what says so.
   */
  rootRead: string | null
  /**
   * Whether `$.session.version()` has answered in this load of the module.
   * Until it has, every refresh asks again and its answer replaces the held
   * version, which may be a CLI fallback. See `syncVersion`.
   */
  versionFromEngine: boolean
  /**
   * The manifest's `userConfig` values, as `register`'s second parameter
   * handed them over. Fixed for this activation: a change to them reloads
   * the module, so a closure is the right place for them.
   */
  options: PluginOptions
  /**
   * The quota windows already notified, by `quotaKey`. Seeded from the atom
   * on the first refresh after a load, then checked and marked with no
   * `await` between, so two refreshes in flight cannot both notify.
   */
  notified: Set<string> | null
}

/**
 * The argument a tool row shows beside the tool name.
 *
 * `tool.call` carries the tool's own arguments on `e`, so this is a lookup,
 * not the guess-by-shape that the transcript parser has to make.
 */
/**
 * Which key each tool's useful argument is under, mirrored from
 * `providers/transcript.rs`'s `extract_target`.
 *
 * A generic key sweep is not enough and was quietly wrong: `Grep` carries
 * both `path` and `pattern`, and reading `path` first showed the directory
 * being searched instead of what was being looked for. Tools whose argument
 * is under none of the generic keys — `WebSearch`, `Skill`, `SendMessage`,
 * `Monitor`, `PushNotification` — showed no target at all.
 */
const TARGET_KEYS: Record<string, readonly string[]> = {
  Read: ['file_path'],
  Write: ['file_path'],
  Edit: ['file_path'],
  NotebookEdit: ['file_path'],
  Bash: ['command'],
  PowerShell: ['command'],
  Monitor: ['script_id', 'pattern'],
  PushNotification: ['title'],
  Advisor: ['query'],
  MCPSearch: ['query'],
  Glob: ['pattern'],
  Grep: ['pattern'],
  WebFetch: ['url'],
  WebSearch: ['query'],
  Skill: ['skill'],
  SendMessage: ['to'],
  LSP: ['command'],
}

/** The generic sweep, for a tool with no entry above. */
const FALLBACK_TARGET_KEYS = ['file_path', 'command', 'pattern'] as const

/**
 * The argument a tool call shows in the band.
 *
 * Returned whole, only flattened to one line: the renderer truncates per tool
 * kind and per available width (`render/activity/cells/recent_tool.rs` keeps
 * the *tail* of a Read or Edit path, so cutting the head off here lost the
 * filename and showed the wrong end).
 */
export function targetOf(event: Record<string, unknown>): string | undefined {
  const name = typeof event.tool === 'string' ? event.tool : ''
  const read = (key: string): string | undefined => {
    const value = event[key]
    return typeof value === 'string' && value.length > 0 ? value : undefined
  }

  // `AskUserQuestion` keeps its text one level down, in the first question.
  if (name === 'AskUserQuestion') {
    const first = Array.isArray(event.questions) ? event.questions[0] : undefined
    const asked = (first as { question?: unknown } | undefined)?.question
    return typeof asked === 'string' && asked.length > 0 ? singleLine(asked) : undefined
  }

  for (const key of TARGET_KEYS[name] ?? FALLBACK_TARGET_KEYS) {
    const value = read(key)
    if (value !== undefined) return singleLine(value)
  }
  return undefined
}

/** `fmt::sanitize_single_line`: a row is one line, so these become spaces. */
function singleLine(text: string): string {
  return text.replace(/[\n\r\t]/g, ' ')
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
 * Applies one change to the task items. One write, and the only one.
 *
 * It used to fold the result into the `todo` atom as a second write, and two
 * task calls in flight together could interleave: both commit their items
 * correctly, then the one that committed the *older* snapshot writes its fold
 * last and the row drops or reopens a task until some later task event
 * rebuilt it. The row is a fold over every item, so it is derived where it is
 * drawn instead — `refresh` folds whatever the items atom holds at that
 * moment, which is always the committed state.
 */
async function rebuildTodo($: EngineInterface, change: (held: Tasks) => Tasks): Promise<void> {
  await update($, taskItems, change)
}

/** Folds one TodoWrite call's arguments into the todo row's shape. */
function todoOf(
  items: { content?: string; status?: string; activeForm?: string }[],
): Todo | null {
  // A cleared list is no list, not a list of nothing. The renderer reads any
  // non-null todo as present and drew `0/0 done · 0 pending` in the ledger
  // the pane shows; both siblings already answer this way —
  // `extract_todo_summary` with `None`, `todoFromTasks` with `null` — and the
  // transcript path's `dispatch_todo_write` passes that `None` straight to
  // `set_todo`, so clearing is what the other path does too.
  if (items.length === 0) return null

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
 * Writes the engine's own version, until the engine has answered once in
 * this load of the module. True once it has.
 *
 * The engine's answer wins over whatever is held, because what is held may
 * be a fallback: the CLI's version from `probeVersion`, or one an older
 * revision of this module wrote — `$.state` survives the reload that
 * upgrades the plugin, and 1.7.0 stored the `claude` on PATH. So the held
 * value is not a reason to stop asking; only the engine's answer is, and the
 * closure flag that records it starts over with the module, as it should.
 *
 * Cheap enough for every refresh — one engine call until it answers, none
 * after — so `refresh` calls it as well as `session.start`: the version is
 * right from the first draw even where `session.start` raced it, and the one
 * place a test can reach it is the tool path. Never rejects.
 */
async function syncVersion(engine: EngineInterface, state: State): Promise<boolean> {
  if (state.versionFromEngine) return true
  try {
    const { version } = await engine.session.version()
    if (version.length === 0) return false
    const held = await read(engine, sessionFacts)
    if (held.version !== version) {
      await update(engine, sessionFacts, facts => ({ ...facts, version }))
    }
    state.versionFromEngine = true
    return true
  } catch {
    // An engine that does not answer, or an atom it would not read. The
    // next refresh asks again; `probeVersion` falls back to the CLI.
    return false
  }
}

/**
 * Resolves Claude Code's own version, once per session.
 *
 * `$.session.version()` first: the engine the session runs on, which is not
 * always the `claude` on PATH. The desktop app runs a Claude Code of its own,
 * and asking the CLI there reported the person's terminal install instead —
 * `2.1.304` over a session running `2.1.299`.
 *
 * The CLI is the fallback for an engine that refuses the call.
 * `CLAUDE_CODE_EXECPATH` is set on the children Claude Code spawns rather
 * than on its own environment, so it is tried and is rarely there. Left null
 * when every way fails: the statusline then draws its unknown marker, which
 * is the truth.
 */
async function probeVersion(engine: EngineInterface, state: State): Promise<void> {
  if (await syncVersion(engine, state)) return
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

  const [usage, modelId, sessionId, agents, tools, surfaces] = await Promise.all([
    engine.session.usage(),
    engine.session.model(),
    engine.session.id(),
    engine.agent.list(),
    engine.tool.list(),
    engine.session.surfaces(),
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

  return { usage, modelId, sessionId, agents, now, surfaces }
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
/**
 * How long a failed walk is kept before the ladder is tried again.
 *
 * Long enough that a session with nothing installed does not re-probe five
 * rungs on every refresh, short enough that following the band's own
 * `npm install -g` line is seen without restarting.
 */
const NEGATIVE_TTL_MS = 10_000

async function resolveBinary(engine: EngineInterface, state: State): Promise<BinaryChoice> {
  const held = await read(engine, binaryChoice)
  const now = await engine.clock.now()
  if (held !== null && (held.kind === 'ok' || now - held.at < NEGATIVE_TTL_MS)) return held

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
      ? { kind: 'missing', at: now }
      : {
          kind: 'too-old',
          path: outdated.path,
          version: formatVersion(outdated.version),
          rung: outdated.rung,
          at: now,
        }
  await update(engine, binaryChoice, () => choice)
  return choice
}

/** Whether a path sits inside a directory, native separators included. */
function under(dir: string, path: string): boolean {
  if (dir.length === 0) return false
  const slashed = (text: string) => text.replace(/\\/g, '/').replace(/\/+$/, '')
  return `${slashed(path)}/`.startsWith(`${slashed(dir)}/`)
}

/**
 * Puts the session's own project root in the atom, and forgets a binary that
 * was found under the old one.
 *
 * `session.start`'s `e.cwd` is where the session began, not where it is.
 * `$.session.root()` is the one that moves: its contract names `/cd`, a
 * host's directory change and a worktree move, and says nested instruction
 * files are read only beneath it — which is exactly what the binary scans
 * for the config row, and what its git shell-out runs in. Held from the
 * start, every later payload described the directory the person left.
 *
 * Read before the ladder rather than inside `collect`, which runs after it:
 * the dev rung is `<root>/target/release/cc-pulseline`, so a choice made
 * under the old root points into the old checkout.
 */
async function syncRoot(engine: EngineInterface, state: State): Promise<void> {
  const root = await engine.session.root()
  const facts = await read(engine, sessionFacts)
  if (facts.cwd !== root) {
    await update(engine, sessionFacts, held => ({ ...held, cwd: root }))
    // Dropped rather than re-walked here: the ladder does that once, on the
    // root just written.
    await update(engine, binaryChoice, () => null)
  }

  // Only a root this function read before and now finds changed is a move.
  // The first read is not one: `cwd` is empty until then, or holds
  // `session.start`'s `e.cwd`, which is where the session began rather than
  // where its root is — and the instructions loaded before this point were
  // loaded for the root either way. Nor is a reload a move, which is why this
  // sits in the closure: it starts over with the module, as it should.
  const left = state.rootRead
  state.rootRead = root
  if (left === null || left === root) return
  // The instruction counts are the other thing the root owns. Left standing,
  // the ENV row reported the project the person had left — and then kept
  // adding the new one's files to it, so the counts were two projects deep
  // for the rest of the session. Reading the root live without this made that
  // worse than leaving both stale: the path was right and the counts were not.
  await update(engine, instructions, loaded => ({
    // Kept: the files that are nobody's project — `~/.claude/CLAUDE.md`,
    // `~/.claude/rules/`. Those were loaded for the session, not for the root.
    claudeMd: loaded.claudeMd.filter(path => !under(left, path)),
    agentsMd: loaded.agentsMd.filter(path => !under(left, path)),
    rules: loaded.rules.filter(path => !under(left, path)),
    // Cleared outright, where the others are filtered: every memory file is
    // keyed by the project it belongs to — the binary reads them from
    // `~/.claude/projects/<encoded-root>/memory/` — so none of the old root's
    // are the new root's. The prefix test cannot see that, because they live
    // under HOME rather than under the root.
    memories: [],
  }))
  // A count of zero is sent as no field at all, so until the engine names the
  // new root's files the binary's own scan stands. That is the right way to
  // be wrong here: the scan reads the directory the session is actually in.
}

/**
 * One band run: spawns the binary on the payload and parses its rows.
 *
 * Never rejects. `fault` is the note for a run that did not answer cleanly —
 * a non-zero exit with its stderr, or a refused spawn — and `null` for one
 * that did, zero rows included.
 */
async function runRows(
  engine: EngineInterface,
  payload: string,
  argv: string[],
): Promise<{ rows: Span[][]; fault: string | null }> {
  try {
    const run = await engine.process.run(argv, { stdin: payload, timeoutMs: 5000 })
    const rows = run.stdout
      .split('\n')
      .filter(line => line.length > 0)
      .map(spansOf)
    return run.exitCode === 0
      ? { rows, fault: null }
      : { rows, fault: `exit ${run.exitCode} — ${noteText(run.stderr).slice(0, 120)}` }
  } catch (error) {
    return { rows: [], fault: `run refused — ${noteText(String(error)).slice(0, 160)}` }
  }
}

/**
 * The desktop band's run: the `desktopLayout` setting, without Nerd Font
 * glyphs.
 *
 * `compact` carries the activity row itself. `rail` and `anchor` do not —
 * they draw identity, context and quota as a powerline bar and stop there,
 * whatever `--rows` allows — and the desktop has no statusline to leave the
 * turn in flight to. So those two are a head, and `activity` is run under it
 * with the rows the head left: the bar on top, the running tools below.
 *
 * The head gets every row but one, so a band with room for two still shows
 * the tool that is running. A fault in either run is the band's fault.
 */
async function runDesktopBand(
  engine: EngineInterface,
  payload: string,
  binaryPath: string,
  state: State,
): Promise<{ rows: Span[][]; fault: string | null }> {
  const layout = desktopLayoutOf(state.options)
  const box = (rows: number) => [
    binaryPath,
    '--rows',
    String(rows),
    '--columns',
    String(state.desktopBandCols),
  ]
  if (layout === 'compact') {
    return runRows(engine, payload, [...box(state.desktopBandRows), '--layout', layout, NO_ICONS])
  }

  const total = Math.max(1, state.desktopBandRows)
  const headRows = Math.max(1, total - 1)
  const ran = await runRows(engine, payload, [...box(headRows), '--layout', layout, NO_ICONS])
  // Clipped here as well as asked for: a head that drew past `--rows` would
  // push the running tool out of the band, which is the row this is for.
  const head = { ...ran, rows: ran.rows.slice(0, headRows) }
  const left = total - head.rows.length
  if (head.fault !== null || left <= 0) return head
  const activity = await runRows(engine, payload, [
    ...box(left),
    '--layout',
    'activity',
    NO_ICONS,
  ])
  return { rows: [...head.rows, ...activity.rows], fault: activity.fault }
}

/**
 * Builds the payload from the engine, runs the binary, parses its rows.
 *
 * **Call it through `drawSoon`, never directly.** It guards its own writes
 * with `runSeq`, but a throw that gets past its internal handlers — an atom
 * read, the write at the end — leaves the atom untouched, and the band's note
 * is keyed on `bandRun` rather than on the row count: a direct caller that
 * lets the rejection go therefore makes the band go quiet about it. `drawSoon`
 * is where that rejection is recorded, under this run's own sequence number.
 *
 * Declared at the top of the file on purpose: the loader only lets `$` be
 * passed to a function declared here, never to a closure inside `register`.
 */
/**
 * A native notification, or a toast where no channel sends one.
 *
 * A session the desktop hosts alone has no terminal, so only the person's
 * Notification hooks run and `isSent` can be false; a hook's `{ deny }` makes
 * the call reject. Either way the toast still says it, so nothing is lost.
 * Never rejects: its callers float it.
 */
async function notifyOrToast(engine: EngineInterface, text: string): Promise<void> {
  try {
    const sent = await engine.ui.notify(text, { title: 'pulseline' })
    if (!sent.isSent) engine.ui.toast(text)
  } catch {
    engine.ui.toast(text)
  }
}

/**
 * Writes this desktop session's spend to `$.store` and reads every other's,
 * deleting what has expired. Each session owns its key, so nothing here is a
 * read-then-write of a value another process writes too.
 *
 * Returns the other sessions' spends, or `null` when this one has no cost to
 * say.
 */
async function syncSpend(
  engine: EngineInterface,
  sessionId: string,
  usd: number | undefined,
  now: number,
): Promise<SessionSpend[] | null> {
  if (usd === undefined) return null
  const own = `${SPEND_PREFIX}${sessionId}`
  // Throttled against what the store holds, not a remembered write: another
  // process's cleanup can read this key stale, and delete it after a fresh
  // write lands. Read back, a deleted key is written again on the next refresh.
  if (shouldWriteSpend(spendOf(await engine.store.get(own)), usd, now)) {
    await engine.store.set(own, { usd, at: now })
  }
  const keys = (await engine.store.keys()).filter(key => key.startsWith(SPEND_PREFIX) && key !== own)
  const others: SessionSpend[] = []
  for (const key of keys) {
    const spend = spendOf(await engine.store.get(key))
    // Malformed is deleted too: nothing this module writes looks like that.
    if (spend === null || now - spend.at > SPEND_EXPIRE_MS) await engine.store.delete(key)
    else others.push(spend)
  }
  return others
}

async function refresh(engine: EngineInterface, state: State): Promise<void> {
  // Each refresh claims a number and only the newest may publish. Several can
  // be in flight at once — parallel tool calls, a turn event landing beside
  // them — each with its own reading and its own spawn, and the write is a
  // plain overwrite. Without this the slowest finisher wins, so an older
  // reading can replace a newer one and leave a finished tool drawn as
  // running until the next event.
  const seq = (state.runSeq += 1)
  // The running list as it is *now*, not as it will be when the payload is
  // built: between here and there are a binary probe, a root read and seven
  // engine calls, and a tool that finishes in that window is deleted from
  // `state.active` by its own hook. Read late, this refresh would either
  // lose the tool it was started to draw, or — from another event's refresh
  // — serialise a tool that had already finished as still running.
  const running = [...state.active.values()]
  const publish = async (change: (out: DrawnOutput) => DrawnOutput): Promise<void> => {
    if (seq !== state.runSeq) return
    // Checked again *inside* the change, because `update` is a read-apply-write
    // with retries: it yields, and a newer refresh can claim the number and
    // publish in that window, which the outer check above has already passed.
    // The change runs inside the transaction and on each retry, so this is the
    // one that decides. Returning the value unchanged is the no-op.
    await update(engine, drawnOutput, out => (seq === state.runSeq ? change(out) : out))
  }

  // Before the ladder, because the dev rung is built from the root.
  // Its own statement: `syncVersion` never rejects, and a refused root read
  // must not cost the version its sync.
  await syncVersion(engine, state)
  try {
    await syncRoot(engine, state)
  } catch {
    // Not worth blanking the band for: the root already held is the one the
    // session started in, and `collect` below reports an engine refusing
    // calls through `stale`.
  }

  // Nothing to run means nothing to draw but the reason. The band draws
  // `runNote` when `bandRun` is `'failed'`, so this is what reaches it.
  const binary = await resolveBinary(engine, state)
  if (binary.kind !== 'ok') {
    const note = binary.kind === 'missing' ? missingMessage() : tooOldMessage(binary.version)
    // `'failed'`, not `'never'`: the ladder walked and came back empty-handed,
    // and this message is the only place an install line can reach the person.
    await publish(out => ({
      ...out,
      rows: [],
      desktopRows: [],
      paneRows: [],
      bandRun: 'failed',
      runNote: note,
    }))
    return
  }

  let collected: Collected
  let stale: string | null = null
  try {
    collected = await collect(engine, state)
    // Only the newest refresh may leave the fallback behind it. Unguarded, an
    // older refresh finishing its collection late replaced a newer reading —
    // and the next refresh whose own collection fails draws that older one,
    // with its stale usage and agent list, under the `stale:` note.
    if (seq === state.runSeq) state.lastCollected = collected
  } catch (error) {
    // Nothing has ever come back whole: there is no reading to draw, and
    // spawning the binary with fabricated zeros would draw a lie.
    if (state.lastCollected === null) {
      await publish(out => ({
        ...out,
        rows: [],
        desktopRows: [],
        paneRows: [],
        bandRun: 'failed',
        runNote: `engine refused — ${noteText(String(error)).slice(0, 160)}`,
      }))
      return
    }
    collected = state.lastCollected
    stale = noteText(String(error)).slice(0, 80)
  }

  // One read per atom the payload is built from. `collect` has already made
  // its own writes, so these are the values the payload goes on to describe.
  const [facts, totals, timings, loaded, written, tasks, pane, held] = await Promise.all([
    read(engine, sessionFacts),
    read(engine, toolTotals),
    read(engine, agentTimings),
    read(engine, instructions),
    read(engine, todoList),
    read(engine, taskItems),
    read(engine, paneWanted),
    read(engine, drawnOutput),
  ])

  // The Task API's row is folded here rather than written beside each change,
  // so it is always the committed items and never an older snapshot's fold.
  // Tasks win while any exist, because `todoFromTasks` answers `null` when
  // none do — which is also how a session that only ever calls `TodoWrite`
  // keeps its own row.
  const todo = todoFromTasks(tasks.items) ?? written

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
    active: running,
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
  let rows: Span[][]
  let desktopRows: Span[][]
  // Normalized, because the annotation is a promise about the contract and
  // not about what the host is holding: `$.state` outlives a reload,
  // including the reload that upgrades the plugin, and an object written
  // before `bandRun` existed has no such field. The atom's initial value
  // only applies when the key is absent, so without this a run that somehow
  // reached the write without setting it would persist `undefined`.
  let bandRun: BandRun = held.bandRun ?? 'never'
  let runNote = held.runNote
  let paneRows: Span[][] = held.paneRows
  let paneNote = held.paneNote

  // Which bands to run. An empty roster is a `-p` run, which has always
  // spawned the terminal's and still does; a session the desktop app hosts
  // alone has no terminal band to draw, so it is not spawned for.
  const surfaces = collected.surfaces
  const onTerminal = surfaces.length === 0 || surfaces.includes('terminal')
  const onDesktop = surfaces.includes('desktop')
  const quiet = desktopLayoutOf(state.options) === QUIET

  // The quiet band's strip and alert rows, and a notification for each quota
  // window that newly passed its limit. Worked out here, never in the render
  // hook, which may not write state.
  const drawsQuiet = onDesktop && quiet
  // `?? null` / `?? []`: an atom written before the runway existed has neither.
  // The last turn's sample, not this reading's `context.tokens`: the
  // threshold is on the breakdown's count, and so is the series.
  const ctxAt = facts.ctxTokens ?? null
  const autoCompact = facts.autoCompact ?? null
  const runway =
    ctxAt === null || autoCompact === null || !autoCompact.enabled
      ? null
      : runwayOf(ctxAt, facts.ctxDeltas ?? [], autoCompact.threshold)
  const alerts: Alert[] = drawsQuiet ? desktopAlerts(collected.usage, facts.tokens, runway) : []
  let strip = drawsQuiet ? stripOf(collected.usage, collected.now) : null
  // Every desktop session says what it spends, whatever its layout, so a
  // quiet one counts the rest; only the quiet band draws the share.
  let spendFault: string | null = null
  if (onDesktop) {
    const usd = collected.usage.cost?.usd
    try {
      const others = await syncSpend(engine, collected.sessionId, usd, collected.now)
      const share = others === null || usd === undefined ? null : shareOf(usd, others, collected.now)
      if (strip !== null && share !== null) strip = { ...strip, share }
    } catch (error) {
      // The band draws on without the share; the note says why it is missing.
      spendFault = `share: store refused — ${noteText(String(error)).slice(0, 80)}`
    }
  }
  state.notified ??= new Set(held.notified ?? [])
  const toNotify: string[] = []
  for (const quota of drawsQuiet ? quotaPast(collected.usage) : []) {
    const key = quotaKey(quota)
    if (state.notified.has(key)) continue
    // Marked before the call: a second refresh in flight must not notify it again.
    state.notified.add(key)
    toNotify.push(quotaNotice(quota, collected.now))
  }
  for (const text of toNotify) void notifyOrToast(engine, text)

  // The two band runs are independent, so they run together: in sequence, a
  // hanging binary held both bands for two timeouts before either updated.
  const [terminalRun, desktopRun] = await Promise.all([
    onTerminal
      ? runRows(engine, payload, [
          binary.path,
          '--rows',
          String(state.bandRows),
          '--columns',
          String(state.bandCols),
          // The band is not a second statusline. `activity` is the one layout
          // that draws no identity and no budget row — those belong to the
          // statusline, which is always on screen and is the only path that can
          // sample tokens often enough to rate them. What is left is the turn in
          // flight, led by the running tools so `--rows 2` outside fullscreen
          // cuts the cumulative counts rather than the live row. Without this the
          // band drew whatever layout the person configured: the statusline's own
          // rows, stacked above the prompt while the statusline drew them below.
          '--layout',
          'activity',
        ])
      : null,
    // A run of its own, because the desktop band is a different layout in a
    // different face: the `desktopLayout` setting (see `DESKTOP_LAYOUTS`),
    // with no Nerd Font glyphs.
    // The quiet band draws from the reading itself, so it spawns nothing.
    onDesktop && !quiet ? runDesktopBand(engine, payload, binary.path, state) : null,
  ])

  rows = terminalRun?.rows ?? []
  desktopRows = desktopRun?.rows ?? []
  // Zero rows with a zero exit is the idle band, not a fault — which is why
  // the health of the run is tracked apart from the count of its rows.
  const done: string[] = []
  const faults: string[] = []
  if (terminalRun !== null) {
    if (terminalRun.fault === null) {
      done.push(`${rows.length}r band ${state.bandRows}x${state.bandCols}`)
    } else faults.push(terminalRun.fault)
  }
  if (spendFault !== null) done.push(spendFault)
  if (drawsQuiet) {
    done.push(`strip + ${alerts.length} alert(s) desktop quiet`)
  } else if (desktopRun !== null) {
    if (desktopRun.fault === null) {
      done.push(`${desktopRows.length}r desktop ${state.desktopBandRows}x${state.desktopBandCols}`)
    } else faults.push(`desktop ${desktopRun.fault}`)
  }

  if (onTerminal || onDesktop) {
    bandRun = faults.length > 0 ? 'failed' : 'ok'
    runNote =
      faults.length > 0
        ? faults.join(' · ')
        : `${done.join(' · ')} · ${running.length} running · ${totals.completedTotal} done · ${paneNote}`
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
      // One row short of the box on purpose: the render hook puts the layout
      // switcher under these rows, so a layout that does honour `--rows` and
      // fills the box exactly would push the switcher — and its hotkeys —
      // below the fold.
      const paneBodyRows = Math.max(1, state.paneHeight - 1)
      const paneArgv = [
        binary.path,
        '--rows',
        String(paneBodyRows),
        '--columns',
        String(state.paneCols),
        '--layout',
        pane.layout,
        // One pane run serves every surface that shows it, so ASCII as soon
        // as the desktop is one of them: readable on a terminal, where a
        // Nerd Font glyph in the desktop's face is a box.
        ...(onDesktop ? [NO_ICONS] : []),
      ]
      const paneRun = await engine.process.run(paneArgv, { stdin: payload, timeoutMs: 5000 })
      paneRows = paneRun.stdout
        .split('\n')
        .filter(line => line.length > 0)
        .map(spansOf)
      paneNote =
        paneRun.exitCode === 0
          ? `pane: ${paneRows.length}r ${pane.layout} at ${state.paneHeight}x${state.paneCols}`
          : `pane: exit ${paneRun.exitCode} — ${noteText(paneRun.stderr).slice(0, 120)}`
    } catch (error) {
      paneRows = []
      paneNote = `pane: run failed — ${noteText(String(error)).slice(0, 160)}`
    }
  }

  const notified = [...state.notified]
  await publish(() => ({
    rows,
    desktopRows,
    alerts,
    strip,
    notified,
    bandRun,
    runNote,
    paneRows,
    paneNote,
  }))
}

/**
 * Writes the turn's effort level, when the hook that saw it carried one.
 *
 * Declared at the top of the file because the loader only lets `$` be passed
 * to a function declared here — a closure inside `register` is refused, and
 * the refusal is a module that does not load at all.
 */
async function rememberEffort(engine: EngineInterface, level: string | undefined): Promise<void> {
  if (level) await update(engine, sessionFacts, facts => ({ ...facts, effort: level }))
}

/**
 * Starts a refresh and does not wait for it, noting a failure on the band.
 *
 * The rule every site follows: await the drawing only where a person is
 * waiting for the drawing itself — `/pulseline`, a keypress, the session's
 * first draw. Everywhere else the engine is waiting on the hook, and a band
 * is not worth holding up the model or the turn for: the binary's
 * `timeoutMs` is five seconds, twice over with the pane open.
 *
 * The engine's contract floats work out of a hook the same way.
 * `$.process.spawn`'s second example runs its loop `void (async () => …)()`
 * from `session.start`, and names what ends the child: the loop, the stream,
 * the signal, the module unloading, and "nothing else does, a hook's return
 * included". So a captured `$` outlives the hook that took it.
 *
 * The promise it hands back is already handled, so chaining onto it is safe
 * and dropping it leaves nothing unhandled.
 */
function drawSoon(engine: EngineInterface, state: State): Promise<void> {
  // `refresh` claims its sequence number in its first statement, and an
  // `async function` runs to its first `await` synchronously — so the number
  // read here is the one the run below claimed. That is what lets this
  // handler, which sits outside `refresh`, apply the same rule `publish`
  // does.
  const run = refresh(engine, state)
  const seq = state.runSeq
  return run.catch(async error => {
    const note = `refresh failed: ${noteText(String(error)).slice(0, 120)}`
    // `bandRun` as well as the note. The band draws its note only for
    // `'failed'`, so a rejection that left `'ok'` behind would hold the rows
    // of the run before it and say nothing about the one that threw. This is
    // the only handler for a throw that got past `refresh`'s own — an atom
    // read, or the write at its end — so there is nowhere else to record it.
    //
    // Sequence-guarded inside the change, exactly as `publish` is: several
    // refreshes are in flight at once, and an older one rejecting after a
    // newer one has published must not stamp `'failed'` over the newer rows,
    // because the render hook puts the note ahead of the rows it holds.
    // Annotated for the same reason `publish`'s callers are not: nothing here
    // types the literal, so `'failed'` would widen to `string`.
    await update(engine, drawnOutput, (out): DrawnOutput =>
      seq === state.runSeq ? { ...out, bandRun: 'failed', runNote: note } : out,
    ).catch(() => undefined)
  })
}

/**
 * Refreshes only if the last run is older than `gapMs`.
 *
 * One caller is left: `classic.ConfigChange`, which a single settings write
 * can raise several times over. Every other site refreshes unconditionally,
 * because a gated refresh is not a draw made late — it is a draw not made at
 * all, and the band's rows are only ever the binary's last stdout.
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
  await drawSoon(engine, state)
}

export const register: Register = (on, options) => {
  const state: State = {
    active: new Map(),
    lastRunMs: 0,
    bandRows: 3,
    bandCols: 120,
    desktopBandRows: 3,
    desktopBandCols: 120,
    desktopBandMeasured: false,
    paneHeight: 12,
    paneCols: 120,
    mcpServers: new Set(),
    lastCollected: null,
    runSeq: 0,
    rootRead: null,
    versionFromEngine: false,
    options,
    notified: null,
  }

  on('session.start', async ($, e, next) => {
    const result = await next(e)
    // The seed, not the source: `syncRoot` reads the root before every ladder
    // walk, and this is the value it falls back to if that read is refused.
    const cwd = e.cwd
    await update($, sessionFacts, facts => ({ ...facts, cwd }))
    // Scaffolding. `$.ui.status` pins an entry beside the prompt and
    // `$.ui.log` writes a transcript line, and a plugin a person installs
    // should say neither: what the band cannot draw, the band itself says.
    //
    // Cleared rather than merely not set. A status entry belongs to the
    // session, not to the load that pinned it, so a version that used to pin
    // one leaves it beside the prompt for the rest of that session — through
    // every reload, because the module that would have replaced it no longer
    // says anything. `undefined` is what takes it down.
    if (DEBUG) $.ui.status('alive')
    else $.ui.status(undefined)
    await probeVersion($, state)
    try {
      await $.command.register({
        name: 'pulseline',
        description: 'Open the statusline in a pane, where it has room for activity rows',
      })
    } catch (error) {
      const note = `command refused — ${noteText(String(error)).slice(0, 120)}`
      // `'failed'` with it. A note the band will not draw is a note nobody
      // reads, and since Phase E the band draws one only for `'failed'`. The
      // `refresh` on the next line replaces both fields the moment the binary
      // answers, which is the right lifetime for it: the band works, so it
      // should go back to drawing activity.
      await update($, drawnOutput, (out): DrawnOutput => ({
        ...out,
        bandRun: 'failed',
        runNote: note,
      }))
    }
    await drawSoon($, state)
    if (DEBUG) $.ui.log((await read($, drawnOutput)).runNote)
    return result
  })

  on('tool.call', async ($, e, next) => {
    const seen = e as unknown as Record<string, unknown>
    const name = String(seen.tool ?? 'tool')
    const id = String(seen.tool_use_id ?? name)

    // The draw of this tool as running, started below and chained onto by the
    // `finally`'s floated draw. Never null once the mark succeeded.
    let drawing: Promise<void> | null = null

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
      // minutes was therefore never shown.
      //
      // Started here and never awaited, so it runs *beside* the tool rather
      // than in front of it. An observational mod must not put its own spawn
      // — five seconds of timeout, twice over with the pane open — between
      // the model and every tool call. The `finally`'s draw chains onto this
      // promise rather than racing it: run together they would both walk the
      // binary ladder, and the sequence guard would drop whichever lost.
      drawing = drawSoon($, state)
    } catch (error) {
      const note = `mark failed: ${noteText(String(error)).slice(0, 120)}`
      // `'failed'` with it, or this report cannot be read: the band draws a
      // note only for `'failed'`. The completion refresh in the `finally`
      // clears it when it succeeds, so what it costs is one draw.
      await update($, drawnOutput, (out): DrawnOutput => ({
        ...out,
        bandRun: 'failed',
        runNote: note,
      }))
    }

    let failed = false
    try {
      const result = await next(e)
      // A deny is its own variant of `ToolCallResult`, and it sets
      // `isError?: undefined` — so `isError` alone read a refused call as a
      // clean completion: it went into the session totals as done, and a
      // denied `TodoWrite` was applied as the current list by the `!failed`
      // branch below. The model reads a deny as an error result, and the
      // transcript path counts it as one, so the two paths agree here.
      failed = result.isError === true || result.deny !== undefined
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
      // Floated, and so never between the settled tool result and the model.
      // Awaited here it was: `timeoutMs: 5000`, twice over with the pane open,
      // is how long an observational mod could hold up the model's next step
      // for a drawing nobody is waiting on. The engine's own contract floats
      // work out of a hook exactly this way — `$.process.spawn`'s second
      // example runs its loop `void (async () => …)()` from `session.start`,
      // and says what kills the child: the loop ending, the stream, the
      // signal, the module unloading, and "nothing else does, a hook's
      // return included". So a captured `$` outlives the hook that took it.
      //
      // The order the awaits used to buy is bought by the chain instead. The
      // start's draw settles first, so `state.active` still holds this tool
      // while that draw reads it and no longer does when this one does; and
      // the ladder is still walked once a session, because the start's
      // refresh has written `binaryChoice` before this one reads it.
      void (async () => {
        try {
          // Bookkeeping first, and not behind the start's draw. Held there,
          // this tool stayed in `state.active` for as long as that draw took
          // — and any other refresh in that window, the turn ending or the
          // next tool starting, drew it as still running. With a slow binary
          // and the pane's second render that phantom lasted seconds.
          // `refresh` takes its own snapshot of the list when it starts, so
          // the start's draw already holds what it was started to draw.
          state.active.delete(id)
          // The count lands before the refresh, and in its own write: an
          // overrun inside `update` must not cost this tool its place in the
          // session totals.
          if (!NOT_TOOL_ACTIVITY.has(name)) {
            // Read before the write, because `update` retries and the stamp
            // must be when the tool finished rather than when the transaction
            // happened to win.
            const finishedAt = await $.clock.now()
            await update($, toolTotals, totals => ({
              counts: {
                ...totals.counts,
                [name]: {
                  count: (totals.counts[name]?.count ?? 0) + 1,
                  failed: (totals.counts[name]?.failed ?? 0) + (failed ? 1 : 0),
                  lastAt: finishedAt,
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
          //
          // Chained behind the start's draw even so: run together, both would
          // walk the binary ladder before either had written its answer, and
          // two of the probing cases say so in exact numbers.
          if (drawing !== null) await drawing
          // `drawSoon` rather than `refresh`: a rejection here is a refresh
          // that threw, and the sequence guard that decides whether it may
          // still claim the band lives there.
          await drawSoon($, state)
          if (DEBUG) $.ui.status((await read($, drawnOutput)).runNote)
        } catch (error) {
          const note = `refresh failed: ${noteText(String(error)).slice(0, 120)}`
          // Guarded like the start site's, because this write *is* the place
          // the failure gets reported: unguarded, a throw here would reach the
          // backstop below and the note would be lost without a word.
          // The note alone, and `bandRun` deliberately left where it is: a
          // refresh that threw has already recorded itself under its own
          // sequence number inside `drawSoon`, so what reaches here is a
          // throw with no run behind it — and a failure that cannot name a
          // run must not outrank the rows a newer one published. Under
          // `DEBUG` the status entry below still says it.
          await update($, drawnOutput, out => ({ ...out, runNote: note })).catch(
            () => undefined,
          )
          if (DEBUG) $.ui.status(note)
        }
        // Nothing above is awaited by the hook any more, so a throw here would
        // be a rejection with no handler. The band is observational and the
        // note above has already had its own try: there is nowhere left to
        // report a failure to report a failure.
      })().catch(() => undefined)
    }
  }).catch(($, e, next) => next(e))

  // `BaseHookInput.effort` is carried by the classic hooks that fire "within a
  // tool-use context (PreToolUse, PostToolUse, Stop, SubagentStop, etc.) …
  // absent for session-lifecycle hooks". `classic.PreToolUse` is out even so:
  // its argument is the tool call envelope rather than the hook input.
  //
  // So two of them. `PostToolUse` is the frequent one, and `Stop` is what a
  // turn with no tool calls in it has — the only place such a turn's effort
  // can be read at all, since `turn.start` carries just `text` and `turnId`
  // and the one event with the request's own effort is `turn.step`, which
  // streams and sits on every model request (the deferred serve-mode work).
  //
  // What stays wrong, because nothing carries it earlier: the effort of a
  // turn is unknown until something in it finishes, so the first tool of the
  // first turn of a session draws without the pip ramp, and a level the
  // person changes mid-turn reaches the band when that turn's first tool
  // returns rather than when they changed it.
  on('classic.PostToolUse', async ($, e, next) => {
    await rememberEffort($, e.effort?.level)
    return next(e)
  }).catch(($, e, next) => next(e))

  // A turn's end, so a turn that called no tool still records the level. Not
  // `SubagentStop`, which carries the subagent's own effort rather than the
  // session's.
  on('classic.Stop', async ($, e, next) => {
    await rememberEffort($, e.effort?.level)
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
      // Native separators on Windows, where `file_path` arrives as
      // `C:\repo\.claude\rules\testing.md` — matched slash-only, those
      // rules and memories fell out of the counts entirely and the band went
      // back to the filesystem scan it is meant to correct.
      const unix = path.replace(/\\/g, '/')
      if (unix.includes('/rules/')) {
        return loaded.rules.includes(path) ? loaded : { ...loaded, rules: [...loaded.rules, path] }
      }
      if (unix.includes('/memory/')) {
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
    // Floated, as the other observational sites are: nobody is waiting on
    // this drawing, and a settings write is not the place to hold the engine
    // up for one.
    void maybeRefresh($, state, 400).catch(() => undefined)
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
    //
    // Floated as well as ungated, and here it matters most: awaited, this one
    // sat *in front of* `next(e)`, so the subagent could not start until the
    // band had drawn.
    void drawSoon($, state)
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
    // so neither end is a burst to bound. Floated for the matching reason too.
    void drawSoon($, state)
    return next(e)
  }).catch(($, e, next) => next(e))

  // The compaction badge: the transcript parser counts `compact_boundary`
  // events, and only a compaction that finished writes one. `PostCompact` is
  // the event that matches it — it carries the summary a compaction produced,
  // so it fires once one has.
  //
  // `PreCompact` was the wrong end. It can be blocked: `SessionCompactSkipped`
  // says "Core answers it too, when a classic PreCompact hook blocks", and the
  // count went up before `next(e)` either way — so a compaction that was
  // vetoed, or that failed, left the badge raised for the rest of the session
  // and sent on every payload after it.
  on('classic.PostCompact', async ($, e, next) => {
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
    // The quiet desktop band's autocompact runway: where the context stands
    // after each main turn, and where autocompact runs. A subagent's turn
    // leaves the main context as it was, and an errored one may not have
    // grown it, so neither is a sample.
    let context: SessionContextUsage | null = null
    if (e.agentId === undefined && e.reason !== 'error') {
      try {
        // `summary` counts locally and sends no request. The series is the
        // breakdown's own total, the count its threshold applies to: the
        // status line's `context.tokens` is the last request's input alone,
        // which lags a turn that ended on a long answer or tool output.
        context = (await $.session.usage({ breakdown: 'summary' })).context
      } catch {
        // Kept as it was: a runway one turn stale beats a turn that fails on
        // the band's account. The hook observes, and the turn's result stands.
      }
    }
    const breakdown = context?.breakdown
    const sampled = breakdown?.totalTokens
    if (usage || erroredAt !== null || context !== null) {
      await update($, sessionFacts, facts => ({
        ...facts,
        // No breakdown, no sample. A compaction shows as a smaller total,
        // which starts the series over.
        ...(sampled !== undefined
          ? {
              ctxTokens: sampled,
              ctxDeltas: nextDeltas(facts.ctxTokens ?? null, facts.ctxDeltas ?? [], sampled),
            }
          : {}),
        autoCompact:
          breakdown === undefined
            ? (facts.autoCompact ?? null)
            : { threshold: breakdown.autoCompactThreshold ?? null, enabled: breakdown.isAutoCompactEnabled },
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
    // Floated, like the tool path's. Awaited, a hanging binary withheld the
    // turn's own result for five seconds, or ten with the pane open — after
    // every turn.
    void drawSoon($, state)
    // A long turn's end, said where the desktop is attached: the person has
    // likely gone to another window. Not a subagent's run, which ends inside
    // the turn that spawned it, and not an interrupt, which the person made.
    if (e.agentId === undefined && !e.isAborted && e.durationMs >= LONG_TURN_MS) {
      if ((await $.session.surfaces()).includes('desktop')) {
        void notifyOrToast($, longTurnNotice(e.durationMs))
      }
    }
    return result
  })

  // The desktop app joining a session already under way. Its band has no rows
  // until a refresh runs for it, and nothing else would run one before the
  // next tool call — so a desktop that attached between turns would show no
  // context or cost until the model next did something.
  on('session.attach', { surface: 'desktop' }, async ($, e, next) => {
    const result = await next(e)
    void drawSoon($, state)
    return result
  }).catch(($, e, next) => next(e))

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    // The terminal and the desktop app, the two surfaces that raise the band.
    // Each has rows of its own, run for its own layout and glyphs.
    const desktop = e.surface === 'desktop'
    if (e.surface !== 'terminal' && !desktop) return next(e)
    // A survey owns the band while it is up; yield rather than fight it.
    if (e.props.hasSurvey) return next(e)

    // Read before anything can return: a read while a render hook draws
    // subscribes this drawing, so the write at the end of `refresh` draws
    // the band again by itself, with nobody calling `$.ui.invalidate`.
    const held = await read($, drawnOutput)

    // What the mods beneath this one draw. The band stacks its rows on top of
    // it rather than in its place: a hook that answered with its own tree
    // alone took the whole chain's band, so another mod's `AbovePrompt`
    // content — a row of buttons, say — vanished whenever this one had rows,
    // which on the desktop is always.
    const below = await next(e)

    // Measured before anything returns, so a terminal resized while nothing
    // is running is still the width the next refresh spawns against. A
    // closure field, not an atom, because `$.state.set` is refused while a
    // render hook draws.
    // Two cells go to the band's own collapse marker at the right edge.
    if (desktop) {
      const desktopRows = e.props.maxRows
      const desktopCols = Math.max(20, e.props.bodyColumns - 2)
      const moved =
        !state.desktopBandMeasured ||
        desktopRows !== state.desktopBandRows ||
        desktopCols !== state.desktopBandCols
      state.desktopBandRows = desktopRows
      state.desktopBandCols = desktopCols
      state.desktopBandMeasured = true
      // The terminal band lives with a box one refresh stale, because the next
      // tool call respawns it at the width measured here. The desktop band is
      // drawn idle as well — it carries the only context and cost the desktop
      // shows — so a box guessed before it was measured, or one the window has
      // since resized away from, would stand until the model next did
      // something: rows packed for 3x120, clipped rather than refitted.
      //
      // Floated, never awaited: the state write at the end of the refresh is
      // refused while a drawing is in flight, and this one returns first. Gated
      // on a change, so the redraw that write causes measures the same box and
      // does not refresh again.
      if (moved) void drawSoon($, state)
    } else {
      state.bandRows = e.props.maxRows
      state.bandCols = Math.max(20, e.props.bodyColumns - 2)
    }
    // `?? []`: an atom written before the desktop band existed has no field.
    const bandRowsHeld = desktop ? (held.desktopRows ?? []) : held.rows

    // A run that failed still draws, whether or not a turn is running: a band
    // that silently disappears tells us nothing about why, and this is the
    // only place a missing or too-old binary can say so. An install line
    // nobody sees fixes nothing.
    //
    // Keyed on how the run ended rather than on how many rows it produced.
    // The band asks for the `activity` layout, whose output is empty whenever
    // nothing is running, so zero rows is the ordinary state between turns —
    // and `rows.length === 0` here would pin this note over an idle prompt
    // for the whole session.
    if (held.bandRun === 'failed' && !DEBUG) {
      const { Box, Text } = $.ui.resolve(e)
      return (
        <Box flexDirection="column">
          <Text mono>{`pulseline: ${held.runNote}`}</Text>
          {below}
        </Box>
      )
    }

    // The quiet band: the strip, then one row for each alert, all stacked on
    // what the mods beneath draw. Every path keeps `below`. No binary rows
    // here, so nothing is drawn from `desktopRows`.
    if (e.surface === 'desktop' && desktopLayoutOf(state.options) === QUIET) {
      // `?? []` / `?? null`: an atom written before the quiet band existed
      // has neither field.
      const alerts = held.alerts ?? []
      const cells = stripCells(held.strip ?? { quota: [] }, state.lastRunMs)
      const share = held.strip?.share ?? null
      const { Box, Text, Button, Svg } = $.ui.resolve(e)
      const rows = alerts.map(alert => {
        const row = alertRow(alert)
        const parts = [
          <Text color={row.color} bold mono>
            {row.label}
          </Text>,
          <Text dimColor mono>
            {row.detail}
          </Text>,
        ]
        if (row.gauge !== null) {
          parts.unshift(<Svg source={row.gauge.source} alt={row.gauge.alt} width={120} height={10} />)
        }
        if (row.compact) {
          // The press raises `ui.press`, handled below; this is its bottom.
          parts.push(<Button key={COMPACT_KEY} label="/compact" onPress={() => {}} />)
        }
        return (
          <Box flexDirection="row" gap={1} alignItems="center">
            {parts}
          </Box>
        )
      })
      if (cells.length > 0) {
        // A gauge cell is a column: its label line over its pace and its
        // detail, so neither wraps beside the gauge.
        const column = (cell: StripCell) => (
          <Box flexDirection="column">
            <Box flexDirection="row" gap={1} alignItems="center">
              {cell.gauge !== null ? (
                <Svg source={cell.gauge.source} alt={cell.gauge.alt} width={64} height={cell.gauge.height} />
              ) : null}
              <Text color={cell.color} mono>
                {cell.label}
              </Text>
            </Box>
            {cell.pace !== null ? (
              <Text color={cell.pace.color} dimColor={cell.pace.color === undefined} mono>
                {cell.pace.text}
              </Text>
            ) : null}
            {cell.detail !== null ? (
              <Text dimColor mono>
                {cell.detail}
              </Text>
            ) : null}
          </Box>
        )
        // The share of spend sits under the gauges, part of the strip: it
        // goes when the strip does.
        rows.unshift(
          <Box flexDirection="column">
            <Box flexDirection="row" gap={2} alignItems="flex-start">
              {cells.map(column)}
            </Box>
            {share !== null ? (
              <Text dimColor mono>
                {shareLine(share)}
              </Text>
            ) : null}
          </Box>,
        )
      }
      if (rows.length === 0) return below
      // Past `maxRows`, the strip goes before an alert: an alert asks for an
      // action, the strip is there to glance at.
      // The strip is one box but stands several rows, so it is counted as tall
      // as it draws: counted as one, it pushed an alert past the band's bottom.
      const room = Math.max(1, e.props.maxRows)
      const tall = stripRows(cells, share !== null) + alerts.length > room
      const shown = tall && cells.length > 0 && alerts.length > 0 ? rows.slice(1) : rows
      return (
        <Box flexDirection="column">
          {shown.slice(0, room)}
          {below}
        </Box>
      )
    }

    // Drawn whether or not a turn is running, so the band holds the state it
    // last drew: the tools that finished, their failures, the agents that
    // ran and the todo, every one of them marked done by the glyph the
    // binary already puts there. `e.props.isWorking` used to gate this, and
    // for a release it had to — the band drew the layout the person had
    // configured, which is the identity and budget rows the statusline draws
    // on its own line, so between turns the two were the same thing twice.
    // Asking for `activity` ended that: what is left between turns is the
    // turn that just happened, which the statusline never shows.
    //
    // Nothing more is needed to keep it there. `turn.complete` refreshes, and
    // the `read` above subscribes this drawing to the atom that refresh
    // writes, so the rows settle to the finished state by themselves and then
    // stand until the next turn moves them.
    //
    // Zero rows still yields: before the first tool call there is nothing to
    // hold, and the activity layout draws nothing when nothing has run.
    // On the desktop this is only the moment before the first run: `compact`
    // always draws its head row.
    if (bandRowsHeld.length === 0 && !DEBUG) return below

    const { Box, Text } = $.ui.resolve(e)
    const notes = DEBUG ? [<Text>{held.runNote}</Text>] : []
    // Clip to what the band shows whole; a taller tree hides behind `n more`.
    // Only this mod's own rows: what the mods beneath drew is theirs to size.
    const drawn = bandRowsHeld.slice(0, Math.max(1, e.props.maxRows - notes.length))

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
            {/*
              `mono`, because the desktop draws a Text in a proportional face
              by default, and the binary packs its columns for a monospaced
              one. The terminal is always monospaced and ignores it.
            */}
            {spans.map(({ children, ...style }) => (
              <Text {...style} mono>
                {children}
              </Text>
            ))}
          </Box>
        ))}
        {below}
      </Box>
    )
  }).catch(($, e, next) => next(e))

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
    await drawSoon($, state)
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
    if (opened.isPlaced) await drawSoon($, state)
    else {
      // Nothing was drawn, so there is nothing to keep the flag for: left
      // true, every later refresh would spawn the binary a second time for a
      // pane that does not exist, and the next `/pulseline` would take the
      // close branch and report closing it rather than trying again.
      await update($, paneWanted, pane => ({ ...pane, open: false }))
      // Not "terminal": the desktop app can decline to place a pane as well.
      return { text: 'pulseline could not place its pane here — a wider window may fit it' }
    }
    return { text: undefined }
  })

  on('ui.render', { component: 'Pane' }, async ($, e, next) => {
    if (e.requestId !== PANE_ID) return next(e)
    if (e.surface !== 'terminal' && e.surface !== 'desktop') return next(e)

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
            {/* Split, and `mono`, for the same reasons as the band's rows above. */}
            {spans.map(({ children, ...style }) => (
              <Text {...style} mono>
                {children}
              </Text>
            ))}
          </Box>
        ))}
        {switcher}
      </Box>
    )
  })

  // The quiet band's `/compact` button: it puts the command in the prompt
  // and leaves the sending to the person. A draft already there is theirs, so
  // it is not replaced; the toast says what to do instead.
  on('ui.press', { element: COMPACT_KEY }, async ($, e, next) => {
    const box = await $.prompt.read()
    if (box.text.trim().length > 0) {
      $.ui.toast('The prompt holds a draft. Send or clear it, then press /compact again.')
    } else {
      await $.prompt.fill({ text: '/compact', mode: 'replace' })
    }
    return next(e)
  }).catch(($, e, next) => next(e))

  on('ui.press', { requestId: PANE_ID }, async ($, e, next) => {
    if (!e.element.startsWith(LAYOUT_KEY)) return next(e)
    const layout = e.element.slice(LAYOUT_KEY.length)
    await update($, paneWanted, pane => ({ ...pane, layout }))
    await drawSoon($, state)
    return next(e)
  })

  on('ui.close', { id: PANE_ID }, async ($, e, next) => {
    await update($, paneWanted, pane => ({ ...pane, open: false }))
    return next(e)
  }).catch(($, e, next) => next(e))
}
