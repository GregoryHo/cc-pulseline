#!/usr/bin/env bash
# Regenerate the README screenshots headlessly — no manual capture.
#
# Pipeline:  cc-pulseline --preview-layouts  →  ANSI block
#            ansi2html.py                     →  self-contained HTML (Nerd Font)
#            headless Chrome                  →  PNG (retina @2x)
#            ImageMagick -trim                →  tight crop + even margin
#
# The font is the only host dependency: a Nerd Font installed locally so
# Chrome reproduces the exact glyphs (icons / gauges / braille). Override
# with PULSELINE_SHOT_FONT. Requires: Google Chrome, python3, ImageMagick.
#
# Usage: ./screenshots/gen-readme-assets.sh

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BIN="${BINARY:-$ROOT/target/release/cc-pulseline}"
OUT="$ROOT/docs/assets"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
FONT="${PULSELINE_SHOT_FONT:-FiraCode Nerd Font}"
FONTSIZE="${PULSELINE_SHOT_FONTSIZE:-17}"
PAGE_BG="#0d0f14"

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

command -v magick >/dev/null || { echo "need ImageMagick (magick)" >&2; exit 1; }
[[ -x "$CHROME" ]] || { echo "Chrome not found at: $CHROME (set \$CHROME)" >&2; exit 1; }
[[ -x "$BIN" ]] || { echo "Building release binary..." >&2; (cd "$ROOT" && cargo build --release >&2); }

# Extract one layout block from --preview-layouts output.
#   block <layout> <busy|idle> <width>
block() {
  "$BIN" --preview-layouts "$3" 2>/dev/null \
    | awk -v l="$1" -v v="$2" '
        $0 ~ "^── "l" @ .* \\("v"\\) ──$" {f=1; next}
        /^── / {f=0}
        f && NF {print}'
}

# Render ANSI from stdin to a trimmed PNG.
#   render <out.png>
render() {
  local out="$1" html="$TMP/x.html" raw="$TMP/x.png"
  python3 "$HERE/ansi2html.py" "$FONT" "$FONTSIZE" > "$html"
  "$CHROME" --headless --disable-gpu --hide-scrollbars \
    --force-device-scale-factor=2 --window-size=2200,1500 \
    --default-background-color=00000000 \
    --screenshot="$raw" "file://$html" >/dev/null 2>&1 || true
  magick "$raw" -trim +repage -bordercolor "$PAGE_BG" -border 24 "$out"
  echo "  wrote $out" >&2
}

mkdir -p "$OUT"
echo "Generating README assets into $OUT ..." >&2

# Hero — default `none` layout, busy: identity · config · budget+trend ·
# dual-window quota · tools · agent · todo (every segment in one frame).
block none busy 104 | render "$OUT/hero-dark.png"

# Layouts — framed `console` over Powerline `rail`, to show the layout system.
{ block console busy 96; echo; block rail busy 96; } | render "$OUT/layouts.png"

echo "Done." >&2
