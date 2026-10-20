import { test, expect, describe } from 'claude-code/testing'
import type { SessionUsage } from 'claude-code'

import {
  COMPACT_KEY,
  alertRow,
  desktopAlerts,
  formatSpan,
  gaugeSvg,
  gitLabel,
  hitRate,
  parseGitStatus,
  quotaPast,
  stripCells,
  stripOf,
} from '../desktop'
import { BAND, BENEATH, DESKTOP_BAND, STAMP, bottom, isDesktopBandSpawn } from './harness'
import type { Spawn } from './harness'

/**
 * The quiet desktop band: one thin strip of quota, cost and the branch, kept
 * on screen, and a row more only when the context or the cache passes its
 * limit — the context's with a `/compact` button.
 *
 * The pure half (`hooks/desktop.ts`) is read directly; the rest is driven
 * through the engine, one refresh (a `tool.call`) before each mount.
 */

const warm = ($: { tool: { call: (e: never) => Promise<unknown> } }, id = 'warm') =>
  $.tool.call({ tool: 'Read', tool_use_id: id, file_path: '/n/warm' } as never)

const renders = (spawns: Spawn[]) =>
  spawns.filter(
    run => run.argv[1] !== '--version' && run.argv[0] !== 'claude' && run.argv[0] !== 'git',
  )

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

/** `git status --porcelain=v2 --branch` on `main`, two ahead, one file changed. */
const GIT_STATUS = [
  '# branch.oid 0123456789abcdef',
  '# branch.head main',
  '# branch.upstream origin/main',
  '# branch.ab +2 -0',
  '1 .M N... 100644 100644 100644 0123 4567 hooks/register.tsx',
].join('\n')

describe('the alerts', () => {
  test('none while the context and the cache are under their limits', () => {
    expect(desktopAlerts(usage(), tokens(900, 50, 50))).toEqual([])
  })

  test('context at its limit opens one, with the tokens used', () => {
    const alerts = desktopAlerts(usage({ context: { window: 200_000, percent: 73 } }), null)
    expect(alerts).toEqual([{ kind: 'ctx', percent: 73, used: 146_000, window: 200_000 }])
    const row = alertRow(alerts[0]!)
    expect(row.label).toBe('CTX 73%')
    expect(row.detail).toBe('146.0k / 200.0k')
    expect(row.compact).toBe(true)
  })

  test('quota opens no row: the strip draws it', () => {
    const full = usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 99 }] })
    expect(desktopAlerts(full, null)).toEqual([])
    expect(quotaPast(full).map(quota => quota.window)).toEqual(['five_hour'])
    expect(quotaPast(usage({ rateLimits: [{ kind: 'seven_day', percentUsed: 84 }] }))).toEqual([])
  })

  test('a rebuilding cache opens a hint; a cold start does not', () => {
    // Under 40% read with half or more written: the cache is being rebuilt.
    const rebuilding = desktopAlerts(usage(), tokens(220, 640, 140))
    expect(rebuilding.map(alert => alert.kind)).toEqual(['cache'])
    expect(alertRow(rebuilding[0]!).detail).toBe('rebuilding · 64% of input is new cache')
    // Under 40% read with little written is a cold start, not worth a line.
    expect(desktopAlerts(usage(), tokens(100, 100, 800))).toEqual([])
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

describe('the strip', () => {
  test('draws each quota window, the cost and its rate, then the branch', () => {
    const strip = stripOf(
      usage({
        rateLimits: [
          { kind: 'five_hour', percentUsed: 41, resetsAt: RESETS_AT },
          { kind: 'seven_day', percentUsed: 22 },
        ],
      }),
      parseGitStatus(GIT_STATUS),
      STAMP,
    )
    const cells = stripCells(strip, STAMP)
    expect(cells.map(cell => [cell.label, cell.detail])).toEqual([
      ['5h 41%', 'resets 47m'],
      ['7d 22%', null],
      ['$3.50', '$1.75/h'],
      ['G:main* ↑2', null],
    ])
    expect(cells.slice(0, 2).every(cell => cell.gauge?.source.startsWith('<svg'))).toBe(true)
    // Only the branch takes a row of its own; the rest sit side by side.
    expect(cells.map(cell => cell.wide === true)).toEqual([false, false, false, true])
  })

  test('the reset reads tight, with no space in the span', () => {
    const resetsAt = new Date(STAMP + (4 * 60 + 22) * 60_000).toISOString()
    const cells = stripCells(
      stripOf(usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 15, resetsAt }] }), null, STAMP),
      STAMP,
    )
    expect(cells[0]?.detail).toBe('resets 4h22m')
  })

  test('with no quota reading, draws the cost alone', () => {
    const cells = stripCells(stripOf(usage(), null, STAMP), STAMP)
    expect(cells.map(cell => cell.label)).toEqual(['$3.50'])
  })

  test('a window past its limit draws red', () => {
    const strip = stripOf(usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 89 }] }), null, STAMP)
    expect(stripCells(strip, STAMP)[0]?.color).toBe('#d75f5f')
  })

  test('reads the branch, its changes and its distance from upstream', () => {
    expect(parseGitStatus(GIT_STATUS)).toEqual({ branch: 'main', dirty: true, ahead: 2, behind: 0 })
    const clean = parseGitStatus('# branch.head feat\n# branch.ab +0 -3\n')
    expect(clean && gitLabel(clean)).toBe('G:feat ↓3')
    expect(parseGitStatus('')).toBeNull()
  })
})

