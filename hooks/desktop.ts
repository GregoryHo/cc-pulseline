import type { SessionUsage } from 'claude-code'

import type { Alert, Tokens } from '../types'

/**
 * The quiet desktop band: what it draws, worked out from figures the module
 * already holds. Pure, so the tests reach it without the engine.
 *
 * The Code tab already shows the model, the effort, the permission mode and a
 * context ring, and its transcript lists every tool. What it does not show is
 * cost, quota and the cache, so the quiet band draws nothing until one of
 * those — or the context — passes its limit.
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
 * The alerts for one reading, context first: it is the one with a button.
 *
 * `now` is `$.clock.now()`, the same clock `usage.startedAt` is on.
 */
export function desktopAlerts(usage: SessionUsage, tokens: Tokens | null, now: number): Alert[] {
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

  const usd = usage.cost?.usd
  const hours = (now - usage.startedAt) / 3_600_000
  // A rate over the first minute is noise: one turn's cost read as an hour's.
  const usdPerHour = usd !== undefined && hours >= 1 / 60 ? usd / hours : undefined
  for (const limit of usage.rateLimits) {
    if (limit.percentUsed < LIMITS.quota) continue
    const resetsMs = limit.resetsAt === undefined ? NaN : Date.parse(limit.resetsAt)
    alerts.push({
      kind: 'quota',
      window: limit.kind,
      percent: limit.percentUsed,
      ...(Number.isFinite(resetsMs) ? { resetsAt: Math.floor(resetsMs / 1000) } : {}),
      ...(usd !== undefined ? { usd } : {}),
      ...(usdPerHour !== undefined ? { usdPerHour } : {}),
    })
  }

  const hit = hitRate(tokens)
  if (hit !== null && hit.read < CACHE_READ_UNDER && hit.creation >= CACHE_CREATION_AT) {
    alerts.push({ kind: 'cache', ...hit })
  }

  return alerts
}

/** What a quota alert is notified under: its window and that window's reset. */
export function alertKey(alert: Alert & { kind: 'quota' }): string {
  return `${alert.window}@${alert.resetsAt ?? 'unknown'}`
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
  /** The label's colour: amber for a warning, red past the quota limit. */
  color: string
  detail: string
  /** Whether the row carries the `/compact` button. */
  compact: boolean
}

export function alertRow(alert: Alert, now: number): AlertRow {
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
    case 'quota': {
      const label = `${windowLabel(alert.window)} ${Math.round(alert.percent)}%`
      const reset = resetsIn(alert.resetsAt, now)
      const cost = [
        alert.usd !== undefined ? `$${alert.usd.toFixed(2)}` : null,
        alert.usdPerHour !== undefined ? `$${alert.usdPerHour.toFixed(2)}/h` : null,
      ]
      return {
        gauge: { source: gaugeSvg(alert.percent, QUOTA_MARKS, RED), alt: `Quota ${label}` },
        label,
        color: RED,
        detail: [reset === null ? null : `resets ${reset}`, ...cost]
          .filter((part): part is string => part !== null)
          .join('  '),
        compact: false,
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

/** The notification a quota alert raises once per window and reset. */
export function quotaNotice(alert: Alert & { kind: 'quota' }, now: number): string {
  const reset = resetsIn(alert.resetsAt, now)
  const head = `${windowLabel(alert.window)} quota at ${Math.round(alert.percent)}%.`
  return reset === null ? head : `${head} It resets in ${reset}.`
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
export function gaugeSvg(percent: number, marks: readonly number[], fill: string): string {
  const width = 120
  const height = 10
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
