#!/usr/bin/env bash
# Deploy the six Beacons contracts to Stellar TESTNET. Never mainnet.
# Keys are local stellar-cli identities (never committed). DEMO timing flags
# are explicit: CYCLE_SECS / UNBOND_SECS default to the demo values.
set -euo pipefail
NET=testnet
[ "$NET" = testnet ] || { echo "testnet only"; exit 1; }
W=contracts/target/wasm32v1-none/release
XLM=CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC
QFX=CDL5JSVI4O4HGUCITMOBBKAMHNAUY6OE4K54A5DJY2PTXSHFJG5XKJ4D   # existing QFX (unchanged)
BLEND_POOL=CCEBVDYM32YNYCVNRXQKDFFPISJJCV557CDZEIRBEE4NCV4KHPQ44HGF # Blend testnet "TestnetV2" pool
CASH_ONLY=${CASH_ONLY:-false}
CYCLE_SECS=${CYCLE_SECS:-120}; UNBOND_SECS=${UNBOND_SECS:-300}; DEMO=${DEMO:-true}
DELAY=300
ADMIN=$(stellar keys address beacons-admin); KEEPER=$(stellar keys address beacons-keeper)
OPS=$(stellar keys address beacons-ops); TREAS=$(stellar keys address beacons-treasury)
d() { local name=$1; shift; stellar contract deploy --wasm $W/$name.wasm --source beacons-admin --network $NET --alias beacons-$name -- "$@" 2>/dev/null | tail -1; }
PEG=$(d beacons_peg_monitor --admin $ADMIN --keeper $KEEPER --qfx $QFX --threshold_bps 100 --timelock_delay $DELAY)
VAULT=$(d beacons_vault --admin $ADMIN --qfx $QFX --xlm $XLM --peg $PEG --unbond_secs $UNBOND_SECS --demo_timing $DEMO --timelock_delay $DELAY)
INS=$(d beacons_insurance_fund --admin $ADMIN --xlm $XLM --target 5000000000 --timelock_delay $DELAY)
RES=$(d beacons_reward_reserve --admin $ADMIN --xlm $XLM --vault $VAULT --insurance $INS --treasury $TREAS --cycle_secs $CYCLE_SECS --demo_timing $DEMO --timelock_delay $DELAY)
ROUTER=$(d beacons_fee_router --admin $ADMIN --xlm $XLM --reserve $RES --insurance $INS --ops $OPS --timelock_delay $DELAY)
ADAPTER=$(d beacons_blend_adapter --admin $ADMIN --vault $VAULT --pool $BLEND_POOL --xlm $XLM --cash_only $CASH_ONLY --timelock_delay $DELAY)
stellar contract invoke --id $VAULT --source beacons-admin --network $NET -- wire --adapter $ADAPTER --reserve $RES >/dev/null
cat > deployments/testnet.json <<J
{
  "network": "testnet",
  "warning": "TESTNET ONLY - UNAUDITED. Demo timing: ${DEMO}",
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "admin": "$ADMIN", "keeper": "$KEEPER", "ops": "$OPS", "treasury": "$TREAS",
  "testFeeSource": "$(stellar keys address beacons-test-fees)",
  "external": { "xlmSac": "$XLM", "qfx": "$QFX", "blendPool": "$BLEND_POOL" },
  "contracts": { "vault": "$VAULT", "feeRouter": "$ROUTER", "rewardReserve": "$RES", "blendAdapter": "$ADAPTER", "insuranceFund": "$INS", "pegMonitor": "$PEG" },
  "params": { "cycleSecs": $CYCLE_SECS, "unbondSecs": $UNBOND_SECS, "demoTiming": $DEMO, "timelockDelaySecs": $DELAY, "cashOnly": $CASH_ONLY, "pegThresholdBps": 100, "insuranceTargetXlm": 500 },
  "deployedAt": "$(date -u +%FT%TZ)"
}
J
cat deployments/testnet.json
