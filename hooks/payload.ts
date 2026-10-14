import type { AgentInfo, SessionUsage } from 'claude-code'

import type { Todo, Tokens, ToolCount } from '../types'

/**
 * The stdin contract between the mod and the cc-pulseline binary.
 *
 * The binary reads the same JSON Claude Code's own statusline hook writes,
 * so this file is the one place that knows that schema: everything else in
 * the mod collects facts, and `buildPayload` is the only thing that names a
 * payload field. It touches no `$` and mutates nothing — hand it the same
 * snapshot twice and it returns the same bytes.
 */

/**
 * One running tool call, keyed by its `tool_use_id`.
 *
 * Not in the `$.state` contract: running tools live in the module's closure,
 * since nothing is mid-flight at the moment a reload happens.
 */
export type ActiveTool = { id: string; name: string; target?: string }

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

  /**
   * Session facts the hooks resolved once.
   *
   * `outputStyle` and `lastApiErrorMs` may arrive `undefined` rather than
   * `null` from a session whose `session` atom the host wrote before these
   * two existed, so both are read defensively below.
   */
  cwd: string
  version: string | null
  effort: string | null
  outputStyle: string | null
  lastApiErrorMs: number | null
  cacheReadTotal: number
  tokens: Tokens | null

  /**
   * Counts the engine answered for. A zero means "nothing seen", and the
   * field is then left out so the binary's own filesystem scan stands.
   */
  mcpCount: number
  claudeMdCount: number
  rulesCount: number
  memoryCount: number

  /**
   * Activity the hooks accumulated.
   *
   * Plain records rather than `Map`s: every one of these but `active` is an
   * atom value now, and an atom value crosses to the host as JSON. Key order
   * is insertion order either way, so the rows come out as they did — tool
   * names are never integer-like, which is the one case where an object
   * would reorder them.
   */
  active: ReadonlyMap<string, ActiveTool>
  counts: Readonly<Record<string, ToolCount>>
  completedTotal: number
  failedTotal: number
  startedAt: Readonly<Record<string, number>>
  finishedAt: Readonly<Record<string, number>>
  agentType: Readonly<Record<string, string>>
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

  const limits: Record<string, { used_percentage: number; resets_at?: number }> = {}
  for (const limit of usage.rateLimits) {
    // Left out when the engine named no reset, and when it named one that
    // does not parse. `resets_at: 0` is not "unknown" to the binary — it is
    // the epoch, and the quota cell then reads `resets <1m` on every refresh
    // for the rest of the session. An absent key is `None` there, which draws
    // the percentage and no reset at all.
    const resetsAt = limit.resetsAt === undefined ? NaN : Date.parse(limit.resetsAt) / 1000
    limits[limit.kind] = {
      used_percentage: limit.percentUsed,
      ...(Number.isFinite(resetsAt) ? { resets_at: Math.floor(resetsAt) } : {}),
    }
  }

  return JSON.stringify({
    session_id: snapshot.sessionId,
    model: { display_name: modelName(snapshot.modelId) },
    output_style: snapshot.outputStyle ? { name: snapshot.outputStyle } : undefined,
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
      completed_counts: Object.entries(snapshot.counts).map(([name, count]) => ({
        name,
        count: count.count,
        failed: count.failed,
      })),
      completed_total: snapshot.completedTotal,
      failed_total: snapshot.failedTotal,
      agents: snapshot.agents.map(agent => ({
        id: agent.id,
        description: agent.description,
        agent_type: snapshot.agentType[agent.id] ?? agent.type,
        started_at: snapshot.startedAt[agent.id] ?? now,
        // A stamp from `SubagentStop`, or the one `collect` wrote the first
        // time the engine stopped listing the agent as running. The `now`
        // below is only reached by a snapshot built without that write — a
        // live refresh always has the stamp, which is what keeps a completed
        // agent's elapsed time still.
        completed_at:
          snapshot.finishedAt[agent.id] ?? (agent.status === 'running' ? null : now),
      })),
      compact_count: snapshot.compactCount,
      last_api_error_ms: snapshot.lastApiErrorMs ?? undefined,
      // Already totalled, because only this side can total it. Left out
      // while nothing has been read, so the binary keeps its own figure
      // rather than being told the session read zero.
      cache_read_total: snapshot.cacheReadTotal > 0 ? snapshot.cacheReadTotal : undefined,
      todo: snapshot.todo,
    },
  })
}
