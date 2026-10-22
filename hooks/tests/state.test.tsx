import { test, expect } from 'claude-code/testing'

import { BAND, BENEATH, PANE, STAMP, USER_COMMAND, bottom, isPaneSpawn } from './harness'

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

// The payload is read off the desktop `compact` band's spawn: the terminal
// band draws alerts alone and runs no binary.
test('an atom-backed count adds up over the session', { options: { desktopLayout: 'compact' } }, async ($, on) => {
  const seen = bottom(on, { surfaces: ['desktop'] })

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
  const seen = bottom(on, { toolResult: { isError: true, result: 'boom' } })
  const ui = await $.ui.mount(BAND)

  // Nothing needs the person yet, so the band yields: what is drawn here is
  // the bottom's own.
  const first = JSON.stringify(await ui.drawn())
  expect(first).toContain(BENEATH)

  // Each call drives a refresh, whose single write to the output atom is the
  // only thing that can redraw the band: every `$.ui.invalidate` call was
  // removed in A3. The third failure in a row opens an alert.
  for (const id of ['r1', 'r2', 'r3']) {
    await $.tool.call({ tool: 'Read', tool_use_id: id, file_path: '/nowhere/three' })
  }
  await seen.settle()

  const second = JSON.stringify(await ui.drawn())
  expect(second).toContain('Read failed 3× in a row')
  expect(second).toContain(BENEATH)

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
  const paneArgv = seen.spawns.filter(isPaneSpawn).at(-1)?.argv ?? []
  expect(paneArgv[paneArgv.indexOf('--layout') + 1]).toBe('rail')
})
