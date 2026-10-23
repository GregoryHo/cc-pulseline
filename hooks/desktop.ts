import type { SessionUsage } from 'claude-code'

import type { ActiveTool } from './payload'
import type { Alert, FailStreak, Strip, Tokens } from '../types'

/**
 * The quiet desktop band: what it draws, worked out from figures the module
 * already holds. Pure, so the tests reach it without the engine.
 *
 * The Code tab already shows the model, the effort, the permission mode, the
 * branch and a context ring, and its transcript lists every tool. What it
 * does not show is where quota is heading, cost, and the cache. So the quiet
 * band keeps one thin strip of quota and cost, and adds a row only when the
 * context or the cache passes its limit.
 *
 * The terminal band is quieter still. Its statusline carries the quota, the
 * cost and the context, and Claude Code's own transcript every tool, so it
 * draws nothing at all until something needs the person: the same tool
 * failing again and again, a tool running long, a quota on pace to run out,
 * the context near autocompact, a cache rebuilding.
 */

/**
 * When an alert opens: the binary's own marks, so the band and the pane
 * agree. CTX's critical mark is 70 (`ThemePalette::ctx_marks()`), quota's 85.
 */
export const LIMITS = { ctx: 70, quota: 85 } as const

/** The gauge's threshold marks, drawn as ticks: the binary's `[55, 70]` and `[50, 85]`. */
const CTX_MARKS = [55, 70]
const QUOTA_MARKS = [50, 85]

/**
 * The cold-cache ladder of the binary's `C:%` colour: under 40% of the input
 * read from cache, with half or more written to it, is a cache rebuilding.
 */
const CACHE_READ_UNDER = 0.4
const CACHE_CREATION_AT = 0.5

/**
 * How near autocompact opens the context alert early, in turns: enough to
 * finish the task at hand and compact at its end rather than mid-task.
 */
export const RUNWAY_TURNS = 5

/** How many turns' growth the runway averages over. */
const RUNWAY_SAMPLES = 5

/** How many failures in a row of one tool open the terminal band's alert. */
export const FAIL_STREAK = 3

/** How long one tool call runs before the terminal band says so. */
export const LONG_TOOL_MS = 60_000

/** How often a long call's row is redrawn, so its time moves. */
export const LONG_TICK_MS = 15_000

/** How long a turn runs before its end is worth a notification. */
export const LONG_TURN_MS = 3 * 60_000

/** The `desktopLayout` value that draws this band rather than the binary's rows. */
export const QUIET = 'quiet'

/** The `/compact` button's key: its address at `ui.press`. */
export const COMPACT_KEY = 'pulseline:compact'

const AMBER = '#d7875f'
const RED = '#d75f5f'
const BLUE = '#7a9cc6'

/** The last turn's input, split into cache read, cache written, and the rest. */
export function hitRate(tokens: Tokens | null): { read: number; creation: number } | null {
  if (tokens === null) return null
  const read = tokens.cache_read_input_tokens
  const made = tokens.cache_creation_input_tokens
  const all = read + made + tokens.input_tokens
  if (all === 0) return null
  return { read: read / all, creation: made / all }
}

/**
 * The growth series after a main turn ends with the context at `tokens`: its
 * growth appended, the oldest dropped past `RUNWAY_SAMPLES`. A context that
 * shrank was compacted (or cleared), so the series starts over from it.
 */
export function nextDeltas(last: number | null, deltas: readonly number[], tokens: number): number[] {
  if (last === null || tokens < last) return []
  return [...deltas, tokens - last].slice(-RUNWAY_SAMPLES)
}

/**
 * Whole turns left before autocompact runs at `threshold`, at the recent
 * turns' average growth; `null` with fewer than two turns' growth, growth
 * that is flat or falling, or no threshold (autocompact off).
 */
export function runwayOf(current: number, deltas: readonly number[], threshold: number | null): number | null {
  if (threshold === null || deltas.length < 2) return null
  const average = deltas.reduce((sum, delta) => sum + delta, 0) / deltas.length
  if (average <= 0) return null
  return Math.max(0, Math.floor((threshold - current) / average))
}

/**
 * The alert rows for one reading, context first: it is the one with a button.
 * Quota has no row of its own; the strip draws it, and `quotaPast` notifies.
 *
 * The context's opens at its limit, or earlier once autocompact is
 * `RUNWAY_TURNS` turns off.
 */
