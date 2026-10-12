//! End-to-end unit tests for the six Beacons contracts, against a mock QFX
//! (1:1 XLM peg: deposit/redeem) and a mock Blend pool.
#![cfg(test)]
extern crate std;
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, testutils::{Address as _, Ledger}, token, Address, Env, Map, Vec, MuxedAddress};
use beacons_vault::{StakingVaultClient, Status};
use beacons_reward_reserve::RewardReserveClient;
use beacons_fee_router::FeeRouterClient;
use beacons_blend_adapter::{BlendAdapterClient, Request, Positions};
use beacons_insurance_fund::InsuranceFundClient;
use beacons_peg_monitor::PegMonitorClient;

const U: i128 = 10_000_000;

#[contracttype] #[derive(Clone)] enum QK { Xlm, Bal(Address), Sup }
#[contract] pub struct MockQfx;
#[contractimpl] impl MockQfx {
    pub fn __constructor(e: Env, xlm: Address) { e.storage().instance().set(&QK::Xlm, &xlm); e.storage().instance().set(&QK::Sup, &0i128); }
    fn bal(e: &Env, a: &Address) -> i128 { e.storage().instance().get(&QK::Bal(a.clone())).unwrap_or(0) }
    fn setb(e: &Env, a: &Address, v: i128) { assert!(v >= 0); e.storage().instance().set(&QK::Bal(a.clone()), &v) }
    pub fn xlm(e: Env) -> Address { e.storage().instance().get(&QK::Xlm).unwrap() }
    pub fn total_supply(e: Env) -> i128 { e.storage().instance().get(&QK::Sup).unwrap() }
    pub fn balance(e: Env, id: Address) -> i128 { Self::bal(&e, &id) }
    pub fn transfer(e: Env, from: Address, to: MuxedAddress, amount: i128) { from.require_auth(); let to = to.address(); Self::setb(&e, &from, Self::bal(&e, &from) - amount); Self::setb(&e, &to, Self::bal(&e, &to) + amount); }
    pub fn deposit(e: Env, from: Address, amount: i128) -> i128 { from.require_auth(); token::TokenClient::new(&e, &Self::xlm(e.clone())).transfer(&from, &e.current_contract_address(), &amount); Self::setb(&e, &from, Self::bal(&e, &from) + amount); let s = Self::total_supply(e.clone()); e.storage().instance().set(&QK::Sup, &(s + amount)); amount }
    pub fn redeem(e: Env, from: Address, amount: i128) -> i128 { from.require_auth(); Self::setb(&e, &from, Self::bal(&e, &from) - amount); token::TokenClient::new(&e, &Self::xlm(e.clone())).transfer(&e.current_contract_address(), &from, &amount); let s = Self::total_supply(e.clone()); e.storage().instance().set(&QK::Sup, &(s - amount)); amount }
    /// test hook: burn supply-side XLM to simulate a backing shortfall
    pub fn leak(e: Env, to: Address, amount: i128) { token::TokenClient::new(&e, &Self::xlm(e.clone())).transfer(&e.current_contract_address(), &to, &amount); }
}

#[contracttype] #[derive(Clone)] enum BK { Xlm, Sup(Address) }
#[contract] pub struct MockBlend;
#[contractimpl] impl MockBlend {
    pub fn __constructor(e: Env, xlm: Address) { e.storage().instance().set(&BK::Xlm, &xlm); }
    pub fn submit(e: Env, from: Address, spender: Address, to: Address, requests: Vec<Request>) -> Positions {
        from.require_auth(); spender.require_auth();
        let x = token::TokenClient::new(&e, &e.storage().instance().get::<_, Address>(&BK::Xlm).unwrap());
        for r in requests.iter() {
            let k = BK::Sup(from.clone()); let s: i128 = e.storage().instance().get(&k).unwrap_or(0);
            if r.request_type == 0 { x.transfer(&spender, &e.current_contract_address(), &r.amount); e.storage().instance().set(&k, &(s + r.amount)); }
            else { let a = if r.amount > s { s } else { r.amount }; x.transfer(&e.current_contract_address(), &to, &a); e.storage().instance().set(&k, &(s - a)); }
        }
        Self::get_positions(e, from)
    }
    pub fn get_positions(e: Env, address: Address) -> Positions {
        let s: i128 = e.storage().instance().get(&BK::Sup(address)).unwrap_or(0);
        let mut m = Map::new(&e); m.set(0u32, s);
        Positions { liabilities: Map::new(&e), collateral: Map::new(&e), supply: m }
    }
}

