#!/bin/bash
# usage: shoot_morph.sh <fixture-dir> <window-title> <out-png>
# Launches the fixture binary (builds it first if missing), screenshots its
# client window, kills it. Prints the shot path on success.
set -u
D="$1"; TITLE="$2"; OUT="$3"
BIN=$(find "$D/.morph/output" -maxdepth 1 -type f -executable 2>/dev/null | head -1)
if [ -z "$BIN" ]; then
  echo "building $D..."
  (cd "$D" && /home/piyush/My_Projects/morph/target/debug/morph build --no-upx > /tmp/parity_build.log 2>&1) || { echo "BUILDFAIL: $D"; exit 1; }
  BIN=$(find "$D/.morph/output" -maxdepth 1 -type f -executable | head -1)
  [ -z "$BIN" ] && { echo "NOBIN: $D"; exit 1; }
fi
BIN=$(realpath "$BIN")
ABS_D=$(realpath "$D")
export XAUTHORITY=$(ls /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1)
# Wayland sessions: force the GLFW X11 backend so xwininfo/xwd can see it.
export MORPH_GLFW_X11=1
# Launch from the fixture dir: relative asset paths (images, fonts) and
# morph's own run semantics resolve against the project directory.
# setsid(1) puts the app in its own process group so the cleanup kill
# below reaps the app itself and not just the subshell (plain `kill $PID`
# would orphan it and poison later screenshots with stale windows).
(cd "$ABS_D" && setsid "$BIN" > /tmp/parity_app.log 2>&1) &
PID=$!
sleep 9
WID=$(timeout 20 xwininfo -root -tree 2>/dev/null | grep -F "\"$TITLE\"" | tail -1 | awk '{print $1}')
if [ -z "$WID" ]; then echo "NOWINDOW: $D ($TITLE)"; pkill -9 -x "$(basename "$BIN")" 2>/dev/null; exit 1; fi
if timeout 30 xwd -id $WID -silent -out /tmp/parity_shot.xwd && timeout 30 convert /tmp/parity_shot.xwd "$OUT"; then
  echo "OK: $OUT"
else
  echo "SHOTFAIL: $D"
fi
# Exact-name kill: reaps the app even though it was orphaned past the
# setsid subshell (plain `kill $PID` only hits the subshell). Stale
# instances would otherwise poison later screenshots.
pkill -9 -x "$(basename "$BIN")" 2>/dev/null
sleep 1
