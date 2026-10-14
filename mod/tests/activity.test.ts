import { test, expect, describe } from 'claude-code/testing'

import { bottom } from './harness'

/**
 * The hooks' side: what each event accumulates, read off the payload the mod
 * hands the binary.
 *
 * These are the cases the old node:test file covered, carried over. The
 * payload's *shape* is payload.test.ts; this is about what reaches it.
 *
 * Every case ends by driving a refresh — a `tool.call` where the event under
 * test does not refresh by itself — because the payload is only built when
 * the mod spawns the binary. `bottom`'s clock moves 1000ms per read, which
 * keeps `maybeRefresh`'s 400ms gate open.
 */

/** A tool call with no arguments worth showing. */
const call = (id: string) => ({ tool: 'Read' as const, tool_use_id: id, file_path: `/n/${id}` })

describe('tool calls', () => {
  test('a tool is running while it runs, and counted once it is done', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({ tool: 'Read', tool_use_id: 't1', file_path: '/work/src/main.rs' })

    const payload = seen.lastPayload()
    expect(payload.activity?.tools).toEqual([])
    expect(payload.activity?.completed_counts).toEqual([{ name: 'Read', count: 1, failed: 0 }])
    expect(payload.activity?.completed_total).toBe(1)
    expect(payload.activity?.failed_total).toBe(0)
  })

  test('a tool that is still running is not yet counted', async ($, on) => {
    const seen = bottom(on)
    // A tool is in `active` only between the mark and the `finally`, and the
    // kit dispatches one call at a time, so no payload can be built while
    // one is running — the row's own shape is payload.test.ts's and the
    // argument it shows is parse.test.ts's. What is assertable here is that
    // the running list is empty by the time a call has returned, which is
    // the other half of the old case.
    await $.tool.call(call('solo'))

    for (const payload of seen.payloads()) {
      expect(payload.activity?.tools).toEqual([])
    }
  })

  test('a failed tool call is counted as failed', async ($, on) => {
    const seen = bottom(on, { toolResult: { isError: true, result: 'boom' } })
    await $.tool.call({ tool: 'Bash', tool_use_id: 't2', command: 'false' })

    const payload = seen.lastPayload()
    expect(payload.activity?.completed_counts).toEqual([{ name: 'Bash', count: 1, failed: 1 }])
    expect(payload.activity?.failed_total).toBe(1)
  })

  test('counts add up over many calls, per tool and in total', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call(call('r1'))
    await $.tool.call(call('r2'))
    await $.tool.call(call('r3'))
    await $.tool.call({ tool: 'Bash', tool_use_id: 'b1', command: 'true' })

    const payload = seen.lastPayload()
    expect(payload.activity?.completed_total).toBe(4)
    expect(payload.activity?.completed_counts).toEqual([
      { name: 'Read', count: 3, failed: 0 },
      { name: 'Bash', count: 1, failed: 0 },
    ])
  })
})

describe('TodoWrite', () => {
  test('becomes the todo row', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TodoWrite',
      tool_use_id: 't3',
      todos: [
        { content: 'Write the harness', activeForm: 'Writing the harness', status: 'in_progress' },
        { content: 'Commit it', activeForm: 'Committing it', status: 'pending' },
        { content: 'Read the types', activeForm: 'Reading the types', status: 'completed' },
      ],
    })

    const todo = seen.lastPayload().activity?.todo
    expect(todo?.text).toBe('Writing the harness')
    expect(todo?.total).toBe(3)
    expect(todo?.completed).toBe(1)
    expect(todo?.pending).toBe(1)
    expect(todo?.in_progress_items).toEqual([{ text: 'Writing the harness' }])
    expect(todo?.all_done).toBe(false)
    expect(todo?.is_task_api).toBe(true)
  })

  test('all done is marked as such', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TodoWrite',
      tool_use_id: 't4',
      todos: [
        { content: 'One', activeForm: 'Doing one', status: 'completed' },
        { content: 'Two', activeForm: 'Doing two', status: 'completed' },
      ],
    })

    const todo = seen.lastPayload().activity?.todo
    expect(todo?.all_done).toBe(true)
    expect(todo?.completed).toBe(2)
  })
})

