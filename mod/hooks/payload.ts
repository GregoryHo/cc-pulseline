import type { AgentInfo, SessionUsage } from 'claude-code'

/**
 * The stdin contract between the mod and the cc-pulseline binary.
 *
 * The binary reads the same JSON Claude Code's own statusline hook writes,
 * so this file is the one place that knows that schema: everything else in
 * the mod collects facts, and `buildPayload` is the only thing that names a
 * payload field. It touches no `$` and mutates nothing — hand it the same
 * snapshot twice and it returns the same bytes.
 */

/** Turn token counts, kept from the last `turn.complete`. */
export type Tokens = {
  input_tokens: number
  output_tokens: number
  cache_read_input_tokens: number
  cache_creation_input_tokens: number
}

/** One running tool call, keyed by its `tool_use_id`. */
export type ActiveTool = { id: string; name: string; target?: string }

/** Session totals per tool name, as the completed-counts row shows them. */
export type ToolCount = { count: number; failed: number }

/** The todo list as the last TodoWrite call left it. */
export type Todo = {
  text: string
  pending: number
  completed: number
  total: number
  in_progress_items: { text: string }[]
  all_done: boolean
  is_task_api: boolean
}

/**
 * Everything one payload is built from: what the engine answered for this
 * refresh, and what the hooks have accumulated since the session began.
 *
 * The maps are read-only here on purpose. An agent with no `startedAt`
 * record must be stamped *before* this is built, or its start time would be
 * the moment of each draw rather than the moment it was first seen; the
 * caller owns that stamping, which is why this stays pure.
 */
export type PayloadSnapshot = {
  /** What the engine answered this refresh. */
  sessionId: string
  modelId: string
  usage: SessionUsage
  agents: readonly AgentInfo[]
  now: number

  /** Session facts the hooks resolved once. */
  cwd: string
  version: string | null
  effort: string | null
  tokens: Tokens | null

  /**
   * Counts the engine answered for. A zero means "nothing seen", and the
   * field is then left out so the binary's own filesystem scan stands.
   */
  mcpCount: number
  claudeMdCount: number
  rulesCount: number
  memoryCount: number

  /** Activity the hooks accumulated. */
  active: ReadonlyMap<string, ActiveTool>
  counts: ReadonlyMap<string, ToolCount>
  completedTotal: number
  failedTotal: number
  startedAt: ReadonlyMap<string, number>
  finishedAt: ReadonlyMap<string, number>
  agentType: ReadonlyMap<string, string>
  compactCount: number
  todo: Todo | null
}

/**
 * Turns a model id into the name the statusline shows.
 *
 * The engine hands out ids (`claude-opus-5[1m]`); the statusline payload
 * that Claude Code itself writes carries a display name, and there is no
 * call for that one, so this rebuilds it.
 */
export function modelName(id: string): string {
  const isMillion = id.includes('[1m]')
  const base = id
    .replace('[1m]', '')
    .replace(/^claude-/, '')
    .replace(/-\d{8}$/, '')
  const words = base
    .split('-')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
  return isMillion ? `${words} (1M context)` : words
}

/** The JSON the binary reads on stdin. */
export function buildPayload(snapshot: PayloadSnapshot): string {
  const { usage, now } = snapshot

  const limits: Record<string, { used_percentage: number; resets_at: number }> = {}
  for (const limit of usage.rateLimits) {
    limits[limit.kind] = {
      used_percentage: limit.percentUsed,
      resets_at: limit.resetsAt ? Math.floor(Date.parse(limit.resetsAt) / 1000) : 0,
    }
  }

  return JSON.stringify({
    session_id: snapshot.sessionId,
    model: { display_name: modelName(snapshot.modelId) },
    version: snapshot.version ?? undefined,
    effort: snapshot.effort ? { level: snapshot.effort } : undefined,
    cwd: snapshot.cwd,
    workspace: { current_dir: snapshot.cwd },
    context_window: {
      context_window_size: usage.context.window,
      used_percentage: usage.context.percent,
      current_usage: snapshot.tokens ?? undefined,
    },
    cost: usage.cost ? { total_cost_usd: usage.cost.usd } : undefined,
    rate_limits: limits,
    // Only the counts the engine answered for; the rest stay as the
    // binary's own filesystem scan found them.
    env: {
      mcp_count: snapshot.mcpCount,
      claude_md_count: snapshot.claudeMdCount > 0 ? snapshot.claudeMdCount : undefined,
      rules_count: snapshot.rulesCount > 0 ? snapshot.rulesCount : undefined,
      memory_count: snapshot.memoryCount > 0 ? snapshot.memoryCount : undefined,
    },
    activity: {
      tools: [...snapshot.active.values()].map(tool => ({
        id: tool.id,
        name: tool.name,
        target: tool.target ?? null,
      })),
      completed_counts: [...snapshot.counts.entries()].map(([name, count]) => ({
        name,
        count: count.count,
        failed: count.failed,
      })),
      completed_total: snapshot.completedTotal,
      failed_total: snapshot.failedTotal,
      agents: snapshot.agents.map(agent => ({
        id: agent.id,
        description: agent.description,
        agent_type: snapshot.agentType.get(agent.id) ?? agent.type,
        started_at: snapshot.startedAt.get(agent.id) ?? now,
        // A real stop timestamp when SubagentStop gave one; otherwise the
        // status is all there is to go on.
        completed_at:
          snapshot.finishedAt.get(agent.id) ?? (agent.status === 'running' ? null : now),
      })),
      compact_count: snapshot.compactCount,
      todo: snapshot.todo,
    },
  })
}
