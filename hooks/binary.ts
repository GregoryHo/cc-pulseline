/**
 * Where the cc-pulseline binary is, and whether it is new enough.
 *
 * Everything here is pure: the ladder is a list of paths built from four
 * strings, and the version gate is string parsing. The probing itself lives
 * in register.tsx, because a function handed `$` has to be declared in the
 * module the loader scans for `$` calls — the host refuses a call its scan
 * did not list.
 *
 * No binary is bundled and none is downloaded. `$.fs.write` is text-only,
 * `FsStat` carries no mode bit and there is no chmod, so a downloaded file
 * could never be made executable; and the npm package already ships seven
 * platforms with musl detection, so there is nothing here to rebuild.
 */

/**
 * The oldest binary whose CLI this mod can drive.
 *
 * A rung that answers but is older is passed over, not accepted and not
 * fatal: the walk goes on and a later rung may be new enough. That is the
 * real shape of the machine this was written on — `cargo install` left an
 * older copy on PATH while the dev build was current — and a ladder that
 * stopped at the first thing to answer would have taken the old one. The
 * too-old message is only reached when no rung at all is new enough.
 *
 * It tracks the release, because each release so far has added the CLI the
 * band of that release drives. 1.6.0 added `--layout activity`, which the
 * band now always asks for, and an unknown layout name is not an error to
 * the binary: it warns on stderr, falls back to `none` and exits 0. So a
 * 1.5.x binary would answer a band spawn with the identity and budget rows
 * — the statusline, drawn a second time above the prompt and now resident —
 * and `bandRun` would read `'ok'`. Refusing it outright is the only way that
 * failure gets said out loud.
 *
 * 1.7.0 added `--no-icons`, which every spawn for the desktop app carries.
 * An older binary ignores an unknown flag the same way, so on the desktop it
 * would draw Nerd Font glyphs the app's face does not have — a row of boxes,
 * recorded as `'ok'`.
 */
export const MIN_VERSION: Version = [1, 7, 0]

/** How long one probe may take. Five rungs, so this bounds the whole ladder. */
export const PROBE_TIMEOUT_MS = 400

export type Version = [number, number, number]

/** Which rung a candidate came from, for the note and for the tests. */
export type Rung = 'configured' | 'bundled' | 'installed' | 'path' | 'dev'

export type Candidate = { path: string; rung: Rung }

export type Where = {
  /** The `binaryPath` userConfig field; empty when the person left it unset. */
  configured?: string | undefined
  /** `$.plugin.root`: the folder holding plugin.json. */
  pluginRoot: string
  /** The person's home directory, for the installed path. */
  home: string
  /** The session's working directory, for the dev rung. */
  cwd: string
}

export const BINARY_NAME = 'cc-pulseline'

/**
 * The ladder, in the order it is tried.
 *
 * Each rung is only a path; whether anything is there is settled by running
 * it, never by `$.fs.stat`. That is not caution — on a machine that has used
 * the statusline, `~/.claude/pulseline/` exists and holds `config.toml` and
 * no binary, and a stat of the *directory* answers yes. Running the
 * candidate is the only thing that tells a binary from a folder, and it
 * doubles as the version check.
 */
export function candidates(where: Where): Candidate[] {
  const found: Candidate[] = []
  const add = (path: string, rung: Rung) => {
    if (path.length > 0 && !found.some(one => one.path === path)) found.push({ path, rung })
  }

  // 1. What the person named, which always wins.
  add((where.configured ?? '').trim(), 'configured')

  // 2. Shipped beside the mod. Reserved for a bundling decision that has not
  //    been taken, so this is expected to miss. No engine call answers the
  //    platform, so there is no `<platform>` segment to build: whoever
  //    bundles picks the layout and this rung follows it.
  if (where.pluginRoot.length > 0) add(`${where.pluginRoot}/bin/${BINARY_NAME}`, 'bundled')

  // 3. Where scripts/install.sh puts it on its download path.
  if (where.home.length > 0) add(`${where.home}/.claude/pulseline/${BINARY_NAME}`, 'installed')

  // 4. On PATH — the npm package, or `cargo install` landing in ~/.cargo/bin,
  //    which is what install.sh does when it is run inside the repo.
  add(BINARY_NAME, 'path')

  // 5. The dev rung, and labelled as such. It cannot be dropped: this
  //    repository's own session draws from its own build, and install.sh run
  //    inside the repo never writes to the installed path. It is the session's
  //    working directory rather than anyone's absolute path, so it works in
  //    any checkout.
  if (where.cwd.length > 0) add(`${where.cwd}/target/release/${BINARY_NAME}`, 'dev')

  return found
}

/** Reads `cc-pulseline 1.5.0` — or any line carrying a three-part version. */
export function parseVersion(stdout: string): Version | null {
  const found = stdout.match(/(\d+)\.(\d+)\.(\d+)/)
  if (found === null) return null
  const [major, minor, patch] = [Number(found[1]), Number(found[2]), Number(found[3])]
  if (!Number.isFinite(major) || !Number.isFinite(minor) || !Number.isFinite(patch)) return null
  return [major, minor, patch]
}

export function formatVersion(version: Version): string {
  return version.join('.')
}

/** True when `version` is at least `MIN_VERSION`. */
export function isNewEnough(version: Version, minimum: Version = MIN_VERSION): boolean {
  for (let at = 0; at < 3; at += 1) {
    const have = version[at] ?? 0
    const need = minimum[at] ?? 0
    if (have !== need) return have > need
  }
  return true
}

const INSTALL = 'npm install -g @cc-pulseline/cc-pulseline'

/** What the band says when no rung answered. */
export function missingMessage(): string {
  return `cc-pulseline not found — install it with: ${INSTALL}`
}

/**
 * What the band says when a binary answered but is too old.
 *
 * It names the statusline on purpose. Every version below the floor draws
 * the statusline perfectly well; what they lack is a band flag — `--rows`
 * and `--columns` before 1.5.0, `--layout activity` before 1.6.0, `--no-icons` before 1.7.0 — and none
 * of them refuses. They draw the classic layout into the band at the wrong
 * width, row count or both. Someone reading "too old" would otherwise go
 * looking for a broken statusline that is not broken.
 */
export function tooOldMessage(version: string): string {
  return (
    `cc-pulseline ${version} is too old — the band needs ${formatVersion(MIN_VERSION)} ` +
    `or newer; your statusline is unaffected. Update with: ${INSTALL}`
  )
}