describe('classic events', () => {
  test('effort rides in on a classic hook input', async ($, on) => {
    const seen = bottom(on)
    await $.classic.PostToolUse({
      tool_name: 'Read',
      tool_input: {},
      tool_response: {},
      tool_use_id: 'p1',
      effort: { level: 'xhigh' },
    })
    await $.tool.call(call('after-effort'))

    expect(seen.lastPayload().effort).toEqual({ level: 'xhigh' })
  })

  test('compaction is counted from PreCompact', async ($, on) => {
    const seen = bottom(on)
    await $.classic.PreCompact({ trigger: 'auto', custom_instructions: null })
    await $.classic.PreCompact({ trigger: 'manual', custom_instructions: null })
    await $.tool.call(call('after-compact'))

    expect(seen.lastPayload().activity?.compact_count).toBe(2)
  })

  test('instruction counts come from what the engine loaded, deduped', async ($, on) => {
    const seen = bottom(on)
    const load = (file_path: string, load_reason: 'session_start' | 'compact') =>
      $.classic.InstructionsLoaded({ file_path, memory_type: 'Project', load_reason })

    await load('/work/CLAUDE.md', 'session_start')
    await load('/home/me/.claude/CLAUDE.md', 'session_start')
    await load('/work/.claude/rules/testing.md', 'session_start')
    // The same file twice must not count twice.
    await load('/work/CLAUDE.md', 'compact')
    await $.tool.call(call('after-instructions'))

    const env = seen.lastPayload().env
    expect(env?.claude_md_count).toBe(2)
    expect(env?.rules_count).toBe(1)
    // Nothing seen means the field is left out and the binary's scan stands.
    expect(env?.memory_count).toBeUndefined()
  })

  test('an agent gets a real start and stop time', async ($, on) => {
    const seen = bottom(on, {
      agents: [{ id: 'a1', description: 'Map pain points', type: 'general-purpose', status: 'running' }],
    })
    await $.classic.SubagentStart({ agent_id: 'a1', agent_type: 'Explore' })
    await $.classic.SubagentStop({
      agent_id: 'a1',
      agent_type: 'Explore',
      agent_transcript_path: '/tmp/a1.jsonl',
      stop_hook_active: false,
    })

    const agent = seen.lastPayload().activity?.agents?.[0]
    expect(agent?.id).toBe('a1')
    // The type SubagentStart named wins over the one the list reports.
    expect(agent?.agent_type).toBe('Explore')
    expect(agent?.started_at).toBeGreaterThan(0)
    expect(agent?.completed_at).toBeGreaterThan(agent?.started_at ?? 0)
  })

  test('a settings change refreshes without waiting out the binary cache', async ($, on) => {
    const seen = bottom(on)
    const before = seen.spawns.length
    await $.classic.ConfigChange({ source: 'project_settings', file_path: '/work/.claude/settings.json' })

    expect(seen.spawns.length).toBeGreaterThan(before)
    expect(seen.lastPayload().session_id).toBe('session-under-test')
  })
})

describe('the engine answers for the environment', () => {
  test('the MCP count is the servers serving tools, not the ones configured', async ($, on) => {
    const seen = bottom(on, {
      tools: [
        { name: 'Read', description: 'read a file', mcp: false },
        { name: 'mcp__linear__create_issue', description: '', mcp: true },
        { name: 'mcp__linear__list_issues', description: '', mcp: true },
        { name: 'mcp__slack__send', description: '', mcp: true },
      ],
    })
    await $.tool.call(call('after-tools'))

    // Four MCP tools, two servers.
    expect(seen.lastPayload().env?.mcp_count).toBe(2)
  })

  test('no MCP tools is a count of zero, not a missing field', async ($, on) => {
    const seen = bottom(on, { tools: [{ name: 'Read', description: 'read a file', mcp: false }] })
    await $.tool.call(call('after-none'))

    expect(seen.lastPayload().env?.mcp_count).toBe(0)
  })
})

describe('the spawn itself', () => {
  test('the binary is told the box it draws into', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call(call('r1'))

    const argv = seen.spawns.filter(run => run.argv[1] !== '--version').at(-1)?.argv ?? []
    expect(argv).toContain('--rows')
    expect(argv).toContain('--columns')
  })

  test('the payload is valid JSON carrying the session', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call(call('r1'))

    const payload = seen.lastPayload()
    expect(payload.session_id).toBe('session-under-test')
    expect(payload.model?.display_name).toBe('Opus 5 (1M context)')
    expect(payload.context_window?.context_window_size).toBe(1_000_000)
  })
})
