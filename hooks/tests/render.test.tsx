import { test, expect, describe } from 'claude-code/testing'

import {
  BAND,
  BENEATH,
  COLOURED_ROWS,
  DESKTOP_BAND,
  DESKTOP_PANE,
  IDLE_BAND,
  PANE,
  STAMP,
  USER_COMMAND,
  bottom,
  isDesktopBandSpawn,
  isPaneSpawn,
} from './harness'
import type { Spawn } from './harness'

/**
 * What the band and the pane actually draw.
 *
 * This is the batch that replaces watching the band by eye. The mod only
 * has rows once it has run the binary, so each case drives one refresh
 * (a `tool.call`) before mounting, with the stand-in binary answering
 * `COLOURED_ROWS` instead of echoing the payload.
 */

/**
 * The binary-drawn desktop layouts. `quiet`, the default, runs no binary for
 * the band (see `desktop.test.tsx`), so the cases about those runs name one.
 */
const COMPACT = { options: { desktopLayout: 'compact' } }

const ESC = String.fromCharCode(27)
const warm = ($: { tool: { call: (e: never) => Promise<unknown> } }) =>
  $.tool.call({ tool: 'Read', tool_use_id: 'warm', file_path: '/n/warm' } as never)

/** Three calls of one tool, each answered by the bottom's `toolResult`. */
const thrice = async ($: { tool: { call: (e: never) => Promise<unknown> } }, tool = 'Bash') => {
  for (const id of ['f1', 'f2', 'f3']) {
    await $.tool.call({ tool, tool_use_id: id, command: 'cargo test --release' } as never)
  }
}

/** The text a mounted band drew, the chain's beneath it included. */
const drawnOf = async (ui: { drawn: () => Promise<unknown> }) => JSON.stringify(await ui.drawn())

