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

  test('a running tool reaches the band while it runs, and is gone once it returns', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call(call('solo'))

    // This case used to assert the opposite — that no payload ever carried a
    // running tool — which was true and was the bug: the rows the band draws
    // are the binary's stdout, so the entry in `state.active` only reaches it
    // through a spawn made while the call is still in flight. One refresh on
    // the way in is what puts it there.
    const whileRunning = seen.payloads().filter(p => (p.activity?.tools?.length ?? 0) > 0)
    expect(whileRunning.length).toBe(1)
    expect(whileRunning[0]?.activity?.tools).toEqual([
      { id: 'solo', name: 'Read', target: '/n/solo' },
    ])

    // And the other half of the old case: nothing lingers once it returns.
    expect(seen.lastPayload().activity?.tools).toEqual([])
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

describe('what counts as tool activity', () => {
  test('the todo row\'s own calls and Agent are not tools', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TodoWrite',
      tool_use_id: 'b1',
      todos: [{ content: 'One', activeForm: 'Doing one', status: 'pending' }],
    })
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'b2',
      subject: 'One',
      description: 'The first',
    })
    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'b3', taskId: 'task-1', status: 'completed' })
    await $.tool.call({ tool: 'Agent', tool_use_id: 'b4', description: 'Look around', prompt: 'go' })
    await $.tool.call({ tool: 'ToolSearch', tool_use_id: 'b5', query: 'select:Read', max_results: 1 })
    await $.tool.call(call('real'))

    // The transcript path dispatches each of these somewhere else and returns
    // before its tool tracking, so the band's totals have to do the same or
    // they count the mod's own bookkeeping.
    const payload = seen.lastPayload()
    expect(payload.activity?.completed_counts).toEqual([{ name: 'Read', count: 1, failed: 0 }])
    expect(payload.activity?.completed_total).toBe(1)
    // And an Agent call is never also drawn as a running tool.
    for (const seenPayload of seen.payloads()) {
      for (const tool of seenPayload.activity?.tools ?? []) {
        expect(tool.name).toBe('Read')
      }
    }
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
    // Two unfinished: the pending one and the running one. This used to
    // assert 1 — only the `pending` status — which made the ledger's
    // `done = completed.max(total - pending)` read the running item as done.
    expect(todo?.pending).toBe(2)
    expect(todo?.in_progress_items).toEqual([{ text: 'Writing the harness' }])
    expect(todo?.all_done).toBe(false)
    expect(todo?.is_task_api).toBe(true)
  })

  test('a refused TodoWrite is not drawn as the current list', async ($, on) => {
    const seen = bottom(on, { toolResult: { isError: true, result: 'refused' } })
    await $.tool.call({
      tool: 'TodoWrite',
      tool_use_id: 't6',
      todos: [{ content: 'Never happened', activeForm: 'Never happening', status: 'in_progress' }],
    })

    expect(seen.lastPayload().activity?.todo).toBeNull()
  })

  test('a lone running todo is not reported as done', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TodoWrite',
      tool_use_id: 't5',
      todos: [{ content: 'Only', activeForm: 'Doing only', status: 'in_progress' }],
    })

    const todo = seen.lastPayload().activity?.todo
    // The ledger reads `done = completed.max(total - pending)`, so a running
    // item left out of `pending` drew `1/1 done · 0 pending`.
    expect(todo?.completed).toBe(0)
    expect(todo?.pending).toBe(1)
    expect(todo?.all_done).toBe(false)
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

