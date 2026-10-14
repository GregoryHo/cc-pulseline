import { test, expect } from 'claude-code/testing'

import { BAND, PANE, STAMP, USER_COMMAND, bottom } from './harness'

/**
 * A3: the session's values live in `$.state`, so they outlive a hot reload.
 *
 * A reload is not simulated here, and cannot be from a test — `validate`
 * listing these as `$.state` keys held to the contract in `../types`, plus
 * the engine's own guarantee that `$.state` survives a reload, is the chain.
 * What is worth testing is the half that is this mod's own: that an atom
 * accumulates rather than holding the last value, and that writing an atom a
 * render hook read draws it again with nobody calling `$.ui.invalidate`.
 */

test('an atom-backed count adds up over the session', async ($, on) => {
  const seen = bottom(on)

  // Three calls, two of one tool: what reaches the payload must be the
  // running sum, which is what a closure field would lose on every reload.
  await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/nowhere/one' })
  await $.tool.call({ tool: 'Read', tool_use_id: 'r2', file_path: '/nowhere/two' })
  await $.tool.call({ tool: 'Bash', tool_use_id: 'b1', command: 'true' })
  await seen.settle()

  const payload = seen.lastPayload()
  expect(payload.activity?.completed_total).toBe(3)
  expect(payload.activity?.completed_counts).toEqual([
    { name: 'Read', count: 2, failed: 0, last_completed_at: STAMP },
    { name: 'Bash', count: 1, failed: 0, last_completed_at: STAMP },
  ])
})

test('writing the output atom draws the band again', async ($, on) => {
  const seen = bottom(on)
  const ui = await $.ui.mount(BAND)

  // Nothing run yet, so the band draws its note rather than disappearing.
  const first = await ui.find({ type: 'Text' })
  expect(first?.text).toContain('binary: not run yet')

  // A tool call drives a refresh, whose single write to the output atom is
  // the only thing that can redraw the band: every `$.ui.invalidate` call
  // was removed in A3.
  await $.tool.call({ tool: 'Read', tool_use_id: 'r3', file_path: '/nowhere/three' })
  await seen.settle()

  const second = await ui.find({ type: 'Text' })
  expect(second?.text).not.toContain('binary: not run yet')

  await ui.unmount()
})

test('the person’s pane choice is state, not a closure field', async ($, on) => {
  const seen = bottom(on)

  await $.command.run({ command: 'pulseline', ...USER_COMMAND })
  await seen.settle()
  const ui = await $.ui.mount(PANE)
  await ui.press({ key: 'layout:rail' })
  await ui.unmount()

  // The layout the person picked is what the binary is asked for next, and
  // it is held by the host rather than by the module's own variables.
  const paneArgv = seen.spawns.filter(run => run.argv.includes('--layout')).at(-1)?.argv ?? []
  expect(paneArgv[paneArgv.indexOf('--layout') + 1]).toBe('rail')
})
