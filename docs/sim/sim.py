"""Quasaria Beacons: illustrative simulations of the v1 fixes.
All figures are illustrative. Units: all amounts in XLM. Stake is QFX, valued 1:1 in XLM (QFX is fully XLM-backed);
rewards are paid in XLM, so results don't depend on the dollar price of XLM.
Run: python3 sim.py   -> writes CSVs in this folder.
"""
import csv, math

TVL = 1_000_000.0
EPOCHS_PER_DAY = 6
EPOCHS_PER_YEAR = 365 * EPOCHS_PER_DAY  # 2190
BLEND_RATE = 0.005         # Blend XLM supply pool APR, illustrative base case (observed ~0.02% 30-d avg, Oct 2026; was ~9% incl. emissions earlier in 2026)
FEE_SHARE = 0.40           # Beacon share of gross DEX trading fees
INS_SHARE = 0.10           # 10% of gross fees -> insurance (taken from protocol/treasury side)
APR_CAP = 0.25

def write(name, rows):
    with open(name, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)

def daily_rev(fees, alloc):
    return FEE_SHARE * fees + BLEND_RATE * alloc * TVL / 365

# ---------------------------------------------------------------- 1. Whale / weights / 5% cap
TIER_W = {"Spark":1.00,"Flare":1.05,"Beacon":1.10,"Pulsar":1.15,"Quasar":1.20,"Singularity":1.25}
LOCK_W = {"none":1.00,"30d":1.10,"90d":1.25}
RETAIL = [("Spark",200,.10),("Flare",1_000,.15),("Beacon",5_000,.25),("Pulsar",20_000,.30),("Quasar",50_000,.20)]
LOCK_MIX = {"none":.40,"30d":.30,"90d":.30}
D_DAY = daily_rev(300, 0.40)   # same daily distribution for both systems: $300/day fees, 40% Blend

def groups(whale_frac, whale_lock):
    g = [dict(name="Whale", tier="Singularity", lock=whale_lock, n=1.0, stake=whale_frac*TVL)]
    rest = (1-whale_frac)*TVL
    for tier, size, share in RETAIL:
        for lk, ls in LOCK_MIX.items():
            val = rest*share*ls
            g.append(dict(name=f"{tier}-{lk}", tier=tier, lock=lk, n=val/size, stake=size))
    return g

def allocate(gs, use_tier, cap):
    for x in gs:
        x["w"] = x["stake"]*LOCK_W[x["lock"]]*(TIER_W[x["tier"]] if use_tier else 1.0)
    capped = set()
    while True:
        free = [x for x in gs if x["name"] not in capped]
        left = 1.0 - sum(x["n"]*cap for x in gs if x["name"] in capped)
        sw = sum(x["n"]*x["w"] for x in free)
        new = False
        for x in free:
            s = left*x["w"]/sw
            if cap and s > cap + 1e-12: capped.add(x["name"]); new = True
        if not new: break
    for x in gs:
        x["share"] = cap if x["name"] in capped else left*x["w"]/sw
        x["capped"] = x["name"] in capped
    return gs

whale_rows = []
for wf in (0.10, 0.30, 0.50):
    for wl in ("90d", "none"):
        for system, use_tier, cap in (("v0 tier multipliers, no cap", True, 0.0), ("v1 flat + lock boost, no cap", False, 0.0), ("v1 flat + lock boost + 5% cap", False, 0.05)):
            gs = allocate(groups(wf, wl), use_tier, cap)
            # excess: what capped wallets would have got without cap (same weights) minus cap
            gs_nc = allocate(groups(wf, wl), use_tier, 0.0)
            excess = sum((b["share"]-a["share"])*a["n"] for a, b in zip(gs, gs_nc) if a["capped"])
            def apr(x): return x["share"]*D_DAY*365/x["stake"]
            wh = gs[0]; ret = gs[1:]
            ret_val = sum(x["n"]*x["stake"] for x in ret)
            ret_apr = sum(x["share"]*x["n"] for x in ret)*D_DAY*365/ret_val
            get = lambda nm: next(x for x in gs if x["name"]==nm)
            whale_rows.append({
                "whale_pct_tvl": int(wf*100), "whale_lock": wl, "system": system,
                "whale_reward_share_pct": round(wh["share"]*100,2),
                "whale_apr_pct": round(apr(wh)*100,2),
                "retail_avg_apr_pct": round(ret_apr*100,2),
                "spark_nolock_apr_pct": round(apr(get("Spark-none"))*100,2),
                "pulsar_90d_apr_pct": round(apr(get("Pulsar-90d"))*100,2),
                "quasar_90d_apr_pct": round(apr(get("Quasar-90d"))*100,2),
                "whale_per_xlm_vs_spark_nolock": round(apr(wh)/apr(get("Spark-none")),3),
                "capped_excess_redistributed_xlm_day": round(excess*D_DAY,2),
                "wallets_capped": ";".join(x["name"] for x in gs if x["capped"]) or "-",
            })
write("whale.csv", whale_rows)