describe('the Task API', () => {
  test('TaskCreate opens a pending item that reaches the payload', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k1',
      subject: 'Wire the ladder',
      description: 'Five rungs, probed by --version',
    })
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k2',
      subject: 'Write its tests',
      description: 'One per rung',
    })

    const todo = seen.lastPayload().activity?.todo
    expect(todo?.total).toBe(2)
    expect(todo?.pending).toBe(2)
    expect(todo?.completed).toBe(0)
    expect(todo?.in_progress_items).toEqual([])
    expect(todo?.is_task_api).toBe(true)
  })

  test('TaskUpdate names the running task by its active form', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k1',
      subject: 'Wire the ladder',
      description: 'Five rungs, probed by --version',
      activeForm: 'Wiring the ladder',
    })
    // The id comes from TaskCreate's own result, which is what a later
    // TaskUpdate names. The bottom hands out `task-1`, `task-2`, … in order.
    await $.tool.call({
      tool: 'TaskUpdate',
      tool_use_id: 'k2',
      taskId: 'task-1',
      status: 'in_progress',
    })

    const todo = seen.lastPayload().activity?.todo
    expect(todo?.in_progress_items?.[0]?.text).toBe('Wiring the ladder')
    expect(typeof todo?.in_progress_items?.[0]?.started_at).toBe('number')
    // Everything not completed is pending, the running item included — the
    // transcript path's arithmetic, mirrored.
    expect(todo?.pending).toBe(1)
  })

  test('a completed task is counted, and the last one marks the row done', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k1',
      subject: 'One',
      description: 'The first',
    })
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k2',
      subject: 'Two',
      description: 'The second',
    })
    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'k3', taskId: 'task-1', status: 'completed' })

    let todo = seen.lastPayload().activity?.todo
    expect(todo?.completed).toBe(1)
    expect(todo?.all_done).toBe(false)

    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'k4', taskId: 'task-2', status: 'completed' })
    todo = seen.lastPayload().activity?.todo
    expect(todo?.completed).toBe(2)
    expect(todo?.all_done).toBe(true)
  })

  test('a refused TaskCreate leaves no task behind', async ($, on) => {
    const seen = bottom(on, { toolResult: { isError: true, result: 'refused' } })
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k1',
      subject: 'One',
      description: 'The first',
    })

    // Written before the call was answered, this would be a phantom item —
    // and, when the id was a local count rather than the engine's, every
    // later update would address the phantom instead of the real task.
    expect(seen.lastPayload().activity?.todo).toBeNull()
  })

  test('a TaskUpdate carrying no status leaves the status alone', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k1',
      subject: 'One',
      description: 'The first',
    })
    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'k2', taskId: 'task-1', status: 'completed' })
    // `status` is optional: this call only renames the task.
    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'k3', taskId: 'task-1', subject: 'One, renamed' })

    const todo = seen.lastPayload().activity?.todo
    expect(todo?.completed).toBe(1)
    expect(todo?.all_done).toBe(true)
  })

  test('a deleted task leaves the row', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k1',
      subject: 'One',
      description: 'The first',
    })
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k2',
      subject: 'Two',
      description: 'The second',
    })
    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'k3', taskId: 'task-2', status: 'deleted' })

    const todo = seen.lastPayload().activity?.todo
    expect(todo?.total).toBe(1)
  })

  test('the start is stamped once, so the elapsed time does not restart', async ($, on) => {
    const seen = bottom(on)
    await $.tool.call({
      tool: 'TaskCreate',
      tool_use_id: 'k1',
      subject: 'One',
      description: 'The only one',
    })
    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'k2', taskId: 'task-1', status: 'in_progress' })
    const first = seen.lastPayload().activity?.todo?.in_progress_items?.[0]?.started_at
    // Asserted on its own, or the comparison below would pass on two
    // undefineds and say nothing about the stamp.
    expect(typeof first).toBe('number')

    // The clock has moved between the two, so a re-stamp would show.
    await $.tool.call({ tool: 'TaskUpdate', tool_use_id: 'k3', taskId: 'task-1', status: 'in_progress' })
    const again = seen.lastPayload().activity?.todo?.in_progress_items?.[0]?.started_at

    expect(again).toBe(first)
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

  test('an AGENTS.md is counted in its own column, not as a CLAUDE.md', async ($, on) => {
    const seen = bottom(on)
    const load = (file_path: string) =>
      $.classic.InstructionsLoaded({ file_path, memory_type: 'Project', load_reason: 'session_start' })
    await load('/work/CLAUDE.md')
    await load('/work/AGENTS.md')
    await $.tool.call(call('one'))

    const env = seen.lastPayload().env
    // The binary scans and draws the two separately (`env.rs`'s
    // `count_agents_md` has its own ENV column), so folding AGENTS.md into
    // the CLAUDE.md count showed one file twice.
    expect(env?.claude_md_count).toBe(1)
    expect(env?.agents_md_count).toBe(1)
  })

  test('a completed agent keeps the stop time it was first given', async ($, on) => {
    // No SubagentStop for this one — the engine just stops calling it running,
    // which is the case where the time has to be inferred and then held.
    const seen = bottom(on, {
      agents: [
        { id: 'a1', description: 'Map pain points', type: 'general-purpose', status: 'completed' },
      ],
    })
    await $.tool.call(call('one'))
    seen.advance(5_000)
    await $.tool.call(call('two'))

    const stamps = seen
      .payloads()
      .map(payload => payload.activity?.agents?.[0]?.completed_at)
      .filter((stamp): stamp is number => typeof stamp === 'number')

    // More than one draw, or the case proves nothing about holding a value.
    expect(stamps.length).toBeGreaterThan(1)
    // The renderer shows `completed_at - started_at`, so a second answer here
    // is a completed agent whose age keeps growing.
    expect(new Set(stamps).size).toBe(1)
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

  test('the cache-read total is summed over turns, not replaced', async ($, on) => {
    const seen = bottom(on)
    const turn = (cacheRead: number) =>
      $.turn.complete({
        reason: 'answer',
        answer: '',
        durationMs: 1,
        isAborted: false,
        turnId: `t-${cacheRead}`,
        usage: {
          input_tokens: 10,
          output_tokens: 20,
          cache_read_input_tokens: cacheRead,
          cache_creation_input_tokens: 5,
          model: 'claude-opus-5[1m]',
        },
      })

    await turn(1_000_000)
    await turn(3_400_000)

    // The whole bug class in one assertion: the binary cannot total this
    // itself, because every refresh re-sends the same last-turn usage, so a
    // payload carrying 3_400_000 would be the last value rather than the sum.
    expect(seen.lastPayload().activity?.cache_read_total).toBe(4_400_000)
  })

  test('a turn with no usage leaves the cache-read total alone', async ($, on) => {
    const seen = bottom(on)
    await $.turn.complete({
      reason: 'aborted',
      answer: '',
      durationMs: 1,
      isAborted: true,
      turnId: 't-none',
    })

    // Nothing read, so the field is left out and the binary keeps its own
    // figure rather than being told the session read zero.
    expect(seen.lastPayload().activity?.cache_read_total).toBeUndefined()
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
