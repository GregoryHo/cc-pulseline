import type { SessionUsage } from 'claude-code'

import type { Alert, GitHead, Strip, Tokens } from '../types'

/**
 * The quiet desktop band: what it draws, worked out from figures the module
 * already holds. Pure, so the tests reach it without the engine.
 *
 * The Code tab already shows the model, the effort, the permission mode and a
 * context ring, and its transcript lists every tool. What it does not show is
 * quota, cost, the branch and the cache. So the quiet band keeps one thin
 * strip of the first three, and adds a row only when the context or the
 * cache passes its limit.
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
 * The alert rows for one reading, context first: it is the one with a button.
 * Quota has no row of its own; the strip draws it, and `quotaPast` notifies.
 */
export function desktopAlerts(usage: SessionUsage, tokens: Tokens | null): Alert[] {
  const alerts: Alert[] = []

  const ctx = usage.context.percent
  if (ctx !== undefined && ctx >= LIMITS.ctx) {
    alerts.push({
      kind: 'ctx',
      percent: ctx,
      used: usage.context.tokens ?? Math.round((usage.context.window * ctx) / 100),
      window: usage.context.window,
    })
  }

  const hit = hitRate(tokens)
  if (hit !== null && hit.read < CACHE_READ_UNDER && hit.creation >= CACHE_CREATION_AT) {
    alerts.push({ kind: 'cache', ...hit })
  }

  return alerts
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
 * cost and its rate, and the branch. `now` is `$.clock.now()`, the clock
 * `usage.startedAt` is on.
 */
export function stripOf(usage: SessionUsage, git: GitHead | null, now: number): Strip {
  const usd = usage.cost?.usd
  const hours = (now - usage.startedAt) / 3_600_000
  // A rate over the first minute is noise: one turn's cost read as an hour's.
  const usdPerHour = usd !== undefined && hours >= 1 / 60 ? usd / hours : undefined
  return {
    quota: quotaWindows(usage),
    ...(usd !== undefined ? { usd } : {}),
    ...(usdPerHour !== undefined ? { usdPerHour } : {}),
    git,
  }
}

/**
 * The branch out of `git status --porcelain=v2 --branch`: its name, whether
 * the tree has changes, and how far it is ahead of and behind its upstream.
 * `null` for output that names no branch, as outside a repository.
 */
export function parseGitStatus(stdout: string): GitHead | null {
  let branch: string | null = null
  let ahead = 0
  let behind = 0
  let dirty = false
  for (const line of stdout.split('\n')) {
    if (line.startsWith('# branch.head ')) branch = line.slice('# branch.head '.length).trim()
    else if (line.startsWith('# branch.ab ')) {
      const [plus, minus] = line.slice('# branch.ab '.length).trim().split(' ')
      ahead = Math.abs(Number.parseInt(plus ?? '0', 10)) || 0
      behind = Math.abs(Number.parseInt(minus ?? '0', 10)) || 0
    } else if (line.length > 0 && !line.startsWith('#')) dirty = true
  }
  return branch === null ? null : { branch, dirty, ahead, behind }
}

/** `G:main* ↑2 ↓1`, as the statusline's identity row draws it. */
export function gitLabel(git: GitHead): string {
  return [
    `G:${git.branch}${git.dirty ? '*' : ''}`,
    git.ahead > 0 ? `↑${git.ahead}` : null,
    git.behind > 0 ? `↓${git.behind}` : null,
  ]
    .filter((part): part is string => part !== null)
    .join(' ')
}

/** `five_hour` → `5h`, `seven_day` → `7d`; any other window as the engine names it. */
export function windowLabel(window: string): string {
  if (window === 'five_hour') return '5h'
  if (window === 'seven_day') return '7d'
  return window
}

/** `6m 12s`, `2h 5m`, `47m`: the two largest units, a trailing zero dropped. */
export function formatSpan(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000))
  const parts: [number, string][] = [
    [Math.floor(total / 3600), 'h'],
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

export function alertRow(alert: Alert): AlertRow {
  switch (alert.kind) {
    case 'ctx': {
      const label = `CTX ${Math.round(alert.percent)}%`
      return {
        gauge: { source: gaugeSvg(alert.percent, CTX_MARKS, AMBER), alt: `Context ${label}` },
        label,
        color: AMBER,
        detail: `${formatTokens(alert.used)} / ${formatTokens(alert.window)}`,
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

/** The notification a quota window raises once per reset. */
export function quotaNotice(quota: QuotaWindow, now: number): string {
  const reset = resetsIn(quota.resetsAt, now)
  const head = `${windowLabel(quota.window)} quota at ${Math.round(quota.percent)}%.`
  return reset === null ? head : `${head} It resets in ${reset}.`
}

/** One cell of the strip: an optional small gauge, a label, a dim detail. */
export type StripCell = {
  gauge: { source: string; alt: string } | null
  label: string
  /** The label's colour; absent, the surface's own text colour. */
  color?: string
  detail: string | null
}

/** The strip as cells, in the order drawn; empty when it has nothing to say. */
export function stripCells(strip: Strip, now: number): StripCell[] {
  const cells: StripCell[] = strip.quota.map(quota => {
    const label = `${windowLabel(quota.window)} ${Math.round(quota.percent)}%`
    const fill = quota.percent >= QUOTA_MARKS[1]! ? RED : quota.percent >= QUOTA_MARKS[0]! ? AMBER : BLUE
    const reset = resetsIn(quota.resetsAt, now)
    return {
      gauge: { source: gaugeSvg(quota.percent, QUOTA_MARKS, fill, 64, 8), alt: `Quota ${label}` },
      label,
      ...(quota.percent >= QUOTA_MARKS[1]! ? { color: RED } : {}),
      detail: reset === null ? null : `resets ${reset}`,
    }
  })
  if (strip.usd !== undefined) {
    cells.push({
      gauge: null,
      label: `$${strip.usd.toFixed(2)}`,
      detail: strip.usdPerHour !== undefined ? `$${strip.usdPerHour.toFixed(2)}/h` : null,
    })
  }
  if (strip.git !== null) cells.push({ gauge: null, label: gitLabel(strip.git), detail: null })
  return cells
}

/** The notification a long turn raises when it ends. */
export function longTurnNotice(durationMs: number): string {
  return `The turn ended after ${formatSpan(durationMs)}.`
}

/**
 * A bar gauge as an SVG document: a track, the filled share, and a tick at
 * each threshold mark. Drawn as an image on the desktop, so its colours hold
 * on a light and a dark theme alike: the track is translucent.
 */
export function gaugeSvg(
  percent: number,
  marks: readonly number[],
  fill: string,
  width = 120,
  height = 10,
): string {
  const at = (share: number) => Math.round((width * Math.min(100, Math.max(0, share))) / 100)
  const ticks = marks
    .map(mark => `<rect x="${at(mark)}" width="1" height="${height}" fill="#ffffff" fill-opacity="0.7"/>`)
    .join('')
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">` +
    `<rect width="${width}" height="${height}" rx="3" fill="#8a8578" fill-opacity="0.3"/>` +
    `<rect width="${at(percent)}" height="${height}" rx="3" fill="${fill}"/>` +
    ticks +
    `</svg>`
  )
}
