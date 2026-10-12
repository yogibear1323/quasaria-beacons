# Quasaria Beacons
### Concept & Brand Direction · v0.1 · October 2026
*Prepared for Robert Walker · Quasaria (Stellar DEX ecosystem)*

---

## Key results: simulated v1 fixes *(illustrative)*

**Decisions locked (Oct 11, 2026):** stake is QFX (1:1 XLM-backed), rewards are paid in XLM, all tier pricing is in QFX, there is no per-wallet reward cap at launch, **the Blend XLM pool is the default** (asset-matched; USDC is a non-default alternative), and **QFX reaches Blend via Option B**: the vault redeems staked QFX to XLM 1:1, supplies 40% to Blend, keeps 60% as a buffer, and re-mints QFX 1:1 on exit.

Script and CSVs are in `/workspace/quasaria-beacons/sim/` (rerun with `python3 sim.py`). The final parameter set is in §5.

**Shared assumptions:** 1M QFX staked. **All amounts are in XLM.** Stake is valued in QFX, and QFX = XLM 1:1, so dollar values move with the XLM price and no figure here needs redoing. Base case of 300 XLM/day gross DEX fees with 40% going to Beacons. **Blend XLM supply pool APR 0.5% (illustrative base case).** Today's observed rate is much lower: Blend V2's XLM pool shows under 0.01% now and about 0.02% on a 30-day average (DefiLlama, Oct 11, 2026). It was around 9% including emissions earlier in 2026. The rate is variable. Insurance takes 10% of gross fees from the protocol/treasury side, so stakers keep their full 40%. 4-hour epochs (6/day). Reserve release S/42. APR cap 25% on staked value. Ops costs excluded. The retail mix by value is Spark 10% (200-QFX wallets), Flare 15% (1k QFX), Beacon 25% (5k), Pulsar 30% (20k), Quasar 20% (50k). Lock mix is 40% none / 30% 30d / 30% 90d. One whale holds Singularity. Both systems distribute the same daily amount: 125.48 XLM/day, which is a 4.58% average APR.

### K1. Flat per-XLM weights + open lock boosts + 5% per-wallet cap

| Whale share of TVL (90d lock) | v0 tier multipliers | v1 flat + boost, no cap | v1 flat + boost + 5% cap |
|---|---:|---:|---:|
| **10%**: whale APR / retail avg | 5.65% / 4.46% | 5.11% / 4.52% | 2.29% / 4.83% |
| **30%**: whale APR / retail avg | 5.37% / 4.24% | 4.98% / 4.41% | 0.76% / 6.22% |
| **50%**: whale APR / retail avg | 5.12% / 4.04% | 4.86% / 4.30% | 0.46% / 8.70% |
| Whale per XLM vs Spark with no lock | 1.56× | 1.25× (lock only) | 0.52× / 0.13× / 0.05× |
| Capped excess redistributed, XLM/day | n/a | n/a | 8.41 / 34.03 / 59.56 |

- **Flat weights remove the tier advantage on their own.** A whale's per-XLM edge falls from 1.56× to exactly the lock boost anyone can get (1.25× at 90 days, or 1.00× with no lock). Retail average APR rises 0.05–0.26 points.
- **The 5% cap overcorrects.** Any wallet above ~5% of rewards is pinned at 5%. A 10% whale earns 2.29% while retail earns 4.83%. At 1M QFX staked the cap also catches ordinary 50k-QFX Quasar wallets. The capped excess is redistributed pro rata by weight to uncapped wallets (8–60 XLM/day here), so the average stays 4.58%.
- **A per-wallet cap is Sybil-bypassable.** A whale can split into 20 wallets. **Decision (confirmed):** flat weights + open lock boosts, **no hard per-wallet cap at launch**, and publish wallet concentration on the Observatory. (Whale running with no lock: v0 1.25× → v1 1.00× per XLM. See `whale.csv`.)

### K2. Withdrawals: 7-day unbonding, 2% of TVL per cycle exit cap, FIFO

Scenario: **30% of TVL (300k XLM) requests exit at once.** The keeper starts retrieving from the Blend XLM pool at request time to restore the target allocation. At 95% utilization we assume we can retrieve only 5k XLM per cycle (30k XLM/day). At normal utilization the full amount comes back in one cycle.

