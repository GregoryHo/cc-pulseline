import { test, expect, describe } from 'claude-code/testing'

import {
  MIN_VERSION,
  candidates,
  formatVersion,
  isNewEnough,
  missingMessage,
  parseVersion,
  tooOldMessage,
} from '../hooks/binary'
import { BAND, bottom } from './harness'

/**
 * Phase B: finding the binary, and refusing one that cannot draw the band.
 *
 * The ladder itself is a pure list, so its order and contents are asserted
 * directly. What needs the kit is the probing: that a hit on the first rung
 * stops the walk, that an old version is refused, and that both failures
 * reach the band as something a person can act on.
 */

const WHERE = {
  pluginRoot: '/plugins/pulseline-mod',
  home: '/home/me',
  cwd: '/work/cc-pulseline',
}

describe('the ladder', () => {
  test('is five rungs in order, with the dev rung last', () => {
    const ladder = candidates({ ...WHERE, configured: '/custom/cc-pulseline' })
    expect(ladder.map(rung => rung.rung)).toEqual([
      'configured',
      'bundled',
      'installed',
      'path',
      'dev',
    ])
    expect(ladder.map(rung => rung.path)).toEqual([
      '/custom/cc-pulseline',
      '/plugins/pulseline-mod/bin/cc-pulseline',
      '/home/me/.claude/pulseline/cc-pulseline',
      'cc-pulseline',
      '/work/cc-pulseline/target/release/cc-pulseline',
    ])
  })

  test('an unset path drops its rung rather than probing an empty string', () => {
    for (const configured of [undefined, '', '   ']) {
      const ladder = candidates({ ...WHERE, configured })
      expect(ladder.map(rung => rung.rung)).toEqual(['bundled', 'installed', 'path', 'dev'])
    }
  })

  test('the dev rung follows the session, not anyone’s absolute path', () => {
    const ladder = candidates({ ...WHERE, cwd: '/somewhere/else' })
    expect(ladder.at(-1)?.path).toBe('/somewhere/else/target/release/cc-pulseline')
  })

  test('a missing home or cwd leaves PATH as the only rung left', () => {
    const ladder = candidates({ pluginRoot: '', home: '', cwd: '' })
    expect(ladder.map(rung => rung.path)).toEqual(['cc-pulseline'])
  })

  test('a configured path that repeats another rung is probed once', () => {
    const ladder = candidates({ ...WHERE, configured: 'cc-pulseline' })
    expect(ladder.filter(rung => rung.path === 'cc-pulseline').length).toBe(1)
  })
})

describe('the version gate', () => {
  test('reads the binary’s own --version line', () => {
    expect(parseVersion('cc-pulseline 1.5.0')).toEqual([1, 5, 0])
    expect(parseVersion('cc-pulseline 1.5.0\n')).toEqual([1, 5, 0])
    expect(parseVersion('cc-pulseline 10.2.33')).toEqual([10, 2, 33])
  })

  test('anything without a version is not a binary we know', () => {
    expect(parseVersion('')).toBeNull()
    expect(parseVersion('command not found')).toBeNull()
    // A two-part version is not enough to judge.
    expect(parseVersion('cc-pulseline 1.5')).toBeNull()
  })

  test('1.5.0 is the floor', () => {
    expect(formatVersion(MIN_VERSION)).toBe('1.5.0')
    expect(isNewEnough([1, 5, 0])).toBe(true)
    expect(isNewEnough([1, 4, 0])).toBe(false)
    // 1.4.9 is still short of 1.5.0; a bigger patch does not make up a minor.
    expect(isNewEnough([1, 4, 9])).toBe(false)
    expect(isNewEnough([1, 5, 1])).toBe(true)
    expect(isNewEnough([2, 0, 0])).toBe(true)
    expect(isNewEnough([0, 9, 9])).toBe(false)
  })

  test('the too-old message says the statusline is fine, because it is', () => {
    const message = tooOldMessage('1.4.0')
    expect(message).toContain('1.4.0 is too old')
    expect(message).toContain('the band needs 1.5.0 or newer; your statusline is unaffected')
    expect(message).toContain('npm install -g @cc-pulseline/cc-pulseline')
  })

  test('the missing message names the install command', () => {
    expect(missingMessage()).toContain('npm install -g @cc-pulseline/cc-pulseline')
  })
})

