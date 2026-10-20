import { test, expect, describe } from 'claude-code/testing'
import type { SessionUsage } from 'claude-code'

import {
  COMPACT_KEY,
  alertRow,
  desktopAlerts,
  formatSpan,
  gaugeSvg,
  hitRate,
} from '../desktop'
import { BAND, BENEATH, DESKTOP_BAND, STAMP, bottom, isDesktopBandSpawn } from './harness'
import type { Spawn } from './harness'

/**
 * The quiet desktop band: empty until context, quota or the cache passes its
 * limit, then a gauge, a line and — for context — a `/compact` button.
 *
 * The pure half (`hooks/desktop.ts`) is read directly; the rest is driven
 * through the engine, one refresh (a `tool.call`) before each mount.
 */

const warm = ($: { tool: { call: (e: never) => Promise<unknown> } }, id = 'warm') =>
  $.tool.call({ tool: 'Read', tool_use_id: id, file_path: '/n/warm' } as never)

const renders = (spawns: Spawn[]) =>
  spawns.filter(run => run.argv[1] !== '--version' && run.argv[0] !== 'claude')

const usage = (over: Partial<SessionUsage> = {}): SessionUsage => ({
  startedAt: STAMP - 2 * 3_600_000,
  context: { window: 200_000, percent: 40 },
  rateLimits: [],
  cost: { usd: 3.5 },
  ...over,
})

const tokens = (read: number, creation: number, input: number) => ({
  input_tokens: input,
  output_tokens: 0,
  cache_read_input_tokens: read,
  cache_creation_input_tokens: creation,
})

/** A reset 47 minutes after the test clock, as the engine names one. */
const RESETS_AT = new Date(STAMP + 47 * 60_000).toISOString()

describe('the alerts', () => {
  test('none while every value is under its limit', () => {
    expect(desktopAlerts(usage(), tokens(900, 50, 50), STAMP)).toEqual([])
  })

  test('context at its limit opens one, with the tokens used', () => {
    const alerts = desktopAlerts(
      usage({ context: { window: 200_000, percent: 73 } }),
      null,
      STAMP,
    )
    expect(alerts).toEqual([{ kind: 'ctx', percent: 73, used: 146_000, window: 200_000 }])
    const row = alertRow(alerts[0]!, STAMP)
    expect(row.label).toBe('CTX 73%')
    expect(row.detail).toBe('146.0k / 200.0k')
    expect(row.compact).toBe(true)
  })

  test('a quota window past its limit carries its reset and the cost rate', () => {
    const alerts = desktopAlerts(
      usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 89, resetsAt: RESETS_AT }] }),
      null,
      STAMP,
    )
    expect(alerts).toHaveLength(1)
    const row = alertRow(alerts[0]!, STAMP)
    expect(row.label).toBe('5h 89%')
    expect(row.detail).toBe('resets 47m  $3.50  $1.75/h')
    expect(row.compact).toBe(false)
  })

  test('a quota window under its limit opens nothing', () => {
    const quiet = usage({ rateLimits: [{ kind: 'seven_day', percentUsed: 84 }] })
    expect(desktopAlerts(quiet, null, STAMP)).toEqual([])
  })

  test('a rebuilding cache opens a hint; a cold start does not', () => {
    // Under 40% read with half or more written: the cache is being rebuilt.
    const rebuilding = desktopAlerts(usage(), tokens(220, 640, 140), STAMP)
    expect(rebuilding.map(alert => alert.kind)).toEqual(['cache'])
    expect(alertRow(rebuilding[0]!, STAMP).detail).toBe('rebuilding · 64% of input is new cache')
    // Under 40% read with little written is a cold start, not worth a line.
    expect(desktopAlerts(usage(), tokens(100, 100, 800), STAMP)).toEqual([])
    expect(hitRate(tokens(0, 0, 0))).toBeNull()
  })

  test('spans read as their two largest units', () => {
    expect(formatSpan(372_000)).toBe('6m 12s')
    expect(formatSpan(47 * 60_000)).toBe('47m')
    expect(formatSpan(7_500_000)).toBe('2h 5m')
    expect(formatSpan(0)).toBe('0s')
  })

  test('the gauge fills its share and ticks each mark', () => {
    const svg = gaugeSvg(50, [55, 70], '#d7875f')
    expect(svg.startsWith('<svg')).toBe(true)
    expect(svg).toContain('<rect width="60" height="10" rx="3" fill="#d7875f"/>')
    expect(svg).toContain('x="66"')
    expect(svg).toContain('x="84"')
  })
})

