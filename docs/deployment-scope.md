# Quasaria Beacons: Deployment Scope
### Scoping only · nothing has been deployed · v0.1 · October 2026

---

## Summary

This document scopes what it takes to ship Beacons: what is built, in what order, at roughly what cost, and what still needs deciding. **Nothing here is deployed.** All amounts are illustrative estimates. XLM figures assume 1 QFX = 1 XLM, and dollar figures are labelled as such.

**Locked decisions it builds on:**

- Stake is QFX, and rewards are paid in XLM.
- Tier minimums are 100 / 500 / 2,500 / 10,000 / 50,000 / 250,000 QFX. Every tier earns the same share per QFX; tiers give perks only.
- Lock boosts are 1.10× (30 days) and 1.25× (90 days). There is no per-wallet reward cap.
- The default Blend pool is the XLM supply pool.
- QFX reaches Blend via **Option B**: the vault redeems QFX → XLM 1:1, supplies 40% to Blend, keeps 60% as a buffer, and re-mints QFX 1:1 on exit.

**What QFX already is (code-verified):** QFX is the custom Soroban contract **QuasariaFlux** (not a SAC), and it holds its own XLM backing.

- `deposit()` mints 1:1 with no fee and can be paused by the guardian.
- `redeem()` burns 1:1 with no fee and can't be paused.
- `assert_backed` enforces XLM ≥ supply.
- The admin can move XLM only through a timelocked contract upgrade.

Option B works with this code as it stands. **No QFX change is needed.**

---

## 1. Components

### 1.1 On-chain (Soroban)

| Contract | Responsibility | Key parameters |
|---|---|---|
| **Beacon staking vault** | Accepts QFX and redeems it to XLM via `redeem()` (Option B). Tracks each position: QFX amount, tier (perks only) and lock. Reward-per-share index (acc += D_e / Σw; pending = w · (acc − debt)), so per-user gas is spent only at claim. One-epoch warm-up before new stake earns. 7-day unbonding with no rewards. FIFO exit queue capped at 2% of TVL per 4-hour cycle. Re-mints QFX 1:1 via `deposit()` on exit, or pays XLM if deposits are paused. | Tier minimums in QFX; boosts 1.00 / 1.10 / 1.25; warm-up 1 epoch; unbond 42 epochs; exit cap 2% of the TVL snapshot per cycle |
| **Fee router** (DEX-integrated) | Receives the protocol's fee cut from Quasaria DEX pairs and splits it into stakers / insurance / ops. Logs each bucket per pair with tx hashes for attribution. | Stakers 40% (adjustable 20–40% behind the timelock), insurance 10%, ops = remainder |
| **Reserve / distributor** | Holds reserve S (XLM). Each 4-hour epoch it releases D_e = min(S/42, cap_e), where cap_e = 25% APR × staked value ÷ 2,190 epochs per year. Anything above 30 days of payouts flows to insurance (up to its target), then to buyback/treasury. **Epoch advance should be permissionless**, so anyone can call it if the keeper is down. | 1/42 per epoch; 25% APR cap; 30-day overflow threshold |
| **Blend adapter** | Supplies 40% of staked value (as XLM) to Blend's XLM supply pool and keeps the 60% cash buffer. Harvests interest to the reserve. Emergency withdraw (guardian), plus a utilization guard that pauses new supply if the pool stays above 95% utilization for 24 h. | Target 40% (range 30–50%); buffer 60% |
| **Insurance fund** | Receives 10% of the protocol's fee cut plus reserve overflow, and covers Blend losses first. Any uncovered loss becomes a **pro-rata haircut on all exits**, so nobody gets a first-out advantage. | Target 5% of the deployed amount (≈ 20k XLM at 1M QFX staked) |
| **Peg monitor / circuit breaker** | Compares QFX and XLM. If QFX deviates more than 1%, it pauses new stakes and tier upgrades. Claims and exits stay open. | Threshold 1%; recovery rule TBD (e.g. 24 h back within band) |
| **Admin** | Every parameter and upgrade sits behind a **multisig + timelock**. The guardian can pause, but can't move funds. | e.g. 3-of-5 hardware keys; 48–72 h timelock |

### 1.2 Off-chain

| Component | Responsibility | Notes |
|---|---|---|
| **Keeper** | Advances epochs every 4 h, harvests Blend interest, runs the daily auto-claim, processes the FIFO exit queue and refills the buffer from Blend during unbonding. | Always-on hosting (managed container or VPS) with a second-region failover, alerting, and a funded fee account |
| **Indexer** | Builds payout-source attribution (fees per pair, Blend interest, insurance, overflow) with tx hashes. Feeds the Lightpath, The Signal and Observatory. Reconciles every payout against on-chain events. | Public read API; reconciliation alerts |
| **Frontend wiring** | Replaces the current *Not live yet* Back Office preview with live tier cards, stake/unbond/claim flows, queue position and ETA, and a live APR that shows the comparison with the forgone 12% QFX holder yield. Each card pulses only on a real payout tx. | Same Beacons theme; testnet first |

---

## 2. Step-by-step sequence