| Blend alloc | Cash buffer (XLM) | From Blend (XLM) | Restored, normal / 95% util | Exit payouts (FIFO, 20k/cycle) | Min buffer after | Shortfall cycles |
|---|---:|---:|---:|---:|---:|---:|
| 30% | 700k | 90k | 0 h / 68 h | hour 168 → 224 | 70% | 0 |
| 40% | 600k | 120k | 0 h / 92 h | hour 168 → 224 | 60% | 0 |
| 50% | 500k | 150k | 0 h / 116 h | hour 168 → 224 | 50% | 0 |
| *v0: 80%, instant, no cap* | 200k | 240k | 0 h / 188 h | *hour 0* | 0% at 95% util | **19 (≈ 76 h stuck)** |

- At 30–50% allocation, **cash alone covers a 30% run**, and Blend retrieval finishes well inside the 7-day unbonding even at 95% utilization.
- A **60% run** stress test also has zero shortfall: payouts run from hour 168 to 284, and Blend is restored by hour 236 in the worst case (50% alloc at 95% util).
- The v0 design (80% deployed, instant exits) would leave withdrawals stuck for about 3 days in the same 95%-utilization event.

### K3. Insurance (10% of gross fees ≈ 30 XLM/day) vs Blend bad debt/exploit

The table shows staker loss as % of TVL, **without → with insurance**, at 40% allocation (400k XLM deployed). Insurance accrued is 2,738 XLM at 3 months, 5,475 XLM at 6 months and 10,950 XLM at 12 months.

| Loss of deployed amount | 3 months | 6 months | 12 months |
|---|---:|---:|---:|
| 5% (20k XLM) | 2.00 → 1.73% | 2.00 → 1.45% | 2.00 → **0.91%** |
| 20% (80k XLM) | 8.00 → 7.73% | 8.00 → 7.45% | 8.00 → 6.91% |
| 50% (200k XLM) | 20.00 → 19.73% | 20.00 → 19.45% | 20.00 → 18.91% |
| 100% (400k XLM) | 40.00 → 39.73% | 40.00 → 39.45% | 40.00 → 38.91% |

- At 300 XLM/day, fee-funded insurance only meaningfully softens **small** losses. At 12 months it covers 55% of a 5% loss and less than 14% of anything larger. Full results for 30% and 50% allocation are in `insurance.csv`. A 5% loss there costs stakers 1.50 → 0.40% (30%) and 2.50 → 1.41% (50%) at 12 months.
- Fully covering a 5% loss within 12 months needs about **548 XLM/day** of fees, or a **treasury seed**. **The real protection is the allocation cap plus pool selection.** Insurance is a backstop, so disclose Blend loss risk as a direct staker risk.

### K4. Reserve overflow: above 30 days of payouts → insurance (to target) → buyback/treasury

Scenario: 90 days of high fees on 1M QFX staked. The 25% cap (about 114 XLM/epoch, or 685 XLM/day) binds once fees exceed ~1,699 XLM/day.

| 90-day fee path (XLM/day) | Reserve end, no rule (XLM) | Reserve end, 30-day rule | To buyback/treasury (30d) | Avg staker APR |
|---|---:|---:|---:|---:|
| Ramp 1,000 → 2,000 | 6,199 (9.1 days) | 6,199 (not triggered) | 0 | 19.93% |
| Flat 2,000 | 14,091 (20.6 days) | 14,091 (not triggered) | 0 | 24.04% |
| Flat 3,000 (stress) | 48,912 (71.4 days) | **20,548 (30.0 days)** | **22,464** | 24.52% |

- In the requested 1,000–2,000 XLM/day range, the reserve grows by only about 121 XLM/day at the peak, so a **30-day rule does not trigger within 90 days**. It would trigger at about day 144 under a flat 2,000 XLM/day. A 14-day rule triggers at 2,000 XLM/day and sends 944 XLM onward. The rule mainly matters for **sustained revenue well above the cap** (e.g. 3,000 XLM/day).
- Staker payouts are identical with or without the rule, because the cap binds either way. The rule only decides where excess goes after 30 days of runway. The insurance fund reaches its 20k XLM target (5% of deployed) in the stress case. **Disclose that revenue above the cap and the 30-day runway is not paid to stakers.**