export function desktopAlerts(usage: SessionUsage, tokens: Tokens | null, runway: number | null = null): Alert[] {
  const alerts: Alert[] = []

  const ctx = usage.context.percent
  const near = runway !== null && runway <= RUNWAY_TURNS
  if (ctx !== undefined && (ctx >= LIMITS.ctx || near)) {
    alerts.push({
      kind: 'ctx',
      percent: ctx,
      used: usage.context.tokens ?? Math.round((usage.context.window * ctx) / 100),
      window: usage.context.window,
      ...(runway !== null ? { runway } : {}),
    })
  }

  const hit = hitRate(tokens)
  if (hit !== null && hit.read < CACHE_READ_UNDER && hit.creation >= CACHE_CREATION_AT) {
    alerts.push({ kind: 'cache', ...hit })
  }

  return alerts
}

/** Whether the desktop's quiet band draws an alert: its strip and transcript carry the rest. */
export function onDesktopBand(alert: Alert): boolean {
  return alert.kind === 'ctx' || alert.kind === 'cache'
}

/**
 * The failure run after one main-loop tool call ends: a failure of the same
 * tool adds to it, of another starts a new one; a success of the same tool
 * ends it, and of another leaves it be.
 */
export function nextStreak(
  streak: FailStreak | null,
  tool: string,
  failed: boolean,
  target: string | undefined,
): FailStreak | null {
  const same = streak !== null && streak.tool === tool
  if (!failed) return same ? null : streak
  return { tool, count: same ? streak.count + 1 : 1, ...(target !== undefined ? { target } : {}) }
}

/**
 * Every alert for one reading, most pressing first: what failed, what is
 * running long, the context, the cache. The tools first, because they are
 * this turn's and the rest can wait for its end. Each band filters the list
 * (`onTerminalBand`, `onDesktopBand`). `now` is `$.clock.now()`.
 */
export function alertsOf(
  usage: SessionUsage,
  tokens: Tokens | null,
  runway: number | null,
  streak: FailStreak | null,
  active: readonly ActiveTool[],
  now: number,
): Alert[] {
  const alerts: Alert[] = []
  if (streak !== null && streak.count >= FAIL_STREAK) alerts.push({ kind: 'fails', ...streak })
  for (const tool of active) {
    if (now - tool.startedAt < LONG_TOOL_MS) continue
    const { name, target, startedAt } = tool
    alerts.push({ kind: 'long', tool: name, startedAt, ...(target !== undefined ? { target } : {}) })
  }
  alerts.push(...desktopAlerts(usage, tokens, runway))
  return alerts
}

/**
 * Whether the terminal band draws an alert. Its statusline carries the
 * quota pace, the context percent and the cache, so what is left is what
 * only the mod knows: a failure run, a long call, and how many turns are
 * left before autocompact.
 */
export function onTerminalBand(alert: Alert): boolean {
  // The desktop also opens it at 70%; the terminal's statusline has that.
  if (alert.kind === 'ctx') return alert.runway !== undefined && alert.runway <= RUNWAY_TURNS
  return alert.kind === 'fails' || alert.kind === 'long'
}

/** One alert as the terminal band draws it: the context row is its runway alone. */
export function terminalRow(alert: Alert, now: number): AlertRow {
  const row = alertRow(alert, now)
  return alert.kind === 'ctx' ? { ...row, gauge: null, label: row.detail, detail: '' } : row
}

/**
 * How long until the terminal band must refresh for a long call: when the
 * oldest running one crosses `LONG_TOOL_MS`, then every `LONG_TICK_MS` so its
 * time moves; `null` with nothing running. No event marks either moment.
 */
export function nextLongCheck(active: readonly ActiveTool[], now: number): number | null {
  if (active.length === 0) return null
  const oldest = Math.min(...active.map(tool => tool.startedAt))
  const left = LONG_TOOL_MS - (now - oldest)
  return left > 0 ? Math.max(1000, left) : LONG_TICK_MS
}

/** One quota window as the strip holds it. */
export type QuotaWindow = Strip['quota'][number]

/** The engine's rate-limit windows, their resets as epoch seconds. */
function quotaWindows(usage: SessionUsage): QuotaWindow[] {
  return usage.rateLimits.map(limit => {
    const resetsMs = limit.resetsAt === undefined ? NaN : Date.parse(limit.resetsAt)
    return {
      window: limit.kind,
      percent: limit.percentUsed,
      ...(Number.isFinite(resetsMs) ? { resetsAt: Math.floor(resetsMs / 1000) } : {}),
    }
  })
}

