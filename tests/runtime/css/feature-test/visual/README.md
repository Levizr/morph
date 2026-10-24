# Shadow + responsive parity

Reference captures for the `css/feature-test` fixture. The Morph app itself is
`../src/App.mx` plus `../src/style.css`; this folder adds the Chrome version and
the captured comparison. The scene combines the `text-shadow` approximation
with the width media queries; the 560px pass checks that a live resize switches
to the narrow variant.

| File | Contents |
|---|---|
| `shadow-responsive.html` | Chrome version of the fixture UI (links `../src/style.css`) |
| `screenshot-morph-800.png` | Morph at 800x600 |
| `screenshot-chrome-800.png` | Chrome at 800x600 |
| `compare-800.png` | Morph left, Chrome right |
| `screenshot-morph-560.png` | Morph after live-resizing the running window to 560x600 |
| `screenshot-chrome-560.png` | Chrome at 560x600 |
| `compare-560.png` | Morph left, Chrome right |
| `shadow-closeup.png` | 3x shadow crop, baseline aligned (Morph top, Chrome bottom) |

Chrome is normalized to Morph's UA defaults: 8px body margin, `line-height: 1.4`,
and DejaVu Sans. Without that, Chrome's serif default and `line-height: normal`
hide the shadow comparison behind unrelated layout differences.

## Reproduce

From the repo root:

```bash
# Morph, X11 backend so the client window can be captured.
(cd tests/runtime/css/feature-test && \
  ../../../../target/debug/morph build --no-upx && \
  MORPH_GLFW_X11=1 DISPLAY=:0 ./.morph/output/feature-test)

# Chrome, from the HTML app above.
google-chrome --headless=new --no-sandbox --disable-gpu --hide-scrollbars \
  --force-device-scale-factor=1 --window-size=800,600 \
  --screenshot=tests/runtime/css/feature-test/visual/screenshot-chrome-800.png \
  file://$PWD/tests/runtime/css/feature-test/visual/shadow-responsive.html
```

Resize the running Morph window to `560x600` and rerender Chrome with
`--window-size=560,600` for the narrow pair.

Final wide shadow-band check (aligned below the glyphs): Morph mean RGB
`[22.07, 21.91, 31.15]`, Chrome `[19.97, 19.66, 29.71]`; mean absolute error
`2.18`, shadow-crop RMSE `0.0602`. Small remaining differences are glyph
rasterization and Morph's pre-existing border/line-box metrics.
