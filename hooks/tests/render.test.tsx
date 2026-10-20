import { test, expect, describe } from 'claude-code/testing'

import {
  BAND,
  BENEATH,
  COLOURED_ROWS,
  DESKTOP_BAND,
  DESKTOP_PANE,
  IDLE_BAND,
  PANE,
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

const ESC = String.fromCharCode(27)
const warm = ($: { tool: { call: (e: never) => Promise<unknown> } }) =>
  $.tool.call({ tool: 'Read', tool_use_id: 'warm', file_path: '/n/warm' } as never)

describe('the band', () => {
  test('a span keeps BOTH its text and its colour', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS })
    await warm($)
    const ui = await $.ui.mount(BAND)

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

  test('no escape sequence reaches a Text child', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS })
    await warm($)
    const ui = await $.ui.mount(BAND)

    const tree = JSON.stringify(await ui.drawn())
    expect(tree).not.toContain(ESC)
    expect(tree).not.toContain('\\u001b')
    expect(tree).toContain('M:Opus 5')

    await ui.unmount()
  })

  test('braille, box drawing and a plane-15 Nerd Font glyph all land', async ($, on) => {
    // The width-1 BMP restriction in the contract is the cell grid's, on its
    // `cells` field, and that grid is not what the band uses: `TextProps`
    // carries no code-point restriction. So the gauges, the box drawing and
    // the Nerd Font icons the binary emits all pass through as text. What is
    // refused is a control character, which `stripControls` removes.
    const glyphs = '\u{F0CCD} ⣀⣄⣦ ╭─╮ ▰─ 6'
    bottom(on, { stdout: glyphs })
    await warm($)
    const ui = await $.ui.mount(BAND)

    const drawn = (await ui.find({ type: 'Text' }))?.text ?? ''
    expect(drawn).toContain('\u{F0CCD}') // a plane-15 Nerd Font glyph
    expect(drawn).toContain('⣦') // braille, as the sparkline draws
    expect(drawn).toContain('╭') // box drawing, as the console frame draws
    expect(drawn).toContain('▰') // the gauge's filled mark
    expect(drawn).toBe(glyphs)

    await ui.unmount()
  })

  test('it clips to maxRows, and redraws to a new one', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS })
    await warm($)

    // Four rows of output into a band that shows two.
    const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, maxRows: 2 } })
    const rowsAt = async () => {
      const tree = await ui.drawn()
      // The outer Box is a column of row Boxes.
      const children = (tree as { children?: unknown[] }).children ?? []
      return children.filter(
        child => (child as { type?: string } | null)?.type === 'Box',
      ).length
    }

    expect(await rowsAt()).toBe(2)

    await ui.redraw({ ...BAND.props, maxRows: 4 })
    expect(await rowsAt()).toBe(4)

    await ui.redraw({ ...BAND.props, maxRows: 1 })
    expect(await rowsAt()).toBe(1)

    await ui.unmount()
  })

  test('the width it was given is the width the binary is told', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await warm($)

    const ui = await $.ui.mount(BAND)
    // Narrow the band, then drive another refresh so the new width is used.
    await ui.redraw({ ...BAND.props, bodyColumns: 60 })
    await $.tool.call({ tool: 'Read', tool_use_id: 'narrow', file_path: '/n/narrow' })
    await seen.settle()

    const argv = seen.spawns.filter(run => run.argv[1] !== '--version').at(-1)?.argv ?? []
    // Two cells go to the band's own collapse marker.
    expect(argv[argv.indexOf('--columns') + 1]).toBe('58')

    await ui.unmount()
  })

  test('a panic on stderr still reaches the band as a note', async ($, on) => {
    const seen = bottom(on, {
      exitCode: 101,
      stderr: '\u001b[31mthread \u2018main\u2019 panicked\u001b[0m\nat src/main.rs:1:1',
    })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })
    await seen.settle()

    // Unflattened, this took the whole band down with it rather than the one
    // line: `ui.render (AbovePrompt) refused: a text child holds a control
    // character (an escape sequence); the engine drew its own` — so the note
    // explaining the failure was the thing the failure removed.
    const ui = await $.ui.mount(BAND)
    const text = (await ui.find({ type: 'Text' }))?.text ?? ''
    expect(text).toContain('exit 101')
    expect(text).toContain('panicked')
    // The second line is kept, and kept apart: a run of control characters
    // becomes one space, so the location does not run into the message.
    expect(text).toContain('panicked at src/main.rs:1:1')
    await ui.unmount()
  })

  test('with nothing run yet it yields rather than noting that', async ($, on) => {
    bottom(on)
    const ui = await $.ui.mount(BAND)

    // Nothing has gone wrong — the walk simply has not finished. The band
    // asks for the `activity` layout, whose output is empty whenever nothing
    // is running, so "no rows" is the ordinary state and a note keyed on it
    // would sit over the prompt for the session. Only a failed run speaks.
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).toContain(BENEATH)
    expect(drawn).not.toContain('pulseline:')

    await ui.unmount()
  })

  test('a run that failed draws its note even with nothing running', async ($, on) => {
    const seen = bottom(on, { exitCode: 2, stderr: 'boom' })
    await warm($)

    // The one thing the band says while idle. A binary that is missing or
    // broken raises no event of its own, so if this draw yielded too there
    // would be no surface left to say so on.
    const ui = await $.ui.mount(IDLE_BAND)
    const text = await ui.find({ type: 'Text' })
    expect(text?.text).toContain('pulseline:')
    expect(text?.text).toContain('exit 2')
    expect(seen.spawns.length).toBeGreaterThan(0)

    await ui.unmount()
  })

  test('a band with rows draws them while a turn runs', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS })
    await warm($)

    const ui = await $.ui.mount(BAND)
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).toContain('T:Read: .../main.rs')
    expect(drawn).not.toContain(BENEATH)

    await ui.unmount()
  })

  test('the band asks for the activity layout, not the configured one', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    // Mounted first, so the spawn is sized from the band's measured box
    // rather than the pre-measurement default.
    const ui = await $.ui.mount(BAND)
    await warm($)
    await seen.settle()

    const bandArgv = seen.spawns.filter(run => !isPaneSpawn(run)).at(-1)?.argv ?? []
    // Without this the band drew whatever layout the person had configured —
    // the statusline's own identity and budget rows, stacked above the prompt
    // while the statusline drew them below. `activity` is the one layout that
    // draws neither, which is what makes the band a second canvas rather than
    // a copy of one.
    expect(bandArgv[bandArgv.indexOf('--layout') + 1]).toBe('activity')
    // And still told its own box, not the pane's.
    expect(bandArgv[bandArgv.indexOf('--rows') + 1]).toBe('4')
    expect(bandArgv[bandArgv.indexOf('--columns') + 1]).toBe('118')

    await ui.unmount()
  })

  test('idle, the band holds the state it last drew', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS })
    await warm($)

    // `e.props.isWorking` gated this for a release, because the band drew the
    // layout the person had configured — the identity and budget rows the
    // statusline draws on its own line, which stacked made the two the same
    // thing twice. Asking for `activity` ended that: between turns the band
    // holds the turn that just happened, which the statusline never shows.
    const ui = await $.ui.mount(IDLE_BAND)
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).toContain('T:Read: .../main.rs')
    expect(drawn).not.toContain(BENEATH)
    expect(seen.spawns.length).toBeGreaterThan(0)
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

  test('a survey owns the band, so the mod yields', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS })
    await warm($)
    const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, hasSurvey: true } })

    const tree = JSON.stringify(await ui.drawn())
    expect(tree).not.toContain('M:Opus 5')
    // What the chain beneath drew stands instead, which is what yielding means.
    expect(tree).toContain(BENEATH)

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
    expect(first.text).toContain('wider terminal')

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

    // The band is unaffected by the pane coming and going.
    const band = await $.ui.mount(BAND)
    expect((await band.findAll({ type: 'Text' })).some(text => text.text === 'M:Opus 5')).toBe(true)
    await band.unmount()
  })
})