describe('the quiet band', () => {
  test('is the default, spawns no binary for the band, and yields under every limit', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    expect(renders(seen.spawns).filter(isDesktopBandSpawn)).toEqual([])

    const ui = await $.ui.mount(DESKTOP_BAND)
    expect(JSON.stringify(await ui.drawn())).toContain(BENEATH)
    await ui.unmount()
  })

  test('context past its limit draws a gauge and a /compact button', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { window: 200_000, percent: 73 } })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)

    const svgs = await ui.findAll({ type: 'Svg' })
    expect(svgs).toHaveLength(1)
    expect(svgs[0]?.props.alt).toBe('Context CTX 73%')
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.map(text => text.text)).toEqual(['CTX 73%', '146.0k / 200.0k'])
    expect(texts.every(text => text.props.mono === true)).toBe(true)
    const buttons = await ui.findAll({ type: 'Button' })
    expect(buttons.map(button => button.props.label)).toEqual(['/compact'])

    await ui.unmount()
  })

  test('the /compact button fills an empty prompt and does not send it', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { percent: 73 } })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)
    await ui.press({ key: COMPACT_KEY })
    expect(seen.fills).toEqual(['/compact'])
    expect(seen.toasts).toEqual([])
    await ui.unmount()
  })

  test('the /compact button keeps a draft and says why', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { percent: 73 }, draft: 'half a thought' })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)
    await ui.press({ key: COMPACT_KEY })
    expect(seen.fills).toEqual([])
    expect(seen.toasts).toHaveLength(1)
    await ui.unmount()
  })

  test('a quota window past its limit is notified once, across refreshes', async ($, on) => {
    const seen = bottom(on, {
      surfaces: ['desktop'],
      usage: { rateLimits: [{ kind: 'five_hour', percentUsed: 89, resetsAt: RESETS_AT }] },
    })
    await warm($, 'first')
    await seen.settle()
    await warm($, 'second')
    await seen.settle()
    expect(seen.notifications).toEqual(['5h quota at 89%. It resets in 47m.'])

    const ui = await $.ui.mount(DESKTOP_BAND)
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.map(text => text.text)[0]).toBe('5h 89%')
    expect(await ui.findAll({ type: 'Button' })).toEqual([])
    await ui.unmount()
  })

  test('a notification no channel sends becomes a toast', async ($, on) => {
    const seen = bottom(on, {
      surfaces: ['desktop'],
      notifySent: false,
      usage: { rateLimits: [{ kind: 'seven_day', percentUsed: 92 }] },
    })
    await warm($)
    await seen.settle()
    expect(seen.notifications).toEqual(['7d quota at 92%.'])
    expect(seen.toasts).toEqual(['7d quota at 92%.'])
  })

  test('a terminal session has no alerts and no notifications', async ($, on) => {
    const seen = bottom(on, {
      usage: { percent: 90, rateLimits: [{ kind: 'five_hour', percentUsed: 99 }] },
    })
    await warm($)
    await seen.settle()
    expect(seen.notifications).toEqual([])
    // The terminal band runs as it always has: the activity layout.
    const run = renders(seen.spawns).at(-1)
    expect(run?.argv[run.argv.indexOf('--layout') + 1]).toBe('activity')
    const ui = await $.ui.mount(BAND)
    expect(await ui.findAll({ type: 'Svg' })).toEqual([])
    await ui.unmount()
  })
})

describe('a long turn', () => {
  const turn = (durationMs: number, extra: Record<string, unknown> = {}) => ({
    reason: 'answer' as const,
    answer: '',
    durationMs,
    isAborted: false,
    turnId: 'turn',
    ...extra,
  })

  test('is notified when it ends, where the desktop is attached', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'] })
    await $.turn.complete(turn(372_000))
    await seen.settle()
    expect(seen.notifications).toEqual(['The turn ended after 6m 12s.'])
  })

  test('is not notified under three minutes, interrupted, or in a subagent', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'] })
    await $.turn.complete(turn(179_000))
    await $.turn.complete({ ...turn(400_000), reason: 'aborted', isAborted: true })
    await $.turn.complete(turn(400_000, { agentId: 'agent-1' }))
    await seen.settle()
    expect(seen.notifications).toEqual([])
  })

  test('is not notified in a terminal session', async ($, on) => {
    const seen = bottom(on)
    await $.turn.complete(turn(400_000))
    await seen.settle()
    expect(seen.notifications).toEqual([])
  })
})
