/**
 * The refresh gate, as a function of three numbers.
 *
 * Pulled out for the same reason `buildPayload` was: the decision is
 * arithmetic, and arithmetic can be tested without a clock, a spawn or a
 * mounted drawing. `maybeRefresh` keeps the part that is genuinely about
 * timing — reading the clock and claiming the window — and asks this.
 *
 * Seven sites can trigger a refresh and the mod sits in the tool-call
 * chain, so a spawn on the way in delays the tool itself; this bounds how
 * often that happens while a burst goes through.
 *
 * It bounds the way in only. A tool call's completion refreshes
 * unconditionally, because a gated completion leaves the band drawing a tool
 * that has already finished.
 */
export function shouldRefresh(lastRunMs: number, nowMs: number, gapMs: number): boolean {
  // `>=`, so a gap of 0 always refreshes — a test that wants every call to
  // go through asks for 0 rather than having to move a clock.
  return nowMs - lastRunMs >= gapMs
}
