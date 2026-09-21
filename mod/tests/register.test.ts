/**
 * Runs the mod's hooks against a fake engine, with no Claude Code session.
 *
 *     node --experimental-strip-types --test mod/tests/register.test.ts
 *
 * `claude plugin test` is named in the binary but is not a command yet on
 * 2.1.278, so the hooks are driven here instead: every assertion is about
 * what the mod would hand the binary, read off the recorded `process.run`.
 */

import assert from 'node:assert/strict'
import { test } from 'node:test'

import { register } from '../hooks/register.ts'

type AnyHook = (engine: unknown, event: unknown, next: unknown) => unknown

type Recorded = { argv: readonly string[]; stdin: string }

/** Registers the mod's hooks and gives a fake `$` that records the runs. */
function harness() {
  const hooks: { event: string; matcher: Record<string, unknown>; hook: AnyHook }[] = []
  const runs: Recorded[] = []
  let nowMs = 1_700_000_000_000

  const on = (event: string, a: unknown, b?: unknown) => {
    const matcher = (b === undefined ? {} : a) as Record<string, unknown>
    hooks.push({ event, matcher, hook: (b ?? a) as AnyHook })
    return { catch: () => undefined }
  }

  const engine = {
    session: {
      usage: async () => ({
        context: { window: 1_000_000, percent: 9, tokens: 90_000 },
        rateLimits: [
          { kind: 'five_hour', percentUsed: 33, resetsAt: '2026-09-21T12:00:00Z' },
        ],
        cost: { usd: 1.53 },
      }),
      model: async () => 'claude-opus-5[1m]',
      id: async () => 'session-under-test',
    },
    tool: {
      list: async () => [
        { name: 'Read', description: 'read a file', mcp: false },
        { name: 'mcp__linear__create_issue', description: '', mcp: true },
        { name: 'mcp__linear__list_issues', description: '', mcp: true },
        { name: 'mcp__slack__send', description: '', mcp: true },
      ],
    },
    agent: {
      list: async () => [
        { id: 'a1', description: 'Map pain points', type: 'Explore', status: 'running' },
      ],
    },
    clock: { now: async () => (nowMs += 1_000) },
    env: { get: async () => undefined },
    command: { register: async () => ({}) },
    process: {
      run: async (argv: readonly string[], init?: { stdin?: string }) => {
        runs.push({ argv, stdin: init?.stdin ?? '' })
        // The version probe asks the CLI; everything else is the statusline.
        if (argv[1] === '--version') {
          return { exitCode: 0, stdout: '2.1.278 (Claude Code)', stderr: '' }
        }
        return { exitCode: 0, stdout: 'row one\nrow two', stderr: '' }
      },
    },
    ui: {
      status: () => undefined,
      log: () => undefined,
      invalidate: () => undefined,
      open: async () => undefined,
      close: async () => undefined,
      resolve: () => ({
        Box: (props: unknown) => ({ kind: 'Box', props }),
        Text: (props: unknown) => ({ kind: 'Text', props }),
        Button: (props: unknown) => ({ kind: 'Button', props }),
      }),
    },
  }

  register(on as never)

  const fire = async (event: string, argument: unknown, result: unknown = {}) => {
    const fields = argument as Record<string, unknown>
    const found = hooks.filter(
      entry =>
        entry.event === event &&
        Object.entries(entry.matcher).every(([key, value]) => fields[key] === value),
    )
    assert.ok(found.length > 0, `no hook on ${event}`)
    let last: unknown
    for (const entry of found) {
      last = await entry.hook(engine, argument, async () => result)
    }
    return last
  }

  /** The payload of the most recent statusline run, parsed. */
  const lastPayload = () => {
    const statusline = runs.filter(run => run.argv[1] !== '--version')
    const last = statusline[statusline.length - 1]
    assert.ok(last, 'the binary was never run')
    return JSON.parse(last.stdin)
  }

  return { engine, fire, runs, lastPayload }
}

test('session start builds a payload the binary accepts', async () => {
  const { fire, lastPayload, runs } = harness()
  await fire('session.start', { cwd: '/work' })

  const payload = lastPayload()
  assert.equal(payload.session_id, 'session-under-test')
  assert.equal(payload.model.display_name, 'Opus 5 (1M context)')
  assert.equal(payload.version, '2.1.278')
  assert.equal(payload.cwd, '/work')
  assert.equal(payload.context_window.context_window_size, 1_000_000)
  assert.equal(payload.rate_limits.five_hour.used_percentage, 33)
  assert.ok(payload.rate_limits.five_hour.resets_at > 0, 'reset time is epoch seconds')
  assert.equal(payload.cost.total_cost_usd, 1.53)
  assert.ok(
    runs.some(run => run.argv.includes('--rows')),
    'the binary is told the box it draws into',
  )
})