/** The quota windows at or past their limit: each is notified once per reset. */
export function quotaPast(usage: SessionUsage): QuotaWindow[] {
  return quotaWindows(usage).filter(quota => quota.percent >= LIMITS.quota)
}

/** What a quota window is notified under: the window and its reset. */
export function quotaKey(quota: QuotaWindow): string {
  return `${quota.window}@${quota.resetsAt ?? 'unknown'}`
}

/**
 * The strip the quiet band keeps on screen: every quota window, the session
 * cost and its rate. `now` is `$.clock.now()`, the clock
 * `usage.startedAt` is on.
 */
export function stripOf(usage: SessionUsage, now: number): Strip {
  const usd = usage.cost?.usd
  const hours = (now - usage.startedAt) / 3_600_000
  // A rate over the first minute is noise: one turn's cost read as an hour's.
  const usdPerHour = usd !== undefined && hours >= 1 / 60 ? usd / hours : undefined
  return {
    quota: quotaWindows(usage),
    ...(usd !== undefined ? { usd } : {}),
    ...(usdPerHour !== undefined ? { usdPerHour } : {}),
  }
}

/**
 * What one desktop session keeps in `$.store` so the others can see it: its
 * cost so far, and when it last said so. Quota is the account's, shared by
 * every session, so only the sessions' own costs can say which one burns it.
 *
 * One key per session, never a shared total: the store is one file across
 * processes, and a read-then-write of a shared value loses updates.
 */
export type SessionSpend = { usd: number; at: number }

/** The store key a session's spend is kept under. */
export const SPEND_PREFIX = 'session.'

/** A session that has not said anything for this long is not active. */
export const SPEND_ACTIVE_MS = 10 * 60_000

/** A key this old is deleted, so the store (4 MiB in all) cannot grow forever. */
export const SPEND_EXPIRE_MS = 24 * 3_600_000

/** How often an unchanged spend is written again, to stay active. */
export const SPEND_WRITE_MS = 30_000

/** A store value as a spend, or `null` for anything else: never cast. */
export function spendOf(value: unknown): SessionSpend | null {
  if (typeof value !== 'object' || value === null) return null
  const { usd, at } = value as Record<string, unknown>
  return typeof usd === 'number' && typeof at === 'number' ? { usd, at } : null
}

/**
 * Whether to write the spend again: it moved, or the last write is going
 * stale. Never over a newer one: refreshes overlap, and the one that read
 * the clock first can reach the store last.
 */
export function shouldWriteSpend(last: SessionSpend | null, usd: number, now: number): boolean {
  if (last === null) return true
  if (last.at > now) return false
  return last.usd !== usd || now - last.at >= SPEND_WRITE_MS
}

/**
 * This session's share of what the active sessions have spent, with how many
 * others are active; `null` with none, or nothing spent, since a lone
 * session's 100% says nothing. `others` are the other sessions' spends.
 */
export function shareOf(
  usd: number,
  others: readonly SessionSpend[],
  now: number,
): NonNullable<Strip['share']> | null {
  const active = others.filter(other => now - other.at <= SPEND_ACTIVE_MS)
  const total = active.reduce((sum, other) => sum + other.usd, usd)
  if (active.length === 0 || total <= 0) return null
  return { share: usd / total, others: active.length }
}

/** `this session ~40% of spend · 2 others active` */
export function shareLine(share: NonNullable<Strip['share']>): string {
  const others = share.others === 1 ? '1 other' : `${share.others} others`
  return `this session ~${Math.round(share.share * 100)}% of spend · ${others} active`
}

/** `five_hour` → `5h`, `seven_day` → `7d`; any other window as the engine names it. */
export function windowLabel(window: string): string {
  if (window === 'five_hour') return '5h'
  if (window === 'seven_day') return '7d'
  return window
}

/** `6m 12s`, `2h 5m`, `6d 10h`: the two largest units, a trailing zero dropped. */
export function formatSpan(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000))
  const parts: [number, string][] = [
    [Math.floor(total / 86_400), 'd'],
    [Math.floor((total % 86_400) / 3600), 'h'],
    [Math.floor((total % 3600) / 60), 'm'],
    [total % 60, 's'],
  ]
  const from = parts.findIndex(([value]) => value > 0)
  if (from < 0) return '0s'
  return parts
    .slice(from, from + 2)
    .filter(([value], index) => index === 0 || value > 0)
    .map(([value, unit]) => `${value}${unit}`)
    .join(' ')
}

