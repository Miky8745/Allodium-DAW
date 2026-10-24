#!/bin/bash
# Headless playback profile (see CLAUDE.md › Profiling). Runs the app on an
# invisible Xvfb display (a minimized/hidden WebKitGTK window throttles
# timers and stops rAF, which wrecks the numbers), autoplays a project muted
# in Song then pattern mode, and prints a per-phase fps/stall summary; the
# full [perf] log (per-second stats, every stall, most-mutated elements)
# stays in <log>.
#
# usage: scripts/profile.sh <panels> <seconds> <log> [project]
#   panels: none | all | comma list of channelrack,pianoroll,mixer,overview,noplaylist
cd "$(dirname "$0")/.."
env -u WAYLAND_DISPLAY GDK_BACKEND=x11 ALLODIUM_PROFILE=1 ALLODIUM_AUTOPLAY="${4:-test}" \
  ALLODIUM_AUTOPLAY_SECONDS="$2" ALLODIUM_AUTOPLAY_PANELS="$1" \
  timeout 300 xvfb-run -a -s "-screen 0 1920x1080x24" npm run tauri dev > "$3" 2>&1
# summary: avg fps / max stall in song and pattern phases
awk -v name="$1" '
/EVENT phase song start/ {ph="song"} /EVENT phase pattern start/ {ph="pattern"} /EVENT phase .* end/ {ph=""}
ph!="" && /fps=/ { match($0,/fps=[0-9]+/); f=substr($0,RSTART+4,RLENGTH-4); n[ph]++; s[ph]+=f; if (!(ph in mn) || f<mn[ph]) mn[ph]=f }
ph!="" && /STALL/ { match($0,/STALL [0-9]+/); g=substr($0,RSTART+6,RLENGTH-6)+0; st[ph]++; if (g>mx[ph]) mx[ph]=g }
END { for (p in n) printf "%-28s %-8s avgFps=%5.1f minFps=%3d stalls=%3d maxStall=%4dms\n", name, p, s[p]/n[p], mn[p], st[p], mx[p] }' "$3"
