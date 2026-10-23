import { test, expect, describe } from 'claude-code/testing'
import type { SessionUsage } from 'claude-code'

import {
  COMPACT_KEY,
  alertRow,
  alertsOf,
  desktopAlerts,
  formatSpan,
  gaugeSvg,
  hitRate,
  nextDeltas,
  nextLongCheck,
  nextStreak,
  onTerminalBand,
  quotaPast,
  runwayOf,
  shareLine,
  shareOf,
  shouldWriteSpend,
  spendOf,
  stripCells,
  stripOf,
  stripRows,
  terminalRow,
} from '../desktop'
import { BAND, BENEATH, DESKTOP_BAND, STAMP, bottom, isDesktopBandSpawn } from './harness'
import type { Recorder, Spawn, UsageOptions } from './harness'

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
    run => run.argv[1] !== '--version' && run.argv[0] !== 'claude',
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

describe('the alerts', () => {
  test('none while the context and the cache are under their limits', () => {
    expect(desktopAlerts(usage(), tokens(900, 50, 50))).toEqual([])
  })

  test('context at its limit opens one, with the tokens used', () => {
    const alerts = desktopAlerts(usage({ context: { window: 200_000, percent: 73 } }), null)
    expect(alerts).toEqual([{ kind: 'ctx', percent: 73, used: 146_000, window: 200_000 }])
    const row = alertRow(alerts[0]!, STAMP)
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
    expect(alertRow(rebuilding[0]!, STAMP).detail).toBe('rebuilding · 64% of input is new cache')
    // Under 40% read with little written is a cold start, not worth a line.
    expect(desktopAlerts(usage(), tokens(100, 100, 800))).toEqual([])
    expect(hitRate(tokens(0, 0, 0))).toBeNull()
  })

  test('spans read as their two largest units', () => {
    expect(formatSpan(372_000)).toBe('6m 12s')
    expect(formatSpan(47 * 60_000)).toBe('47m')
    expect(formatSpan(7_500_000)).toBe('2h 5m')
    expect(formatSpan(0)).toBe('0s')
    expect(formatSpan((154 * 60 + 46) * 60_000)).toBe('6d 10h')
    expect(formatSpan(2 * 86_400_000)).toBe('2d')
  })

  test('a paced gauge stands a tick taller than its bar at the time gone', () => {
    const svg = gaugeSvg(15, [50, 85], '#7a9cc6', 64, 8, 25)
    expect(svg).toContain('height="12"')
    expect(svg).toContain('<rect x="16" width="2" height="12"')
    expect(svg).toContain('<rect y="2" width="10" height="8" rx="3" fill="#7a9cc6"/>')
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
  test('draws each quota window, then the cost and its rate', () => {
    const strip = stripOf(
      usage({
        rateLimits: [
          { kind: 'five_hour', percentUsed: 41, resetsAt: RESETS_AT },
          { kind: 'seven_day', percentUsed: 22 },
        ],
      }),
      STAMP,
    )
    const cells = stripCells(strip, STAMP)
    expect(cells.map(cell => [cell.label, cell.detail])).toEqual([
      ['5h 41%', 'resets 47m'],
      ['7d 22%', null],
      ['$3.50', '$1.75/h'],
    ])
    expect(cells.slice(0, 2).every(cell => cell.gauge?.source.startsWith('<svg'))).toBe(true)
  })

  test('the reset reads tight, with no space in the span', () => {
    const resetsAt = new Date(STAMP + (4 * 60 + 22) * 60_000).toISOString()
    const cells = stripCells(
      stripOf(usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 15, resetsAt }] }), STAMP),
      STAMP,
    )
    expect(cells[0]?.detail).toBe('resets 4h22m')
  })

  test('a reset days away reads in days', () => {
    const resetsAt = new Date(STAMP + (154 * 60 + 46) * 60_000).toISOString()
    const cells = stripCells(
      stripOf(usage({ rateLimits: [{ kind: 'seven_day', percentUsed: 15, resetsAt }] }), STAMP),
      STAMP,
    )
    expect(cells[0]?.detail).toBe('resets 6d10h')
  })

  test('a window on pace to stay under its limit projects its share at reset', () => {
    // 64 minutes into five hours, 4% used: about 19% by the reset.
    const resetsAt = new Date(STAMP + (3 * 60 + 56) * 60_000).toISOString()
    const cells = stripCells(
      stripOf(usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 4, resetsAt }] }), STAMP),
      STAMP,
    )
    expect(cells[0]?.pace).toEqual({ text: '→ ~19% at reset' })
    expect(cells[0]?.color).toBeUndefined()
    expect(cells[0]?.gauge?.height).toBe(12)
  })

  test('a window on pace to run out says when, in amber', () => {
    // 13h14m into seven days, 15% used: the rest is gone in about 3d 2h,
    // long before the reset.
    const resetsAt = new Date(STAMP + (154 * 60 + 46) * 60_000).toISOString()
    const cells = stripCells(
      stripOf(usage({ rateLimits: [{ kind: 'seven_day', percentUsed: 15, resetsAt }] }), STAMP),
      STAMP,
    )
    expect(cells[0]?.pace).toEqual({ text: 'full in ~3d2h', color: '#d7875f' })
    expect(cells[0]?.color).toBe('#d7875f')
    expect(cells[0]?.gauge?.source).toContain('fill="#d7875f"')
  })

  test('projects nothing too early in a window, or without a reset to measure from', () => {
    // 10 minutes into five hours is under the 5% a projection waits for.
    const early = new Date(STAMP + (4 * 60 + 50) * 60_000).toISOString()
    const cells = stripCells(
      stripOf(
        usage({
          rateLimits: [
            { kind: 'five_hour', percentUsed: 3, resetsAt: early },
            { kind: 'seven_day', percentUsed: 22 },
            { kind: 'monthly', percentUsed: 10, resetsAt: early },
          ],
        }),
        STAMP,
      ),
      STAMP,
    )
    expect(cells.slice(0, 3).map(cell => cell.pace)).toEqual([null, null, null])
    expect(cells[0]?.gauge?.height).toBe(8)
  })

  test('with no quota reading, draws the cost alone', () => {
    const cells = stripCells(stripOf(usage(), STAMP), STAMP)
    expect(cells.map(cell => cell.label)).toEqual(['$3.50'])
  })

  test('a window past its limit draws red', () => {
    const strip = stripOf(usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 89 }] }), STAMP)
    expect(stripCells(strip, STAMP)[0]?.color).toBe('#d75f5f')
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
      usage: { rateLimits: [{ kind: 'five_hour', percentUsed: 41, resetsAt: RESETS_AT }] },
    })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)

    const svgs = await ui.findAll({ type: 'Svg' })
    expect(svgs.map(svg => svg.props.alt)).toEqual(['Quota 5h 41%'])
    const texts = (await ui.findAll({ type: 'Text' })).map(text => text.text)
    expect(texts).toEqual([
      '5h 41%',
      '→ ~49% at reset',
      'resets 47m',
      '$1.53',
      '$0.01/h',
      BENEATH,
    ])
    // No button while nothing asks for an action.
    expect(await ui.findAll({ type: 'Button' })).toEqual([])
    // The Code tab shows the branch itself, so the strip never reads one.
    expect(seen.spawns.some(run => run.argv[0] === 'git')).toBe(false)
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

  test('a terminal session leaves the context percent and the quota to its statusline', async ($, on) => {
    const seen = bottom(on, {
      usage: { percent: 90, rateLimits: [{ kind: 'five_hour', percentUsed: 99 }] },
    })
    await warm($)
    await seen.settle()
    expect(seen.notifications).toEqual([])
    // The terminal band runs no binary: its statusline draws the rows.
    expect(renders(seen.spawns)).toEqual([])
    const ui = await $.ui.mount(BAND)
    expect(await ui.findAll({ type: 'Svg' })).toEqual([])
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).not.toContain('CTX 90%')
    expect(drawn).not.toContain('5h 99%')
    expect(await ui.findAll({ type: 'Button' })).toEqual([])
    await ui.unmount()
  })
})

