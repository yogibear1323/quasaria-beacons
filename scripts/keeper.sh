#!/usr/bin/env bash
# Beacons keeper (TESTNET ONLY). Every POLL seconds:
#   1. peg-monitor.check()      - on-chain QFX backing (XLM in QFX / QFX supply)
#   2. reward-reserve.run_cycle() when the cycle is due (4h prod; demo cycle if deployed with demoTiming=true)
#   3. vault.process_exits()    when the exit queue is non-empty
# PRICE_SOURCE=dex additionally reports the DEX-implied XLM-per-QFX price
# (QFX/QUSD vs XLM/QUSD pools). Off by default: the thin testnet pools sit ~9% off peg.
set -uo pipefail
cd "$(dirname "$0")/.."
D=deployments/testnet.json
j() { jq -r "$1" $D; }
[ "$(j .network)" = testnet ] || { echo "testnet only"; exit 1; }
VAULT=$(j .contracts.vault); RES=$(j .contracts.rewardReserve); PEG=$(j .contracts.pegMonitor)
POLL=${POLL:-30}; PRICE_SOURCE=${PRICE_SOURCE:-none}
echo "keeper start $(date -Is) demoTiming=$(j .params.demoTiming) cycleSecs=$(j .params.cycleSecs)"
inv() { stellar contract invoke --network testnet --source beacons-keeper --id "$@" 2>&1; }
view() { stellar contract invoke --network testnet --source beacons-keeper --send=no --id "$@" 2>/dev/null | tail -1; }
while true; do
  now=$(date +%s)
  inv $PEG -- check | grep -E "tx/|\{" | tail -1 | sed "s/^/[$(date +%T)] peg.check /"
  if [ "$PRICE_SOURCE" = dex ]; then
    x=$(view CBBUQW2VGV5ZH2NPLSVZF5BQM7PPKAWTKDZVK2AVO4DV4KYIS7FSXHB5 -- reserves); q=$(view CBPJ5AXOBTN37PALD62OVPHIJ3BLCFX6XLSCMS3ICX6F6GUVNNM4WFLK -- reserves)
    price=$(python3 -c "import json;x=json.loads('$x');q=json.loads('$q');print(int((int(q[1])/int(q[0]))/(int(x[1])/int(x[0]))*1e7))")
    inv $PEG -- report_price --price $price | tail -1 | sed "s/^/[$(date +%T)] peg.report_price $price /"
  fi
  next=$(view $RES -- state | jq -r .next_cycle)
  if [ -n "$next" ] && [ "$now" -ge "$next" ]; then
    inv $RES -- run_cycle | grep -E "tx/|\{" | tr '\n' ' ' | sed "s/^/[$(date +%T)] reserve.run_cycle /"; echo
  fi
  ql=$(view $VAULT -- state | jq -r .queue_len)
  if [ "${ql:-0}" != 0 ]; then
    inv $VAULT -- process_exits | grep -E "tx/|^\"?[0-9]" | tr '\n' ' ' | sed "s/^/[$(date +%T)] vault.process_exits /"; echo
  fi
  date +%s > "${HEARTBEAT:-/dev/null}"
  [ "${ONCE:-0}" = 1 ] && break
  sleep $POLL
done
