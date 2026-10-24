# grid-test — CSS Grid geometry parity

Static `display: grid` scenes with solid-color cells and no text or borders, so
pixel bounds expose track, gap, span and alignment geometry directly.

| Grid | Coverage |
|---|---|
| `.grid` | Fixed + `1fr` columns, fixed rows, distinct `column-gap` / `row-gap`, spanning cells |
| `.grid2` | `repeat()` + `minmax()`, fractional columns, `grid-auto-rows`, two-value `gap` shorthand |
| `.grid3` | Named `grid-template-areas`, centered items via `justify-items` / `align-items` |

The fixture resets `body` margin and padding so the comparison isolates grid
geometry from user-agent defaults.

## Verify

`ref.html` uses the same stylesheet and DOM as `src/App.mx`.

```bash
# Chrome reference (from this dir):
google-chrome --headless=new --no-sandbox --disable-gpu --hide-scrollbars \
  --force-device-scale-factor=1 --window-size=600,560 \
  --screenshot=screenshot-chrome.png ref.html

# Morph (X server usually on :0):
/path/to/morph build --no-upx
MORPH_GLFW_X11=1 DISPLAY=:0 ./.morph/output/grid-test &  # window: "Grid Test" 600x560
import -window <id> screenshot-morph.png                # xwininfo -root -tree | grep "Grid Test"

# Compare the solid-color cell bounds; all track edges line up within the
# 1px edge-antialiasing tolerance.
```

`screenshot-morph.png`, `screenshot-chrome.png` and `compare.png` (Morph left,
Chrome right) are the captures for this fixture.

## Regression fixed

CSS `row-gap` / `column-gap` reached the IR and were emitted as
`rowGap.value` / `columnGap.value`, but the generated C++ never set
`rowGapSet` / `columnGapSet`. The grid runtime therefore fell back to the
single `gap` field and rendered zero gaps. The emitter now sets both flags
(including hover/active deltas), and the `gap` shorthand parser accepts the
two-value `row column` form. Before this fix, `.grid` had 0px gaps and `.grid2`
used a single `gap` value rejected as unparsable; both scenes now match Chrome.
