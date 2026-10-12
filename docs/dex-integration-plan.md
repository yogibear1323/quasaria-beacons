# Beacons × Quasaria DEX: integration plan

> **Preview · not deployed · illustrative.** Testnet planning only. Nothing on mainnet, no new contract deploys in this pass, **no change to the QFX contract** (Option B is settled: the vault redeems QFX→XLM 1:1, supplies 40% to Blend's XLM pool, keeps 60% as a buffer, and re-mints QFX 1:1 on exit). Not a guaranteed return.

Written Oct 11, 2026 against `yogibear1323/quasaria` @ `292a533` (after PR #38).

## 1. Key finding: there is no protocol fee cut today

`contracts/amm-pool` charges `fee_bps` (30 bps on the testnet pools) on the swap input. **The whole fee stays in the reserves and accrues to LPs**, except an optional referral cut (`registry.share_bps`, re-capped at 50% of the fee) paid to the trader's referrer. There is **no `fee_to` / protocol-share path** in the AMM, so the "40% of fees to stakers" in the Beacons write-up has no source on the DEX yet.

Other protocol revenue that does exist today:
- **Leverage vault** (`open_fee_bps` = 10 bps of notional) → goes to the vault's own reserve (LP/insurance side of the vault), not a treasury.
- **Lending** → `reserve_factor` share of interest accrues to each reserve's `treasury` (admin-withdrawable via timelock).

**Consequence:** the gross-vs-protocol question is decided by the code. Beacons can only ever receive a share of a *protocol* fee that someone first carves out of LP fees. Options:
1. **(Recommended) Add a protocol fee to new pool deployments** (Uniswap-v2 `fee_to` style: e.g. 1/6 of the 30 bps fee, ≈5 bps, minted as LP shares to a `fee_to` address on `sync`, or transferred per swap), deploy *new* testnet pools with it, and point `fee_to` at the fee router. Existing pools stay as they are (LPs keep 100%).
2. Route the lending `treasury` and (optionally) a slice of leverage open fees into the fee router via timelocked withdrawals by the keeper. Small, but real and auditable.
3. Not recommended: "40% of gross swap fees". It would take 40% of LP income away from LPs and make Pools less attractive.

Re-run the sims on **protocol-share** volume: the 12%-holder-yield break-even is ~808 XLM/day of fees *entering the router* per 1M QFX staked (Beacons gets 40% of that). At a 5 bps protocol cut that needs ≈ 1.6M XLM/day of swap volume per 1M QFX staked. This must be shown honestly in the preview.

## 2. Map of existing pieces: stays / changes / new

| DEX piece | Today | Stays | Changes | New |
|---|---|---|---|---|
| **Stake page / Orbit staking** (`contracts/staking`, `CBB6WT3K…63K`) | MasterChef-style pools, pre-funded rewards, optional lock, timelocked admin | Contract and page as-is; existing positions untouched | Stake page gets a "Beacons (preview)" link card explaining the difference (Orbit = pre-funded emissions; Beacons = revenue share) | Later: Beacons positions listed next to Orbit positions |
| **Earn** | Hub: Stake / Hold QFX / Provide liquidity + calculators | All cards | Adds a 4th card: Beacons (preview) → `/beacons` | Beacons calculator = the simulator |
| **QFX Mint & Rewards** (QFX `CDL5JSVI…J4D`, 12% holder yield from a pre-funded reserve) | Mint/redeem 1:1, holder yield per second | **QFX contract unchanged** | Copy: staked QFX leaves the wallet and stops earning holder yield (vault must be `yield_exempt`, like Orbit). Comparison shown on Beacons | — |
| **Pools / LP fees** (`contracts/amm-pool`, router) | 30 bps, 100% to LPs (minus referral cut) | Existing pools | — | New testnet pool generation with protocol `fee_to` → fee router (§1). Pools page shows "LP share / protocol share" per pool |
| **Referrals** (`contracts/referral`) | Referral cut paid inside the swap, capped at 50% of the fee | Contract | Order of cuts: referral cut first (from LP fee), then protocol share; document it | Fee router can optionally fund the referral pool (as the write-up proposes) |
| **Quests / XP ranks** (Stardust, Comet, Nova, Pulsar, Quasar) | XP for learning/habits; not money; no volume rewards | All rules (XP is never money, no leaderboards) | — | "Constellations" = a quest track: *Learn Beacons* (read unbonding/peg/risks), *Run the simulator*, *Stake on testnet once*, *Claim a payout*. Constellation XP perk from tiers is **cosmetic only** and must not reward deposit size (flat XP per tier reached, capped) to keep the existing XP rule |
| **Back Office** | Beacons preview panel (tiers, Blend XLM default, empty payout table) | Panel | Links to `/beacons`; later fills payout table from the indexer | Monitoring tiles (§6) |
| **Keepers** (`bot/src/keeper.ts`, oracle-feed every 300 s) | Liquidations, oracle refresh | — | Move to an always-on host (box gets suspended) | Beacons keeper: 4-hour cycle, Blend supply/withdraw, unbonding queue, daily auto-claim |
| **Oracle feed** (mock SEP-40 `CBHMDDCD…2DY`; mainnet → Reflector) | XLM/USD prices | — | — | Peg monitor reads QFX/XLM from the AMM QFX pools (TWAP) + QFX `reserves()` (`fully_backed`). XLM/USD not needed (everything is XLM-on-XLM) |

## 3. Beacons vs Orbit staking and the 12% holder yield (recommendation)

**Beacons sits beside them, no replacement in this phase.**
1. **Now:** `/beacons` preview page (this PR), testnet contracts built and soaked next to Orbit. Users choose: hold QFX (12% from the pre-funded reserve, while it lasts), Orbit pools (pre-funded emissions) or Beacons (revenue share, variable).
2. **After the testnet soak:** publish the comparison live (Beacons APR vs holder yield vs Orbit) and the holder-yield reserve runway.
3. **Migration (only if Beacons proves revenue-backed):** stop adding new Orbit pools; let existing ones run to the end of their funded rewards (no forced exits); phase the 12% holder yield down by timelocked `SetApr` steps announced in advance toward a fee-backed rate. This is a decision for Robert and does **not** need a QFX code change (APR is an existing timelocked parameter).

## 4. Nav / IA

- Nav: `… Earn · Stake · Beacons · QFX Mint & Rewards …` (Beacons right after Stake). Label carries "Preview".
- Route `/beacons`, quasar theme, banner *Preview · not deployed · illustrative*.
- Entry points: Stake page header, Earn hub card, Back Office Beacons panel.
- Later: `/beacons/observatory` (public stats), tier cards on the Rewards page.

## 5. Testnet build order (no deploys in this pass)

| # | Piece | Needs from the DEX |
|---|---|---|
| 1 | **Protocol fee in AMM (new pool wasm)** | New `fee_to` + `protocol_share` param in `amm-pool`, timelocked like `SetFeeBps`; new QFX/XLM and XLM/QUSD testnet pools |
| 2 | **Fee router** | `fee_to` address of the pools; lending treasury withdrawals; split 40 stakers / 10 insurance / 50 ops; admin = multisig + timelock (reuse `quasaria_gov`) |
| 3 | **Insurance fund** | Receives 10% from the router + overflow; payout only to the vault on a recorded Blend loss |
| 4 | **Staking vault (beacon-vault)** | QFX `redeem`/`deposit` (unchanged contract), `yield_exempt` flag on QFX for the vault, tiers 100…250,000, lock boosts, 7-day unbonding, 2%/cycle exit cap, FIFO, one-epoch warm-up; XLM fallback when `deposit` is paused |
| 5 | **Reward reserve / distributor** | Router inflow; 1/42 per 4 h, 25% cap, 30-day overflow → insurance → buyback/treasury; reward-per-share index |
| 6 | **Blend adapter** | Blend testnet XLM pool address; 40% target (30–50%), emergency withdraw, harvest |
| 7 | **Peg monitor** | QFX/XLM AMM pool reserves (TWAP) + `qfx.reserves()`; pauses vault `stake`/upgrades above 1% drift |
| 8 | **Keeper** | Extends `bot/src/keeper.ts`; always-on host; 4-hour cycle, harvest, unbonding retrieval, daily auto-claim |
| 9 | **Indexer** | Events from router/vault/distributor/adapter → payout attribution (Lightpath) for `/beacons` and the Back Office |
| 10 | **Frontend wiring** | Replace preview numbers with live reads; stake/unbond/claim via the existing wallet + `invokeContract` helpers |

Then the 4–8 week soak, audit (Beacons + QFX peg review), legal review, hardware multisig, capped mainnet launch signed by Robert (out of scope here).

## 6. Back Office monitoring (to add with the testnet contracts)

- Router inflow per source (pool, lending treasury) per cycle, and protocol fee volume.
- Reserve level vs 30-day threshold, cycles at the 25% cap, overflow destinations.
- Blend: deployed amount vs target, supply rate, utilization, withdrawable liquidity.
- Buffer vs queued exits, queue length and ETA, exit cap usage.
- Peg drift vs 1% and pause state; QFX `fully_backed`.
- Insurance balance vs 5% of deployed target; keeper heartbeat (last cycle time).
- Beacons APR vs holder yield vs Orbit APR.

## 7. Open decisions for Robert

1. Add a protocol fee to new pools (and its size, e.g. 5 of 30 bps)? Without it Beacons has almost no fee source.
2. Confirm "40%" = 40% of the **protocol** share (the code leaves no other sensible meaning).
3. Blend allocation 40% vs lower while Blend's XLM rate is near zero.
4. Holder-yield phase-down timing (only after Beacons proves out).
