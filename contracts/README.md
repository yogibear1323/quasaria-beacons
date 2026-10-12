# Beacons contracts — Stellar TESTNET, unaudited

Six Soroban contracts (soroban-sdk 28), plus the shared `quasaria-gov` helper vendored unchanged from `yogibear1323/quasaria` (two-step admin, guardian pause, 300 s timelock on testnet).

| Contract | What it does |
|---|---|
| `vault` | Stake QFX (tiers Spark 100 → Singularity 250,000 QFX are labels; flat per-QFX share; 30d 1.1x / 90d 1.25x boosts; no wallet cap). Redeems QFX→XLM 1:1 via the existing QFX `redeem`, lends 40% through the Blend adapter, keeps 60% cash. One-epoch warm-up. Unbond (7 days prod) → FIFO queue, max 2% of principal per cycle, re-mints QFX via QFX `deposit` on exit. New stakes/upgrades blocked while the peg monitor is tripped. One position per address; `add_stake` tops it up (the top-up warms up one epoch; the existing stake keeps earning). |
| `fee-router` | Splits XLM fees 40% reward reserve / 10% insurance / 50% ops; tags each deposit with a source symbol. |
| `reward-reserve` | Each cycle (4 h prod) pays min(balance/42, 25%-APR cap) in XLM to the vault; reserve above 30 days of capped payouts overflows to insurance (to target) then treasury. |
| `blend-adapter` | Option B: supplies the vault's lend slice to the Blend testnet XLM pool (request type 0), withdraws (type 1) on exits. `cash_only` mode holds XLM idle. |
| `insurance-fund` | Holds XLM; payouts only via Critical timelocked action; exposes target. |
| `peg-monitor` | Trips at >1% drift of QFX backing (on-chain) or keeper-reported price; admin resume only once back in band. |

The QFX contract (`stellar-dex/contracts/reward-token`) is **not modified**.

```
cd contracts && cargo test -p beacons-tests   # 10 end-to-end unit tests (mock QFX + mock Blend)
stellar contract build
../scripts/deploy-testnet.sh                  # DEMO=true CYCLE_SECS=120 UNBOND_SECS=300 by default
../scripts/keeper.sh                          # peg check, run_cycle when due, process_exits
```

Demo timing (`demo_timing=true`) is an explicit constructor flag; without it the contracts only accept 4-hour cycles and 7-day unbonding. Switch with the timelocked `SetCycle(14400,false)` / `SetUnbond(604800,false)` actions. Keys are local stellar-cli identities and are never committed. Testnet deployment: `deployments/testnet.json`; verification transactions: `e2e/tx-log.txt`.