describe('the band', () => {
  test('while nothing needs the person it draws nothing and runs no binary', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await warm($)
    await seen.settle()

    // The statusline draws the identity and the budget, and Claude Code's own
    // transcript every tool, so a band of activity said it all twice.
    expect(seen.spawns.filter(run => run.argv[1] !== '--version' && run.argv[0] !== 'claude')).toEqual([])
    for (const band of [BAND, IDLE_BAND]) {
      const ui = await $.ui.mount(band)
      const drawn = await drawnOf(ui)
      expect(drawn).toContain(BENEATH)
      expect(drawn).not.toContain('M:Opus 5')
      await ui.unmount()
    }
  })

  test('the same tool failing three times in a row draws its row', async ($, on) => {
    const seen = bottom(on, { toolResult: { isError: true, result: 'boom' } })
    const ui = await $.ui.mount(BAND)
    await $.tool.call({ tool: 'Bash', tool_use_id: 'f1', command: 'cargo test --release' } as never)
    await $.tool.call({ tool: 'Bash', tool_use_id: 'f2', command: 'cargo test --release' } as never)
    expect(await drawnOf(ui)).not.toContain('failed')

    await $.tool.call({ tool: 'Bash', tool_use_id: 'f3', command: 'cargo test --release' } as never)
    await seen.settle()
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.find(text => text.text === '✘ Bash failed 3× in a row')?.props.color).toBe('#d75f5f')
    expect(texts.some(text => text.text === 'cargo test --release')).toBe(true)
    // Drawn above what the mods beneath draw, never in its place.
    const drawn = await drawnOf(ui)
    expect(drawn.indexOf('failed 3×')).toBeLessThan(drawn.indexOf(BENEATH))
    await ui.unmount()
  })

  test("a subagent's failures are not the person's run", async ($, on) => {
    const seen = bottom(on, { toolResult: { isError: true, result: 'boom' } })
    for (const id of ['a1', 'a2', 'a3']) {
      await $.tool.call({ tool: 'Bash', tool_use_id: id, command: 'false', agentId: 'agent-1' } as never)
    }
    await seen.settle()
    const ui = await $.ui.mount(BAND)
    expect(await drawnOf(ui)).not.toContain('failed')
    await ui.unmount()
  })

  test('a tool running past a minute draws its row, and its time moves', async ($, on) => {
    const seen = bottom(on, { toolDelayMs: 120_000 })
    const ui = await $.ui.mount(BAND)
    const call = $.tool.call({ tool: 'Bash', tool_use_id: 'b1', command: 'cargo build --release' } as never)
    await seen.settle()
    expect(await drawnOf(ui)).not.toContain('running')

    // No event marks the minute: the band's own timer refreshes at it.
    await seen.advance(60_000)
    await seen.settle()
    expect(await drawnOf(ui)).toContain('◷ Bash running 1m')
    expect(await drawnOf(ui)).toContain('cargo build --release')
    await seen.advance(15_000)
    await seen.settle()
    expect(await drawnOf(ui)).toContain('◷ Bash running 1m 15s')

    // Once it answers, the row goes.
    await seen.advance(45_000)
    await call
    await seen.settle()
    expect(await drawnOf(ui)).not.toContain('running')
    await ui.unmount()
  })

  test('the context near its limit draws a row with the /compact button and no Svg', async ($, on) => {
    bottom(on, { usage: { percent: 90 } })
    await warm($)
    const ui = await $.ui.mount(BAND)
    expect(await drawnOf(ui)).toContain('CTX 90%')
    expect(await ui.findAll({ type: 'Button' })).toHaveLength(1)
    // The terminal draws no `Svg`: the gauge is the desktop's.
    expect(await ui.findAll({ type: 'Svg' })).toEqual([])
    await ui.unmount()
  })

  test('a quota on pace to run out before its reset draws its row', async ($, on) => {
    // Seven days in a window of seven, half used one day in: on pace for 350%.
    const resetsAt = new Date(STAMP + 6 * 86_400_000).toISOString()
    bottom(on, { usage: { rateLimits: [{ kind: 'seven_day', percentUsed: 50, resetsAt }] } })
    await warm($)
    const ui = await $.ui.mount(BAND)
    const drawn = await drawnOf(ui)
    expect(drawn).toContain('7d on pace to run out in ~1d')
    expect(drawn).toContain('resets 6d')
    await ui.unmount()
  })

  test('alerts stand most pressing first, clipped to maxRows', async ($, on) => {
    bottom(on, { toolResult: { isError: true, result: 'boom' }, usage: { percent: 90 } })
    await thrice($)
    const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, maxRows: 1 } })
    const drawn = await drawnOf(ui)
    expect(drawn).toContain('Bash failed 3× in a row')
    expect(drawn).not.toContain('CTX 90%')
    await ui.redraw({ ...BAND.props, maxRows: 4 })
    const both = await drawnOf(ui)
    expect(both.indexOf('failed 3×')).toBeLessThan(both.indexOf('CTX 90%'))
    await ui.unmount()
  })

  test('every alert span is monospaced', async ($, on) => {
    bottom(on, { toolResult: { isError: true, result: 'boom' }, usage: { percent: 90 } })
    await thrice($)
    const ui = await $.ui.mount(BAND)
    const texts = (await ui.findAll({ type: 'Text' })).filter(text => text.text !== BENEATH)
    expect(texts.length).toBeGreaterThan(0)
    for (const text of texts) expect(text.props.mono, text.text).toBe(true)
    await ui.unmount()
  })

  test('with nothing run yet it yields rather than noting that', async ($, on) => {
    bottom(on)
    const ui = await $.ui.mount(BAND)

    // Nothing has gone wrong — the walk simply has not finished. A quiet band
    // draws nothing while nothing needs the person, so "no rows" is the
    // ordinary state and a note keyed on it would sit over the prompt for the
    // session. Only a failed run speaks.
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).toContain(BENEATH)
    expect(drawn).not.toContain('pulseline:')

    await ui.unmount()
  })

  test('idle with nothing to run, the band still says why', async ($, on) => {
    const seen = bottom(on, { binaryVersion: null })
    // A refresh first, so the ladder has run out and the note is the real
    // one rather than "not run yet".
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })
    await seen.settle()
    const ui = await $.ui.mount(IDLE_BAND)

    // The one thing that outranks yielding: an install line nobody sees fixes
    // nothing, and this is the only place a missing binary can say so.
    const text = await ui.find({ type: 'Text' })
    expect(text?.text).toContain('cc-pulseline not found')

    await ui.unmount()
  })

  test('what the mods beneath draw stays, under the band, on both surfaces', COMPACT, async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS, surfaces: ['terminal', 'desktop'], usage: { percent: 90 } })
    await warm($)

    // Answering with its own tree alone, the band took the whole chain's:
    // another mod's `AbovePrompt` content vanished whenever this one had rows,
    // which on the desktop is always. The bottom stands in for that mod.
    for (const [band, own] of [[BAND, 'CTX 90%'], [DESKTOP_BAND, 'M:Opus 5']] as const) {
      const ui = await $.ui.mount(band)
      const drawn = JSON.stringify(await ui.drawn())
      expect(drawn, band.surface).toContain(own)
      expect(drawn, band.surface).toContain(BENEATH)
      // The band's rows first, the chain's beneath them.
      expect(drawn.indexOf(own), band.surface).toBeLessThan(drawn.indexOf(BENEATH))
      await ui.unmount()
    }
  })

  test('a failed run notes it above what the mods beneath draw', async ($, on) => {
    bottom(on, { binaryVersion: null, surfaces: ['terminal', 'desktop'] })
    await warm($)

    for (const band of [IDLE_BAND, DESKTOP_BAND]) {
      const ui = await $.ui.mount(band)
      const drawn = JSON.stringify(await ui.drawn())
      expect(drawn, band.surface).toContain('pulseline:')
      expect(drawn, band.surface).toContain(BENEATH)
      await ui.unmount()
    }
  })

  test('a survey owns the band, so the mod yields', async ($, on) => {
    bottom(on, { usage: { percent: 90 } })
    await warm($)
    const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, hasSurvey: true } })

    const tree = JSON.stringify(await ui.drawn())
    expect(tree).not.toContain('CTX 90%')
    // What the chain beneath drew stands instead, which is what yielding means.
    expect(tree).toContain(BENEATH)

    await ui.unmount()
  })
})

