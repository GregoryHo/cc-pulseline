import { test, expect, describe } from 'claude-code/testing'

import { shouldRefresh } from '../hooks/throttle'

/**
 * The refresh gate, tested as arithmetic rather than as timing.
 *
 * Counting spawns through a moved `mock.clock` would test the same rule
 * through three layers of engine, and would be the kind of test that fails
 * for reasons that have nothing to do with the rule.
 */
describe('shouldRefresh', () => {
  test('refuses a second refresh inside the gap', () => {
    expect(shouldRefresh(1_000, 1_399, 400)).toBe(false)
  })

  test('allows one exactly at the gap', () => {
    expect(shouldRefresh(1_000, 1_400, 400)).toBe(true)
  })

  test('allows one past the gap', () => {
    expect(shouldRefresh(1_000, 5_000, 400)).toBe(true)
  })

  test('a zero gap always allows, so a test need not move a clock', () => {
    expect(shouldRefresh(1_000, 1_000, 0)).toBe(true)
  })

  test('a never-stamped session refreshes at once', () => {
    expect(shouldRefresh(0, 1_700_000_000_000, 400)).toBe(true)
  })

  test('a clock that went backwards does not refresh', () => {
    // Not expected, but the arithmetic should not invert into a storm.
    expect(shouldRefresh(5_000, 1_000, 400)).toBe(false)
  })
})
