#!/usr/bin/env bash
# Beacons keeper supervisor + watchdog (TESTNET only). Same pattern as bot/scripts/oracle-feed.sh.
#   scripts/keeper-supervisor.sh start|stop|restart|status|logs
# Restarts keeper.sh when it exits or its heartbeat (written after every loop) is older than
# KEEPER_STALE_SEC (default 3 x POLL + 120). Crash loop (>=5 restarts / 10 min) -> back off 5 min.
# Clock jumps (VM suspend/resume) reset the stale timer. Boot: ~/.bashrc / ~/.profile call
# `start` (idempotent), so the keeper comes back with the first shell after a box restart.
set -uo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${KEEPER_STATE:-/workspace/logs}"; mkdir -p "$STATE"
LOG="$STATE/beacons-keeper.log"; HB="$STATE/beacons-keeper.heartbeat"; PIDFILE="$STATE/beacons-keeper-supervisor.pid"
export POLL="${POLL:-300}"; STALE="${KEEPER_STALE_SEC:-$((POLL * 3 + 120))}"; CHECK=15; GRACE=60
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH" HEARTBEAT="$HB"
log() { echo "$(date -u +%FT%TZ) [keeper-supervisor] $*" >> "$LOG"; }
stop_child() { kill -TERM -- "-$1" 2>/dev/null || kill -TERM "$1" 2>/dev/null; for _ in $(seq 1 10); do kill -0 "$1" 2>/dev/null || return 0; sleep 1; done; kill -KILL -- "-$1" 2>/dev/null; }
supervise() {
  local restarts=() child=""
  trap 'log "supervisor stopping"; [ -n "$child" ] && stop_child "$child"; rm -f "$PIDFILE"; exit 0' TERM INT
  while true; do
    started=$(date +%s); setsid "$DIR/scripts/keeper.sh" >> "$LOG" 2>&1 < /dev/null & child=$!
    log "keeper started pid $child (POLL=$POLL, stale ${STALE}s)"; last=$started; stale_since=0
    while kill -0 "$child" 2>/dev/null; do
      sleep "$CHECK" & wait $!; now=$(date +%s)
      [ $((now - last)) -gt $((CHECK * 4)) ] && { log "clock jumped $((now - last))s — resetting stale timer"; stale_since=0; started=$now; }
      last=$now; [ $((now - started)) -lt "$GRACE" ] && continue
      hb=$(cat "$HB" 2>/dev/null || echo 0)
      if [ $((now - hb)) -gt "$STALE" ] && [ $((now - started)) -gt "$STALE" ]; then
        [ "$stale_since" -eq 0 ] && stale_since=$now
        [ $((now - stale_since)) -ge 30 ] && { log "no heartbeat for $((now - hb))s — restarting $child"; stop_child "$child"; break; }
      else stale_since=0; fi
    done
    wait "$child" 2>/dev/null; log "keeper $child exited ($?)"
    now=$(date +%s); restarts+=("$now"); recent=0; for t in "${restarts[@]}"; do [ $((now - t)) -lt 600 ] && recent=$((recent + 1)); done
    if [ "$recent" -ge 5 ]; then log "crash loop — backing off 300 s"; sleep 300; else sleep 5; fi
  done
}
case "${1:-status}" in
  start) if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then echo "already running (supervisor $(cat "$PIDFILE"))"; exit 0; fi
         nohup setsid "$0" __supervise >/dev/null 2>&1 </dev/null & sleep 1; echo "started keeper supervisor $(cat "$PIDFILE" 2>/dev/null) — log: $LOG";;
  __supervise) echo $$ > "$PIDFILE"; supervise;;
  stop) [ -f "$PIDFILE" ] && kill "$(cat "$PIDFILE")" 2>/dev/null; sleep 2; echo stopped;;
  restart) "$0" stop; "$0" start;;
  status) if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then echo "running (supervisor $(cat "$PIDFILE")), last heartbeat $(( $(date +%s) - $(cat "$HB" 2>/dev/null || echo 0) ))s ago"; else echo "not running"; exit 1; fi;;
  logs) tail -n 50 "$LOG";;
esac
