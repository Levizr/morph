# flex-test — flex-direction reverse + flex-wrap reverse parity

Covers the historically broken flex cases (each was parsed but mis-laid-out):

| Container (`id`) | Props | Correct (browser) behavior |
|---|---|---|
| `rowrev` | `flex-direction: row-reverse` | First box (red) is **rightmost**, row packed right |
| `colrev` | `flex-direction: column-reverse` | First box (red) is at the **bottom** |
| `wrap` | `flex-wrap: wrap`, fixed height | First line (red+green) on **top** |
| `wraprev` | `flex-wrap: wrap-reverse`, fixed height | First line (red+green) at the **bottom** |

Boxes use solid colors and fixed `50x40` sizes with `flex-shrink: 0`
so the comparison is purely about box geometry (no font dependence).

## Verify against a browser

`ref.html` mirrors `src/style.css` 1:1 (same classes/sizes/colors).

```bash
# 1. Chrome reference (from this dir):
google-chrome --headless --disable-gpu --no-sandbox \
  --window-size=400,500 --screenshot=chrome.png ref.html

# 2. Morph (needs an X server, usually on :0):
/path/to/morph build --no-upx   # deletes stale binary first if rebuilding
./.morph/output/flex-test &     # 400x500 window titled "Flex Test"
import -window <id> morph.png   # find <id> via: xwininfo -root -tree | grep "Flex Test"

# 3. Compare chrome.png vs morph.png: order, packing side and line
#    stacking must match.
```
