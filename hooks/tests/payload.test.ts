import { test, expect, describe } from 'claude-code/testing'

import { buildPayload, modelName } from '../payload'
import type { PayloadSnapshot } from '../payload'

import type { PayloadShape } from './harness'

/**
 * The stdin contract, tested straight.
 *
 * `buildPayload` is pure and takes a plain snapshot, so everything about
 * the shape of the JSON — which fields appear, which are left out, how the
 * engine's answers are folded — is assertable without an engine, a spawn or
 * a drawing. The hooks' side of it (what accumulates into the snapshot) is
 * activity.test.ts.
 */

const BASE: PayloadSnapshot = {
  sessionId: 'session-under-test',
  modelId: 'claude-opus-5[1m]',
  usage: {
    startedAt: 1_699_000_000_000,
    context: { window: 1_000_000, percent: 43 },
    rateLimits: [],
    cost: { usd: 1.53 },
  },
  agents: [],
  now: 1_700_000_000_000,
  cwd: '/work',
  version: '2.1.296',
  effort: null,
  outputStyle: null,
  lastApiErrorMs: null,
  cacheReadTotal: 0,
  tokens: null,
  mcpCount: 0,
  claudeMdCount: 0,
  rulesCount: 0,
  memoryCount: 0,
  active: new Map(),
  counts: {},
  completedTotal: 0,
  failedTotal: 0,
  startedAt: {},
  finishedAt: {},
  agentType: {},
  compactCount: 0,
  todo: null,
}

const built = (over: Partial<PayloadSnapshot> = {}): PayloadShape =>
  JSON.parse(buildPayload({ ...BASE, ...over })) as PayloadShape

describe('modelName', () => {
  test('rebuilds the display name the statusline payload carries', () => {
    expect(modelName('claude-opus-5[1m]')).toBe('Opus 5 (1M context)')
    expect(modelName('claude-sonnet-4-5')).toBe('Sonnet 4 5')
  })

  test('drops a dated suffix', () => {
    expect(modelName('claude-haiku-4-20250101')).toBe('Haiku 4')
  })
})

