#!/usr/bin/env python3
"""Convert 256-color ANSI terminal output to a self-contained HTML page.

Renders with a locally-installed Nerd Font so Chrome reproduces the exact
glyphs (icons, gauges, braille sparklines) cc-pulseline emits. Used to
generate README screenshots headlessly via the Chrome MCP.
"""
import sys
import html
import re

# xterm 256-color palette -> #rrggbb
def xterm256():
    base = [
        (0, 0, 0), (205, 0, 0), (0, 205, 0), (205, 205, 0),
        (0, 0, 238), (205, 0, 205), (0, 205, 205), (229, 229, 229),
        (127, 127, 127), (255, 0, 0), (0, 255, 0), (255, 255, 0),
        (92, 92, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
    ]
    cube = [0, 95, 135, 175, 215, 255]
    colors = list(base)
    for r in cube:
        for g in cube:
            for b in cube:
                colors.append((r, g, b))
    for i in range(24):
        v = 8 + 10 * i
        colors.append((v, v, v))
    return ["#%02x%02x%02x" % c for c in colors]

PALETTE = xterm256()
ESC = re.compile(r"\x1b\[([0-9;]*)m")


def render_line(line):
    out = []
    pos = 0
    fg = None
    bg = None
    bold = False
    reverse = False

    def open_span():
        f, b = (bg, fg) if reverse else (fg, bg)
        styles = []
        if f is not None:
            styles.append(f"color:{PALETTE[f]}")
        if b is not None:
            styles.append(f"background:{PALETTE[b]}")
        if reverse and f is None:
            # reverse with no explicit fg: invert against page bg
            styles.append("color:#16181d")
            styles.append("background:#c8c8c8")
        if bold:
            styles.append("font-weight:600")
        return f'<span style="{";".join(styles)}">' if styles else "<span>"

    span_open = False
    for m in ESC.finditer(line):
        text = line[pos:m.start()]
        if text:
            if not span_open:
                out.append(open_span())
                span_open = True
            out.append(html.escape(text))
        if span_open:
            out.append("</span>")
            span_open = False
        params = [p for p in m.group(1).split(";")] if m.group(1) else ["0"]
        i = 0
        while i < len(params):
            p = params[i] or "0"
            if p == "0":
                fg = bg = None
                bold = reverse = False
            elif p == "1":
                bold = True
            elif p == "7":
                reverse = True
            elif p == "27":
                reverse = False
            elif p == "22":
                bold = False
            elif p == "39":
                fg = None
            elif p == "49":
                bg = None
            elif p == "38" and i + 2 < len(params) and params[i + 1] == "5":
                fg = int(params[i + 2]); i += 2
            elif p == "48" and i + 2 < len(params) and params[i + 1] == "5":
                bg = int(params[i + 2]); i += 2
            i += 1
        pos = m.end()
    tail = line[pos:]
    if tail:
        out.append(open_span())
        out.append(html.escape(tail))
        out.append("</span>")
    return "".join(out)


def main():
    raw = sys.stdin.read().rstrip("\n")
    font = sys.argv[1] if len(sys.argv) > 1 else "FiraCode Nerd Font"
    size = sys.argv[2] if len(sys.argv) > 2 else "16"
    lines = [render_line(l) for l in raw.split("\n")]
    body = "\n".join(lines)
    print(f"""<!DOCTYPE html><html><head><meta charset="utf-8"><style>
  html,body{{margin:0;background:#0d0f14;}}
  #shot{{display:inline-block;background:#16181d;padding:22px 26px;border-radius:10px;}}
  pre{{margin:0;font-family:'{font}','Symbols Nerd Font',monospace;
       font-size:{size}px;line-height:1.5;letter-spacing:0;
       white-space:pre;color:#c0caf5;-webkit-font-smoothing:antialiased;}}
</style></head><body><div id="shot"><pre>{body}</pre></div></body></html>""")


if __name__ == "__main__":
    main()