struct W<'a> { e: Env, xlm: token::StellarAssetClient<'a>, xt: token::TokenClient<'a>, qfx: MockQfxClient<'a>, vault: StakingVaultClient<'a>, reserve: RewardReserveClient<'a>,
    router: FeeRouterClient<'a>, adapter: BlendAdapterClient<'a>, ins: InsuranceFundClient<'a>, peg: PegMonitorClient<'a>, blend: Address, keeper: Address, ops: Address, treasury: Address }

fn setup<'a>(cash_only: bool) -> W<'a> {
    let e = Env::default(); e.mock_all_auths();
    e.ledger().set_timestamp(1_000_000);
    let admin = Address::generate(&e); let keeper = Address::generate(&e); let ops = Address::generate(&e); let treasury = Address::generate(&e);
    let sac = e.register_stellar_asset_contract_v2(admin.clone());
    let xa = sac.address();
    let qa = e.register(MockQfx, (xa.clone(),));
    let blend = e.register(MockBlend, (xa.clone(),));
    let peg = e.register(beacons_peg_monitor::PegMonitor, (admin.clone(), keeper.clone(), qa.clone(), 100u32, 60u64));
    let vault = e.register(beacons_vault::StakingVault, (admin.clone(), qa.clone(), xa.clone(), peg.clone(), 300u64, true, 60u64));
    let ins = e.register(beacons_insurance_fund::InsuranceFund, (admin.clone(), xa.clone(), 100 * U, 60u64));
    let reserve = e.register(beacons_reward_reserve::RewardReserve, (admin.clone(), xa.clone(), vault.clone(), ins.clone(), treasury.clone(), 120u64, true, 60u64));
    let router = e.register(beacons_fee_router::FeeRouter, (admin.clone(), xa.clone(), reserve.clone(), ins.clone(), ops.clone(), 60u64));
    let adapter = e.register(beacons_blend_adapter::BlendAdapter, (admin.clone(), vault.clone(), blend.clone(), xa.clone(), cash_only, 60u64));
    let v = StakingVaultClient::new(&e, &vault); v.wire(&adapter, &reserve);
    W { xlm: token::StellarAssetClient::new(&e, &xa), xt: token::TokenClient::new(&e, &xa), qfx: MockQfxClient::new(&e, &qa), vault: v,
        reserve: RewardReserveClient::new(&e, &reserve), router: FeeRouterClient::new(&e, &router), adapter: BlendAdapterClient::new(&e, &adapter),
        ins: InsuranceFundClient::new(&e, &ins), peg: PegMonitorClient::new(&e, &peg), blend, keeper, ops, treasury, e }
}
fn user(w: &W, qfx: i128) -> Address { let u = Address::generate(&w.e); w.xlm.mint(&u, &(qfx + 10 * U)); w.qfx.deposit(&u, &qfx); u }
fn tick(w: &W, s: u64) { let t = w.e.ledger().timestamp(); w.e.ledger().set_timestamp(t + s); }

#[test]
fn tiers_and_boosts() {
    assert_eq!(beacons_vault::tier_of(99 * U), 0);
    assert_eq!(beacons_vault::tier_of(100 * U), 1);
    assert_eq!(beacons_vault::tier_of(2_500 * U), 3);
    assert_eq!(beacons_vault::tier_of(250_000 * U), 6);
    assert_eq!(beacons_vault::boost_bps(30), Some(11_000));
    assert_eq!(beacons_vault::boost_bps(90), Some(12_500));
    assert_eq!(beacons_vault::boost_bps(7), None);
}