describe('buildPayload', () => {
  test('carries the session the engine named', () => {
    const payload = built()
    expect(payload.session_id).toBe('session-under-test')
    expect(payload.model?.display_name).toBe('Opus 5 (1M context)')
    expect(payload.version).toBe('2.1.296')
    expect(payload.cwd).toBe('/work')
    expect(payload.workspace?.current_dir).toBe('/work')
  })

  test('carries the context window and the cost', () => {
    const payload = built()
    expect(payload.context_window?.context_window_size).toBe(1_000_000)
    expect(payload.context_window?.used_percentage).toBe(43)
    expect(payload.cost?.total_cost_usd).toBe(1.53)
  })

  test('leaves cost out where the host keeps no ledger', () => {
    const payload = built({ usage: { ...BASE.usage, cost: undefined } })
    expect(payload.cost).toBeUndefined()
  })

  test('current_usage is absent before the first turn', () => {
    expect(built().context_window?.current_usage).toBeUndefined()
    const withTokens = built({
      tokens: {
        input_tokens: 10,
        output_tokens: 20,
        cache_read_input_tokens: 30,
        cache_creation_input_tokens: 40,
      },
    })
    expect(withTokens.context_window?.current_usage?.input_tokens).toBe(10)
    expect(withTokens.context_window?.current_usage?.cache_read_input_tokens).toBe(30)
  })

  test('folds the rate-limit windows by kind, resets_at in epoch seconds', () => {
    const payload = built({
      usage: {
        ...BASE.usage,
        rateLimits: [
          { kind: 'five_hour', percentUsed: 33, resetsAt: '2026-09-21T12:00:00Z' },
          { kind: 'seven_day', percentUsed: 71 },
        ],
      },
    })
    expect(payload.rate_limits?.five_hour?.used_percentage).toBe(33)
    expect(payload.rate_limits?.five_hour?.resets_at).toBe(1_789_992_000)
    // No reset time is 0 rather than absent, which is what the binary reads.
    expect(payload.rate_limits?.seven_day?.resets_at).toBe(0)
  })

  test('an env count of zero is left out, so the binary keeps its own scan', () => {
    const payload = built({ mcpCount: 2, claudeMdCount: 0, rulesCount: 3, memoryCount: 0 })
    // The MCP count is always sent: zero servers serving tools is a fact.
    expect(payload.env?.mcp_count).toBe(2)
    expect(payload.env?.claude_md_count).toBeUndefined()
    expect(payload.env?.rules_count).toBe(3)
    expect(payload.env?.memory_count).toBeUndefined()
  })

  test('running tools keep their order and null their missing target', () => {
    const payload = built({
      active: new Map([
        ['t1', { id: 't1', name: 'Read', target: '/work/src/main.rs' }],
        ['t2', { id: 't2', name: 'Bash', target: undefined }],
      ]),
    })
    expect(payload.activity?.tools).toEqual([
      { id: 't1', name: 'Read', target: '/work/src/main.rs' },
      { id: 't2', name: 'Bash', target: null },
    ])
  })

  test('completed counts keep insertion order', () => {
    const payload = built({
      counts: { Read: { count: 12, failed: 0 }, Bash: { count: 8, failed: 2 } },
      completedTotal: 20,
      failedTotal: 2,
    })
    expect(payload.activity?.completed_counts).toEqual([
      { name: 'Read', count: 12, failed: 0 },
      { name: 'Bash', count: 8, failed: 2 },
    ])
    expect(payload.activity?.completed_total).toBe(20)
    expect(payload.activity?.failed_total).toBe(2)
  })

  test('an agent takes its recorded times and type over the list', () => {
    const payload = built({
      agents: [{ id: 'a1', description: 'Map pain points', type: 'general-purpose', status: 'completed' }],
      startedAt: { a1: 1_699_999_000_000 },
      finishedAt: { a1: 1_699_999_500_000 },
      agentType: { a1: 'Explore' },
    })
    const agent = payload.activity?.agents?.[0]
    expect(agent?.agent_type).toBe('Explore')
    expect(agent?.started_at).toBe(1_699_999_000_000)
    expect(agent?.completed_at).toBe(1_699_999_500_000)
  })

  test('a running agent with no stop record has none', () => {
    const payload = built({
      agents: [{ id: 'a2', description: 'Reading', type: 'Explore', status: 'running' }],
    })
    expect(payload.activity?.agents?.[0]?.completed_at).toBeNull()
  })

  test('a settled agent with no stop record is stopped now', () => {
    const payload = built({
      agents: [{ id: 'a3', description: 'Drafting', type: 'Explore', status: 'completed' }],
    })
    expect(payload.activity?.agents?.[0]?.completed_at).toBe(BASE.now)
  })

  test('the optional session fields are left out until they are known', () => {
    const bare = built()
    expect(bare.effort).toBeUndefined()
    expect(bare.output_style).toBeUndefined()
    expect(bare.activity?.last_api_error_ms).toBeUndefined()

    const known = built({ effort: 'xhigh', outputStyle: 'explanatory', lastApiErrorMs: 1_700_000_000_500 })
    expect(known.effort).toEqual({ level: 'xhigh' })
    expect(known.output_style).toEqual({ name: 'explanatory' })
    expect(known.activity?.last_api_error_ms).toBe(1_700_000_000_500)
  })

  test('the same snapshot gives the same bytes, and nothing is mutated', () => {
    const snapshot: PayloadSnapshot = {
      ...BASE,
      agents: [{ id: 'a9', description: 'Nothing recorded', type: 'Explore', status: 'running' }],
    }
    const once = buildPayload(snapshot)
    const twice = buildPayload(snapshot)
    expect(once).toBe(twice)
    // The agent start-time memo belongs to `collect`, not here: a pure
    // builder must not have written one on the way through.
    expect(Object.keys(snapshot.startedAt)).toEqual([])
  })
})
