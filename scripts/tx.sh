#!/usr/bin/env bash
# usage: tx.sh <source-identity> "<label>" <contract-id> -- fn args...   (testnet; appends to e2e/tx-log.txt)
cd "$(dirname "$0")/.."; src=$1 label=$2; shift 2
for try in 1 2 3; do
  out=$(stellar contract invoke --network testnet --source $src --id "$@" 2>&1)
  h=$(echo "$out" | grep -oE "tx/[0-9a-f]{64}" | tail -1 | cut -d/ -f2)
  [ -n "$h" ] && break; sleep 5
done
r=$(echo "$out" | tail -1); [ -z "$h" ] && r=$(echo "$out" | grep -E "Error\(|error" | head -2 | tr '\n' ' ')
echo "$label | ${h:-FAILED} | $r" | tee -a e2e/tx-log.txt
