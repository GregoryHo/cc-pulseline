import { test, expect } from 'claude-code/testing'
import type { On } from 'claude-code'

/**
 * A3: the session's values live in `$.state`, so they outlive a hot reload.
 *
 * A reload is not simulated here — `validate` proving these are `$.state`
 * keys held to the contract, plus the engine's own guarantee that `$.state`
 * survives a reload, is the chain. What is worth testing is the half that is
 * this mod's own: that the atom accumulates rather than holding the last
 * value, and that writing an atom a render hook read draws it again.
 */


const BAND = {
  plugin: 'pulseline-mod',
  surface: 'terminal',
  component: 'AbovePrompt',
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 4,
    bodyColumns: 120,
    scroll: { offset: 0, bodyRows: 4 },
    view: {},
  },
} as const

/**
 * Answers everything `refresh` asks the engine for, with the binary echoing
 * the payload it was handed back as its output.
 *
 * `$.state` is not readable from a test — the host refuses a `$` call the
 * caller's own scan does not list — so the running totals are asserted where
 * they are load-bearing: in the JSON the mod puts on the binary's stdin.
 */
const bottom = (on: On) => {
  // A tool call answers with `{ result }`; every other noun call with
  // `{ value }`, which is what its `next(e)` resolves to.
  on('tool.call', () => ({ result: 'ok', isReadOnly: true }))
  // A moving clock, so `maybeRefresh`'s 400ms gate opens for every call
  // instead of throttling all but the first.
  let nowMs = 1_700_000_000_000
  on('clock.now', () => ({ value: (nowMs += 1_000) }))
  on('session.usage', () => ({
    value: {
      startedAt: 1_699_000_000_000,
      context: { window: 1_000_000, percent: 10 },
      rateLimits: [],
      cost: { usd: 1 },
    },
  }))
  on('session.model', () => ({ value: 'claude-opus-5[1m]' }))
  on('session.id', () => ({ value: 'session-under-test' }))
  on('agent.list', () => ({ value: [] }))
  on('tool.list', () => ({ value: [] }))
  // The stand-in binary echoes the payload it was handed, so the band draws
  // the JSON and the test can read the totals straight off it.
  on('process.run', ($$, e) => ({
    value: {
      exitCode: 0,
      stdout: String(e.init?.stdin ?? ''),
      stderr: '',
      isStdoutTruncated: false,
      isStderrTruncated: false,
    },
  }))
}

test('an atom-backed count adds up over the session', async ($, on) => {
  bottom(on)

  // Three calls, two of one tool: what reaches the payload must be the
  // running sum, which is what a closure field would lose on every reload.
  await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/nowhere/one' })
  await $.tool.call({ tool: 'Read', tool_use_id: 'r2', file_path: '/nowhere/two' })
  await $.tool.call({ tool: 'Bash', tool_use_id: 'b1', command: 'true' })

  const ui = await $.ui.mount(BAND)
  const drawn = await ui.find({ type: 'Text' })
  const payload = drawn?.text ?? ''

  expect(payload).toContain('"completed_total":3')
  expect(payload).toContain('{"name":"Read","count":2,"failed":0}')
  expect(payload).toContain('{"name":"Bash","count":1,"failed":0}')
  await ui.unmount()
})

test('writing the output atom draws the band again', async ($, on) => {
  on('tool.call', () => ({ result: 'ok', isReadOnly: true }))

  const ui = await $.ui.mount(BAND)

  // Nothing run yet, so the band draws its note rather than disappearing.
  const first = await ui.find({ type: 'Text' })
  expect(first?.text).toContain('binary: not run yet')

  // A tool call drives a refresh. There is no process in a test, so the
  // refresh cannot reach the binary and writes a note saying why — which is
  // all this needs: it is a write to the atom the band read while drawing.
  await $.tool.call({ tool: 'Read', tool_use_id: 'r3', file_path: '/nowhere/three' })

  const second = await ui.find({ type: 'Text' })
  expect(second?.text).not.toContain('binary: not run yet')
  await ui.unmount()
})