1. **Local tests.** Unit, property and fuzz tests for the vault index math, unbonding/FIFO, cap and overflow, Option B redeem/re-mint (including the paused-deposit fallback), the fee-router split and the circuit breaker. Simulate against the published sims (`sim/`).
2. **Testnet deploy.** Deploy all contracts against testnet QuasariaFlux and Blend's testnet XLM pool. Confirm the pool version and parameters (collateral/liability factors, utilization cap, rate curve, backstop) and its withdrawal behavior.
3. **Always-on infra.** Keeper with failover, indexer, monitoring dashboards and alerts (missed epoch, reconciliation mismatch, peg deviation, Blend utilization).
4. **Soak, 4–8 weeks on testnet.** Pass criteria:
    - 100% of payouts reconcile to fee and Blend events, with zero manual top-ups.
    - Keeper uptime ≥ 99.5%, and any missed epoch is caught up by a permissionless call.
    - Drills pass: a 30% exit run, Blend emergency withdraw, a > 1% depeg pause, paused-deposit exits paid in XLM, and an uncovered-loss haircut.
    - No open critical or high bugs.
    - Testers understand that the APR is variable and how it compares with the 12% holder yield.
5. **Audit + legal.** External Soroban audit of all Beacon contracts **and** a review of the QuasariaFlux peg and backing reserve (the upgrade path especially). In parallel, legal review of classification, geo-restrictions and all copy.
6. **Hardware-wallet multisig.** Generate keys, run a signing ceremony, set the timelock, and transfer admin of all contracts, including the QFX upgrade key, to the multisig.
7. **Capped mainnet launch, signed by Robert.** Robert signs the deploy and parameter transactions. Launch with a **TVL cap** (e.g. 250k–1M QFX) and the insurance seed in place. Raise the cap in steps after stable epochs and fee data.

---

## 3. Funding (rough, illustrative estimates)

| Item | Estimate | Notes |
|---|---:|---|
| Testnet | Free | Friendbot funds; only engineering time |
| Mainnet contract deploys + account reserves | ~tens of XLM | Wasm uploads, instances, base reserves and the fee account |
| Keeper transaction fees | ~a few XLM per month | ~6 epochs/day + harvests + auto-claims; varies with Soroban resource fees |
| Always-on hosting | ~$20–100 per month *(estimate)* | Keeper + indexer + failover + monitoring |
| Insurance seed | ~20k XLM (≈ $20k at the sims' 1:1 illustration) | From the sims: 5% of 400k XLM deployed at 1M QFX staked; scales with TVL |
| Security audit | ~$30k–$100k+ | Scope: vault, router, distributor, adapter, insurance, breaker, plus the QFX peg review |
| Legal | ~$10k–$50k | Classification, terms, disclosures, geo policy |
| Incentive budget (optional) | Fixed, capped, time-boxed | Shown as its own Lightpath bucket; never presented as organic yield |

---

## 4. Open decisions and risks

- **How staked QFX reaches Blend: decided, Option B** (the vault redeems QFX → XLM, 40% goes to Blend, 60% stays as buffer, and QFX is re-minted on exit). **Option A (lending the peg reserve) is rejected:** it needs a QFX upgrade, breaks `assert_backed` and puts Blend risk on every QFX holder. Option B items still open:
    - The guardian's `deposit()` pause blocks re-mint. Fallback: pay exits in XLM while paused, or add a vault exemption through an upgrade.
    - Stakers **forgo the 12% QFX holder yield** while staked. Disclose this and include it in every APR comparison. Break-even is about 808 XLM/day of fees per 1M QFX staked.
    - Circulating QFX supply shrinks while staked, so this should be explained publicly.
    - Pro-rata haircut rule on exit after an uncovered Blend loss.
    - **The QFX upgrade key is the main trust point.** Put it under multisig + timelock and disclose it.
- **Low early fee volume:** at 300 XLM/day per 1M QFX, the APR is about 4.6%, below the holder yield. Launch capped and grow TVL with volume.
- **Blend risk:** smart contract, bad debt, and liquidity at high utilization. The XLM supply rate is near zero today (under 0.01% now, about 0.02% on a 30-day average). Revisit the 40% allocation if it stays there.
- **Peg reserve audit:** tiers and accounting depend on QuasariaFlux's 1:1 backing, which needs its own audit scope.
- **Regulatory classification:** fee-share staking may be treated as a security in some jurisdictions. Get legal sign-off before mainnet.
- **Keeper uptime:** permissionless epoch advance, failover hosting and alerts. A missed epoch delays payouts but must never lose them.
- **Fee base definition:** confirm whether "protocol fee cut" means gross swap fees or the protocol share after LPs. The sims assumed 40% of gross fees.

---

## 5. Recommended order of operations

1. Lock the parameter set and the open Option B items (paused-deposit fallback, haircut rule, holder-yield disclosure).
2. Build the contracts and run local tests, including the drills as automated tests.
3. Deploy to testnet. Confirm the Blend XLM pool version and parameters.
4. Stand up the keeper, indexer and monitoring on always-on hosting.
5. Soak for 4–8 weeks until every pass criterion holds.
6. Freeze the code, then run the audit (Beacons + QFX peg) and legal review in parallel. Fix findings and re-verify.
7. Set up the hardware-wallet multisig + timelock, and move all admin keys, including QFX upgrade, to it.
8. Fund the insurance seed and launch fee account.
9. Capped mainnet launch, signed by Robert. Publish the Observatory from day one.
10. Raise the TVL cap in steps based on fee volume, reconciliation history and incident-free epochs.