describe('the quiet band', () => {
  test('is the default and spawns no binary for the band', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    expect(renders(seen.spawns).filter(isDesktopBandSpawn)).toEqual([])
    expect(renders(seen.spawns)).toEqual([])
  })

  test('keeps the strip on screen, above what the mods beneath draw', async ($, on) => {
    const seen = bottom(on, {
      surfaces: ['desktop'],
      gitStatus: GIT_STATUS,
      usage: { rateLimits: [{ kind: 'five_hour', percentUsed: 41, resetsAt: RESETS_AT }] },
    })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)

    const svgs = await ui.findAll({ type: 'Svg' })
    expect(svgs.map(svg => svg.props.alt)).toEqual(['Quota 5h 41%'])
    const texts = (await ui.findAll({ type: 'Text' })).map(text => text.text)
    expect(texts).toEqual(['5h 41%', 'resets 47m', '$1.53', '$0.01/h', 'G:main* ↑2', BENEATH])
    // No button while nothing asks for an action.
    expect(await ui.findAll({ type: 'Button' })).toEqual([])
    await ui.unmount()
  })

  test('with nothing to draw, draws only what the mods beneath draw', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { cost: undefined } })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)
    expect(JSON.stringify(await ui.drawn())).toContain(BENEATH)
    expect(await ui.findAll({ type: 'Svg' })).toEqual([])
    expect((await ui.findAll({ type: 'Text' })).map(text => text.text)).toEqual([BENEATH])
    await ui.unmount()
  })

  test('context past its limit adds a gauge and a /compact button under the strip', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { window: 200_000, percent: 73 } })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)

    const svgs = await ui.findAll({ type: 'Svg' })
    expect(svgs.map(svg => svg.props.alt)).toEqual(['Context CTX 73%'])
    // The strip (cost only: no quota reading), the alert, then the chain beneath.
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.map(text => text.text)).toEqual(['$1.53', '$0.01/h', 'CTX 73%', '146.0k / 200.0k', BENEATH])
    const own = texts.filter(text => text.text !== BENEATH)
    expect(own.every(text => text.props.mono === true)).toBe(true)
    const buttons = await ui.findAll({ type: 'Button' })
    expect(buttons.map(button => button.props.label)).toEqual(['/compact'])

    await ui.unmount()
  })

  test('past its rows, drops the strip before an alert', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { percent: 73 } })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount({ ...DESKTOP_BAND, props: { ...DESKTOP_BAND.props, maxRows: 1 } })
    const texts = (await ui.findAll({ type: 'Text' })).map(text => text.text)
    expect(texts[0]).toBe('CTX 73%')
    expect(texts).toContain(BENEATH)
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

    // The strip draws it; no row of its own, no button.
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

  test('a terminal session has no strip, no alerts and no notifications', async ($, on) => {
    const seen = bottom(on, {
      gitStatus: GIT_STATUS,
      usage: { percent: 90, rateLimits: [{ kind: 'five_hour', percentUsed: 99 }] },
    })
    await warm($)
    await seen.settle()
    expect(seen.notifications).toEqual([])
    expect(seen.spawns.some(run => run.argv[0] === 'git')).toBe(false)
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