/** `146.0k`, `1.2M`: the binary's own token format. */
function formatTokens(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}k`
  return String(count)
}

/** How long until a reset, rounded to the minute; `null` when none was named. */
function resetsIn(resetsAt: number | undefined, now: number): string | null {
  if (resetsAt === undefined) return null
  return formatSpan(Math.round((resetsAt * 1000 - now) / 60_000) * 60_000)
}

/** How long each rate-limit window runs, so a reset says how far in it is. */
const WINDOW_MS: Record<string, number> = { five_hour: 5 * 3_600_000, seven_day: 7 * 86_400_000 }

/**
 * How far into a window a projection waits: before it, one burst reads as a
 * whole window's pace. 15 minutes of five hours, 8.4 hours of seven days.
 */
const PACE_FROM = 0.05

/** A window's average pace so far, carried to its reset. */
type Pace = {
  /** The share of the window gone, 0 to 1. */
  elapsed: number
  /** The percent used by the reset at this pace; 100 or more runs out first. */
  projected: number
  /** At or past 100: how long until the window runs out. */
  fullInMs?: number
}

/** `null` for a window of unknown length, with no reset, too early in, or unused. */
function paceOf(quota: QuotaWindow, now: number): Pace | null {
  const length = WINDOW_MS[quota.window]
  if (length === undefined || quota.resetsAt === undefined || quota.percent <= 0) return null
  const gone = length - (quota.resetsAt * 1000 - now)
  const elapsed = gone / length
  if (elapsed < PACE_FROM || elapsed > 1) return null
  const projected = quota.percent / elapsed
  if (projected < 100) return { elapsed, projected }
  return { elapsed, projected, fullInMs: ((100 - quota.percent) * gone) / quota.percent }
}

/** A pace as the strip's line under the label: tight, as the reset is. */
function paceLine(pace: Pace): { text: string; color?: string } {
  if (pace.fullInMs === undefined) return { text: `→ ~${Math.round(pace.projected)}% at reset` }
  const span = formatSpan(Math.round(pace.fullInMs / 60_000) * 60_000).replace(' ', '')
  return { text: `full in ~${span}`, color: AMBER }
}

/** One alert as the band draws it: an optional gauge, a label, the detail. */
export type AlertRow = {
  /** The `Svg` gauge's markup, and what it says for a reader that cannot see it. */
  gauge: { source: string; alt: string } | null
  label: string
  /** The label's colour: amber for a warning. */
  color: string
  detail: string
  /** Whether the row carries the `/compact` button. */
  compact: boolean
}

/**
 * `now` moves a long call's time on: the render passes the last refresh's
 * clock, and the refresh timer keeps that within `LONG_TICK_MS`.
 */
export function alertRow(alert: Alert, now: number): AlertRow {
  switch (alert.kind) {
    case 'fails':
      return {
        gauge: null,
        label: `✘ ${alert.tool} failed ${alert.count}× in a row`,
        color: RED,
        detail: alert.target ?? '',
        compact: false,
      }
    case 'long':
      return {
        gauge: null,
        label: `◷ ${alert.tool} running ${formatSpan(Math.max(0, now - alert.startedAt))}`,
        color: AMBER,
        detail: alert.target ?? '',
        compact: false,
      }
    case 'ctx': {
      const label = `CTX ${Math.round(alert.percent)}%`
      return {
        gauge: { source: gaugeSvg(alert.percent, CTX_MARKS, AMBER), alt: `Context ${label}` },
        label,
        color: AMBER,
        detail:
          alert.runway === undefined
            ? `${formatTokens(alert.used)} / ${formatTokens(alert.window)}`
            : runwayText(alert.runway),
        compact: true,
      }
    }
    case 'cache':
      return {
        gauge: null,
        label: `cache ${Math.round(alert.read * 100)}%`,
        color: AMBER,
        detail: `rebuilding · ${Math.round(alert.creation * 100)}% of input is new cache`,
        compact: false,
      }
  }
}

/** `~4 turns to autocompact`; under one turn's growth left, the next turn. */
function runwayText(turns: number): string {
  if (turns === 0) return 'autocompact next turn'
  return `~${turns} turn${turns === 1 ? '' : 's'} to autocompact`
}

/** The notification a quota window raises once per reset. */
export function quotaNotice(quota: QuotaWindow, now: number): string {
  const reset = resetsIn(quota.resetsAt, now)
  const head = `${windowLabel(quota.window)} quota at ${Math.round(quota.percent)}%.`
  return reset === null ? head : `${head} It resets in ${reset}.`
}

/** One cell of the strip: an optional small gauge, a label, a dim detail. */
export type StripCell = {
  /** `height` is the image's: a paced gauge's tick stands above and below its bar. */
  gauge: { source: string; alt: string; height: number } | null
  label: string
  /** The label's colour; absent, the surface's own text colour. */
  color?: string
  /** Where the window's pace lands it, over the detail; amber when it runs out first. */
  pace: { text: string; color?: string } | null
  detail: string | null
}

/** The strip as cells, in the order drawn; empty when it has nothing to say. */
export function stripCells(strip: Strip, now: number): StripCell[] {
  const cells: StripCell[] = strip.quota.map(quota => {
    const label = `${windowLabel(quota.window)} ${Math.round(quota.percent)}%`
    const pace = paceOf(quota, now)
    const runsOut = pace?.fullInMs !== undefined
    const fill =
      quota.percent >= QUOTA_MARKS[1]! ? RED : quota.percent >= QUOTA_MARKS[0]! || runsOut ? AMBER : BLUE
    const color = quota.percent >= QUOTA_MARKS[1]! ? RED : runsOut ? AMBER : undefined
    const reset = resetsIn(quota.resetsAt, now)
    const tick = pace === null ? undefined : pace.elapsed * 100
    const source = gaugeSvg(quota.percent, QUOTA_MARKS, fill, 64, 8, tick)
    return {
      gauge: { source, alt: `Quota ${label}`, height: pace === null ? 8 : 12 },
      label,
      ...(color !== undefined ? { color } : {}),
      pace: pace === null ? null : paceLine(pace),
      // No space inside the span: the column is narrow, and `4h22m` stays on one line.
      detail: reset === null ? null : `resets ${reset.replace(' ', '')}`,
    }
  })
  if (strip.usd !== undefined) {
    cells.push({
      gauge: null,
      pace: null,
      label: `$${strip.usd.toFixed(2)}`,
      detail: strip.usdPerHour !== undefined ? `$${strip.usdPerHour.toFixed(2)}/h` : null,
    })
  }
  return cells
}

/**
 * How many rows the strip stands: its tallest cell (a label, then a pace and
 * a reset under it), and the share line under them all.
 */
export function stripRows(cells: readonly StripCell[], share: boolean): number {
  if (cells.length === 0) return 0
  const tallest = Math.max(
    ...cells.map(cell => 1 + (cell.pace !== null ? 1 : 0) + (cell.detail !== null ? 1 : 0)),
  )
  return tallest + (share ? 1 : 0)
}

/** The notification a long turn raises when it ends. */
export function longTurnNotice(durationMs: number): string {
  return `The turn ended after ${formatSpan(durationMs)}.`
}

/**
 * A bar gauge as an SVG document: a track, the filled share, and a tick at
 * each threshold mark. Drawn as an image on the desktop, so its colours hold
 * on a light and a dark theme alike: the track is translucent.
 *
 * With `pace`, the share of the window gone, the image grows 2px above and
 * below the bar for a tick there: a fill past it is a window used faster
 * than it runs.
 */
export function gaugeSvg(
  percent: number,
  marks: readonly number[],
  fill: string,
  width = 120,
  height = 10,
  pace?: number,
): string {
  const at = (share: number) => Math.round((width * Math.min(100, Math.max(0, share))) / 100)
  const inset = pace === undefined ? 0 : 2
  const tall = height + 2 * inset
  const y = inset === 0 ? '' : `y="${inset}" `
  const ticks = marks
    .map(mark => `<rect x="${at(mark)}" ${y}width="1" height="${height}" fill="#ffffff" fill-opacity="0.7"/>`)
    .join('')
  const paceTick =
    pace === undefined
      ? ''
      : `<rect x="${Math.min(width - 2, at(pace))}" width="2" height="${tall}" fill="#8a8578"/>`
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${tall}" viewBox="0 0 ${width} ${tall}">` +
    `<rect ${y}width="${width}" height="${height}" rx="3" fill="#8a8578" fill-opacity="0.3"/>` +
    `<rect ${y}width="${at(percent)}" height="${height}" rx="3" fill="${fill}"/>` +
    ticks +
    paceTick +
    `</svg>`
  )
}
