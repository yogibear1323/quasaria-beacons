//! Quasaria Beacons — fee router (TESTNET, UNAUDITED).
//! Splits XLM fees: 40% reward reserve (stakers), 10% insurance, 50% ops.
//! On testnet the Quasaria DEX has no protocol fee share, so the router is
//! fed by a clearly labelled test fee source; every deposit carries a
//! `source` tag that is recorded per source.
#![no_std]
use quasaria_gov as gov;
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, panic_with_error, token, Address, BytesN, Env, Symbol};

#[contracttype]
#[derive(Clone)]
enum Key { Xlm, Reserve, Insurance, Ops, Split, Total, BySource(Symbol) }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Split { pub stakers_bps: u32, pub insurance_bps: u32, pub ops_bps: u32 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouterAction { SetSplit(Split), SetOps(Address), Upgrade(BytesN<32>), SetDelay(u64), SetGuardian(Address) }

impl gov::TimelockAction for RouterAction {
    fn delay_class(&self) -> gov::DelayClass {
        match self { RouterAction::Upgrade(_) | RouterAction::SetDelay(_) | RouterAction::SetOps(_) => gov::DelayClass::Critical, _ => gov::DelayClass::Standard }
    }
    fn validate(&self, e: &Env) { if let RouterAction::SetSplit(s) = self { check(e, s) } }
}

fn check(e: &Env, s: &Split) { if s.stakers_bps + s.insurance_bps + s.ops_bps != 10_000 { panic_with_error!(e, gov::GovError::MathOverflow); } }

#[contractevent(topics = ["fees_routed"])]
pub struct FeesRouted { #[topic] pub source: Symbol, pub from: Address, pub amount: i128, pub stakers: i128, pub insurance: i128, pub ops: i128 }

#[contract]
pub struct FeeRouter;
gov::governance_entrypoints!(FeeRouter, RouterAction);
gov::pause_entrypoints!(FeeRouter);

#[contractimpl]
impl FeeRouter {
    pub fn __constructor(e: Env, admin: Address, xlm: Address, reserve: Address, insurance: Address, ops: Address, timelock_delay: u64) {
        gov::init(&e, &admin, timelock_delay);
        let s = e.storage().instance();
        s.set(&Key::Xlm, &xlm); s.set(&Key::Reserve, &reserve); s.set(&Key::Insurance, &insurance); s.set(&Key::Ops, &ops);
        s.set(&Key::Split, &Split { stakers_bps: 4_000, insurance_bps: 1_000, ops_bps: 5_000 });
        s.set(&Key::Total, &0i128);
    }

    /// Pull `amount` XLM from `from` and split it. Returns (stakers, insurance, ops).
    pub fn deposit_fees(e: Env, from: Address, amount: i128, source: Symbol) -> (i128, i128, i128) {
        from.require_auth();
        gov::when_not_paused(&e);
        if amount <= 0 { panic_with_error!(&e, gov::GovError::MathOverflow); }
        let s = e.storage().instance();
        let sp: Split = s.get(&Key::Split).unwrap();
        let x = token::TokenClient::new(&e, &s.get::<_, Address>(&Key::Xlm).unwrap());
        let st = amount * sp.stakers_bps as i128 / 10_000;
        let ins = amount * sp.insurance_bps as i128 / 10_000;
        let ops = amount - st - ins;
        let r: Address = s.get(&Key::Reserve).unwrap();
        let i: Address = s.get(&Key::Insurance).unwrap();
        let o: Address = s.get(&Key::Ops).unwrap();
        if st > 0 { x.transfer(&from, &r, &st); }
        if ins > 0 { x.transfer(&from, &i, &ins); }
        if ops > 0 { x.transfer(&from, &o, &ops); }
        let t: i128 = s.get(&Key::Total).unwrap();
        s.set(&Key::Total, &(t + amount));
        let k = Key::BySource(source.clone());
        let b: i128 = e.storage().persistent().get(&k).unwrap_or(0);
        e.storage().persistent().set(&k, &(b + amount));
        gov::bump_instance(&e);
        FeesRouted { source, from, amount, stakers: st, insurance: ins, ops }.publish(&e);
        (st, ins, ops)
    }

    pub fn split(e: Env) -> Split { e.storage().instance().get(&Key::Split).unwrap() }
    pub fn total_routed(e: Env) -> i128 { e.storage().instance().get(&Key::Total).unwrap() }
    pub fn routed_by_source(e: Env, source: Symbol) -> i128 { e.storage().persistent().get(&Key::BySource(source)).unwrap_or(0) }
    pub fn destinations(e: Env) -> (Address, Address, Address) {
        let s = e.storage().instance();
        (s.get(&Key::Reserve).unwrap(), s.get(&Key::Insurance).unwrap(), s.get(&Key::Ops).unwrap())
    }

    pub fn execute_action(e: Env, action: RouterAction) {
        gov::consume(&e, &action);
        match action {
            RouterAction::SetSplit(sp) => { check(&e, &sp); e.storage().instance().set(&Key::Split, &sp) }
            RouterAction::SetOps(o) => e.storage().instance().set(&Key::Ops, &o),
            RouterAction::Upgrade(h) => gov::upgrade_now(&e, &h),
            RouterAction::SetDelay(d) => gov::set_delay_now(&e, d),
            RouterAction::SetGuardian(g) => gov::set_guardian_now(&e, &g),
        }
    }
}