describe('probing', () => {
  test(
    'a hit on the first rung stops the walk',
    { options: { binaryPath: '/custom/cc-pulseline' } },
    async ($, on) => {
      const seen = bottom(on)
      await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })

      // Exactly one candidate probed, and it was the configured one. The
      // other four rungs are never run.
      expect(seen.probes).toEqual(['/custom/cc-pulseline'])
      // And the binary it then draws with is that one.
      const argv = seen.spawns.filter(run => run.argv[1] !== '--version').at(-1)?.argv ?? []
      expect(argv[0]).toBe('/custom/cc-pulseline')
    },
  )

  test('the ladder is walked in order until something answers', async ($, on) => {
    // Only the installed rung has a binary on it.
    const seen = bottom(on, {
      home: '/home/me',
      binaryAt: ['/home/me/.claude/pulseline/cc-pulseline'],
    })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })

    // The bundled rung was tried first and missed; the installed one answered;
    // PATH and the dev rung were never reached.
    expect(seen.probes.length).toBe(2)
    expect(seen.probes.at(-1)).toBe('/home/me/.claude/pulseline/cc-pulseline')
    const argv = seen.spawns.filter(run => run.argv[1] !== '--version').at(-1)?.argv ?? []
    expect(argv[0]).toBe('/home/me/.claude/pulseline/cc-pulseline')
  })

  test('a too-old rung does not stop the walk', async ($, on) => {
    // This is the real shape of the machine this was written on: `cargo
    // install` left 1.4.0 on PATH, while the dev build is 1.5.0. A ladder
    // that stopped at the first thing to answer would have picked the old
    // one and drawn the classic layout into the band at the wrong width —
    // the failure the version gate exists to catch.
    const seen = bottom(on, {
      home: '/home/me',
      binaryVersions: {
        '/home/me/.claude/pulseline/cc-pulseline': '1.4.0',
        'cc-pulseline': '1.5.0',
      },
    })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })

    // The old rung was probed and passed over; the newer one is what draws.
    expect(seen.probes).toContain('/home/me/.claude/pulseline/cc-pulseline')
    const argv = seen.spawns.filter(run => run.argv[1] !== '--version').at(-1)?.argv ?? []
    expect(argv[0]).toBe('cc-pulseline')
  })

  test('the ladder is walked once a session, not once a refresh', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })
    const afterFirst = seen.probes.length
    await $.tool.call({ tool: 'Read', tool_use_id: 'r2', file_path: '/n/two' })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r3', file_path: '/n/three' })

    expect(seen.probes.length).toBe(afterFirst)
  })

  test('a settings change makes it look again', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })
    const afterFirst = seen.probes.length

    await $.classic.ConfigChange({ source: 'user_settings' })

    // The resolution was forgotten, so the ladder was walked again — which
    // is how a binary installed mid-session gets picked up.
    expect(seen.probes.length).toBeGreaterThan(afterFirst)
  })
})

describe('what the band says when it cannot draw', () => {
  test('a binary that is too old is refused, and the band says so', async ($, on) => {
    const seen = bottom(on, { binaryVersion: '1.4.0' })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })

    // Every rung with a path was tried, and every one refused. Three, not
    // five: `binaryPath` is unset here, and the dev rung is built from the
    // session's cwd, which arrives on `session.start` — an event a test
    // cannot raise, `NounEventOf.session` having no `start`. That rung is
    // covered by the pure ladder tests above instead.
    expect(seen.probes.length).toBe(3)
    expect(seen.probes.at(0)).toContain('/bin/cc-pulseline')
    expect(seen.probes.at(1)).toBe('/home/under-test/.claude/pulseline/cc-pulseline')
    expect(seen.probes.at(2)).toBe('cc-pulseline')
    // Nothing was drawn with it: no spawn carrying a payload.
    expect(seen.spawns.filter(run => run.argv[1] !== '--version')).toEqual([])

    const ui = await $.ui.mount(BAND)
    const text = (await ui.find({ type: 'Text' }))?.text ?? ''
    expect(text).toContain('1.4.0 is too old')
    expect(text).toContain('your statusline is unaffected')
    expect(text).toContain('npm install -g @cc-pulseline/cc-pulseline')
    await ui.unmount()
  })

  test('the too-old note names the newest copy found, not the first', async ($, on) => {
    // Two old rungs. Naming the first would send the person updating to a
    // version they are already past.
    bottom(on, {
      home: '/home/me',
      binaryVersions: {
        '/home/me/.claude/pulseline/cc-pulseline': '1.3.0',
        'cc-pulseline': '1.4.0',
      },
    })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })

    const ui = await $.ui.mount(BAND)
    const text = (await ui.find({ type: 'Text' }))?.text ?? ''
    expect(text).toContain('1.4.0 is too old')
    expect(text).not.toContain('1.3.0')
    await ui.unmount()
  })

  test('nothing found at all names the install command in the band', async ($, on) => {
    const seen = bottom(on, { binaryVersion: null })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })

    expect(seen.probes.length).toBe(3)
    expect(seen.spawns.filter(run => run.argv[1] !== '--version')).toEqual([])

    const ui = await $.ui.mount(BAND)
    const text = (await ui.find({ type: 'Text' }))?.text ?? ''
    expect(text).toContain('cc-pulseline not found')
    expect(text).toContain('npm install -g @cc-pulseline/cc-pulseline')
    // Not an empty band, and not the stale not-run-yet note.
    expect(text).not.toContain('binary: not run yet')
    await ui.unmount()
  })

  test('a candidate that answers nothing useful is skipped, not trusted', async ($, on) => {
    // Something is on PATH under that name, but it is not this binary.
    const seen = bottom(on, { binaryVersion: 'not-a-version' })
    await $.tool.call({ tool: 'Read', tool_use_id: 'r1', file_path: '/n/one' })

    expect(seen.spawns.filter(run => run.argv[1] !== '--version')).toEqual([])
    const ui = await $.ui.mount(BAND)
    expect((await ui.find({ type: 'Text' }))?.text).toContain('cc-pulseline not found')
    await ui.unmount()
  })
})