#[test]
fn stake_redeems_and_lends_40pct() {
    let w = setup(false);
    let a = user(&w, 2_500 * U);
    let p = w.vault.stake(&a, &(2_500 * U), &0);
    assert_eq!(p.tier, 3);
    assert_eq!(w.qfx.balance(&a), 0);
    assert_eq!(w.qfx.balance(&w.vault.address), 0); // redeemed to XLM
    assert_eq!(w.xt.balance(&w.vault.address), 1_500 * U); // 60% cash
    assert_eq!(w.xt.balance(&w.blend), 1_000 * U); // 40% on Blend
    assert_eq!(w.adapter.supplied(), 1_000 * U);
}

#[test]
fn cash_only_mode_holds_idle() {
    let w = setup(true);
    let a = user(&w, 1_000 * U);
    w.vault.stake(&a, &(1_000 * U), &0);
    assert_eq!(w.xt.balance(&w.adapter.address), 400 * U);
    assert_eq!(w.xt.balance(&w.blend), 0);
}

#[test]
fn router_splits_40_10_50() {
    let w = setup(false);
    let src = Address::generate(&w.e); w.xlm.mint(&src, &(1_000 * U));
    let (s, i, o) = w.router.deposit_fees(&src, &(1_000 * U), &symbol_short!("TESTFEE"));
    assert_eq!((s, i, o), (400 * U, 100 * U, 500 * U));
    assert_eq!(w.xt.balance(&w.reserve.address), 400 * U);
    assert_eq!(w.ins.balance(), 100 * U);
    assert_eq!(w.xt.balance(&w.ops), 500 * U);
    assert_eq!(w.router.routed_by_source(&symbol_short!("TESTFEE")), 1_000 * U);
}

#[test]
fn warmup_then_flat_share_with_boost_and_xlm_payout() {
    let w = setup(false);
    let a = user(&w, 1_000 * U); let b = user(&w, 1_000 * U);
    w.vault.stake(&a, &(1_000 * U), &0);
    w.vault.stake(&b, &(1_000 * U), &90);
    let src = Address::generate(&w.e); w.xlm.mint(&src, &(100_000 * U));
    w.router.deposit_fees(&src, &(10 * U), &symbol_short!("TESTFEE")); // 4 XLM to reserve
    // cycle 1: nobody eligible yet (warm-up) -> no payout
    tick(&w, 120); let r = w.reserve.run_cycle();
    assert_eq!(r.payout, 0); assert_eq!(r.cycle, 1);
    assert_eq!(w.vault.total_weight(), 2_250 * U);
    // too early
    assert!(w.reserve.try_run_cycle().is_err());
    tick(&w, 120); let r = w.reserve.run_cycle();
    assert!(r.payout > 0);
    let pa = w.vault.position(&a).unwrap().claimable; let pb = w.vault.position(&b).unwrap().claimable;
    assert!(pb > pa); // 1.25x boost
    assert!((pb * 10_000 / pa - 12_500).abs() <= 1);
    let before = w.xt.balance(&a);
    assert_eq!(w.vault.claim(&a), pa);
    assert_eq!(w.xt.balance(&a) - before, pa); // paid in XLM
}

#[test]
fn reserve_divisor_and_apr_cap_and_overflow() {
    let w = setup(false);
    let a = user(&w, 10_000 * U); w.vault.stake(&a, &(10_000 * U), &0);
    tick(&w, 120); w.reserve.run_cycle(); // activate
    let src = Address::generate(&w.e); w.xlm.mint(&src, &(1_000_000 * U));
    // tiny balance: payout = balance/42 (below cap)
    w.router.deposit_fees(&src, &(U / 10), &symbol_short!("TESTFEE"));
    let bal = w.xt.balance(&w.reserve.address);
    tick(&w, 120); let r = w.reserve.run_cycle();
    let cap = 10_000 * U * 2_500 * 120 / (10_000 * 31_536_000);
    assert_eq!(r.payout, core::cmp::min(bal / 42, cap));
    // big balance: capped at 25% APR of principal for one 120 s cycle
    w.router.deposit_fees(&src, &(100_000 * U), &symbol_short!("TESTFEE"));
    tick(&w, 120); let r = w.reserve.run_cycle();
    assert_eq!(r.payout, cap);
    // overflow beyond 30 days of capped payouts went to insurance (to target) then treasury
    assert_eq!(r.to_insurance, 0); assert!(r.to_treasury > 0); // insurance already above target from its 10% router share
    assert!(w.ins.balance() >= 100 * U);
    assert_eq!(w.xt.balance(&w.treasury), r.to_treasury);
}