/**
 * The binary's rows as the desktop's `compact` band draws them: the one band
 * that still draws them, so the cases about turning ANSI into spans are its.
 */
const DESKTOP = { stdout: COLOURED_ROWS, surfaces: ['desktop'] } as const
const IDLE_DESKTOP_BAND = { ...DESKTOP_BAND, props: { ...DESKTOP_BAND.props, isWorking: false } } as const

describe("the desktop band's rows", () => {
  test('a span keeps BOTH its text and its colour', COMPACT, async ($, on) => {
    bottom(on, DESKTOP)
    await warm($)
    const ui = await $.ui.mount(DESKTOP_BAND)

    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.length).toBeGreaterThan(0)

    // The regression guard for the JSX spread fix. `<Text {...span} />`
    // compiles, type-checks and keeps `color` while silently dropping
    // `children`, so a test that only checked the colour would have passed
    // against the broken form. Both halves are asserted.
    for (const text of texts) {
      expect(text.text.length, `a Text drew no characters: ${JSON.stringify(text.props)}`)
        .toBeGreaterThan(0)
    }

    const coloured = texts.filter(text => typeof text.props.color === 'string')
    expect(coloured.length, 'no Text carried a colour').toBeGreaterThan(0)

    // The exact colours `spansOf` read off the fixture's SGR codes.
    const byColour = new Map(coloured.map(text => [text.props.color as string, text.text]))
    expect(byColour.get('#00afff')).toBe('M:Opus 5')
    expect(byColour.get('#808080')).toBe('CTX:43%')
    expect(byColour.get('#ff0000')).toBe('!2')
    // Bold rode along with the first one.
    expect(texts.find(text => text.text === 'M:Opus 5')?.props.bold).toBe(true)

    await ui.unmount()
  })

  test('no escape sequence reaches a Text child', COMPACT, async ($, on) => {
    bottom(on, DESKTOP)
    await warm($)
    const ui = await $.ui.mount(DESKTOP_BAND)

    const tree = JSON.stringify(await ui.drawn())
    expect(tree).not.toContain(ESC)
    expect(tree).not.toContain('\\u001b')
    expect(tree).toContain('M:Opus 5')

    await ui.unmount()
  })

  test('braille, box drawing and a plane-15 Nerd Font glyph all land', COMPACT, async ($, on) => {
    // The width-1 BMP restriction in the contract is the cell grid's, on its
    // `cells` field, and that grid is not what the band uses: `TextProps`
    // carries no code-point restriction. So the gauges, the box drawing and
    // the Nerd Font icons the binary emits all pass through as text. What is
    // refused is a control character, which `stripControls` removes.
    const glyphs = '\u{F0CCD} ⣀⣄⣦ ╭─╮ ▰─ 6'
    bottom(on, { ...DESKTOP, stdout: glyphs })
    await warm($)
    const ui = await $.ui.mount(DESKTOP_BAND)

    const drawn = (await ui.find({ type: 'Text' }))?.text ?? ''
    expect(drawn).toContain('\u{F0CCD}') // a plane-15 Nerd Font glyph
    expect(drawn).toContain('⣦') // braille, as the sparkline draws
    expect(drawn).toContain('╭') // box drawing, as the console frame draws
    expect(drawn).toContain('▰') // the gauge's filled mark
    expect(drawn).toBe(glyphs)

    await ui.unmount()
  })

  test('it clips to maxRows, and redraws to a new one', COMPACT, async ($, on) => {
    bottom(on, DESKTOP)
    await warm($)

    // Four rows of output into a band that shows two.
    const ui = await $.ui.mount({ ...DESKTOP_BAND, props: { ...DESKTOP_BAND.props, maxRows: 2 } })
    const rowsAt = async () => {
      const tree = await ui.drawn()
      // The outer Box is a column of row Boxes.
      const children = (tree as { children?: unknown[] }).children ?? []
      return children.filter(
        child => (child as { type?: string } | null)?.type === 'Box',
      ).length
    }

    expect(await rowsAt()).toBe(2)

    await ui.redraw({ ...DESKTOP_BAND.props, maxRows: 4 })
    expect(await rowsAt()).toBe(4)

    await ui.redraw({ ...DESKTOP_BAND.props, maxRows: 1 })
    expect(await rowsAt()).toBe(1)

    await ui.unmount()
  })

  test('a panic on stderr still reaches the band as a note', COMPACT, async ($, on) => {
    const seen = bottom(on, {
      surfaces: ['desktop'],
      exitCode: 101,
      stderr: '\u001b[31mthread ‘main’ panicked\u001b[0m\nat src/main.rs:1:1',
    })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })
    await seen.settle()

    // Unflattened, this took the whole band down with it rather than the one
    // line: `ui.render (AbovePrompt) refused: a text child holds a control
    // character (an escape sequence); the engine drew its own` — so the note
    // explaining the failure was the thing the failure removed.
    const ui = await $.ui.mount(DESKTOP_BAND)
    const text = (await ui.find({ type: 'Text' }))?.text ?? ''
    expect(text).toContain('exit 101')
    expect(text).toContain('panicked')
    // The second line is kept, and kept apart: a run of control characters
    // becomes one space, so the location does not run into the message.
    expect(text).toContain('panicked at src/main.rs:1:1')
    await ui.unmount()
  })

  test('a run that failed draws its note even with nothing running', COMPACT, async ($, on) => {
    const seen = bottom(on, { surfaces: ['desktop'], exitCode: 2, stderr: 'boom' })
    await warm($)

    // The one thing the band says while idle. A binary that is missing or
    // broken raises no event of its own, so if this draw yielded too there
    // would be no surface left to say so on.
    const ui = await $.ui.mount(IDLE_DESKTOP_BAND)
    const text = await ui.find({ type: 'Text' })
    expect(text?.text).toContain('pulseline:')
    expect(text?.text).toContain('exit 2')
    expect(seen.spawns.length).toBeGreaterThan(0)

    await ui.unmount()
  })
})

