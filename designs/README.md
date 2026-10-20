# cc-pulseline design docs

Living design documents. Each file is either the architectural reference for
a shipped subsystem or an iteration brief with open items. When a design is
fully absorbed into code with no open follow-ups, it is deleted — the code
is then the source of truth.

## Current docs

None at the moment. The activity width-budget spec was absorbed into
[`docs/architecture.md`](../docs/architecture.md) → Activity Width Budget.

## When to add / delete a doc

- **Add** a doc when the work spans more than one PR or has variations to
  weigh against each other. The doc lives until its open items close.
- **Delete** a doc when every Must / Should item ships and the residual
  Could / Open items are no longer interesting. Don't keep "historical
  record" docs — `git log designs/` covers retired briefs, and any
  rationale worth preserving belongs in the user-facing docs (e.g.
  `docs/layouts.md` carries the Variation B rationale from the now-
  deleted composability redesign brief, and the "added visual elements
  must add information or rhythm" principle from the now-deleted
  console-redesign brief).
