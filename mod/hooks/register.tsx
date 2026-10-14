import { atom, read, update } from 'claude-code'
import type { AgentInfo, EngineInterface, Register, SessionUsage } from 'claude-code'

import { buildPayload } from './payload'
import type { ActiveTool } from './payload'
import type { Span, SpanStyle, Todo } from '../types'

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

/**
 * The session's values, held by the host so a hot reload does not zero them.
 *
 * `plugin` and `key` are source literals on purpose: `claude plugin validate`
 * lists every `$.state` key and holds it to the contract in `../types`, and
 * only a family member's `id` may be computed.
 */
const sessionFacts = atom({ plugin: 'pulseline-mod', key: 'session' } as const, {
  cwd: '',
  version: null,
  effort: null,
  tokens: null,
  compactCount: 0,
})

const toolTotals = atom({ plugin: 'pulseline-mod', key: 'tools' } as const, {
  counts: {},
  completedTotal: 0,
  failedTotal: 0,
})

const agentTimings = atom({ plugin: 'pulseline-mod', key: 'agents' } as const, {
  startedAt: {},
  finishedAt: {},
  agentType: {},
})

const instructions = atom({ plugin: 'pulseline-mod', key: 'instructions' } as const, {
  claudeMd: [],
  rules: [],
  memories: [],
})

const todoList = atom({ plugin: 'pulseline-mod', key: 'todo' } as const, null)

/** The person's own pane choices: a reload must not close it or reset it. */
const paneWanted = atom({ plugin: 'pulseline-mod', key: 'pane' } as const, {
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
const drawnOutput = atom({ plugin: 'pulseline-mod', key: 'output' } as const, {
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
 * clear `maybeRefresh`'s gate and spawn the binary twice.
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
    let added = false
    for (const agent of agents) {
      if (startedAt[agent.id] === undefined) {
        startedAt[agent.id] = now
        added = true
      }
    }
    return added ? { ...timings, startedAt } : timings
  })

  return { usage, modelId, sessionId, agents, now }
}

/**
 * Builds the payload from the engine, runs the binary, parses its rows.
 *
 * Declared at the top of the file on purpose: the loader only lets `$` be
 * passed to a function declared here, never to a closure inside `register`.
 */
async function refresh(engine: EngineInterface, state: State): Promise<void> {
  let collected: Collected
  let stale: string | null = null
  try {
    collected = await collect(engine, state)
    state.lastCollected = collected
  } catch (error) {
    // Nothing has ever come back whole: there is no reading to draw, and
    // spawning the binary with fabricated zeros would draw a lie.
    if (state.lastCollected === null) {
      await update(engine, drawnOutput, out => ({
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
    tokens: facts.tokens,
    mcpCount: state.mcpServers.size,
    claudeMdCount: loaded.claudeMd.length,
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
      BINARY,
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
        BINARY,
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

  await update(engine, drawnOutput, () => ({ rows, runNote, paneRows, paneNote }))
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
      if (name === 'TodoWrite' && Array.isArray(seen.todos)) {
        const todo = todoOf(seen.todos as { content?: string; status?: string }[])
        await update($, todoList, () => todo)
      }
      state.active.set(id, { id, name, target: targetOf(seen) })
    } catch (error) {
      const note = `mark failed: ${String(error).slice(0, 120)}`
      await update($, drawnOutput, out => ({ ...out, runNote: note }))
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
        // The count lands before the refresh, and in its own write: an
        // overrun inside `maybeRefresh` must not cost this tool its place
        // in the session totals.
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
        await maybeRefresh($, state, 400)
        $.ui.status((await read($, drawnOutput)).runNote)
      } catch (error) {
        const note = `refresh failed: ${String(error).slice(0, 120)}`
        await update($, drawnOutput, out => ({ ...out, runNote: note }))
        $.ui.status(note)
      }
    }
  })

  // Classic hook inputs carry `effort` on every tool-use-context event, so
  // one cheap hook closes the gap a streaming `turn.step` hook would have.
  // `classic.PreToolUse` is the exception: its argument is the tool call
  // envelope, not the hook input, so the effort rides on PostToolUse.
  on('classic.PostToolUse', async ($, e, next) => {
    const level = e.effort?.level
    if (level) await update($, sessionFacts, facts => ({ ...facts, effort: level }))
    return next(e)
  })

  // What actually reached the model's context, as against what exists on
  // disk: the engine names each instruction file as it loads it.
  on('classic.InstructionsLoaded', async ($, e, next) => {
    const result = await next(e)
    const path = e.file_path
    await update($, instructions, loaded => {
      // A file named twice must not count twice, which is what the Sets
      // these arrays replaced were for.
      if (path.endsWith('CLAUDE.md') || path.endsWith('AGENTS.md')) {
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
  })

  // A settings file changed: re-read rather than wait out the binary's
  // ten-second environment cache. Throttled, unlike the other refresh
  // sites: the rest are once-per-turn or the person's own doing, while one
  // settings write can raise a burst of these.
  on('classic.ConfigChange', async ($, e, next) => {
    const result = await next(e)
    await maybeRefresh($, state, 400)
    return result
  })

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
    return next(e)
  })

  on('classic.SubagentStop', async ($, e, next) => {
    const now = await $.clock.now()
    const id = e.agent_id
    await update($, agentTimings, timings => ({
      ...timings,
      finishedAt: { ...timings.finishedAt, [id]: now },
    }))
    await maybeRefresh($, state, 400)
    return next(e)
  })

  // The compaction badge: the transcript parser counts `compact_boundary`
  // events; here the engine says so directly.
  on('classic.PreCompact', async ($, e, next) => {
    await update($, sessionFacts, facts => ({
      ...facts,
      compactCount: facts.compactCount + 1,
    }))
    return next(e)
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    const usage = e.usage
    if (usage) {
      await update($, sessionFacts, facts => ({
        ...facts,
        tokens: {
          input_tokens: usage.input_tokens,
          output_tokens: usage.output_tokens,
          cache_read_input_tokens: usage.cache_read_input_tokens,
          cache_creation_input_tokens: usage.cache_creation_input_tokens,
        },
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
      return <Text>{`pulseline-mod: ${held.runNote}`}</Text>
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
    await $.ui.open({ id: PANE_ID, title: 'pulseline', rows: 12, closeOnEscape: true })
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
  })
}
