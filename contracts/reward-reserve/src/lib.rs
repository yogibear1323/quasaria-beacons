//! Quasaria Beacons — reward reserve (TESTNET, UNAUDITED).
//! Receives the stakers' 40% of fees (XLM). Each cycle (4h; a shorter cycle
//! only with the labelled demo flag) the keeper calls `run_cycle`, which
//! pays min(balance/42, 25%-APR cap on staked principal) to the vault.
//! Anything above 30 days of capped payouts is overflow: it goes to the
//! insurance fund until it reaches target, then to the treasury.
#![no_std]
use quasaria_gov as gov;
use soroban_sdk::{contract, contractclient, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token, Address, BytesN, Env};

pub const YEAR: i128 = 31_536_000;
pub const PROD_CYCLE: u64 = 14_400;

#[contractclient(name = "VaultClient")]
pub trait Vault { fn total_weight(env: Env) -> i128; fn total_principal(env: Env) -> i128; fn notify_reward(env: Env, amount: i128) -> u32; }
#[contractclient(name = "InsClient")]
pub trait Ins { fn below_target(env: Env) -> i128; }

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReserveError { TooEarly = 1, BadTiming = 2 }

#[contracttype]
#[derive(Clone)]
enum Key { Xlm, Vault, Ins, Treasury, CycleSecs, Demo, Last, Divisor, AprCap, Paid, ToIns, ToTreasury }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CycleResult { pub cycle: u32, pub payout: i128, pub to_insurance: i128, pub to_treasury: i128, pub balance_after: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReserveState { pub balance: i128, pub cycle_secs: u64, pub demo_timing: bool, pub last_cycle: u64, pub next_cycle: u64,
    pub divisor: u32, pub apr_cap_bps: u32, pub next_payout: i128, pub total_paid: i128, pub total_to_insurance: i128, pub total_to_treasury: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReserveAction { SetCycle(u64, bool), SetTreasury(Address), Upgrade(BytesN<32>), SetDelay(u64), SetGuardian(Address) }
impl gov::TimelockAction for ReserveAction {
    fn delay_class(&self) -> gov::DelayClass { match self { ReserveAction::SetCycle(_, _) | ReserveAction::SetGuardian(_) => gov::DelayClass::Standard, _ => gov::DelayClass::Critical } }
}

#[contractevent(topics = ["reward_cycle"])]
pub struct RewardCycle { #[topic] pub cycle: u32, pub payout: i128, pub to_insurance: i128, pub to_treasury: i128 }

#[contract]
pub struct RewardReserve;
gov::governance_entrypoints!(RewardReserve, ReserveAction);
gov::pause_entrypoints!(RewardReserve);

fn i<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(e: &Env, k: &Key) -> T { e.storage().instance().get(k).unwrap() }
fn add(e: &Env, k: &Key, d: i128) { let v: i128 = i(e, k); e.storage().instance().set(k, &(v + d)) }

fn plan(e: &Env) -> (i128, i128, i128) {
    let x = token::TokenClient::new(e, &i::<Address>(e, &Key::Xlm));
    let bal = x.balance(&e.current_contract_address());
    let v = VaultClient::new(e, &i(e, &Key::Vault));
    let secs: u64 = i(e, &Key::CycleSecs);
    let cap_cycle = v.total_principal() * i::<u32>(e, &Key::AprCap) as i128 * secs as i128 / (10_000 * YEAR);
    let mut payout = bal / i::<u32>(e, &Key::Divisor) as i128;
    if payout > cap_cycle { payout = cap_cycle; }
    if v.total_weight() == 0 { payout = 0; }
    let horizon = (30 * 86_400 / secs) as i128;
    let keep = cap_cycle * horizon;
    let overflow = bal - payout - keep;
    (bal, payout, if overflow > 0 { overflow } else { 0 })
}

#[contractimpl]
impl RewardReserve {
    pub fn __constructor(e: Env, admin: Address, xlm: Address, vault: Address, insurance: Address, treasury: Address, cycle_secs: u64, demo_timing: bool, timelock_delay: u64) {
        if !demo_timing && cycle_secs != PROD_CYCLE { panic_with_error!(&e, ReserveError::BadTiming); }
        gov::init(&e, &admin, timelock_delay);
        let s = e.storage().instance();
        s.set(&Key::Xlm, &xlm); s.set(&Key::Vault, &vault); s.set(&Key::Ins, &insurance); s.set(&Key::Treasury, &treasury);
        s.set(&Key::CycleSecs, &cycle_secs); s.set(&Key::Demo, &demo_timing); s.set(&Key::Last, &e.ledger().timestamp());
        s.set(&Key::Divisor, &42u32); s.set(&Key::AprCap, &2_500u32);
        s.set(&Key::Paid, &0i128); s.set(&Key::ToIns, &0i128); s.set(&Key::ToTreasury, &0i128);
    }

