/* Quasaria Beacons simulator: a JS port of docs/sim/sim.py (reserve + distribution section).
   Concept · not deployed · illustrative. All amounts in XLM (QFX valued 1:1). Not a guaranteed return. */
(function (root) {
  const EPOCHS_PER_DAY = 6, EPOCHS_PER_YEAR = 365 * EPOCHS_PER_DAY; // 2190
  const FEE_SHARE = 0.40, INS_SHARE = 0.10, APR_CAP = 0.25, HOLDER_YIELD = 0.12;
  const LOCK_W = { none: 1.00, "30d": 1.10, "90d": 1.25 };
  const dailyRev = (fees, alloc, tvl, rate) => FEE_SHARE * fees + rate * alloc * tvl / 365;

  /* Same loop as run_reserve() in sim.py, with optional user stake, drought and Blend-loss events. */
  function run(p) {
    const tvl = p.tvl, alloc = p.alloc, rate = p.rate, days = p.days || 90;
    const ruleDays = p.ruleDays === undefined ? 30 : p.ruleDays;
    const feeFn = p.feeFn || (() => p.fees);
    const capE = APR_CAP * tvl / EPOCHS_PER_YEAR, insTarget = 0.05 * alloc * tvl;
    let S = 42 * dailyRev(p.startFees ?? p.fees, alloc, tvl, rate) / EPOCHS_PER_DAY;
    let ins = p.insStart || 0, buyback = 0, paid = 0, capped = 0, haircut = 0, lossInfo = null;
    const stake = Math.min(p.stake || 0, tvl), wU = stake * (LOCK_W[p.lock] || 1);
    const sumW = (tvl - stake) + wU; // other stakers at weight 1.00 (illustrative)
    let userPaid = 0; const rows = [];
    for (let e = 0; e < days * EPOCHS_PER_DAY; e++) {
      const d = e / EPOCHS_PER_DAY;
      let f = feeFn(d);
      if (p.drought && d >= p.drought[0] && d < p.drought[1]) f = 0;
      S += dailyRev(f, alloc, tvl, rate) / EPOCHS_PER_DAY;
      ins += INS_SHARE * f / EPOCHS_PER_DAY;
      if (p.blendLoss && !lossInfo && d >= p.blendLoss.day) {
        const L = p.blendLoss.pct * alloc * tvl, cover = Math.min(ins, L);
        ins -= cover; haircut = (L - cover) / tvl;
        lossInfo = { day: p.blendLoss.day, loss: L, covered: cover, stakerLoss: L - cover, haircutPct: haircut * 100 };
      }
      const D = Math.min(S / 42, capE); if (D >= capE - 1e-9) capped++; S -= D; paid += D;
      if (e >= 1) userPaid += D * wU / sumW; // one-epoch warm-up
      if (ruleDays) {
        const thr = ruleDays * EPOCHS_PER_DAY * D;
        if (S > thr) { const x = S - thr; S = thr; const toIns = Math.min(x, Math.max(insTarget - ins, 0)); ins += toIns; buyback += x - toIns; }
      }
      if (e % EPOCHS_PER_DAY === EPOCHS_PER_DAY - 1)
        rows.push({ day: Math.floor(e / EPOCHS_PER_DAY) + 1, fees: f, reserve: S, insurance: ins, buyback,
          payoutPerEpoch: D, userPerEpoch: D * wU / sumW, aprPct: D * EPOCHS_PER_YEAR / tvl * 100 });
    }
    return { rows, reserveEnd: S, insuranceEnd: ins, buybackEnd: buyback, paid, epochsAtCap: capped,
      avgAprPct: paid / tvl * 365 / days * 100, userPaid, userAprPct: stake ? userPaid / stake * 365 / days * 100 : 0,
      userPerEpochSteady: (Math.min(dailyRev(p.fees, alloc, tvl, rate) / EPOCHS_PER_DAY, capE)) * wU / sumW,
      lossInfo, capPerEpoch: capE };
  }
  // Steady-state APR on staked value (no lock weight), like §2.3 of the write-up.
  const steadyApr = (fees, alloc, tvl, rate) => Math.min(dailyRev(fees, alloc, tvl, rate) * 365 / tvl, APR_CAP);
  // Fees/day where Beacons APR equals the 12% holder yield.
  const breakEvenFees = (alloc, tvl, rate) => (HOLDER_YIELD * tvl - rate * alloc * tvl) / (FEE_SHARE * 365);
  root.BeaconsSim = { run, steadyApr, breakEvenFees, dailyRev, LOCK_W, HOLDER_YIELD, EPOCHS_PER_DAY, EPOCHS_PER_YEAR };
  if (typeof module !== "undefined") module.exports = root.BeaconsSim;
})(typeof window !== "undefined" ? window : globalThis);