---

## 0. Why not the LMNS model

lmns.site sells six "virtual mining devices" (50–5,000 XLM) and promises a **fixed 165% APY paid in XLM every 4 hours**, plus referral bonuses. The appeal is real: simple tiers, frequent payouts, a sense of owning a machine that produces.

The economics aren't. Nothing is mined, and no revenue source is disclosed. A fixed yield that doesn't come from real revenue can only be paid out of new deposits, so payouts depend on growth, and the scheme fails once inflows slow. Referral rewards tied to deposits make that dependency worse. Structurally this is a Ponzi shape, and it carries **securities risk** (an investment contract with an expectation of profit from others' efforts) and **fraud risk** (misrepresenting where returns come from).

**Quasaria keeps the appeal and drops the promise:** tiers, frequent payouts and visible progress, all funded only by revenue that can be checked on-chain.

---

## 1. Three alternatives

### 1.1 Beacons: fee-share staking *(recommended first)*

| | |
|---|---|
| **Core mechanic** | Lock QFX (pegged 1:1 to XLM, fully backed) into one of six tiers, priced in QFX: **Spark · Flare · Beacon · Pulsar · Quasar · Singularity**. Each Beacon receives a share of real protocol revenue. |
| **How users earn** | Pro-rata share of a **fee pool** (a set % of Quasaria DEX trading fees) plus **Blend yield** from supplying part of the stake to **Blend's XLM supply pool (default)**. Paid in XLM on a fixed cadence (e.g. every 4 h). APR is **variable and live**, with each payout broken down by source. |
| **Tier benefits** | **v1:** every XLM earns the same share in every tier. Tiers unlock perks only (trading-fee discounts, Constellation XP, visuals). Optional lock boosts are open to all (none / 30 d / 90 d → 1.00× / 1.10× / 1.25×). Boosts change *weight*, not a promised rate. See §2 and the Key results. |
| **On Stellar** | **Soroban contracts:** `beacon-vault` (deposits, locks, tier/weight accounting), `fee-router` (receives the protocol share of DEX fees and splits it into Beacon pool / treasury / referral pool), `blend-adapter` (supplies 30–50% (target 40%) of staked value to Blend's XLM supply pool (default; asset-matched to XLM payouts), keeps the rest as a cash buffer, harvests interest), `distributor` (4-hour epochs, reward-per-share index, claim anytime or daily keeper auto-claim). **Assets:** QFX stake and native XLM payouts. QFX is the custom Soroban contract **QuasariaFlux** (not a SAC), and it holds its own XLM backing. `deposit()` mints 1:1 with no fee and can be paused by the guardian. `redeem()` burns 1:1 with no fee and can't be paused. `assert_backed` enforces XLM ≥ supply. The admin can move XLM only through a timelocked contract upgrade. **Flow (Option B):** the vault redeems staked QFX → XLM, supplies 40% to Blend's XLM pool, keeps 60% as a buffer, and re-mints QFX 1:1 on exit. No QFX contract change is needed. Optional non-transferable position receipt. |
| **Revenue source** | Actual DEX swap fees + Blend XLM pool interest (variable; near zero at today's rates). If volume is zero, fee yield is zero. That's the point. |
| **Key risks** | Smart-contract bugs; Blend XLM pool risk (utilization, bad debt, liquidity to withdraw, variable supply rate); low fee volume → low APR; XLM price volatility (QFX tracks XLM); QFX peg/backing dependency; lock illiquidity; stakers give up the 12% QFX holder yield while staked; regulatory treatment of revenue share. |
| **Disclosures** | Variable rate, no guaranteed return; live revenue sources with history; Blend exposure % and its risks (losses there hit stakers beyond the insurance fund); QFX peg dependency and circuit breaker; the forgone 12% QFX holder yield; the exit haircut rule after an uncovered Blend loss; 7-day unbonding, exit cap and lock terms; audit status; fee-split parameters and who can change them. |

### 1.2 Constellations: quests & progression

| | |
|---|---|
| **Core mechanic** | Users earn **non-transferable badge assets** for real activity: trading, providing liquidity, completing Autopay deals. Badges add up to XP and move users up the existing ranks (**Stardust → … → Quasar**). |
| **How users earn** | **Fee rebates** that grow with rank, plus claims on a **capped, fixed-budget QFX rewards pool** per season. **No purchase to join.** |
| **On Stellar** | Badges as a Soroban `badge` contract (soulbound: transfer disabled) or Stellar assets with `AUTH_REQUIRED` + clawback-only issuance. An off-chain indexer computes quest completion, and a signed attestation lets the contract mint the badge. Rebates are settled by `fee-router`. The season pool is held in a time-locked contract. |
| **Revenue source** | Rebates are a share of fees users already paid. The QFX pool is a pre-set budget, fully backed by treasury XLM, never open-ended. |
| **Key risks** | Sybil/wash farming (mitigations: min-volume thresholds, fee-weighted XP, cooldowns); badge inflation; redemption pressure on the QFX backing reserve. |
| **Disclosures** | Pool size and season dates; rebate schedule by rank; anti-abuse rules; QFX tracks XLM via 1:1 backing, which is not a promise of any dollar value. |

### 1.3 Lanterns: visualized LP positions

| | |
|---|---|
| **Core mechanic** | Each liquidity position appears as a **lantern** that brightens as it earns swap fees and dims when idle or out of range. |
| **How users earn** | Swap fees from their LP share, plus incentives from a **capped budget** on selected pairs. |
| **On Stellar** | Native Stellar liquidity pools or Quasaria's Soroban AMM. A `lantern` view contract/indexer reads pool share and accrued fees. Incentives come from a capped `gauge` contract. |
| **Revenue source** | Trader swap fees, plus a time-boxed incentive budget. |
| **Key risks** | **Impermanent loss**; incentive drop-off causing liquidity flight; low-volume pairs. |
| **Disclosures** | Plain-language IL explainer with a worked example; incentive end dates; fee vs incentive breakdown per lantern. |

### 1.4 Referrals, done right

- Referrers earn a **share of the actual trading fees** their referred users generate (e.g. 10% of the protocol's take), paid from the `fee-router` referral pool. **Never a cut of deposits.**
- **Capped:** per-referee duration (e.g. 12 months) and a per-referrer monthly ceiling.
- **Disclosed:** the referred user can see that a referral link is attached and what share goes to the referrer. No multi-level tiers.

### 1.5 Recommendation

1. **Launch Beacons first.** It has the closest product appeal to LMNS (six tiers, 4-hour payouts) and real revenue behind every payout.
2. **Constellations as the progression layer.** XP ranks and badges feed Beacon perks (fee discounts) and keep users trading, which grows the fee pool.
3. **Lanterns next**, once DEX volume justifies LP incentives.

---

## 2. Beacons: tier pricing & fee-share math *(all figures illustrative; v1 changes from the Key results)*

### 2.1 Tiers & boosts

| Tier | Minimum lock (QFX) | v0 weight (superseded) | v1 weight | v1 perk: trading-fee discount |
|---|---:|---:|---:|---:|
| Spark | 100 | 1.00 | 1.00 | 0% |
| Flare | 500 | 1.05 | 1.00 | 5% |
| Beacon | 2,500 | 1.10 | 1.00 | 10% |
| Pulsar | 10,000 | 1.15 | 1.00 | 15% |
| Quasar | 50,000 | 1.20 | 1.00 | 20% |
| Singularity | 250,000 | 1.25 | 1.00 | 25% |

**Lock boost m_lock (open to every tier):** none 1.00 · 30 days 1.10 · 90 days 1.25. Under v0 the maximum combined multiplier was 1.5625×. Under v1 it is 1.25×, available to anyone.

The ~5× steps between minimums follow familiar device-tier pricing. Under v1, tiers are about status and perks, not yield: the same XLM with the same lock earns the same reward in every tier. All minimums are in **QFX**, not XLM or dollars. QFX = XLM 1:1, so dollar thresholds move with XLM automatically. *Peg dependency: see §4.*

### 2.2 Revenue → reserve → distribution (per 4-hour epoch *e*)

- **Revenue:** R_e = 0.40·F_trade,e + Y_blend,e − ops_e. Beacons receive 40% of trading fees, plus interest from the Blend allocation, minus operating costs. *v0 assumed 80% deployed with a 20% buffer. v1 deploys 30–50% (target 40%) to Blend's XLM supply pool (default), and a separate 10% of gross fees from the protocol side funds insurance.*
- **Reserve:** R_e goes into the reserve S. *v1 overflow rule:* reserve above 30 days of payouts goes to insurance (up to 5% of the deployed amount), then to buyback/treasury.
- **Distribution:** D_e = min(S / 42, cap_e), with cap_e = 25% × V_staked × (4 h / 1 y) = 25% × V_staked / 2,190, where V_staked is total staked QFX valued 1:1 in XLM. Payouts are in XLM, so the cap and APR are XLM-on-XLM and don't depend on the dollar price. Anything not paid stays in S. Paying out 1/42 of the reserve each epoch smooths payouts over about 7 days (42 epochs).
- **Per user:** w_i = stake_i (QFX) · m_tier · m_lock (v1: m_tier = 1); payout_i = D_e · w_i / Σw. New stake starts earning after one full epoch.
- **Implementation (reward-per-share index):** each epoch, acc += D_e / Σw. For a user, pending_i = w_i · (acc − debt_i), and debt_i resets to acc on claim or stake change. Epoch processing costs the same gas regardless of user count, and per-user gas is spent only at claim. Users can claim anytime, or a keeper auto-claims daily.
- **Displayed APR** = (ΣD in XLM over the last 42 epochs ÷ staked QFX, at 1 QFX = 1 XLM) × 52, shown with the **25% ceiling**.
- **Attribution:** each epoch logs its buckets (trading fees per pair, Blend interest) with tx hashes. Each payout is split pro rata by source on the dashboard (the Lightpath).

### 2.3 Worked examples (verified; 1M QFX staked, fees and payouts in XLM, ops excluded; v0 = 80% deployed at the original 4% generic Blend rate)

| | 300 XLM/day fees | 600 XLM/day fees |
|---|---:|---:|
| Beacon fee share (40%) | 120.00 XLM | 240.00 XLM |
| Blend: 4% × 800k XLM ÷ 365 | 87.67 XLM | 87.67 XLM |
| **Daily distributable** | **207.67 XLM** | **327.67 XLM** |
| Per 4-hour epoch | 34.61 XLM | 54.61 XLM |
| Annualized (× 365 ÷ 1M QFX) | **7.58% APR** | **11.96% APR** |
| Dashboard figure (7-day sum × 52) | 7.56% | 11.93% |
| Mix: fees / Blend | 58% / 42% | 73% / 27% |
| **v1: 40% in Blend XLM pool at 0.5%: daily / APR** | **125.48 XLM / 4.58%** | **245.48 XLM / 8.96%** |
| v1 XLM-rate sensitivity: 0% / 2% / 4% | 4.38% / 5.18% / 5.98% | 8.76% / 9.56% / 10.36% |
| v1 mix: fees / Blend (0.5%) | 96% / 4% | 98% / 2% |
| **vs holding QFX (12% holder yield, forgone while staked)** | **−7.42 pts** | **−3.04 pts** |

The brief's "12–13%" for the 600 XLM/day case works out to **11.96%**, just under 12%. The dashboard's ×52 annualizes 364 days, so it reads about 0.3% (relative) lower than ×365. Both cases sit far below the 25% cap (≈ 114 XLM/epoch, or 685 XLM/day on 1M QFX at weight 1.0). Ops costs reduce both figures. **At today's XLM supply rates, Beacon yield is almost entirely trading fees.** **Staked QFX stops earning the 12% QFX holder yield**, so Beacons only beats simply holding QFX once Beacon fees are above about **808 XLM/day per 1M QFX staked** (0.4 × F × 365 + 2,000 = 120,000). The dashboard must show this comparison next to the live APR. Blend adds about 0.2 APR points at 0.5%, and roughly nothing at the observed ~0.02%.

### 2.4 Edge cases

- **Zero-fee epochs:** R_e is just Blend XLM pool interest (near zero at today's rates). Payouts continue from the reserve and decline gradually, never suddenly. The dashboard shows the drop honestly.
- **Bootstrap:** a **capped, pre-funded incentive budget** (e.g. a fixed XLM amount over 8–12 weeks) feeds S and is labeled as its own Lightpath bucket ("Launch incentives"), so it never looks like organic yield. It ends on a published date.
- **Withdrawals (v1):** 7-day unbonding (no rewards while unbonding), then a FIFO release capped at 2% of TVL per cycle. The keeper retrieves from Blend during unbonding. Show the queue position and ETA, and never pay exits out of other users' rewards. See K2.
- **QFX deposit pause:** re-minting on exit uses `deposit()`, which the guardian can pause. Fallback: pay exits in XLM while it is paused, or add a vault exemption through a QFX upgrade. `redeem()` can't be paused, so entry is never blocked by a pause.
- **Exit after an uncovered Blend loss:** a pro-rata haircut applies to every unbonding and future exit, so there is no first-out advantage. The haircut is published on the Observatory.
- **Depeg circuit breaker:** if QFX deviates more than 1% from XLM, new stakes and tier upgrades pause automatically. Claims and exits keep working.
- **Cap binding:** excess stays in S and is paid later. It is never redirected without disclosure.

---

## 3. Brand & theme: Beacons

**Tagline: *Real light, not promised light.***

**Tone:** calm, confident, transparent. Short sentences. Numbers before adjectives. No hype words ("guaranteed", "passive income machine", "to the moon").

**Naming**

- Tiers: **Spark · Flare · Beacon · Pulsar · Quasar · Singularity**
- Product surfaces: **Beacons** (staking), **The Signal** (live payout feed), **Lightpath** (payout source breakdown), **Pulse** (each 4-hour distribution), **Fee Router** (shown openly in the UI), **Observatory** (public stats dashboard).

**Visual feel**

- Near-black field (#07060B), violet (#8B5CF6) and cyan (#22D3EE) accents, white type with generous spacing.
- The **Singularity Q** mark: a ring with an off-axis tail and a soft violet-to-cyan halo.
- **The glow is the proof.** A Beacon pulses only when a real on-chain payout lands, and each pulse links to its transaction. No decorative idle animations that suggest earnings.

**Positioning vs LMNS:** LMNS sells devices that appear to produce money. Beacons show the pipes: every payout traces back to swap fees and Blend interest, with transaction hashes.

### Sample hero copy

> **Real light, not promised light.**
> Lock QFX in a Beacon and earn a share of real Quasaria trading fees and Blend yield, paid in XLM every 4 hours. Every pulse is an on-chain payout you can verify.
> **[ Light a Beacon ]**  **[ See where payouts come from → ]**

### Three feature blurbs

- **Every pulse has a source.** Each 4-hour payout is split into trading fees and Blend yield, linked to the transactions that funded it.
- **Live rates, honestly variable.** Your APR moves with real volume. We show the current rate, the 7- and 30-day history, and what drives it.
- **Climb by commitment.** Six tiers priced in QFX, from Spark (100) to Singularity (250,000). Every QFX earns the same share; higher tiers unlock lower trading fees, and longer locks (open to all) add a boost.

### Tier card mock

A dark glass card, 3:4 ratio. Top-left is the tier glyph (a small star whose rays grow per tier), with **PULSAR** in tracked caps and "Tier 4 of 6 · min 10,000 QFX" in mono. Center is a slowly breathing cyan orb that flashes once when a payout lands. Below that:

- **Live APR 4.6%** *(variable)*, with a small sparkline (30 d)
- **Lightpath:** Trading fees 96% · Blend XLM pool 4%
- **Weight 1.25×** (flat tier weight × 90-day lock boost) **· Trading discount 15%**
- **Your stake:** 12,500 QFX (= 12,500 XLM) · 90-day lock
- **Last pulse:** +2.41 XLM · 14:00 · tx ↗
- Footer microcopy: *"Rates vary with protocol revenue. Not a guaranteed return."*

### Required small print

> Beacon rewards are variable and depend entirely on Quasaria trading volume and the variable supply rate of Blend's XLM pool. They are not a guaranteed or fixed return, and past payouts do not predict future ones. Locked funds may be unavailable until the lock ends. Funds supplied to Blend carry smart-contract and liquidity risk. QFX is backed 1:1 by XLM and moves with XLM's price. That backing depends on audited contracts and reserves, and new stakes pause if QFX deviates more than 1% from XLM. While staked, QFX doesn't earn the QFX holder yield. If Blend suffers a loss the insurance doesn't cover, exits take a pro-rata haircut. Beacons launches on **Stellar testnet first**. Testnet assets have no value. Not available where prohibited. Not financial advice.

---

## 4. Testnet MVP & open questions

### MVP (testnet, ~6–8 weeks)

1. **Contracts:** `beacon-vault` (QFX stake, XLM payouts; six QFX-priced tiers; 30/90-day locks), `fee-router` (configurable split; admin behind a multisig + timelock), `distributor` (4-hour epochs, pull claims), `blend-adapter` against Blend testnet (40% deployed), reserve + 25% APR cap + overflow rule, unbonding queue, insurance vault, QFX/XLM peg monitor + circuit breaker.
2. **Simulated revenue:** replay real Quasaria testnet swaps, plus a scripted volume generator to stress low/high fee scenarios.
3. **App:** Beacon dashboard, tier cards, The Signal payout feed, Lightpath breakdown, and the Observatory public stats page.
4. **Constellations v0:** three quests (first trade, first LP, first Autopay deal), soulbound badges and XP sync with existing ranks.
5. **Hardening:** unit and fuzz tests, internal review, then an external audit before mainnet. Bug bounty at launch.
6. **Success criteria:** payouts reconcile with fee events to 100%; zero manual top-ups; clear user understanding in testing that APR is variable.

### Open questions

- **Fee volume:** what daily volume makes the yield meaningful? Per §2, with 1M QFX staked, every 100 XLM/day of DEX fees adds about 1.46% APR on top of a ~0.2% Blend baseline (0.5% XLM supply rate). Reaching ~10% needs roughly 671 XLM/day in fees, or about 224k XLM/day of volume at a 0.30% fee. We need real volume data before mainnet.
- **Peg dependency (note):** tiers and all accounting rely on QFX's 1:1 XLM backing, which QuasariaFlux holds itself and enforces with `assert_backed`. The peg mechanism and backing reserve must be audited alongside the Beacon contracts. A **depeg circuit breaker** pauses new stakes and tier upgrades if QFX deviates more than 1% from XLM.
- **Blend pool: decided, the XLM supply pool is the default** (asset-matched to the QFX = XLM stake and the XLM payouts, so there is no price mismatch). USDC remains a non-default alternative, which would need a hedge. **Still to confirm on testnet:** which Blend pool and version (V2 XLM fixed pool vs another), its parameters (collateral/liability factors, utilization cap, interest-rate curve, backstop size, any reserve factor), and its withdrawal behavior at high utilization. **XLM supply-rate variability:** today's rate is near zero (under 0.01% now, ~0.02% 30-day average), so the Blend allocation adds risk for little yield unless borrow demand or emissions return. Revisit the 40% allocation if it stays near zero. **QFX → Blend route: decided, Option B.** The vault redeems staked QFX to XLM 1:1, supplies 40% to Blend's XLM pool, keeps 60% as a buffer, and re-mints QFX 1:1 on exit. Today's QuasariaFlux code supports this with no QFX change. **Option A (lending the peg reserve) is rejected:** it needs a QFX upgrade, breaks `assert_backed` (XLM ≥ supply), and puts Blend risk on every QFX holder.
- **Open items from Option B:**
    - **Deposit pause blocks re-mint:** fall back to paying exits in XLM while paused, or add a vault exemption through an upgrade.
    - **Forgone 12% QFX holder yield:** disclose it and include it in every APR comparison. Break-even is about 808 XLM/day of fees per 1M QFX staked.
    - **Circulating QFX supply shrinks while staked:** the vault holds XLM, not QFX. Communicate this so supply charts don't read as outflows.
    - **Pro-rata exit haircut rule** after any uncovered Blend loss.
    - **The QFX upgrade key is the main trust point:** it is the only path that can move backing XLM. Put it under hardware-wallet multisig with a timelock, and disclose it.
- **Per-wallet cap:** dropped in the recommendation because it is Sybil-bypassable and overcorrects (K1). Revisit only if concentration becomes a governance issue.
- **Cap basis:** cap on staked QFX value (1 QFX = 1 XLM, unboosted) so the displayed 25% ceiling holds exactly. Lock boosts reallocate the payout but don't raise its total.
- **Legal review:** does fee-share staking count as a security in target jurisdictions? Geo-restrictions, KYC needs, and the wording of all marketing and small print should go to counsel before mainnet.
- **QFX tokenomics:** QFX is fully backed 1:1, so any QFX rewards (Constellations) must be backed by treasury XLM. Size the season budget, treasury vs Beacon fee split and any buyback policy on that basis.
- **Blend exposure:** confirm the 40% cap and the withdrawal-queue handling when the XLM pool's utilization is high.
- **Governance:** who can change fee splits, and on what timelock.


---

## 5. Final recommended parameter set *(illustrative; pre-legal review, testnet first)*

| Parameter | Value | Why (see) |
|---|---|---|
| Beacon fee share | 40% of gross DEX trading fees | Core revenue (§2) |
| Insurance funding | 10% of gross fees from the protocol side; target 5% of deployed; treasury seed ≈ 20k XLM recommended | Fee accrual alone is small at 300 XLM/day (K3) |
| Stake & payout assets | Stake QFX (1:1 XLM-backed); rewards paid in XLM | Robert's decision |
| Tier minimums | **100 / 500 / 2,500 / 10,000 / 50,000 / 250,000 QFX** | Familiar ~5× ladder; dollar value moves with XLM (§2.1) |
| Tier reward weight | **Flat 1.00** (tiers = perks: 0–25% trading discount, XP, visuals) | Removes whale edge (K1) |
| Lock boosts | None 1.00 · 30 d 1.10 · 90 d 1.25, open to all | Commitment reward anyone can get (K1) |
| Per-wallet reward cap | **None at launch**; publish concentration | 5% cap overcorrects and is Sybil-bypassable (K1) |
| Epoch / payout | 4 h; reward-per-share index; claim anytime + daily keeper auto-claim | Gas-efficient (§2.2) |
| Reward start | After one full epoch | Blocks just-in-time deposits |
| Smoothing | D_e = min(S/42, cap_e) (~7 days) | Stable payouts (§2.2) |
| APR ceiling | 25% of staked QFX value (1 QFX = 1 XLM), paid in XLM | Prevents spikes from looking like a promise (K4) |
| Reserve overflow | Above 30 days of payouts → insurance (to target) → buyback/treasury | Bounds the reserve; rarely triggers below ~2k XLM/day per 1M QFX (K4) |
| Blend pool & allocation | **Blend XLM supply pool (default)**, 40% (range 30–50%); USDC is a non-default alternative; pool version and parameters confirmed on testnet | Asset-matched; withstands a 30–60% run (K2); XLM supply rate variable (0.5% illustrative) |
| Cash buffer | 60% (inverse of allocation) | Covers exits without Blend (K2) |
| Unbonding | 7 days; no rewards while unbonding | Time to retrieve funds (K2) |
| Exit cap | 2% of TVL snapshot per 4 h cycle (12%/day), FIFO | Orderly runs; 30% exit done 56 h after unbonding (K2) |
| Utilization guard | Pause new deployment if Blend util > 95% for 24 h | Liquidity protection |
| Referrals | 10% of protocol take from referees' fees; 12 months; monthly ceiling; disclosed | Fee-based, never deposit-based (§1.4) |
| QFX → Blend route | **Option B:** vault redeems QFX → XLM 1:1; 40% to Blend XLM pool, 60% buffer; re-mint QFX 1:1 on exit (XLM payout fallback if `deposit()` is paused) | Works with today's QuasariaFlux; Option A rejected |
| Exit after uncovered Blend loss | Pro-rata haircut on all exits; published | No first-out advantage |
| Depeg circuit breaker | Pause new stakes and tier upgrades if QFX deviates > 1% from XLM | Peg dependency (§4) |
| QFX upgrade key | Hardware-wallet multisig + timelock; disclosed as the main trust point | Only path that can move backing XLM |
| Launch gates | Testnet → external audit (incl. QFX peg + backing reserve) → legal sign-off → capped mainnet | §4 |