describe('the pane', () => {
  test('/pulseline opens it, and a second call closes it', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })

    const opened = await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    expect(opened?.text).toBeUndefined()
    // Opening fills the rows before the pane mounts, which means a spawn
    // carrying --layout has already happened.
    expect(seen.lastPanePayload().session_id).toBe('session-under-test')

    const closed = await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    expect(closed?.text).toBe('pulseline pane closed')
  })

  test('it draws the rows and a switcher with the live layout undimmed', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    const ui = await $.ui.mount(PANE)

    const buttons = await ui.findAll({ type: 'Button' })
    expect(buttons.map(button => button.props.label)).toEqual([
      '1: ledger',
      '2: budgets',
      '3: compact',
      '4: rail',
    ])
    // Only the live layout is undimmed.
    expect(buttons.map(button => button.props.dimColor)).toEqual([false, true, true, true])

    // And the statusline rows are there beside it, with their colours.
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.some(text => text.text === 'M:Opus 5' && text.props.color === '#00afff')).toBe(true)

    await ui.unmount()
  })

  test('pressing a layout switches it and re-runs the binary for it', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    const ui = await $.ui.mount(PANE)

    await ui.press({ key: 'layout:budgets' })

    const buttons = await ui.findAll({ type: 'Button' })
    expect(buttons.map(button => button.props.dimColor)).toEqual([true, false, true, true])

    const paneArgv = seen.spawns.filter(isPaneSpawn).at(-1)?.argv ?? []
    expect(paneArgv[paneArgv.indexOf('--layout') + 1]).toBe('budgets')

    await ui.unmount()
  })

  test('every layout the switcher offers can be pressed', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    const ui = await $.ui.mount(PANE)

    for (const [index, name] of ['ledger', 'budgets', 'compact', 'rail'].entries()) {
      await ui.press({ key: `layout:${name}` })

      const buttons = await ui.findAll({ type: 'Button' })
      const undimmed = buttons.findIndex(button => button.props.dimColor === false)
      expect(undimmed, `pressing ${name} should light row ${index}`).toBe(index)

      const paneArgv = seen.spawns.filter(isPaneSpawn).at(-1)?.argv ?? []
      expect(paneArgv[paneArgv.indexOf('--layout') + 1]).toBe(name)
    }

    await ui.unmount()
  })

  test('the binary is told the pane box, not the band box', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    const ui = await $.ui.mount(PANE)
    // The first pane spawn happened before the pane was measured; press to
    // drive one with the measured box.
    await ui.press({ key: 'layout:ledger' })

    const paneArgv = seen.spawns.filter(isPaneSpawn).at(-1)?.argv ?? []
    // One short of the twelve-row box: the switcher takes the last row, and a
    // layout that fills the box exactly would push it below the fold.
    expect(paneArgv[paneArgv.indexOf('--rows') + 1]).toBe('11')
    expect(paneArgv[paneArgv.indexOf('--columns') + 1]).toBe('118')

    await ui.unmount()
  })

  test('opening the pane runs the binary again once the pane is placed', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })

    // Two runs, not one: the first fills the rows so the pane never opens
    // empty, and the second is the one sized to the box the pane was drawn
    // into — `ui.open` resolves once drawn, so a live surface has measured it
    // by then. Without it the pane keeps the initial 12x120 rows until some
    // later tool call or press happens to refresh it.
    //
    // What this case pins is that the second run happens when the pane was
    // placed. The box it carries is the next case's: here nothing has drawn
    // the pane yet, because the kit draws only on `ui.mount`.
    const paneRuns = seen.spawns.filter(isPaneSpawn).length
    expect(paneRuns).toBe(2)
  })

  test('a pane the surface would not place does not stay wanted', async ($, on) => {
    // The surface refusing the pane — too narrow a terminal, say.
    const seen = bottom(on, { stdout: COLOURED_ROWS, panePlaced: false })

    const first = await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    expect(first.text).toContain('could not place its pane')

    // Left wanted, this second call would take the close branch and claim to
    // have closed a pane that was never placed.
    const second = await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    expect(second.text).not.toContain('closed')
    expect(seen.spawns.length).toBeGreaterThan(0)
  })

  test('the /pulseline toggle stops the pane spawn', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    const layoutRuns = () => seen.spawns.filter(isPaneSpawn).length

    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    const whileOpen = layoutRuns()
    expect(whileOpen).toBeGreaterThan(0)

    // The person's own way of closing it. `ui.close` is raised by the
    // engine when a pane goes, not by a noun call, so a test cannot dispatch
    // it: `$.ui.close` is not on the test engine at all (only `ui.render`,
    // `resolve`, `scroll` and `focus` are noun calls). The hook that answers
    // it is one `update` setting the same flag this path sets.
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    await $.tool.call({ tool: 'Read', tool_use_id: 'after-close', file_path: '/n/after' })

    // No further --layout spawn: the mod knows the pane is gone.
    expect(layoutRuns()).toBe(whileOpen)
  })

  test('dropping the pane instance leaves the mod drawing the band alone', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })

    const pane = await $.ui.mount(PANE)
    expect((await pane.findAll({ type: 'Button' })).length).toBe(4)
    await pane.unmount()

    // The band is unaffected by the pane coming and going: it draws what the
    // mods beneath draw, the pane's rows nowhere in it.
    const band = await $.ui.mount(BAND)
    const drawn = JSON.stringify(await band.drawn())
    expect(drawn).toContain(BENEATH)
    expect(drawn).not.toContain('M:Opus 5')
    await band.unmount()
  })
})