#[test]
fn unbond_fifo_with_2pct_cap_and_remint() {
    let w = setup(false);
    let a = user(&w, 10_000 * U); let b = user(&w, 100 * U); let c = user(&w, 100 * U);
    w.vault.stake(&a, &(10_000 * U), &0); w.vault.stake(&b, &(100 * U), &0); w.vault.stake(&c, &(100 * U), &30);
    tick(&w, 120); w.reserve.run_cycle();
    assert!(w.vault.try_unbond(&c).is_err()); // 30-day lock
    w.vault.unbond(&b);
    assert_eq!(w.vault.process_exits(), 0); // still unbonding
    tick(&w, 300);
    // cap = 2% of 10,200 = 204 -> b fully paid
    assert_eq!(w.vault.process_exits(), 100 * U);
    assert_eq!(w.qfx.balance(&b), 100 * U); // re-minted QFX
    assert_eq!(w.vault.position(&b).unwrap().status, Status::Closed);
    tick(&w, 30 * 86_400);
    w.vault.unbond(&a); w.vault.unbond(&c);
    tick(&w, 300);
    // budget left this cycle = 204 - 100 = 104 -> partial fill of a, c waits (FIFO)
    w.vault.process_exits();
    assert_eq!(w.vault.position(&a).unwrap().amount, 10_000 * U - 104 * U);
    assert_eq!(w.qfx.balance(&c), 0);
    tick(&w, 120); w.reserve.run_cycle();
    let paid = w.vault.process_exits(); // new cycle budget: 2% of 10,000
    assert_eq!(paid, (10_000 * U - 104 * U + 100 * U) * 200 / 10_000);
    // needed cash beyond buffer pulled from Blend
    let st = w.vault.state(); assert!(st.cash >= 0);
}

#[test]
fn exits_pull_from_blend_when_cash_short() {
    let w = setup(false);
    let a = user(&w, 1_000 * U); w.vault.stake(&a, &(1_000 * U), &0);
    let b = user(&w, 100_000 * U); w.vault.stake(&b, &(100_000 * U), &0);
    tick(&w, 120); w.reserve.run_cycle();
    w.vault.unbond(&b); tick(&w, 300);
    let blend_before = w.xt.balance(&w.blend);
    // drain cash path: pay out across many cycles until cash buffer exhausted
    for _ in 0..60 { w.vault.process_exits(); tick(&w, 120); w.reserve.run_cycle(); }
    assert!(w.xt.balance(&w.blend) < blend_before);
}

#[test]
fn peg_monitor_pauses_new_stakes() {
    let w = setup(false);
    let a = user(&w, 500 * U); let _holder = user(&w, 1_000 * U);
    w.peg.report_price(&(10_050_000)); // 0.5% drift: fine
    assert!(w.peg.healthy());
    w.peg.report_price(&(9_800_000)); // 2% drift: trip
    assert!(!w.peg.healthy());
    assert!(w.vault.try_stake(&a, &(500 * U), &0).is_err());
    assert!(w.peg.try_resume().is_err()); // still drifting
    w.peg.report_price(&(10_000_000));
    w.peg.resume();
    w.vault.stake(&a, &(500 * U), &0);
    // backing shortfall also trips
    w.qfx.leak(&w.keeper, &(100 * U));
    let st = w.peg.check();
    assert!(st.tripped); assert_eq!(st.reason, 1);
}

#[test]
fn one_position_per_address_and_minimum() {
    let w = setup(false);
    let a = user(&w, 1_000 * U);
    assert!(w.vault.try_stake(&a, &(99 * U), &0).is_err());
    assert!(w.vault.try_stake(&a, &(100 * U), &7).is_err());
    w.vault.stake(&a, &(500 * U), &0);
    assert!(w.vault.try_stake(&a, &(100 * U), &0).is_err());
}
