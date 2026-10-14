/**
 * The refresh gate, as a function of three numbers.
 *
 * Pulled out for the same reason `buildPayload` was: the decision is
 * arithmetic, and arithmetic can be tested without a clock, a spawn or a
 * mounted drawing. `maybeRefresh` keeps the part that is genuinely about
 * timing — reading the clock and claiming the window — and asks this.
 *
 * Several sites can trigger a refresh. The gate guards the ones that fire
 * without a tool call of their own to account for — a run of
 * `classic.ConfigChange` writes, say — so a burst of them does not spawn the
 * binary once each.
 *
 * It does not guard the tool-call path any more, at either end. A gated start
 * is not drawn late but never drawn at all, since the window is closed by the
 * previous call's completion and the next refresh is this call's own, after
 * its entry is gone; and a gated completion leaves a finished tool drawn as
 * running. Both ends of a tool call therefore refresh unconditionally and pay
 * a spawn each, against a floor under 3ms.
 */
export function shouldRefresh(lastRunMs: number, nowMs: number, gapMs: number): boolean {
  // `>=`, so a gap of 0 always refreshes — a test that wants every call to
  // go through asks for 0 rather than having to move a clock.
  return nowMs - lastRunMs >= gapMs
}