describe('the share of spend', () => {
  const OWN = 'session.session-under-test'
  /** Another session's spend, `ago` before the test clock. */
  const spend = (usd: number, ago: number) => ({ usd, at: STAMP - ago })

  test('is this session over every active one, with how many others', () => {
    const share = shareOf(1, [spend(2, 60_000), spend(1, 9 * 60_000)], STAMP)
    expect(share).toEqual({ share: 0.25, others: 2 })
    expect(shareLine(share!)).toBe('this session ~25% of spend · 2 others active')
    expect(shareLine({ share: 0.5, others: 1 })).toBe('this session ~50% of spend · 1 other active')
  })

  test('leaves out a session quiet for over ten minutes', () => {
    expect(shareOf(1, [spend(1, 60_000), spend(5, 11 * 60_000)], STAMP)).toEqual({ share: 0.5, others: 1 })
  })

  test('is nothing for a lone session, or with nothing spent', () => {
    expect(shareOf(4, [spend(5, 11 * 60_000)], STAMP)).toBeNull()
    expect(shareOf(0, [spend(0, 60_000)], STAMP)).toBeNull()
  })

  test('is written again only when it moved or is going stale', () => {
    expect(shouldWriteSpend(null, 1, STAMP)).toBe(true)
    expect(shouldWriteSpend({ usd: 1, at: STAMP - 10_000 }, 1, STAMP)).toBe(false)
    expect(shouldWriteSpend({ usd: 1, at: STAMP - 10_000 }, 1.2, STAMP)).toBe(true)
    expect(shouldWriteSpend({ usd: 1, at: STAMP - 30_000 }, 1, STAMP)).toBe(true)
    // An older refresh landing after a newer one's write leaves it be.
    expect(shouldWriteSpend({ usd: 1.2, at: STAMP + 5_000 }, 1, STAMP)).toBe(false)
  })

  test('the strip stands as tall as its tallest cell, and a row more with the share', () => {
    const resetsAt = new Date(STAMP + (3 * 60 + 56) * 60_000).toISOString()
    const cells = stripCells(
      stripOf(usage({ rateLimits: [{ kind: 'five_hour', percentUsed: 4, resetsAt }] }), STAMP),
      STAMP,
    )
    expect(stripRows(cells, false)).toBe(3)
    expect(stripRows(cells, true)).toBe(4)
    expect(stripRows([], true)).toBe(0)
  })

  test('a strip four rows tall goes before an alert in a four-row band', async ($, on) => {
    const resetsAt = new Date(STAMP + (3 * 60 + 56) * 60_000).toISOString()
    const seen = bottom(on, {
      surfaces: ['desktop'],
      usage: { percent: 73, rateLimits: [{ kind: 'five_hour', percentUsed: 4, resetsAt }] },
      store: { 'session.a': spend(1.53, 60_000) },
    })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount({ ...DESKTOP_BAND, props: { ...DESKTOP_BAND.props, maxRows: 4 } })
    const texts = (await ui.findAll({ type: 'Text' })).map(text => text.text)
    expect(texts[0]).toBe('CTX 73%')
    expect(texts.some(text => text.startsWith('this session'))).toBe(false)
    await ui.unmount()
  })

  test('reads only a well-formed spend from the store', () => {
    expect(spendOf({ usd: 1, at: 2 })).toEqual({ usd: 1, at: 2 })
    expect(spendOf({ usd: '1', at: 2 })).toBeNull()
    expect(spendOf(null)).toBeNull()
    expect(spendOf(3)).toBeNull()
  })

  test('draws under the gauges, with the other active sessions', async ($, on) => {
    const seen = bottom(on, {
      surfaces: ['desktop'],
      store: { 'session.a': spend(1.53, 60_000), 'session.b': spend(3.06, 2 * 60_000) },
    })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.map(text => text.text)).toEqual([
      '$1.53',
      '$0.01/h',
      'this session ~25% of spend · 2 others active',
      BENEATH,
    ])
    expect(texts.filter(text => text.text !== BENEATH).every(text => text.props.mono === true)).toBe(true)
    await ui.unmount()
  })

  test('leaves a stale session out of the count', async ($, on) => {
    const seen = bottom(on, {
      surfaces: ['desktop'],
      store: { 'session.a': spend(1.53, 60_000), 'session.b': spend(9, 11 * 60_000) },
    })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)
    const texts = (await ui.findAll({ type: 'Text' })).map(text => text.text)
    expect(texts).toContain('this session ~50% of spend · 1 other active')
    await ui.unmount()
  })

  test('a lone session draws no row', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], store: { 'session.a': spend(9, 11 * 60_000) } })
    await warm($)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)
    const texts = (await ui.findAll({ type: 'Text' })).map(text => text.text)
    expect(texts).toEqual(['$1.53', '$0.01/h', BENEATH])
    await ui.unmount()
  })

  test("writes this session's own key, and again only once it goes stale", async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'] })
    await warm($, 'first')
    await seen.settle()
    expect(seen.store.get(OWN)).toEqual({ usd: 1.53, at: STAMP })
    // Unchanged ten seconds on: not rewritten.
    await seen.advance(10_000)
    await warm($, 'second')
    await seen.settle()
    expect(seen.store.get(OWN)).toEqual({ usd: 1.53, at: STAMP })
    // Thirty seconds after the write: written again, to stay active.
    await seen.advance(20_000)
    await warm($, 'third')
    await seen.settle()
    expect(seen.store.get(OWN)).toEqual({ usd: 1.53, at: STAMP + 30_000 })
  })

  test("writes its key again at once when another session's cleanup deleted it", async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'] })
    await warm($, 'first')
    await seen.settle()
    seen.store.delete(OWN)
    await seen.advance(10_000)
    await warm($, 'second')
    await seen.settle()
    expect(seen.store.get(OWN)).toEqual({ usd: 1.53, at: STAMP + 10_000 })
  })

  test('a desktop session on another layout still writes its own key', { options: { desktopLayout: 'rail' } }, async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    // The binary drew the band, so this is `rail` and not the quiet default.
    expect(renders(seen.spawns).filter(isDesktopBandSpawn).length).toBeGreaterThan(0)
    expect(seen.store.get(OWN)).toEqual({ usd: 1.53, at: STAMP })
  })

  test('deletes a key over a day old, and one that is not a spend', async ($, on) => {
    const seen = bottom(on, {
      surfaces: ['desktop'],
      store: {
        'session.old': spend(2, 25 * 3_600_000),
        'session.junk': 'not a spend',
        'session.a': spend(1, 20 * 60_000),
        unrelated: 1,
      },
    })
    await warm($)
    await seen.settle()
    expect([...seen.store.keys()].sort()).toEqual([OWN, 'session.a', 'unrelated'].sort())
  })

  test('a terminal session says what it spends too, and draws no share', async ($, on) => {
    // So a desktop session's share counts the terminal ones beside it.
    const seen = bottom(on, { store: { 'session.old': spend(2, 25 * 3_600_000), 'session.a': spend(1, 60_000) } })
    await warm($)
    await seen.settle()
    expect([...seen.store.keys()].sort()).toEqual([OWN, 'session.a'].sort())
    const ui = await $.ui.mount(BAND)
    expect(JSON.stringify(await ui.drawn())).not.toContain('of spend')
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

describe('the autocompact runway', () => {
  const turn = { reason: 'answer' as const, answer: '', durationMs: 1000, isAborted: false, turnId: 'turn' }
  const AUTO = { threshold: 160_000, enabled: true }

  /** One main turn ending with the context at `tokens` of a 200k window. */
  const endTurn = async (
    $: { turn: { complete: (e: typeof turn) => Promise<unknown> } },
    seen: Recorder,
    tokens: number,
    over: UsageOptions = {},
  ) => {
    seen.setUsage({ window: 200_000, tokens, percent: Math.round((tokens / 200_000) * 100), ...over })
    await $.turn.complete(turn)
    await seen.advance(1000)
  }

  /** The quiet band's own texts, the chain beneath left out. */
  const drawnTexts = async (
    $: { ui: { mount: (e: typeof DESKTOP_BAND) => Promise<{ findAll: (q: { type: string }) => Promise<{ text: string }[]>; unmount: () => Promise<void> }> } },
  ) => {
    const ui = await $.ui.mount(DESKTOP_BAND)
    const texts = (await ui.findAll({ type: 'Text' })).map(text => text.text).filter(text => text !== BENEATH)
    const buttons = await ui.findAll({ type: 'Button' })
    await ui.unmount()
    return { texts, buttons: buttons.length }
  }

  test('each turn adds its growth, the last five kept; a shrink starts over', () => {
    expect(nextDeltas(null, [], 100)).toEqual([])
    expect(nextDeltas(100, [], 110)).toEqual([10])
    expect(nextDeltas(160, [10, 10, 10, 10, 10], 170)).toEqual([10, 10, 10, 10, 10])
    expect(nextDeltas(160, [1, 2, 3, 4, 5], 170)).toEqual([2, 3, 4, 5, 10])
    expect(nextDeltas(120_000, [10_000, 10_000], 40_000)).toEqual([])
  })

  test('turns left at the average growth, floored', () => {
    expect(runwayOf(120_000, [10_000, 10_000], 160_000)).toBe(4)
    expect(runwayOf(125_000, [10_000, 12_000], 160_000)).toBe(3)
    expect(runwayOf(165_000, [10_000, 10_000], 160_000)).toBe(0)
  })

  test('none with under two turns of growth, flat or falling growth, or no threshold', () => {
    expect(runwayOf(120_000, [10_000], 160_000)).toBeNull()
    expect(runwayOf(120_000, [0, 0], 160_000)).toBeNull()
    expect(runwayOf(120_000, [10_000, -20_000], 160_000)).toBeNull()
    expect(runwayOf(120_000, [10_000, 10_000], null)).toBeNull()
  })

  test('opens the context alert early, at five turns, and says how many are left', () => {
    const at60 = usage({ context: { window: 200_000, percent: 60, tokens: 120_000 } })
    expect(desktopAlerts(at60, null, 6)).toEqual([])
    const alerts = desktopAlerts(at60, null, 4)
    expect(alerts).toEqual([{ kind: 'ctx', percent: 60, used: 120_000, window: 200_000, runway: 4 }])
    const row = alertRow(alerts[0]!, STAMP)
    expect(row.label).toBe('CTX 60%')
    expect(row.detail).toBe('~4 turns to autocompact')
    expect(row.compact).toBe(true)
    // Past the limit, the runway is said however far it is.
    const at73 = usage({ context: { window: 200_000, percent: 73, tokens: 146_000 } })
    expect(alertRow(desktopAlerts(at73, null, 9)[0]!, STAMP).detail).toBe('~9 turns to autocompact')
    expect(alertRow({ kind: 'ctx', percent: 78, used: 1, window: 2, runway: 1 }, STAMP).detail).toBe('~1 turn to autocompact')
    expect(alertRow({ kind: 'ctx', percent: 79, used: 1, window: 2, runway: 0 }, STAMP).detail).toBe('autocompact next turn')
  })

  test('the band shows it once two turns have grown the context', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { cost: undefined, autoCompact: AUTO } })
    await endTurn($, seen, 100_000)
    await endTurn($, seen, 110_000)
    // One turn's growth is not a pace yet: under its limit, no alert.
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual([])

    await endTurn($, seen, 120_000)
    await seen.settle()
    const drawn = await drawnTexts($)
    expect(drawn.texts).toEqual(['CTX 60%', '~4 turns to autocompact'])
    expect(drawn.buttons).toBe(1)
  })

  test('flat growth draws no runway', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { cost: undefined, autoCompact: AUTO } })
    for (const tokens of [120_000, 120_000, 120_000]) await endTurn($, seen, tokens)
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual([])
  })

  test('a compaction starts the series over', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { cost: undefined, autoCompact: AUTO } })
    for (const tokens of [100_000, 110_000, 120_000]) await endTurn($, seen, tokens)
    // Compacted: the context drops, and the alert closes.
    await endTurn($, seen, 50_000)
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual([])
    // Two fast turns after it. Kept, the drop would average in and read `~1 turn`.
    await endTurn($, seen, 100_000)
    await endTurn($, seen, 150_000)
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual(['CTX 75%', 'autocompact next turn'])
  })

  test('with autocompact off, the alert is as it was', async ($, on) => {
    const off = { enabled: false }
    const seen = bottom(on, { surfaces: ['desktop'], usage: { cost: undefined, autoCompact: off } })
    for (const tokens of [100_000, 110_000, 120_000]) await endTurn($, seen, tokens)
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual([])
    await endTurn($, seen, 146_000)
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual(['CTX 73%', '146.0k / 200.0k'])
  })

  test('a subagent turn is not a sample', async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { cost: undefined, autoCompact: AUTO } })
    await endTurn($, seen, 100_000)
    await endTurn($, seen, 110_000)
    // The main context, unchanged under a subagent's run: a zero would halve the pace.
    seen.setUsage({ tokens: 110_000 })
    await $.turn.complete({ ...turn, agentId: 'agent-1' } as typeof turn)
    await endTurn($, seen, 120_000)
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual(['CTX 60%', '~4 turns to autocompact'])
  })

  test("runs on the breakdown's count, which its threshold is on", async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], usage: { cost: undefined, autoCompact: AUTO } })
    // The last request's input lags the breakdown, which also counts the turn's
    // own answer: the runway is 2 turns, not the 4 the input count reads.
    for (const tokens of [100_000, 110_000, 120_000]) {
      await endTurn($, seen, tokens, { breakdownTokens: tokens + 20_000 })
    }
    await seen.settle()
    expect((await drawnTexts($)).texts).toEqual(['CTX 60%', '~2 turns to autocompact'])
  })

  test('the terminal band draws the runway as text, with the button', async ($, on) => {
    const seen = bottom(on, { usage: { cost: undefined, autoCompact: AUTO } })
    for (const tokens of [100_000, 110_000, 120_000]) await endTurn($, seen, tokens)
    await seen.settle()
    const ui = await $.ui.mount(BAND)
    expect(await ui.findAll({ type: 'Svg' })).toEqual([])
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).not.toContain('CTX 60%')
    expect(drawn).toContain('~4 turns to autocompact')
    expect(await ui.findAll({ type: 'Button' })).toHaveLength(1)
    await ui.unmount()
  })
})

