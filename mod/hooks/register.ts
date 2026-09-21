import type { EngineInterface, On, TextProps } from 'claude-code'

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
 */

const PANE_ID = 'pulseline'

/**
 * The layouts the pane's switcher row offers, in order.
 *
 * `console` is left out on purpose: inside a pane it reads as `ledger` with
 * a label column, and four visibly different views are worth more than five
 * where two look alike.
 */
const LAYOUTS = ['ledger', 'budgets', 'compact', 'rail']

/** Button key prefix, so a press can name the layout it asks for. */
const LAYOUT_KEY = 'layout:'

const BINARY = '/Users/gregho/GitHub/AI/cc-pulseline/target/release/cc-pulseline'

/** Set true to draw the band/run diagnostics above the statusline rows. */
const DEBUG = false

/** One run of text carrying the style the SGR codes before it set. */
type Span = TextProps & { children: string }

/** Turn token counts, kept from the last `turn.complete`. */
type Tokens = {
  input_tokens: number
  output_tokens: number
  cache_read_input_tokens: number
  cache_creation_input_tokens: number
}

// Built from a char code on purpose: a literal escape in this source
// does not survive every editor round-trip.
const ESC = String.fromCharCode(27)
const ANSI_PATTERN = new RegExp(ESC + '\\[([0-9;]*)m', 'g')

/** xterm-256 index to `#rrggbb`: 16 base colours, a 6^3 cube, a 24-step ramp. */
function ansi256Hex(index: number): string {
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
function spansOf(line: string): Span[] {
  const spans: Span[] = []
  let style: TextProps = {}
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
      else if ((code === 38 || code === 48) && codes[at + 1] === '5') {
        const hex = ansi256Hex(Number(codes[at + 2]))
        style = code === 38 ? { ...style, color: hex } : { ...style, backgroundColor: hex }
        at += 2
      }
    }
  }
  push(line.slice(cursor))
  return spans
}

/** One running tool call, keyed by its `tool_use_id`. */
type ActiveTool = { id: string; name: string; target?: string }

/** Session totals per tool name, as the completed-counts row shows them. */
type ToolCount = { count: number; failed: number }

/** The todo list as the last TodoWrite call left it. */
type Todo = {
  text: string
  pending: number
  completed: number
  total: number
  in_progress_items: { text: string }[]
  all_done: boolean
  is_task_api: boolean
}

/** What the hooks collect between draws. */
type State = {
  cwd: string
  tokens: Tokens | null
  rows: Span[][]
  runNote: string
  active: Map<string, ActiveTool>
  counts: Map<string, ToolCount>
  completedTotal: number
  failedTotal: number
  startedAt: Map<string, number>
  todo: Todo | null
  lastRunMs: number
  bandRows: number
  bandCols: number
  paneOpen: boolean
  paneRows: Span[][]
  paneHeight: number
  paneCols: number
  paneNote: string
  version: string | null
  paneLayout: string
}

/**
 * The argument a tool row shows beside the tool name.
 *
 * `tool.call` carries the tool's own arguments on `e`, so this is a lookup,
 * not the guess-by-shape that the transcript parser has to make.
 */
function targetOf(event: Record<string, unknown>): string | undefined {
  const keys = ['file_path', 'command', 'path', 'pattern', 'url', 'prompt', 'description']
  for (const key of keys) {
    const value = event[key]
    if (typeof value === 'string' && value.length > 0) return value.slice(0, 120)
  }
  return undefined
}

/**
 * Turns a model id into the name the statusline shows.
 *
 * The engine hands out ids (`claude-opus-5[1m]`); the statusline payload
 * that Claude Code itself writes carries a display name, and there is no
 * call for that one, so this rebuilds it.
 */
function modelName(id: string): string {
  const isMillion = id.includes('[1m]')
  const base = id
    .replace('[1m]', '')
    .replace(/^claude-/, '')
    .replace(/-\d{8}$/, '')
  const words = base
    .split('-')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
  return isMillion ? `${words} (1M context)` : words
}