    /// Permissionless keeper entry point, once per cycle.
    pub fn run_cycle(e: Env) -> CycleResult {
        gov::when_not_paused(&e);
        let now = e.ledger().timestamp();
        let last: u64 = i(&e, &Key::Last);
        if now < last + i::<u64>(&e, &Key::CycleSecs) { panic_with_error!(&e, ReserveError::TooEarly); }
        e.storage().instance().set(&Key::Last, &now);
        let (_bal, payout, overflow) = plan(&e);
        let me = e.current_contract_address();
        let x = token::TokenClient::new(&e, &i::<Address>(&e, &Key::Xlm));
        let vault: Address = i(&e, &Key::Vault);
        let (mut to_ins, mut to_tr) = (0i128, 0i128);
        if overflow > 0 {
            let ins: Address = i(&e, &Key::Ins);
            let room = InsClient::new(&e, &ins).below_target();
            to_ins = if overflow < room { overflow } else { room };
            to_tr = overflow - to_ins;
            if to_ins > 0 { x.transfer(&me, &ins, &to_ins); }
            if to_tr > 0 { x.transfer(&me, &i::<Address>(&e, &Key::Treasury), &to_tr); }
        }
        if payout > 0 { x.transfer(&me, &vault, &payout); }
        let cycle = VaultClient::new(&e, &vault).notify_reward(&payout);
        add(&e, &Key::Paid, payout); add(&e, &Key::ToIns, to_ins); add(&e, &Key::ToTreasury, to_tr);
        gov::bump_instance(&e);
        RewardCycle { cycle, payout, to_insurance: to_ins, to_treasury: to_tr }.publish(&e);
        CycleResult { cycle, payout, to_insurance: to_ins, to_treasury: to_tr, balance_after: x.balance(&me) }
    }

    pub fn state(e: Env) -> ReserveState {
        let (bal, payout, _) = plan(&e);
        let last: u64 = i(&e, &Key::Last); let secs: u64 = i(&e, &Key::CycleSecs);
        ReserveState { balance: bal, cycle_secs: secs, demo_timing: i(&e, &Key::Demo), last_cycle: last, next_cycle: last + secs,
            divisor: i(&e, &Key::Divisor), apr_cap_bps: i(&e, &Key::AprCap), next_payout: payout,
            total_paid: i(&e, &Key::Paid), total_to_insurance: i(&e, &Key::ToIns), total_to_treasury: i(&e, &Key::ToTreasury) }
    }

    pub fn execute_action(e: Env, action: ReserveAction) {
        gov::consume(&e, &action);
        match action {
            ReserveAction::SetCycle(secs, demo) => {
                if !demo && secs != PROD_CYCLE { panic_with_error!(&e, ReserveError::BadTiming); }
                if secs == 0 { panic_with_error!(&e, ReserveError::BadTiming); }
                e.storage().instance().set(&Key::CycleSecs, &secs); e.storage().instance().set(&Key::Demo, &demo);
            }
            ReserveAction::SetTreasury(t) => e.storage().instance().set(&Key::Treasury, &t),
            ReserveAction::Upgrade(h) => gov::upgrade_now(&e, &h),
            ReserveAction::SetDelay(d) => gov::set_delay_now(&e, d),
            ReserveAction::SetGuardian(g) => gov::set_guardian_now(&e, &g),
        }
    }
}