describe('the terminal alerts', () => {
  const tool = (startedAt: number, name = 'Bash', target?: string) => ({
    id: name,
    name,
    startedAt,
    ...(target !== undefined ? { target } : {}),
  })

  test('a run of failures grows on one tool, starts over on another, and ends on its success', () => {
    const one = nextStreak(null, 'Bash', true, 'false')
    expect(one).toEqual({ tool: 'Bash', count: 1, target: 'false' })
    const two = nextStreak(one, 'Bash', true, 'cargo test')
    expect(two).toEqual({ tool: 'Bash', count: 2, target: 'cargo test' })
    // Another tool succeeding leaves the run be; failing starts its own.
    expect(nextStreak(two, 'Read', false, undefined)).toBe(two)
    expect(nextStreak(two, 'Read', true, '/n/a')).toEqual({ tool: 'Read', count: 1, target: '/n/a' })
    expect(nextStreak(two, 'Bash', false, undefined)).toBeNull()
  })

  test('the run opens its alert at three', () => {
    const at = (count: number) =>
      alertsOf(usage(), null, null, { tool: 'Bash', count }, [], STAMP).filter(alert => alert.kind === 'fails')
    expect(at(2)).toEqual([])
    expect(at(3)).toEqual([{ kind: 'fails', tool: 'Bash', count: 3 }])
    expect(alertRow(at(3)[0]!, STAMP).label).toBe('✘ Bash failed 3× in a row')
  })

  test('a call opens its alert after a minute, and the row counts its time', () => {
    const long = (startedAt: number) => alertsOf(usage(), null, null, null, [tool(startedAt, 'Bash', 'sleep 90')], STAMP)
    expect(long(STAMP - 59_000)).toEqual([])
    const [alert] = long(STAMP - 130_000)
    expect(alert).toEqual({ kind: 'long', tool: 'Bash', target: 'sleep 90', startedAt: STAMP - 130_000 })
    expect(alertRow(alert!, STAMP)).toMatchObject({ label: '◷ Bash running 2m 10s', detail: 'sleep 90' })
  })

  test('the refresh for a long call is due when the oldest crosses the minute, then every 15s', () => {
    expect(nextLongCheck([], STAMP)).toBeNull()
    expect(nextLongCheck([tool(STAMP - 20_000), tool(STAMP - 5_000)], STAMP)).toBe(40_000)
    expect(nextLongCheck([tool(STAMP - 59_900)], STAMP)).toBe(1000)
    expect(nextLongCheck([tool(STAMP - 70_000)], STAMP)).toBe(15_000)
  })

  test('the terminal keeps the context alert only with a runway, and draws it as the runway', () => {
    const near = usage({ context: { window: 200_000, percent: 90 } })
    const [none] = alertsOf(near, null, null, null, [], STAMP)
    expect(onTerminalBand(none!)).toBe(false)
    const [ctx] = alertsOf(near, null, 4, null, [], STAMP)
    expect(onTerminalBand(ctx!)).toBe(true)
    // Open on the desktop at 90%, but 20 turns off is not the terminal's to say.
    const [far] = alertsOf(near, null, 20, null, [], STAMP)
    expect(far).toMatchObject({ kind: 'ctx', runway: 20 })
    expect(onTerminalBand(far!)).toBe(false)
    expect(terminalRow(ctx!, STAMP)).toMatchObject({ gauge: null, label: '~4 turns to autocompact', detail: '', compact: true })
  })

  test('they stand failures, long calls, context, cache; the terminal drops the cache', () => {
    const resetsAt = new Date(STAMP + 6 * 86_400_000).toISOString()
    const kinds = alertsOf(
      usage({ context: { window: 200_000, percent: 90 }, rateLimits: [{ kind: 'seven_day', percentUsed: 50, resetsAt }] }),
      tokens(220, 640, 140),
      null,
      { tool: 'Bash', count: 4 },
      [tool(STAMP - 120_000, 'Read')],
      STAMP,
    )
    expect(kinds.map(alert => alert.kind)).toEqual(['fails', 'long', 'ctx', 'cache'])
    expect(kinds.filter(onTerminalBand).map(alert => alert.kind)).toEqual(['fails', 'long'])
  })

  test('the desktop band draws none of the tool alerts', async ($, on) => {
    const seen = bottom(on, { surfaces: ['terminal', 'desktop'], toolResult: { isError: true, result: 'boom' } })
    for (const id of ['f1', 'f2', 'f3']) await warm($, id)
    await seen.settle()
    const ui = await $.ui.mount(DESKTOP_BAND)
    expect(JSON.stringify(await ui.drawn())).not.toContain('failed')
    await ui.unmount()
    // While the terminal beside it does.
    const terminal = await $.ui.mount(BAND)
    expect(JSON.stringify(await terminal.drawn())).toContain('Read failed 3× in a row')
    await terminal.unmount()
  })
})