/** Folds one TodoWrite call's arguments into the todo row's shape. */
function todoOf(items: { content?: string; status?: string; activeForm?: string }[]): Todo {
  const running = items.filter(item => item.status === 'in_progress')
  const completed = items.filter(item => item.status === 'completed').length
  const pending = items.filter(item => item.status === 'pending').length
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
async function probeVersion(engine: EngineInterface, state: State): Promise<void> {
  if (state.version !== null) return
  const fromEnv = await engine.env.get('CLAUDE_CODE_EXECPATH')
  const guess = fromEnv?.split('/').pop()
  if (guess && /^\d+\.\d+\.\d+$/.test(guess)) {
    state.version = guess
    return
  }
  try {
    const run = await engine.process.run(['claude', '--version'], { timeoutMs: 10000 })
    const found = run.stdout.match(/\d+\.\d+\.\d+/)
    if (found) state.version = found[0]
  } catch {
    state.version = null
  }
}

/**
 * Builds the payload from the engine, runs the binary, parses its rows.
 *
 * Declared at the top of the file on purpose: the loader only lets `$` be
 * passed to a function declared here, never to a closure inside `register`.
 */
async function refresh(engine: EngineInterface, state: State): Promise<void> {
  const usage = await engine.session.usage()
  const model = await engine.session.model()
  const id = await engine.session.id()
  const agents = await engine.agent.list()
  const now = await engine.clock.now()
  // Effort rides on `turn.step`, a streaming event this mod does not hook;
  // Claude Code also exports it to child processes, which is cheaper.
  const effort = await engine.env.get('CLAUDE_EFFORT')
  state.lastRunMs = now

  const limits: Record<string, { used_percentage: number; resets_at: number }> = {}
  for (const limit of usage.rateLimits) {
    limits[limit.kind] = {
      used_percentage: limit.percentUsed,
      resets_at: limit.resetsAt ? Math.floor(Date.parse(limit.resetsAt) / 1000) : 0,
    }
  }

  const payload = JSON.stringify({
    session_id: id,
    model: { display_name: modelName(model) },
    version: state.version ?? undefined,
    effort: effort ? { level: effort } : undefined,
    cwd: state.cwd,
    workspace: { current_dir: state.cwd },
    context_window: {
      context_window_size: usage.context.window,
      used_percentage: usage.context.percent,
      current_usage: state.tokens ?? undefined,
    },
    cost: usage.cost ? { total_cost_usd: usage.cost.usd } : undefined,
    rate_limits: limits,
    activity: {
      tools: [...state.active.values()].map(tool => ({
        id: tool.id,
        name: tool.name,
        target: tool.target ?? null,
      })),
      completed_counts: [...state.counts.entries()].map(([name, count]) => ({
        name,
        count: count.count,
        failed: count.failed,
      })),
      completed_total: state.completedTotal,
      failed_total: state.failedTotal,
      agents: agents.map(agent => {
        const startedAt = state.startedAt.get(agent.id) ?? now
        state.startedAt.set(agent.id, startedAt)
        return {
          id: agent.id,
          description: agent.description,
          agent_type: agent.type,
          started_at: startedAt,
          completed_at: agent.status === 'running' ? null : now,
        }
      }),
      todo: state.todo,
    },
  })

  try {
    const argv = [
      BINARY,
      '--rows',
      String(state.bandRows),
      '--columns',
      String(state.bandCols),
    ]
    const run = await engine.process.run(argv, { stdin: payload, timeoutMs: 5000 })
    state.rows = run.stdout
      .split('\n')
      .filter(line => line.length > 0)
      .map(spansOf)
    state.runNote =
      run.exitCode === 0
        ? `${state.rows.length}r band ${state.bandRows}x${state.bandCols} · ${state.active.size} running · ${state.completedTotal} done · ${state.paneNote}`
        : `exit ${run.exitCode} — ${run.stderr.slice(0, 120)}`
  } catch (error) {
    state.runNote = `run refused — ${String(error).slice(0, 160)}`
    state.rows = []
  }

  if (!state.paneOpen) {
    state.paneRows = []
    return
  }

  // The pane is a different box, so it needs its own render: one row count
  // cannot serve a 1-row band and a 12-row pane.
  try {
    const paneArgv = [
      BINARY,
      '--rows',
      String(state.paneHeight),
      '--columns',
      String(state.paneCols),
      '--layout',
      state.paneLayout,
    ]
    const paneRun = await engine.process.run(paneArgv, { stdin: payload, timeoutMs: 5000 })
    state.paneRows = paneRun.stdout
      .split('\n')
      .filter(line => line.length > 0)
      .map(spansOf)
    state.paneNote =
      paneRun.exitCode === 0
        ? `pane: ${state.paneRows.length}r ${state.paneLayout} at ${state.paneHeight}x${state.paneCols}`
        : `pane: exit ${paneRun.exitCode} — ${paneRun.stderr.slice(0, 120)}`
  } catch (error) {
    state.paneRows = []
    state.paneNote = `pane: run failed — ${String(error).slice(0, 160)}`
  }
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
  if (now - state.lastRunMs < gapMs) return
  await refresh(engine, state)
}

export function register(on: On) {
  const state: State = {
    cwd: '',
    tokens: null,
    rows: [],
    runNote: 'binary: not run yet',
    active: new Map(),
    counts: new Map(),
    completedTotal: 0,
    failedTotal: 0,
    startedAt: new Map(),
    todo: null,
    lastRunMs: 0,
    bandRows: 3,
    bandCols: 120,
    paneOpen: false,
    paneRows: [],
    paneHeight: 12,
    paneCols: 120,
    paneNote: 'pane: not run yet',
    version: null,
    paneLayout: 'ledger',
  }

  on('session.start', async ($, e, next) => {
    const result = await next(e)
    state.cwd = e.cwd
    $.ui.status('alive')
    await probeVersion($, state)
    try {
      await $.command.register({
        name: 'pulseline',
        description: 'Open the statusline in a pane, where it has room for activity rows',
      })
    } catch (error) {
      state.runNote = `command refused — ${String(error).slice(0, 120)}`
    }
    await refresh($, state)
    $.ui.log(state.runNote)
    $.ui.invalidate('ui.render')
    return result
  })

  on('tool.call', async ($, e, next) => {
    const seen = e as unknown as Record<string, unknown>
    const name = String(seen.tool ?? 'tool')
    const id = String(seen.tool_use_id ?? name)

    // Bookkeeping must never throw into the tool chain: a bad event shape
    // would take the tool call down with it, and the band with that.
    try {
      if (name === 'TodoWrite' && Array.isArray(seen.todos)) {
        state.todo = todoOf(seen.todos as { content?: string; status?: string }[])
      }
      state.active.set(id, { id, name, target: targetOf(seen) })
      $.ui.invalidate('ui.render')
    } catch (error) {
      state.runNote = `mark failed: ${String(error).slice(0, 120)}`
    }

    let failed = false
    try {
      const result = await next(e)
      failed = result.isError === true
      return result
    } catch (error) {
      failed = true
      throw error
    } finally {
      try {
        state.active.delete(id)
        const count = state.counts.get(name) ?? { count: 0, failed: 0 }
        count.count += 1
        if (failed) count.failed += 1
        state.counts.set(name, count)
        state.completedTotal += 1
        if (failed) state.failedTotal += 1
        await maybeRefresh($, state, 400)
        $.ui.status(state.runNote)
        $.ui.invalidate('ui.render')
      } catch (error) {
        state.runNote = `refresh failed: ${String(error).slice(0, 120)}`
        $.ui.status(state.runNote)
      }
    }
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    if (e.usage) {
      state.tokens = {
        input_tokens: e.usage.input_tokens,
        output_tokens: e.usage.output_tokens,
        cache_read_input_tokens: e.usage.cache_read_input_tokens,
        cache_creation_input_tokens: e.usage.cache_creation_input_tokens,
      }
    }
    await refresh($, state)
    $.ui.invalidate('ui.render')
    return result
  })

  on('ui.render', { component: 'AbovePrompt' }, ($, e, next) => {
    if (e.surface !== 'terminal') return next(e)
    // A survey owns the band while it is up; yield rather than fight it.
    if (e.props.hasSurvey) return next(e)
    // With no rows, still draw the note: a band that silently disappears
    // tells us nothing about why.
    if (state.rows.length === 0 && !DEBUG) {
      const { Text } = $.ui.resolve(e)
      return Text({ children: `pulseline-mod: ${state.runNote}` })
    }

    state.bandRows = e.props.maxRows
    // Two cells go to the band's own collapse marker at the right edge.
    state.bandCols = Math.max(20, e.props.bodyColumns - 2)

    const { Box, Text } = $.ui.resolve(e)
    const notes = DEBUG ? [Text({ children: state.runNote })] : []
    // Clip to what the band shows whole; a taller tree hides behind `n more`.
    const drawn = state.rows.slice(0, Math.max(1, e.props.maxRows - notes.length))

    return Box({
      flexDirection: 'column',
      children: [
        ...notes,
        ...drawn.map(spans =>
          Box({ flexDirection: 'row', children: spans.map(span => Text(span)) }),
        ),
      ],
    })
  })

  on('command.run', { command: 'pulseline' }, async ($, e, next) => {
    if (state.paneOpen) {
      state.paneOpen = false
      await $.ui.close({ id: PANE_ID })
      return { text: 'pulseline pane closed' }
    }
    // Fill the rows before the pane mounts: a pane that opens empty and
    // fills a beat later depends on the invalidate landing after its first
    // draw, which is a race not worth running.
    state.paneOpen = true
    await refresh($, state)
    await $.ui.open({ id: PANE_ID, title: 'pulseline', rows: 12, closeOnEscape: true })
    $.ui.invalidate('ui.render')
    return { text: undefined }
  })

  on('ui.render', { component: 'Pane' }, ($, e, next) => {
    if (e.requestId !== PANE_ID || e.surface !== 'terminal') return next(e)

    // Two cells go to the pane's own close mark at the right edge.
    state.paneCols = Math.max(20, e.props.bodyColumns - 2)
    state.paneHeight = Math.max(2, e.props.scroll.bodyRows)

    const { Box, Text, Button } = $.ui.resolve(e)
    const drawn =
      state.paneRows.length > 0
        ? state.paneRows
        : [[{ children: state.paneNote }], [{ children: state.runNote }]]

    const switcher = Box({
      flexDirection: 'row',
      gap: 2,
      children: LAYOUTS.map((name, index) =>
        Button({
          key: `${LAYOUT_KEY}${name}`,
          label: `${index + 1}: ${name}`,
          plain: true,
          dimColor: name !== state.paneLayout,
          // The press also raises `ui.press`, where the switch is handled:
          // this handler is its bottom, and it cannot await a re-render.
          onPress: () => {},
        }),
      ),
    })

    return Box({
      flexDirection: 'column',
      children: [
        ...drawn.map(spans =>
          Box({ flexDirection: 'row', children: spans.map(span => Text(span)) }),
        ),
        switcher,
      ],
    })
  })

  on('ui.press', { requestId: PANE_ID }, async ($, e, next) => {
    if (!e.element.startsWith(LAYOUT_KEY)) return next(e)
    state.paneLayout = e.element.slice(LAYOUT_KEY.length)
    await refresh($, state)
    $.ui.invalidate('ui.render')
    return next(e)
  })

  on('ui.close', { id: PANE_ID }, ($, e, next) => {
    state.paneOpen = false
    return next(e)
  })
}
