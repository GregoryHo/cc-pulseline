import { test, expect, describe } from 'claude-code/testing'

import { ansi256Hex, spansOf, targetOf } from '../register'

/**
 * The three pure readers in the hooks module.
 *
 * These were tuned against real sessions and are the parts a render test
 * exercises only glancingly: `spansOf` decides whether any colour reaches
 * the band at all, and `ansi256Hex` is the arithmetic behind every one of
 * them. `targetOf` picks what a running tool's row shows, which no mounted
 * test managed to observe: a tool is in `active` only between the mark and
 * the `finally`, and two calls raised together were never both in flight at
 * the moment a payload was built. Why not — serialised dispatch, or the
 * `finally` landing before the second mark — was not established, so this
 * is tested here rather than through a drawing.
 */

const ESC = String.fromCharCode(27)
const sgr = (codes: string) => `${ESC}[${codes}m`

describe('ansi256Hex', () => {
  test('the sixteen base colours', () => {
    expect(ansi256Hex(0)).toBe('#000000')
    expect(ansi256Hex(1)).toBe('#800000')
    expect(ansi256Hex(15)).toBe('#ffffff')
  })

  test('the 6x6x6 cube', () => {
    // 16 is the cube's origin, 231 its far corner.
    expect(ansi256Hex(16)).toBe('#000000')
    expect(ansi256Hex(231)).toBe('#ffffff')
    // 39 = 16 + 23 -> r 0, g 3 (175), b 5 (255)
    expect(ansi256Hex(39)).toBe('#00afff')
    // 196 = 16 + 180 -> r 5 (255), g 0, b 0
    expect(ansi256Hex(196)).toBe('#ff0000')
  })

  test('the 24-step grey ramp', () => {
    expect(ansi256Hex(232)).toBe('#080808')
    expect(ansi256Hex(244)).toBe('#808080')
    expect(ansi256Hex(255)).toBe('#eeeeee')
  })
})

describe('spansOf', () => {
  test('plain text is one span with no style', () => {
    expect(spansOf('M:Opus 5')).toEqual([{ children: 'M:Opus 5' }])
  })

  test('a 256-colour run carries its colour, and a reset ends it', () => {
    const spans = spansOf(`${sgr('38;5;39')}M:Opus${sgr('0')} | plain`)
    expect(spans).toEqual([
      { color: '#00afff', children: 'M:Opus' },
      { children: ' | plain' },
    ])
  })

  test('bold, dim and inverse are carried as props', () => {
    expect(spansOf(`${sgr('1')}b`)).toEqual([{ bold: true, children: 'b' }])
    expect(spansOf(`${sgr('2')}d`)).toEqual([{ dimColor: true, children: 'd' }])
    expect(spansOf(`${sgr('7')}i`)).toEqual([{ inverse: true, children: 'i' }])
  })

  test('styles accumulate until a reset', () => {
    const spans = spansOf(`${sgr('1')}${sgr('38;5;196')}!2${sgr('0')} +3`)
    expect(spans).toEqual([
      { bold: true, color: '#ff0000', children: '!2' },
      { children: ' +3' },
    ])
  })

  test('a background colour is its own prop', () => {
    expect(spansOf(`${sgr('48;5;39')}bg`)).toEqual([
      { backgroundColor: '#00afff', children: 'bg' },
    ])
  })

  test('SGR 49 hands the background back without touching the foreground', () => {
    // The rail and anchor seams write 49 at a cluster's pointed outer edge.
    // Carried over, the background would paint the point as a rectangle.
    const spans = spansOf(`${sgr('48;5;39')}${sgr('38;5;196')}seg${sgr('49')}\u25b6`)
    expect(spans).toEqual([
      { backgroundColor: '#00afff', color: '#ff0000', children: 'seg' },
      { color: '#ff0000', children: '\u25b6' },
    ])
  })

  test('several codes in one escape are all applied', () => {
    expect(spansOf(`${sgr('1;2;38;5;244')}x`)).toEqual([
      { bold: true, dimColor: true, color: '#808080', children: 'x' },
    ])
  })

  test('no escape sequence survives into a span', () => {
    const spans = spansOf(`${sgr('38;5;39')}a${sgr('0')}b`)
    for (const span of spans) expect(span.children).not.toContain(ESC)
  })

  test('control characters are dropped, since the band refuses a tree with one', () => {
    const spans = spansOf(`a${String.fromCharCode(7)}b${String.fromCharCode(127)}c`)
    expect(spans).toEqual([{ children: 'abc' }])
  })

  test('an empty run makes no span', () => {
    // Two resets back to back have nothing between them.
    expect(spansOf(`${sgr('0')}${sgr('0')}`)).toEqual([])
  })
})

describe('targetOf', () => {
  test('each tool is read at its own key', () => {
    expect(targetOf({ tool: 'Read', file_path: '/work/src/main.rs' })).toBe('/work/src/main.rs')
    expect(targetOf({ tool: 'Bash', command: 'cargo test' })).toBe('cargo test')
    expect(targetOf({ tool: 'WebFetch', url: 'https://example.com' })).toBe('https://example.com')
    expect(targetOf({ tool: 'WebSearch', query: 'braille gauge' })).toBe('braille gauge')
    expect(targetOf({ tool: 'Skill', skill: 'eli5' })).toBe('eli5')
    expect(targetOf({ tool: 'SendMessage', to: 'docs-track' })).toBe('docs-track')
    expect(targetOf({ tool: 'PushNotification', title: 'CI is green' })).toBe('CI is green')
    expect(targetOf({ tool: 'Monitor', script_id: 'b9tf' })).toBe('b9tf')
  })

  test('a Grep shows what it looked for, not where', () => {
    // Both keys are on the call, and a generic sweep that read `path` first
    // drew the directory being searched instead of the pattern.
    expect(targetOf({ tool: 'Grep', pattern: 'spansOf', path: '/work/hooks' })).toBe('spansOf')
  })

  test('a question is read out of the first question', () => {
    expect(
      targetOf({ tool: 'AskUserQuestion', questions: [{ question: 'Merge or keep separate?' }] }),
    ).toBe('Merge or keep separate?')
  })

  test('an unknown tool falls back to file_path, command, pattern — in that order', () => {
    // The same three the transcript path's generic arm uses.
    expect(targetOf({ tool: 'Unheard', command: 'cargo test', file_path: '/a.rs' })).toBe('/a.rs')
    expect(targetOf({ tool: 'Unheard', pattern: '*.rs' })).toBe('*.rs')
  })

  test('nothing to show is undefined, not an empty string', () => {
    expect(targetOf({})).toBeUndefined()
    expect(targetOf({ tool: 'Read', file_path: '' })).toBeUndefined()
    expect(targetOf({ tool: 'TodoWrite', todos: [] })).toBeUndefined()
  })

  test('a long argument is passed whole, for the renderer to truncate', () => {
    // The renderer keeps the *tail* of a Read path, so a head-only clip here
    // threw away the filename and drew the wrong end of the path.
    const long = `/work/${'x'.repeat(300)}/main.rs`
    expect(targetOf({ tool: 'Read', file_path: long })).toBe(long)
  })

  test('a multi-line command becomes one line', () => {
    expect(targetOf({ tool: 'Bash', command: 'cargo test\n  --release' })).toBe(
      'cargo test   --release',
    )
  })
})
