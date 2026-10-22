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
/**
 * One tool kind's tally.
 *
 * `lastAt` is when it last finished, in epoch milliseconds. The binary ranks
 * the kinds by count plus a recency bonus and keeps only
 * `max_completed_tools` of them — four by default — so without a stamp the
 * four a session opened with would outrank the ones it actually leans on.
 */
export type ToolCount = { count: number; failed: number; lastAt: number }

/** The todo list as the last TodoWrite call left it. */
export type Todo = {
  text: string
  pending: number
  completed: number
  total: number
  in_progress_items: { text: string; started_at?: number }[]
  all_done: boolean
  is_task_api: boolean
}

/**
 * One Task API item: `TaskCreate` opens it, `TaskUpdate` moves its status.
 *
 * The transcript path keeps the same four fields (`state/mod.rs`'s `TaskItem`)
 * because the todo row is rebuilt from the whole set on every change — a
 * status arrives with an id and nothing else, so the rest has to be held.
 */
export type TaskItem = {
  subject: string
  status: string
  activeForm?: string
  startedAt?: number
}

/**
 * The Task API's items, by the id the engine gave each one.
 *
 * `TaskCreate`'s *result* carries that id (`{ task: { id } }`), which is the
 * id a later `TaskUpdate` names — so nothing here is counted or guessed, and
 * a refused create leaves no entry behind.
 */
export type Tasks = {
  items: Record<string, TaskItem>
}

/**
 * Facts about the session the hooks resolve as the engine reports them.
 *
 * `cwd` holds the session's project root, which `$.session.root()` answers
 * before every ladder walk: it moves under the mod — `/cd`, a host's
 * directory change, a worktree move — and it is both what the binary scans
 * for the config row and where its git shell-out runs, so a held one reports
 * the project the person left. `session.start`'s `e.cwd` still seeds it, as
 * the value the read falls back to: the window between `register` running
 * and that event arriving draws the project it already knew rather than an
 * empty path, and a payload with no `cwd` resolves to an unknown project
 * where the binary has no env, no git and no project config.
 */
export type SessionFacts = {
  cwd: string
  version: string | null
  effort: string | null
  tokens: Tokens | null
  compactCount: number
  /** What the person chose in place of the default way of answering. */
  outputStyle: string | null
  /**
   * When the last turn ended in an API error, in epoch milliseconds.
   *
   * Never cleared: the binary already drops a reading older than 30s, so a
   * stale one cannot light the badge.
   */
  lastApiErrorMs: number | null
  /**
   * Cache-read tokens summed over the session, for the ledger's CACHE row.
   *
   * The mod has to total this itself. The binary cannot: only the transcript
   * path can dedupe per-API-call usage by `message.id`, and every refresh
   * re-sends the same last-turn usage as `current_usage` — twice over when
   * the pane is open — so accumulating there would multiply-count.
   *
   * Not reset on compaction, deliberately. The transcript path keeps it
   * across `compact_boundary` and clears only the trend window
   * (`providers/transcript.rs`), and the row claims a session total, so
   * resetting here would make the two paths disagree.
   */
  cacheReadTotal: number
  /**
   * The context's tokens at the end of the last main turn, as the engine's
   * `/context` breakdown totals them, for the quiet desktop band's
   * autocompact runway. Absent from an atom an older
   * version wrote, so read it as `?? null`.
   */
  ctxTokens?: number | null
  /**
   * How much each of the last few main turns grew the context, oldest
   * first; cleared when the context shrinks, as a compaction makes it. Read
   * as `?? []`.
   */
  ctxDeltas?: number[]
  /**
   * Where autocompact runs, as the engine's `/context` breakdown last said:
   * `threshold` in tokens, `null` when it named none. Read as `?? null`.
   */
  autoCompact?: { threshold: number | null; enabled: boolean } | null
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
  /**
   * Kept apart from `claudeMd` because the binary counts the two separately:
   * `env.rs`'s `count_agents_md` has its own column on the ENV row, so an
   * AGENTS.md folded into the CLAUDE.md count is drawn twice.
   */
  agentsMd: string[]
  rules: string[]
  memories: string[]
}

/** What the person asked of the pane, which a reload must not undo. */
export type PaneWanted = {
  open: boolean
  layout: string
}