# ---------------------------------------------------------------- 2. Withdrawals
UNBOND_CYCLES = 42          # 7 days
EXIT_CAP = 0.02             # of TVL snapshot per 4h cycle
RETRIEVE = {"normal (<=80% util)": math.inf, "95% util": 5_000.0}   # XLM retrievable from Blend per cycle (assumption)
wd_summary, wd_timeline = [], []
def run_wd(label, alloc, exit_frac, util, unbond, cap_frac):
    blend = alloc*TVL; cash = (1-alloc)*TVL; E = exit_frac*TVL
    target_blend = alloc*(TVL-E)                     # restore target allocation after exits
    queue = E; paid = 0.0; min_cash = cash; shortfall_cycles = 0; restore_c = None; last_pay = None
    per_cycle = cap_frac*TVL if cap_frac else math.inf
    for c in range(0, 85):
        # keeper retrieval from Blend (starts at request time)
        r = min(max(blend-target_blend,0), RETRIEVE[util])
        blend -= r; cash += r
        if restore_c is None and blend <= target_blend+1e-6: restore_c = c
        due = 0.0
        if c >= unbond and queue > 0:
            due = min(queue, per_cycle)
            pay = min(due, cash)
            if pay < due - 1e-6: shortfall_cycles += 1
            cash -= pay; queue -= pay; paid += pay
            if queue <= 1e-6 and last_pay is None: last_pay = c
        min_cash = min(min_cash, cash)
        wd_timeline.append(dict(scenario=label, blend_alloc_pct=int(alloc*100), exit_pct=int(exit_frac*100), utilization=util,
            cycle=c, hour=c*4, cash=round(cash), blend=round(blend), queue=round(queue), paid_cum=round(paid)))
    rem = TVL-E
    wd_summary.append(dict(scenario=label, blend_alloc_pct=int(alloc*100), exit_pct=int(exit_frac*100), utilization=util,
        cash_buffer_start=round((1-alloc)*TVL), blend_to_retrieve=round(alloc*TVL-target_blend),
        blend_restored_by_hour=(restore_c*4 if restore_c is not None else ">336"),
        first_payout_hour=unbond*4, last_payout_hour=(last_pay*4 if last_pay is not None else ">336"),
        cycles_with_shortfall=shortfall_cycles, min_cash=round(min_cash),
        min_buffer_pct_of_remaining_tvl=round(min_cash/rem*100,1)))
for a in (0.30, 0.40, 0.50):
    for u in RETRIEVE:
        for ef in (0.30, 0.60):
            run_wd("v1: 7d unbond, 2%/cycle cap, FIFO", a, ef, u, UNBOND_CYCLES, EXIT_CAP)
for u in RETRIEVE:
    run_wd("v0: instant, no cap", 0.80, 0.30, u, 0, 0.0)
write("withdrawals_summary.csv", wd_summary); write("withdrawals_timeline.csv", wd_timeline)

# ---------------------------------------------------------------- 3. Insurance vs bad debt
ins_rows = []
for a in (0.30, 0.40, 0.50):
    for months in (3, 6, 12):
        fund = INS_SHARE*300*365*months/12
        for loss in (0.05, 0.20, 0.50, 1.00):
            L = loss*a*TVL
            ins_rows.append(dict(blend_alloc_pct=int(a*100), months_accrued=months, insurance_fund=round(fund),
                loss_pct_of_deployed=int(loss*100), loss_xlm=round(L),
                staker_loss_no_ins_pct_tvl=round(L/TVL*100,2), staker_loss_with_ins_pct_tvl=round(max(L-fund,0)/TVL*100,2),
                coverage_pct=round(min(fund/L,1)*100,1)))
write("insurance.csv", ins_rows)

# ---------------------------------------------------------------- 4. Reserve overflow rule
ALLOC = 0.40; CAP_E = APR_CAP*TVL/EPOCHS_PER_YEAR; INS_TARGET = 0.05*ALLOC*TVL
def run_reserve(name, fee_fn, rule_days, days=90):
    S = 42*daily_rev(300, ALLOC)/EPOCHS_PER_DAY      # steady-state reserve at $300/day
    ins = 0.0; buyback = 0.0; paid = 0.0; capped = 0; rows = []
    for e in range(days*EPOCHS_PER_DAY):
        f = fee_fn(e/EPOCHS_PER_DAY)
        S += daily_rev(f, ALLOC)/EPOCHS_PER_DAY
        ins += INS_SHARE*f/EPOCHS_PER_DAY
        D = min(S/42, CAP_E); capped += D >= CAP_E-1e-9; S -= D; paid += D
        if rule_days:
            thr = rule_days*EPOCHS_PER_DAY*D
            if S > thr:
                x = S-thr; S = thr
                to_ins = min(x, max(INS_TARGET-ins,0)); ins += to_ins; buyback += x-to_ins
        if e % EPOCHS_PER_DAY == EPOCHS_PER_DAY-1:
            rows.append(dict(scenario=name, rule=(f"{rule_days}d" if rule_days else "none"), day=e//EPOCHS_PER_DAY+1,
                fees_day=round(f), reserve=round(S), insurance=round(ins), buyback_treasury=round(buyback),
                trailing_apr_pct=round(D*EPOCHS_PER_YEAR/TVL*100,2)))
    return rows, dict(scenario=name, rule=(f"{rule_days}d" if rule_days else "none"), reserve_end=round(S),
        reserve_days_of_cap_payout=round(S/(CAP_E*6),1), insurance_end=round(ins), buyback_treasury_end=round(buyback),
        paid_to_stakers=round(paid), epochs_at_cap=capped, avg_apr_pct=round(paid/TVL*365/days*100,2))
ts, summ = [], []
scen = {"ramp $1,000->$2,000": lambda d: 1000+1000*d/90, "flat $1,500": lambda d: 1500,
        "flat $2,000": lambda d: 2000, "flat $3,000 (stress)": lambda d: 3000}
for n, fn in scen.items():
    for rd in (0, 30, 14):
        r, s = run_reserve(n, fn, rd); ts += r; summ.append(s)
write("reserve_timeseries.csv", ts); write("reserve_summary.csv", summ)
print("done")