test('a tool call is running, then counted', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })

  const call = { tool: 'Read', tool_use_id: 't1', file_path: '/work/src/main.rs' }
  await fire('tool.call', call, { isError: false })

  const payload = lastPayload()
  assert.deepEqual(payload.activity.tools, [], 'the tool is no longer running')
  assert.deepEqual(payload.activity.completed_counts, [{ name: 'Read', count: 1, failed: 0 }])
  assert.equal(payload.activity.completed_total, 1)
  assert.equal(payload.activity.failed_total, 0)
})

test('a failed tool call is counted as failed', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })
  await fire('tool.call', { tool: 'Bash', tool_use_id: 't2', command: 'false' }, { isError: true })

  const payload = lastPayload()
  assert.deepEqual(payload.activity.completed_counts, [{ name: 'Bash', count: 1, failed: 1 }])
  assert.equal(payload.activity.failed_total, 1)
})

test('TodoWrite becomes the todo row', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })
  await fire(
    'tool.call',
    {
      tool: 'TodoWrite',
      tool_use_id: 't3',
      todos: [
        { content: 'Write the harness', activeForm: 'Writing the harness', status: 'in_progress' },
        { content: 'Commit it', status: 'pending' },
        { content: 'Read the types', status: 'completed' },
      ],
    },
    { isError: false },
  )

  const todo = lastPayload().activity.todo
  assert.equal(todo.text, 'Writing the harness')
  assert.equal(todo.total, 3)
  assert.equal(todo.completed, 1)
  assert.equal(todo.pending, 1)
  assert.deepEqual(todo.in_progress_items, [{ text: 'Writing the harness' }])
  assert.equal(todo.all_done, false)
})

test('effort rides in on a classic hook input', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })
  await fire('classic.PostToolUse', { effort: { level: 'xhigh' } })
  await fire('tool.call', { tool: 'Read', tool_use_id: 't4' }, { isError: false })

  assert.deepEqual(lastPayload().effort, { level: 'xhigh' })
})

test('compaction is counted from PreCompact', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })
  await fire('classic.PreCompact', { trigger: 'auto' })
  await fire('classic.PreCompact', { trigger: 'manual' })
  await fire('tool.call', { tool: 'Read', tool_use_id: 't5' }, { isError: false })

  assert.equal(lastPayload().activity.compact_count, 2)
})

test('an agent gets a real start and stop time', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })
  await fire('classic.SubagentStart', { agent_id: 'a1', agent_type: 'Explore' })
  await fire('classic.SubagentStop', { agent_id: 'a1', agent_type: 'Explore' })

  const agent = lastPayload().activity.agents[0]
  assert.equal(agent.id, 'a1')
  assert.equal(agent.agent_type, 'Explore')
  assert.ok(agent.started_at > 0, 'start is stamped')
  assert.ok(agent.completed_at > agent.started_at, 'stop is later than start')
})

test('the band draws styled spans, never an escape sequence', async () => {
  const { fire } = harness()
  await fire('session.start', { cwd: '/work' })

  const tree = (await fire('ui.render', {
    surface: 'terminal',
    component: 'AbovePrompt',
    requestId: 'above-prompt',
    props: { hasSurvey: false, isWorking: false, maxRows: 2, bodyColumns: 120 },
  })) as { kind: string; props: { children: unknown[] } }

  assert.equal(tree.kind, 'Box')
  const text = JSON.stringify(tree)
  assert.ok(!text.includes('\\u001b'), 'no escape sequence reaches a Text child')
  assert.ok(text.includes('row one'), 'the binary output is drawn')
})

test('the MCP count is the servers serving tools, not the ones configured', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })

  // Four MCP tools, two servers.
  assert.equal(lastPayload().env.mcp_count, 2)
})

test('instruction counts come from what the engine loaded', async () => {
  const { fire, lastPayload } = harness()
  await fire('session.start', { cwd: '/work' })

  await fire('classic.InstructionsLoaded', {
    file_path: '/work/CLAUDE.md',
    memory_type: 'Project',
    load_reason: 'session_start',
  })
  await fire('classic.InstructionsLoaded', {
    file_path: '/home/me/.claude/CLAUDE.md',
    memory_type: 'User',
    load_reason: 'session_start',
  })
  await fire('classic.InstructionsLoaded', {
    file_path: '/work/.claude/rules/testing.md',
    memory_type: 'Project',
    load_reason: 'session_start',
  })
  // The same file twice must not count twice.
  await fire('classic.InstructionsLoaded', {
    file_path: '/work/CLAUDE.md',
    memory_type: 'Project',
    load_reason: 'compact',
  })
  await fire('tool.call', { tool: 'Read', tool_use_id: 'e1' }, { isError: false })

  const env = lastPayload().env
  assert.equal(env.claude_md_count, 2)
  assert.equal(env.rules_count, 1)
  assert.equal(env.memory_count, undefined, 'a count with nothing seen stays with the scan')
})
