# flex-test — flex reverse + wrap-reverse + flex-basis + align-self parity

Covers the historically broken flex cases (each was parsed but mis-laid-out):

| Container (`id`) | Props | Correct (browser) behavior |
|---|---|---|
| `rowrev` | `flex-direction: row-reverse` | First box (red) is **rightmost**, row packed right |
| `colrev` | `flex-direction: column-reverse` | First box (red) is at the **bottom** |
| `wrap` | `flex-wrap: wrap`, fixed height | First line (red+green) on **top** |
| `wraprev` | `flex-wrap: wrap-reverse`, fixed height | First line (red+green) at the **bottom** |
| `eq` | `flex: 1` ×2 + fixed 50px | Red/green **equal** (125px each), yellow 50px |
| `pxbasis` | `flex-basis: 100px` ×2, no grow/shrink | Red/green **100px** each, rest empty |
| `pct` | `flex-basis: 50%` ×2, no grow/shrink | Red/green **150px** each |
| `selfrow` | `align-items: flex-start` + per-item `align-self` | Red top, green **center**, blue **bottom**, yellow **stretched** |
| `selfcol` | column, `align-items: center` + one `align-self: stretch` | Red/blue centered, green **full-width band** |

Boxes use solid colors and fixed `50x40` sizes with `flex-shrink: 0`
so the comparison is purely about box geometry (no font dependence).

## Verify against a browser

`ref.html` mirrors `src/style.css` 1:1 (same classes/sizes/colors).

```bash
# 1. Chrome reference (from this dir):
google-chrome --headless --disable-gpu --no-sandbox \
  --window-size=400,930 --screenshot=chrome.png ref.html

# 2. Morph (needs an X server, usually on :0):
/path/to/morph build --no-upx   # deletes stale binary first if rebuilding
./.morph/output/flex-test &     # 400x930 window titled "Flex Test"
import -window <id> morph.png   # find <id> via: xwininfo -root -tree | grep "Flex Test"

# 3. Compare chrome.png vs morph.png: order, packing side and line
#    stacking must match.
```
