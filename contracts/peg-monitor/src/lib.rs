//! Quasaria Beacons — peg monitor (TESTNET, UNAUDITED).
//!
//! Watches the QFX/XLM peg two ways and trips a breaker at > `threshold_bps`
//! (default 100 = 1%) drift:
//! * on-chain backing: XLM held by the QFX contract / QFX total supply
//!   (permissionless `check`), and
//! * market price: XLM per QFX reported by the keeper (`report_price`), which
//!   the keeper derives off-chain from the DEX QFX/QUSD and XLM/QUSD pools.
//! While tripped, the staking vault refuses new stakes and upgrades. Exits
//! and claims are never blocked. Clearing the breaker needs the admin and a
//! healthy last reading.
#![no_std]
use quasaria_gov as gov;
use soroban_sdk::{contract, contractclient, contractimpl, contracttype, panic_with_error, contracterror, token, Address, BytesN, Env};

pub const ONE: i128 = 10_000_000; // 1.0 at 7 decimals

#[contractclient(name = "QfxReadClient")]
pub trait QfxRead {
    fn total_supply(env: Env) -> i128;
    fn xlm(env: Env) -> Address;
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PegError { BadPrice = 1, StillDrifting = 2, BadParam = 3 }

#[contracttype]
#[derive(Clone)]
enum Key { Qfx, Keeper, Threshold, Price, PriceTs, Backing, Tripped, TripReason }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PegAction { SetKeeper(Address), SetThreshold(u32), Upgrade(BytesN<32>), SetDelay(u64), SetGuardian(Address) }

impl gov::TimelockAction for PegAction {
    fn delay_class(&self) -> gov::DelayClass {
        match self { PegAction::Upgrade(_) | PegAction::SetDelay(_) => gov::DelayClass::Critical, _ => gov::DelayClass::Standard }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PegStatus {
    pub healthy: bool,
    pub tripped: bool,
    pub reason: u32, // 0 none, 1 backing, 2 market price
    pub market_price: i128, // XLM per QFX, 7 decimals
    pub market_ts: u64,
    pub backing: i128, // XLM backing per QFX, 7 decimals
    pub threshold_bps: u32,
}

#[contract]
pub struct PegMonitor;

gov::governance_entrypoints!(PegMonitor, PegAction);

fn drift_bps(p: i128) -> i128 { let d = if p > ONE { p - ONE } else { ONE - p }; d * 10_000 / ONE }
fn get<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(e: &Env, k: &Key) -> Option<T> { e.storage().instance().get(k) }

fn evaluate(e: &Env, value: i128, reason: u32) {
    let th: u32 = get(e, &Key::Threshold).unwrap();
    if drift_bps(value) > th as i128 {
        e.storage().instance().set(&Key::Tripped, &true);
        e.storage().instance().set(&Key::TripReason, &reason);
    }
}

#[contractimpl]
impl PegMonitor {
    pub fn __constructor(e: Env, admin: Address, keeper: Address, qfx: Address, threshold_bps: u32, timelock_delay: u64) {
        if threshold_bps == 0 || threshold_bps > 1_000 { panic_with_error!(&e, PegError::BadParam); }
        gov::init(&e, &admin, timelock_delay);
        let s = e.storage().instance();
        s.set(&Key::Qfx, &qfx); s.set(&Key::Keeper, &keeper); s.set(&Key::Threshold, &threshold_bps);
        s.set(&Key::Price, &ONE); s.set(&Key::PriceTs, &0u64); s.set(&Key::Backing, &ONE);
        s.set(&Key::Tripped, &false); s.set(&Key::TripReason, &0u32);
        gov::bump_instance(&e);
    }

    /// Permissionless: read QFX backing on-chain and trip if it drifts.
    pub fn check(e: Env) -> PegStatus {
        let qfx: Address = get(&e, &Key::Qfx).unwrap();
        let q = QfxReadClient::new(&e, &qfx);
        let supply = q.total_supply();
        let reserve = token::TokenClient::new(&e, &q.xlm()).balance(&qfx);
        let backing = if supply == 0 { ONE } else { reserve * ONE / supply };
        e.storage().instance().set(&Key::Backing, &backing);
        // Over-backing (surplus) is not a depeg risk; only under-backing trips.
        evaluate(&e, if backing > ONE { ONE } else { backing }, 1);
        gov::bump_instance(&e);
        Self::status(e)
    }

    /// Keeper: report the market price (XLM per QFX, 7 decimals).
    pub fn report_price(e: Env, price: i128) -> PegStatus {
        let k: Address = get(&e, &Key::Keeper).unwrap();
        k.require_auth();
        if price <= 0 { panic_with_error!(&e, PegError::BadPrice); }
        e.storage().instance().set(&Key::Price, &price);
        e.storage().instance().set(&Key::PriceTs, &e.ledger().timestamp());
        evaluate(&e, price, 2);
        gov::bump_instance(&e);
        Self::status(e)
    }

    /// Guardian or admin: trip manually.
    pub fn trip(e: Env, caller: Address) {
        caller.require_auth();
        if caller != gov::admin(&e) && caller != gov::guardian(&e) { panic_with_error!(&e, gov::GovError::NotGuardian); }
        e.storage().instance().set(&Key::Tripped, &true);
        e.storage().instance().set(&Key::TripReason, &3u32);
    }

    /// Admin: clear the breaker once both readings are back inside the band.
    pub fn resume(e: Env) {
        gov::require_admin(&e);
        let th: u32 = get(&e, &Key::Threshold).unwrap();
        let p: i128 = get(&e, &Key::Price).unwrap();
        let b: i128 = get(&e, &Key::Backing).unwrap();
        if drift_bps(p) > th as i128 || drift_bps(if b > ONE { ONE } else { b }) > th as i128 {
            panic_with_error!(&e, PegError::StillDrifting);
        }
        e.storage().instance().set(&Key::Tripped, &false);
        e.storage().instance().set(&Key::TripReason, &0u32);
    }

    pub fn healthy(e: Env) -> bool { !get::<bool>(&e, &Key::Tripped).unwrap() }

    pub fn status(e: Env) -> PegStatus {
        let tripped: bool = get(&e, &Key::Tripped).unwrap();
        PegStatus {
            healthy: !tripped, tripped,
            reason: get(&e, &Key::TripReason).unwrap(),
            market_price: get(&e, &Key::Price).unwrap(),
            market_ts: get(&e, &Key::PriceTs).unwrap(),
            backing: get(&e, &Key::Backing).unwrap(),
            threshold_bps: get(&e, &Key::Threshold).unwrap(),
        }
    }

    pub fn execute_action(e: Env, action: PegAction) {
        gov::consume(&e, &action);
        match action {
            PegAction::SetKeeper(k) => e.storage().instance().set(&Key::Keeper, &k),
            PegAction::SetThreshold(t) => { if t == 0 || t > 1_000 { panic_with_error!(&e, PegError::BadParam); } e.storage().instance().set(&Key::Threshold, &t) }
            PegAction::Upgrade(h) => gov::upgrade_now(&e, &h),
            PegAction::SetDelay(d) => gov::set_delay_now(&e, d),
            PegAction::SetGuardian(g) => gov::set_guardian_now(&e, &g),
        }
    }
}
