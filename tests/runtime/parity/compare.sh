#!/bin/bash
# usage: compare.sh <morph-png> <chrome-png> <outdir> [ignore WxH+X+Y ...]
# Normalizes sizes (chrome shot decides), blacks out ignore regions in both,
# writes diff + side-by-side, prints RMSE.
set -u
A="$1"; B="$2"; OUTDIR="$3"; shift 3
mkdir -p "$OUTDIR"
PA="$(mktemp "${TMPDIR:-/tmp}/parity_a.XXXXXX.png")"
PB="$(mktemp "${TMPDIR:-/tmp}/parity_b.XXXXXX.png")"
trap 'rm -f "$PA" "$PB"' EXIT
W=$(identify -format "%w" "$B"); H=$(identify -format "%h" "$B")
convert "$A" -resize "${W}x${H}!" "$PA"
convert "$B" -resize "${W}x${H}!" "$PB"
for region in "$@"; do
  if [[ "$region" =~ ^([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+)$ ]]; then
    x2=$(( ${BASH_REMATCH[3]} + ${BASH_REMATCH[1]} )); y2=$(( ${BASH_REMATCH[4]} + ${BASH_REMATCH[2]} ))
    convert "$PA" -fill black -draw "rectangle ${BASH_REMATCH[3]},${BASH_REMATCH[4]} $x2,$y2" "$PA"
    convert "$PB" -fill black -draw "rectangle ${BASH_REMATCH[3]},${BASH_REMATCH[4]} $x2,$y2" "$PB"
  fi
done
SCORE=$(compare -metric RMSE "$PA" "$PB" "$OUTDIR/diff.png" 2>&1 || true)
convert +append "$PA" "$PB" "$OUTDIR/side_by_side.png"
echo "RMSE: $SCORE"