/** The `--layout` a spawn asked for. */
const layoutOf = (run: Spawn | undefined) => run?.argv[run.argv.indexOf('--layout') + 1]

const renders = (spawns: Spawn[]) =>
  spawns.filter(run => run.argv[1] !== '--version' && run.argv[0] !== 'claude')

describe('the desktop app', () => {
  test('its band runs compact with ASCII glyphs, sized to its own box', async ($, on) => {
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

    // The terminal beside it keeps its own run, unchanged.
    const terminalRun = renders(seen.spawns)
      .filter(run => layoutOf(run) === 'activity')
      .at(-1)
    expect(terminalRun?.argv).not.toContain('--no-icons')
    expect(terminalRun?.argv[terminalRun.argv.indexOf('--rows') + 1]).toBe('4')
    expect(terminalRun?.argv[terminalRun.argv.indexOf('--columns') + 1]).toBe('118')

    await terminal.unmount()
    await desktop.unmount()
  })

  test('a terminal session makes no desktop run, and a desktop one no terminal run', async ($, on) => {
    const terminalOnly = bottom(on, { stdout: COLOURED_ROWS })
    await warm($)
    await terminalOnly.settle()
    expect(renders(terminalOnly.spawns).filter(isDesktopBandSpawn)).toEqual([])
    expect(renders(terminalOnly.spawns).some(run => layoutOf(run) === 'activity')).toBe(true)
  })

  test('a session the desktop hosts alone runs only the desktop band', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await warm($)
    await seen.settle()
    const runs = renders(seen.spawns)
    expect(runs.length).toBeGreaterThan(0)
    expect(runs.every(isDesktopBandSpawn)).toBe(true)
  })

  test('every span is monospaced, on both surfaces', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS, surfaces: ['terminal', 'desktop'] })
    await warm($)

    // The desktop draws a Text in a proportional face unless told otherwise,
    // and the binary packed its columns for a monospaced one.
    for (const band of [BAND, DESKTOP_BAND]) {
      const ui = await $.ui.mount(band)
      const texts = await ui.findAll({ type: 'Text' })
      expect(texts.length, `${band.surface} drew no Text`).toBeGreaterThan(0)
      for (const text of texts) {
        expect(text.props.mono, `${band.surface}: ${text.text}`).toBe(true)
      }
      expect(JSON.stringify(await ui.drawn())).toContain('M:Opus 5')
      await ui.unmount()
    }
  })

  test('idle, the desktop band still draws: there is no statusline under it', async ($, on) => {
    bottom(on, { stdout: COLOURED_ROWS, surfaces: ['desktop'] })
    await warm($)
    const ui = await $.ui.mount({
      ...DESKTOP_BAND,
      props: { ...DESKTOP_BAND.props, isWorking: false },
    })
    const drawn = JSON.stringify(await ui.drawn())
    expect(drawn).toContain('M:Opus 5')
    expect(drawn).not.toContain(BENEATH)
    await ui.unmount()
  })

  test('a desktop attaching mid-session gets its band run', async ($, on) => {
    const seen = bottom(on, { stdout: COLOURED_ROWS, surfaces: ['terminal', 'desktop'] })
    const before = renders(seen.spawns).length
    await $.session.attach({ surface: 'desktop', clientId: 'desktop:default' })
    await seen.settle()
    const after = renders(seen.spawns).slice(before)
    expect(after.some(isDesktopBandSpawn)).toBe(true)
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
