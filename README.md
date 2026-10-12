# Quasaria Beacons

> **Concept · not deployed · illustrative.** Nothing here is live on-chain. All rates and figures are illustrative, variable and **not a guaranteed return**. Not an offer.

**Playground:** https://yogibear1323.github.io/quasaria-beacons/ · static site, no backend, no wallet, no transactions.

Beacons lets holders lock **QFX** (pegged 1:1 to XLM, fully backed) into one of six tiers and share real Quasaria protocol revenue, paid in **XLM every 4 hours**. Every QFX earns the same share; tiers unlock perks (trading-fee discounts, XP, visuals).

| Tier | Min lock (QFX) | Trading-fee discount | Reward weight |
|---|---:|---:|---:|
| Spark | 100 | 0% | 1.00 |
| Flare | 500 | 5% | 1.00 |
| Beacon | 2,500 | 10% | 1.00 |
| Pulsar | 10,000 | 15% | 1.00 |
| Quasar | 50,000 | 20% | 1.00 |
| Singularity | 250,000 | 25% | 1.00 |

Lock boost (any tier): none 1.00× · 30 d 1.10× · 90 d 1.25×. No per-wallet cap at launch.

## Key settings
- **Fee split:** 40% stakers · 10% insurance · 50% operations/treasury.
- **Reward schedule:** reserve pays 1/42 per 4-hour cycle; 25% APR cap; reserve above 30 days of payouts → insurance (to 5% of deployed) → buyback/treasury.
- **Peg monitor:** new stakes and tier upgrades pause if QFX drifts > 1% from XLM; claims and exits continue.
- **Unbonding:** 7 days, exit cap 2% of TVL per cycle, FIFO queue, one-epoch warm-up for new stake.
- **Blend pool:** Blend's **XLM** pool is the default (asset-matched to XLM payouts); USDC is a non-default alternative.

## QFX → Blend routing: Option B (decided)
The vault redeems staked QFX → XLM 1:1 (burning it), supplies **40%** to Blend's XLM pool, keeps **60%** as a cash buffer, and **re-mints QFX 1:1 on exit** after the 7-day unbonding. Blend losses land on stakers, covered by the insurance fund first. The QFX peg reserve stays fully backed for all circulating QFX. No QFX contract change is needed (redeem can't be paused; mint can, so exits fall back to XLM while it's paused). Option A (lending the peg reserve's XLM) is rejected: it needs a risky upgrade and puts Blend risk on every QFX holder.

## On-chain scope (not built)
Six Soroban contracts: **staking vault**, **fee router**, **reward reserve**, **Blend adapter**, **insurance fund**, **peg monitor**. Off-chain: a **keeper** bot (4-hour cycles, Blend harvests/withdrawals) and an **indexer** for payout attribution. Admin behind a **multisig with a timelock**. Sequence: local tests → testnet (testnet exchange + Blend testnet XLM pool) → always-on keeper → 4–8 week testnet soak → audit + legal + hardware multisig → small capped mainnet launch.

### Funding (rough, illustrative)
| Item | Estimate |
|---|---|
| Testnet | free |
| Mainnet deploy + reserves | tens of XLM |
| Keeper fees | a few XLM / month |
| Insurance seed | ~20k XLM / ~$20k (illustrative) |
| Audit | ~$30k–$100k+ |
| Legal | ~$10k–$50k |
| Launch incentives | optional |

## Open decisions and risks
- **Blend XLM rate is near zero** (~0.02% 30-day avg, Oct 2026). Sims use 0.5% illustrative; ~96% of yield is trading fees. Consider shrinking or zeroing the **40% Blend allocation** until rates recover.
- **12% holder yield comparison:** staked QFX forgoes the 12% holder yield. Beacons only beats it above **~808 XLM/day of fees per 1M QFX staked** (≈4.6% APR at 300/day, ≈9.0% at 600/day). The 12% yield is pre-funded, not revenue-backed, and needs the same disclosure.
- **Gross vs protocol fee cut:** sims assume 40% of every swap fee; if it's 40% of Quasaria's cut after LPs, every rate is lower.
- Low early fee volume; Blend protocol risk; QFX peg/backing audit; mint-pause fallback; exit haircut rule after an uncovered loss; regulatory treatment.

## Repo layout
- `index.html`, `style.css`, `quasar.css`, `sim.js`, `app.js` – the playground (GitHub Pages).
- `docs/concept.md|pdf` – write-up (stress test, simulations, final settings).
- `docs/deployment-scope.md|pdf` – on-chain deployment scope.
- `docs/backoffice-theme.md` – Back Office theme notes.
- `docs/dex-integration-plan.md` – how Beacons plugs into the Quasaria DEX (stays / changes / new, fee router hook, testnet build order).
- `docs/sim/sim.py` + CSVs – simulations (`python3 sim.py`). `sim.js` is a port of the reserve model; it reproduces `reserve_summary.csv` exactly.
