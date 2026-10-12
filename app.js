(function(){
const $=id=>document.getElementById(id), fmt=(n,d=0)=>n.toLocaleString("en-US",{maximumFractionDigits:d,minimumFractionDigits:d});
// stars (same seeded field as Quasar.jsx)
let s=1337;const r=()=>((s=(s*16807)%2147483647)/2147483647);let st="";
for(let i=0;i<70;i++){const x=r()*1000,y=r()*600,rad=r()<.12?1.4:.7+r()*.5,o=.25+r()*.55;st+=`<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="${rad.toFixed(2)}" fill="#e9e6ff" opacity="${o.toFixed(2)}"/>`;}
$("stars").innerHTML=st;
const TIERS=[["Spark",100,0],["Flare",500,5],["Beacon",2500,10],["Pulsar",10000,15],["Quasar",50000,20],["Singularity",250000,25]];
$("tierGrid").innerHTML=TIERS.map(([n,m,d],i)=>`<div class="card" data-tier="${n}"><div class="muted">Tier ${i+1} of 6</div><h3>${n.toUpperCase()}</h3><div class="big mono">${fmt(m)} <span style="font-size:14px">QFX</span></div><p class="muted">Min lock · ${d}% trading-fee discount · Constellation XP · tier visuals<br>Reward weight: flat 1.00</p></div>`).join("");
// pool
const pool=x=>{$("pXLM").setAttribute("aria-pressed",x);$("pUSDC").setAttribute("aria-pressed",!x);
$("poolNote").innerHTML=x?"<b>XLM pool (default, locked in).</b> Asset-matched: stakers are paid in XLM and the vault holds XLM, so XLM price moves don't create a mismatch.":"<b>USDC pool (alternative, not default).</b> Would need swapping XLM to USDC and back, adding swap costs and XLM/USD price risk to the payout pool. Kept as an option only.";};
$("pXLM").onclick=()=>pool(true);$("pUSDC").onclick=()=>pool(false);pool(true);
// peg
const peg=()=>{const v=+$("peg").value,dr=Math.abs(v-1)*100;$("pegOut").textContent=v.toFixed(3)+" XLM ("+dr.toFixed(1)+"% drift)";
$("pegState").innerHTML=dr>1?'<span class="bad">Paused:</span> new stakes and tier upgrades. Claims and exits keep working.':'<span class="ok">Open:</span> staking and upgrades allowed.';};
$("peg").oninput=peg;peg();
// simulator
const tierOf=q=>{let t="Below Spark";for(const [n,m] of TIERS) if(q>=m)t=n;return t;};
function sim(){
 const fees=+$("fees").value,tvl=+$("tvl").value,stake=Math.max(0,+$("stake").value||0),lock=$("lock").value,rate=+$("rate").value/100,alloc=+$("alloc").value/100;
 $("oFees").textContent=fmt(fees);$("oTvl").textContent=fmt(tvl);$("oRate").textContent=rate*100<1?(rate*100).toFixed(1)+"%":(rate*100).toFixed(1)+"%";$("oAlloc").textContent=fmt(alloc*100)+"%";
 const res=BeaconsSim.run({tvl,alloc,rate,fees,stake,lock,ruleDays:30,days:90,drought:$("drought").checked?[30,60]:null,blendLoss:$("loss").checked?{day:45,pct:.2}:null});
 let apr=res.userAprPct; const lossAmt=res.lossInfo?res.lossInfo.haircutPct*365/90:0; // haircut annualized over the 90-day window
 const net=apr;
 $("rApr").textContent=fmt(net,2)+"%";
 $("rAprNote").textContent=res.lossInfo?`Plus a one-off ${fmt(res.lossInfo.haircutPct,2)}% haircut on your stake from the Blend loss. Variable, illustrative.`:"Variable, illustrative, not guaranteed.";
 $("rCycle").textContent=fmt(res.userPerEpochSteady,3)+" XLM";
 $("rTier").textContent=`Tier: ${tierOf(stake)} · steady state at today's inputs`;
 const d=net-12;$("rVs").innerHTML=`<span class="${d>=0?"ok":"bad"}">${d>=0?"+":""}${fmt(d,2)} pts</span>`;
 const be=BeaconsSim.breakEvenFees(alloc,tvl,rate);$("rBe").textContent=`Break-even ≈ ${fmt(be)} XLM/day of fees at this stake size (≈ 808 per 1M QFX at 0.5%/40%), lock 1.00×.`;
 $("rEnd").innerHTML=`Reserve: <b class="mono">${fmt(res.reserveEnd)}</b> XLM<br>Insurance: <b class="mono">${fmt(res.insuranceEnd)}</b> XLM<br>Buyback/treasury: <b class="mono">${fmt(res.buybackEnd)}</b> XLM<br>Pool avg APR: <b class="mono">${fmt(res.avgAprPct,2)}%</b> · cycles at 25% cap: ${res.epochsAtCap}`;
 $("lossNote").textContent=res.lossInfo?`Blend loss: ${fmt(res.lossInfo.loss)} XLM. Insurance covered ${fmt(res.lossInfo.covered)}; stakers absorb ${fmt(res.lossInfo.stakerLoss)} XLM (${fmt(res.lossInfo.haircutPct,2)}% haircut, pro rata on all exits). The QFX peg reserve is untouched.`:"";
 // chart
 const R=res.rows,W=800,H=240,mx=Math.max(1,...R.map(x=>Math.max(x.reserve,x.insurance,x.buyback)))*1.08;
 const line=(k,c)=>`<polyline fill="none" stroke="${c}" stroke-width="2.2" vector-effect="non-scaling-stroke" points="${R.map((x,i)=>(i/(R.length-1)*W).toFixed(1)+","+(H-x[k]/mx*H).toFixed(1)).join(" ")}"/>`;
 let g="";for(let i=1;i<4;i++)g+=`<line x1="0" x2="${W}" y1="${H*i/4}" y2="${H*i/4}" stroke="#342d52" vector-effect="non-scaling-stroke"/>`;
 $("chart").innerHTML=g+line("buyback","#8b7cff")+line("insurance","#fbbf24")+line("reserve","#22d3ee")+`<text x="6" y="16" fill="#948eb4" font-size="12">${fmt(mx)} XLM</text><text x="${W-60}" y="${H-6}" fill="#948eb4" font-size="12">day 90</text>`;
 document.querySelectorAll("[data-tier]").forEach(c=>c.classList.toggle("sel",c.dataset.tier===tierOf(stake)));
}
["fees","tvl","stake","lock","rate","alloc","drought","loss"].forEach(id=>{$(id).addEventListener("input",sim);$(id).addEventListener("change",sim);});
sim();
})();