/**
 * Which binary the mod resolved, and whether it can be used.
 *
 * Held rather than recomputed because resolving it means running up to five
 * candidates: a closure would re-probe the whole ladder on every hot reload.
 * It is set back to `null` on `classic.ConfigChange`, so a binary installed
 * or a path configured mid-session is picked up — without that an atom would
 * keep one bad answer for the life of the session.
 */
/**
 * Where the binary is, as the ladder last answered.
 *
 * The two negative answers carry `at`, the moment the walk was made, because
 * they expire: the band's own message is `npm install -g …`, and an install
 * raises no event at all — so a cached failure that never expired would
 * outlive the fix it asked for and the band could not recover inside the
 * session. A successful answer has no `at`; it is kept until a settings
 * write clears it.
 */
export type BinaryChoice =
  | { kind: 'ok'; path: string; version: string; rung: string }
  | { kind: 'too-old'; path: string; version: string; rung: string; at: number }
  | { kind: 'missing'; at: number }

/**
 * How the last band run ended, which is not the same question as whether
 * there are rows.
 *
 * The band asks the binary for the `activity` layout, whose output is empty
 * whenever nothing is running — so zero rows is the ordinary state between
 * turns, not a fault. Only `'failed'` is worth saying out loud, and it has
 * to be said whether or not a turn is running: a missing binary raises no
 * event, and an install line nobody sees fixes nothing.
 *
 * `'never'` is the value before the first walk finishes. It draws nothing,
 * so the band does not flash a not-run-yet note over an idle prompt while
 * `session.start` is still probing.
 */
export type BandRun = 'never' | 'ok' | 'failed'

/**
 * One alert row the quiet desktop band draws: a value past its limit.
 *
 * Worked out in `refresh` from `$.session.usage()` and the last turn's
 * tokens (`hooks/desktop.ts`), and published with the rows, because the
 * render hook may read state but never write it. Quota has no row here: the
 * strip draws it, and a window past its limit is notified instead.
 */
export type Alert =
  | {
      kind: 'ctx'
      percent: number
      used: number
      window: number
      /**
       * Whole turns left before autocompact runs, at the recent turns'
       * growth; absent when there is no reading for it, or from an alert an
       * older version wrote.
       */
      runway?: number
    }
  | {
      kind: 'cache'
      /** Shares of the last turn's input: read from cache, written to it. */
      read: number
      creation: number
    }

/**
 * The thin strip the quiet desktop band keeps on screen: what the Code tab
 * does not show. Each part is left out when there is no reading for it.
 */
export type Strip = {
  quota: {
    /** The rate-limit window as the engine names it: `five_hour`, `seven_day`. */
    window: string
    percent: number
    /** Epoch seconds; absent when the engine named no reset. */
    resetsAt?: number
  }[]
  /** Session cost so far, and per hour since the session began. */
  usd?: number
  usdPerHour?: number
  /**
   * This session's share of what every active desktop session has spent,
   * 0 to 1, and how many others are active. Absent with no other session
   * active, and from an atom an older version wrote, so read it as `?? null`.
   */
  share?: { share: number; others: number }
}

/**
 * The binary's last output, kept so a reload redraws rather than blanks.
 *
 * `rows` is the terminal band's, `desktopRows` the desktop app's: the two ask
 * for different layouts, so one set of rows cannot serve both. `bandRun` and
 * `runNote` are shared — a failing run on either surface is said on both.
 */
export type DrawnOutput = {
  rows: Span[][]
  desktopRows: Span[][]
  /**
   * The quiet desktop band's alerts. Absent from an atom an older version
   * wrote, so read it as `?? []`.
   */
  alerts?: Alert[]
  /**
   * The quiet desktop band's strip; `null` when no desktop shows it. Absent
   * from an atom an older version wrote, so read it as `?? null`.
   */
  strip?: Strip | null
  /**
   * The quota windows already notified, one `quotaKey` each, so a window is
   * notified once per reset and not again after a reload. Read as `?? []`.
   */
  notified?: string[]
  paneRows: Span[][]
  bandRun: BandRun
  runNote: string
  paneNote: string
}

declare module 'claude-code' {
  interface PluginState {
    'pulseline': {
      session: SessionFacts
      tools: ToolTotals
      agents: AgentTimings
      instructions: Instructions
      todo: Todo | null
      tasks: Tasks
      pane: PaneWanted
      output: DrawnOutput
      binary: BinaryChoice | null
    }
  }
}
