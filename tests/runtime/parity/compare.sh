#!/bin/bash
# usage: compare.sh <morph-png> <chrome-png> <outdir> [ignore WxH+X+Y ...]
# Normalizes sizes (chrome shot decides), blacks out ignore regions in both,
# writes diff + side-by-side, prints RMSE.
set -u
A="$1"; B="$2"; OUTDIR="$3"; shift 3
mkdir -p "$OUTDIR"
W=$(identify -format "%w" "$B"); H=$(identify -format "%h" "$B")
convert "$A" -resize "${W}x${H}!" /tmp/parity_a.png
convert "$B" -resize "${W}x${H}!" /tmp/parity_b.png
for region in "$@"; do
  if [[ "$region" =~ ^([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+)$ ]]; then
    x2=$(( ${BASH_REMATCH[3]} + ${BASH_REMATCH[1]} )); y2=$(( ${BASH_REMATCH[4]} + ${BASH_REMATCH[2]} ))
    convert /tmp/parity_a.png -fill black -draw "rectangle ${BASH_REMATCH[3]},${BASH_REMATCH[4]} $x2,$y2" /tmp/parity_a.png
    convert /tmp/parity_b.png -fill black -draw "rectangle ${BASH_REMATCH[3]},${BASH_REMATCH[4]} $x2,$y2" /tmp/parity_b.png
  fi
done
SCORE=$(compare -metric RMSE /tmp/parity_a.png /tmp/parity_b.png "$OUTDIR/diff.png" 2>&1 || true)
convert +append /tmp/parity_a.png /tmp/parity_b.png "$OUTDIR/side_by_side.png"
echo "RMSE: $SCORE"