/** The `--layout` a spawn asked for. */
const layoutOf = (run: Spawn | undefined) => run?.argv[run.argv.indexOf('--layout') + 1]

const renders = (spawns: Spawn[]) =>
  spawns.filter(run => run.argv[1] !== '--version' && run.argv[0] !== 'claude')

describe('the desktop app', () => {
  test('its band runs compact with ASCII glyphs, sized to its own box', COMPACT, async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['terminal', 'desktop'] })
    const terminal = await $.ui.mount(BAND)
    const desktop = await $.ui.mount({
      ...DESKTOP_BAND,
      props: { ...DESKTOP_BAND.props, maxRows: 3, bodyColumns: 90 },
    })
    await warm($)
    await seen.settle()

    // The desktop runs no statusline, so its band carries identity and budget
    // itself — and draws in a face with no Nerd Font glyphs.
    const desktopRun = renders(seen.spawns).filter(isDesktopBandSpawn).at(-1)
    expect(layoutOf(desktopRun)).toBe('compact')
    expect(desktopRun?.argv).toContain('--no-icons')
    expect(desktopRun?.argv[desktopRun.argv.indexOf('--rows') + 1]).toBe('3')
    expect(desktopRun?.argv[desktopRun.argv.indexOf('--columns') + 1]).toBe('88')

    // The terminal beside it runs nothing: its band draws alerts alone.
    expect(renders(seen.spawns).every(isDesktopBandSpawn)).toBe(true)

    await terminal.unmount()
    await desktop.unmount()
  })

  test('a terminal session makes no band run at all', COMPACT, async ($, on) => {
    const terminalOnly = bottom(on, { stdout: COLOURED_ROWS })
    await warm($)
    await terminalOnly.settle()
    expect(renders(terminalOnly.spawns)).toEqual([])
  })

  test('measuring the desktop band refits it, and only when the box moves', COMPACT, async ($, on) => {
    // The desktop band is drawn idle, so a refresh made before it was measured
    // (session start, a desktop attaching) would otherwise stand at the 3x120
    // guess until the next tool call.
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    const desktopRuns = () => renders(seen.spawns).filter(isDesktopBandSpawn)
    const sizeOf = (run: Spawn | undefined) => [
      run?.argv[run.argv.indexOf('--rows') + 1],
      run?.argv[run.argv.indexOf('--columns') + 1],
    ]
    expect(sizeOf(desktopRuns().at(-1))).toEqual(['3', '120'])

    const props = { ...DESKTOP_BAND.props, maxRows: 2, bodyColumns: 80 }
    const ui = await $.ui.mount({ ...DESKTOP_BAND, props })
    await seen.settle()
    expect(sizeOf(desktopRuns().at(-1))).toEqual(['2', '78'])

    // The same box again draws, and spawns nothing.
    const before = desktopRuns().length
    await ui.redraw(props)
    await seen.settle()
    expect(desktopRuns().length).toBe(before)

    // A resize does.
    await ui.redraw({ ...props, bodyColumns: 60 })
    await seen.settle()
    expect(sizeOf(desktopRuns().at(-1))).toEqual(['2', '58'])

    await ui.unmount()
  })

  test('a session the desktop hosts alone runs only the desktop band', COMPACT, async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    const runs = renders(seen.spawns)
    expect(runs.length).toBeGreaterThan(0)
    expect(runs.every(isDesktopBandSpawn)).toBe(true)
  })

  test('every span is monospaced, on both surfaces', COMPACT, async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS, surfaces: ['terminal', 'desktop'], usage: { percent: 90 } })
    await warm($)

    // The desktop draws a Text in a proportional face unless told otherwise,
    // and the binary packed its columns for a monospaced one.
    for (const band of [BAND, DESKTOP_BAND]) {
      const ui = await $.ui.mount(band)
      // The mod's own Texts: what the bottom draws beneath it is not its to set.
      const texts = (await ui.findAll({ type: 'Text' })).filter(text => text.text !== BENEATH)
      expect(texts.length, `${band.surface} drew no Text`).toBeGreaterThan(0)
      for (const text of texts) {
        expect(text.props.mono, `${band.surface}: ${text.text}`).toBe(true)
      }
      await ui.unmount()
    }
  })

  test('idle, the desktop band still draws: there is no statusline under it', COMPACT, async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await warm($)
    const ui = await $.ui.mount({
      ...DESKTOP_BAND,
      props: { ...DESKTOP_BAND.props, isWorking: false },
    })
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).toContain('M:Opus 5')
    expect(drawn).toContain(BENEATH)
    await ui.unmount()
  })

  test('a desktop attaching mid-session gets its band run', COMPACT, async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['terminal', 'desktop'] })
    const before = renders(seen.spawns).length
    await $.session.attach({ surface: 'desktop', clientId: 'desktop:default' })
    await seen.settle()
    const after = renders(seen.spawns).slice(before)
    expect(after.some(isDesktopBandSpawn)).toBe(true)
  })

  test('the desktop band draws the layout the setting names', { options: { desktopLayout: 'rail' } }, async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    const run = renders(seen.spawns).filter(isDesktopBandSpawn).at(-1)
    // The powerline bar, on the half-block seams `--no-icons` leaves it, given
    // every row of the 3x120 box but one.
    expect(layoutOf(run)).toBe('rail')
    expect(run?.argv).toContain('--no-icons')
    expect(run?.argv[run.argv.indexOf('--rows') + 1]).toBe('2')

    // Rail draws no activity, and the desktop has no statusline to leave the
    // turn in flight to, so `activity` runs under it in the rows it left. The
    // stand-in binary answers four rows whatever it is asked, so the head is
    // clipped to its two and the one left goes to the activity run.
    const under = renders(seen.spawns)
      .filter(r => layoutOf(r) === 'activity' && r.argv.includes('--no-icons'))
      .at(-1)
    expect(under?.argv[under.argv.indexOf('--rows') + 1]).toBe('1')
  })

  test('an unknown desktop layout falls back to quiet', { options: { desktopLayout: 'ledger' } }, async ($, on) => {
    // `ledger` is not among the field's options, so the engine reads it as
    // unset and the default stands: the quiet band, which runs no binary.
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    expect(renders(seen.spawns).filter(isDesktopBandSpawn)).toEqual([])
  })

  test('the mobile app is not drawn on', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS, surfaces: ['mobile'] })
    await warm($)
    const ui = await $.ui.mount({ ...BAND, surface: 'mobile' })
    expect(JSON.stringify(await ui.drawn())).toContain(BENEATH)
    await ui.unmount()
  })

  test('its pane draws the rows and the switcher, with ASCII glyphs', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    const ui = await $.ui.mount(DESKTOP_PANE)

    const buttons = await ui.findAll({ type: 'Button' })
    expect(buttons.map(button => button.props.label)).toEqual([
      '1: ledger',
      '2: budgets',
      '3: compact',
      '4: rail',
    ])
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.some(text => text.text === 'M:Opus 5')).toBe(true)
    expect(texts.every(text => text.props.mono === true)).toBe(true)

    const paneRun = renders(seen.spawns).filter(isPaneSpawn).at(-1)
    expect(layoutOf(paneRun)).toBe('ledger')
    expect(paneRun?.argv).toContain('--no-icons')

    await ui.unmount()
  })

  test('a terminal pane keeps its icons', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await $.command.run({ command: 'pulseline', ...USER_COMMAND })
    const paneRun = renders(seen.spawns).filter(isPaneSpawn).at(-1)
    expect(paneRun?.argv).not.toContain('--no-icons')
  })
})
