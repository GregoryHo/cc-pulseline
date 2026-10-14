/**
 * The mod's `$.state` contract: what the host holds for the session, and so
 * what survives a hot reload of this code.
 *
 * The test for a field belonging here is not "does it accumulate" but "would
 * the person notice it being lost", because a reload happens every time the
 * mod is edited. That covers three kinds of value: what the hooks have added
 * up over the session, what the person has asked for (their pane, their
 * layout), and the binary's last output, so a reload redraws the rows it
 * already had instead of flashing a not-run-yet note.
 *
 * Geometry (`bandRows`, `bandCols`, `paneCols`, `paneHeight`) is deliberately
 * NOT here: it is written from inside `ui.render`, and the host refuses a
 * state write while a render hook draws.
 *
 * Values are grouped by concern rather than one atom per field: `update` is a
 * read-modify-write with `ifVersion` and a retry, so a group is safe against
 * concurrent writers, and `refresh` sits in the tool-call path where seven
 * round trips beat twenty-one.
 */

/**
 * The styles `spansOf` sets from the binary's SGR codes.
 *
 * Spelled out rather than taken from `TextProps`, because a contract has to
 * be self-contained — `claude plugin validate` refuses one that imports. It
 * is the subset the ANSI parser actually produces, and every member is a
 * `TextProps` member, so a span still spreads onto a `Text`.
 */
export type SpanStyle = {
  color?: string
  backgroundColor?: string
  bold?: boolean
  dimColor?: boolean
  inverse?: boolean
}

/** One run of text carrying the style the SGR codes before it set. */
export type Span = SpanStyle & { children: string }

/** Turn token counts, kept from the last `turn.complete`. */
export type Tokens = {
  input_tokens: number
  output_tokens: number
  cache_read_input_tokens: number
  cache_creation_input_tokens: number
}

/** Session totals per tool name, as the completed-counts row shows them. */
export type ToolCount = { count: number; failed: number }

/** The todo list as the last TodoWrite call left it. */
export type Todo = {
  text: string
  pending: number
  completed: number
  total: number
  in_progress_items: { text: string }[]
  all_done: boolean
  is_task_api: boolean
}

/**
 * Facts about the session the hooks resolve as the engine reports them.
 *
 * `cwd` is the cheapest proof this whole contract works: it is `''` until
 * `session.start`, and a reload re-runs `register` but not `session.start`,
 * so held in a closure it would go empty and the binary would fall back to
 * an unknown project path with no env, git or project config.
 */
export type SessionFacts = {
  cwd: string
  version: string | null
  effort: string | null
  tokens: Tokens | null
  compactCount: number
}

/** What the tool-call hook has counted this session. */
export type ToolTotals = {
  counts: Record<string, ToolCount>
  completedTotal: number
  failedTotal: number
}

/** Agent timings and resolved types, by agent id. */
export type AgentTimings = {
  startedAt: Record<string, number>
  finishedAt: Record<string, number>
  agentType: Record<string, string>
}

/** The instruction files the engine said it loaded, by kind. */
export type Instructions = {
  claudeMd: string[]
  rules: string[]
  memories: string[]
}

/** What the person asked of the pane, which a reload must not undo. */
export type PaneWanted = {
  open: boolean
  layout: string
}

/** The binary's last output, kept so a reload redraws rather than blanks. */
export type DrawnOutput = {
  rows: Span[][]
  paneRows: Span[][]
  runNote: string
  paneNote: string
}

declare module 'claude-code' {
  interface PluginState {
    'pulseline-mod': {
      session: SessionFacts
      tools: ToolTotals
      agents: AgentTimings
      instructions: Instructions
      todo: Todo | null
      pane: PaneWanted
      output: DrawnOutput
    }
  }
}
