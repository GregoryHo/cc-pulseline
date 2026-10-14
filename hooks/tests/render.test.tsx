import { test, expect, describe } from 'claude-code/testing'

import { BAND, BENEATH, COLOURED_ROWS, PANE, USER_COMMAND, bottom } from './harness'

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

  test('with nothing run yet it draws its note rather than disappearing', async ($, on) => {
    bottom(on)
    const ui = await $.ui.mount(BAND)

    const text = await ui.find({ type: 'Text' })
    expect(text?.text).toContain('pulseline:')

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

    const paneArgv = seen.spawns.filter(run => run.argv.includes('--layout')).at(-1)?.argv ?? []
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

      const paneArgv = seen.spawns.filter(run => run.argv.includes('--layout')).at(-1)?.argv ?? []
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

    const paneArgv = seen.spawns.filter(run => run.argv.includes('--layout')).at(-1)?.argv ?? []
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
    const paneRuns = seen.spawns.filter(run => run.argv.includes('--layout')).length
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
    const layoutRuns = () => seen.spawns.filter(run => run.argv.includes('--layout')).length

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
