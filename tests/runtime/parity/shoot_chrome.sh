#!/bin/bash
# usage: shoot_chrome.sh <html-file> <WxH> <out-png>
set -u
HTML="$1"; SIZE="$2"; OUT="$3"
ABS=$(cd "$(dirname "$HTML")" && pwd)/$(basename "$HTML")
timeout 90 google-chrome --headless --disable-gpu --no-sandbox --hide-scrollbars \
  --screenshot="$OUT" --window-size="$SIZE" "file://$ABS" > /dev/null 2>&1 \
  && echo "OK: $OUT" || echo "CHROMEFAIL: $HTML"
