//! Quasaria Beacons — staking vault (TESTNET, UNAUDITED).
//!
//! * Stake QFX. Tiers are labels by QFX amount (Spark 100 / Flare 500 /
//!   Beacon 2,500 / Pulsar 10,000 / Quasar 50,000 / Singularity 250,000);
//!   rewards are a flat per-QFX share, with lock boosts 30d 1.1x, 90d 1.25x.
//!   No per-wallet cap at launch.
//! * Option B: staked QFX is redeemed 1:1 for XLM through the existing QFX
//!   contract (`redeem`); 40% is lent on Blend via the adapter, 60% stays as
//!   a cash buffer. On exit the vault re-mints QFX via QFX `deposit` and
//!   returns QFX to the staker.
//! * Rewards are paid in XLM by the reward reserve once per cycle. New
//!   stakes warm up for one epoch (they start earning at the next cycle).
//! * Exits: 7-day unbonding (configurable for the testnet demo), FIFO queue,
//!   at most 2% of staked principal leaves per cycle (partial fills).
//! * New stakes and upgrades are blocked while the peg monitor is tripped.
//!   One position per address (testnet simplification).
#![no_std]
use quasaria_gov as gov;
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contractclient, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token, vec, Address, BytesN, Env, IntoVal, Symbol,
};

pub const SCALE: i128 = 1_000_000_000_000_000_000;
pub const UNIT: i128 = 10_000_000;
pub const LEND_BPS: i128 = 4_000;
pub const EXIT_CAP_BPS: i128 = 200;
pub const MAX_EXITS_PER_CALL: u32 = 20;

#[contractclient(name = "QfxClient")]
pub trait Qfx {
    fn deposit(env: Env, from: Address, amount: i128) -> i128;
    fn redeem(env: Env, from: Address, amount: i128) -> i128;
}
#[contractclient(name = "PegClient")]
pub trait Peg { fn healthy(env: Env) -> bool; }
#[contractclient(name = "AdapterClient")]
pub trait Adapter { fn supply(env: Env, amount: i128); fn withdraw(env: Env, amount: i128) -> i128; fn supplied(env: Env) -> i128; }

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError { BelowMinimum = 1, BadLock = 2, HasPosition = 3, NoPosition = 4, Locked = 5, NotActive = 6, PegTripped = 7, AlreadyWired = 8, NotWired = 9, BadTiming = 10 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status { Active, Unbonding, Closed }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub amount: i128,       // QFX principal still owed back
    pub weight: i128,       // amount * boost
    pub lock_days: u32,
    pub lock_until: u64,
    pub act_cycle: u32,     // first cycle that earns
    pub debt: i128,         // reward debt (valid once activated)
    pub activated: bool,
    pub claimable: i128,    // XLM rewards settled but unclaimed
    pub status: Status,
    pub ready_at: u64,
    pub tier: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultState {
    pub cycle: u32, pub total_weight: i128, pub pending_weight: i128, pub total_principal: i128,
    pub stakers: u32, pub cash: i128, pub lent: i128, pub rewards_owed: i128, pub rewards_paid: i128,
    pub queue_len: u32, pub queued_amount: i128, pub exit_budget_left: i128, pub unbond_secs: u64, pub demo_timing: bool,
}

#[contracttype]
#[derive(Clone)]
enum Key { Qfx, Xlm, Peg, Adapter, Reserve, Unbond, Demo, Cycle, Acc, TotalWeight, Principal, Stakers, Owed, Paid,
    ExitBase, Exited, QHead, QTail, QAmount, AccAt(u32), PendingW(u32), Pos(Address), Queue(u32) }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VaultAction { SetUnbond(u64, bool), Upgrade(BytesN<32>), SetDelay(u64), SetGuardian(Address) }
impl gov::TimelockAction for VaultAction {
    fn delay_class(&self) -> gov::DelayClass { match self { VaultAction::Upgrade(_) | VaultAction::SetDelay(_) => gov::DelayClass::Critical, _ => gov::DelayClass::Standard } }
}

#[contractevent(topics = ["staked"])]
pub struct Staked { #[topic] pub user: Address, pub amount: i128, pub lock_days: u32, pub tier: u32, pub act_cycle: u32 }
#[contractevent(topics = ["cycle"])]
pub struct CycleEvt { #[topic] pub cycle: u32, pub reward: i128, pub total_weight: i128 }
#[contractevent(topics = ["exit_paid"])]
pub struct ExitPaid { #[topic] pub user: Address, pub amount: i128, pub remaining: i128 }

#[contract]
pub struct StakingVault;
gov::governance_entrypoints!(StakingVault, VaultAction);
gov::pause_entrypoints!(StakingVault);

fn i<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(e: &Env, k: &Key) -> T { e.storage().instance().get(k).unwrap() }
fn set<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(e: &Env, k: &Key, v: &T) { e.storage().instance().set(k, v) }
fn p_get<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(e: &Env, k: &Key) -> Option<T> { e.storage().persistent().get(k) }
fn p_set<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(e: &Env, k: &Key, v: &T) { e.storage().persistent().set(k, v); gov::bump_persistent(e, k) }
fn add(e: &Env, k: &Key, d: i128) { let v: i128 = i(e, k); set(e, k, &(v + d)) }
fn xlm(e: &Env) -> token::TokenClient<'_> { token::TokenClient::new(e, &i::<Address>(e, &Key::Xlm)) }
fn qfx_tok(e: &Env) -> token::TokenClient<'_> { token::TokenClient::new(e, &i::<Address>(e, &Key::Qfx)) }

pub fn tier_of(amount: i128) -> u32 {
    let t = [100i128, 500, 2_500, 10_000, 50_000, 250_000];
    let mut tier = 0u32;
    for (n, th) in t.iter().enumerate() { if amount >= th * UNIT { tier = n as u32 + 1; } }
    tier
}
pub fn boost_bps(lock_days: u32) -> Option<i128> { match lock_days { 0 => Some(10_000), 30 => Some(11_000), 90 => Some(12_500), _ => None } }

fn acc_at(e: &Env, c: u32) -> i128 { p_get(e, &Key::AccAt(c)).unwrap_or(0) }

/// Bring a position's rewards up to date.
fn sync(e: &Env, p: &mut Position) {
    if p.status != Status::Active { return; }
    let cycle: u32 = i(e, &Key::Cycle);
    if cycle < p.act_cycle { return; }
    if !p.activated { p.debt = p.weight * acc_at(e, p.act_cycle) / SCALE; p.activated = true; }
    let acc: i128 = i(e, &Key::Acc);
    let accrued = p.weight * acc / SCALE;
    if accrued > p.debt { p.claimable += accrued - p.debt; p.debt = accrued; }
}

#[contractimpl]
impl StakingVault {
    /// `unbond_secs`: 604800 (7 days) in production timing; shorter only with `demo_timing = true`.
    pub fn __constructor(e: Env, admin: Address, qfx: Address, xlm: Address, peg: Address, unbond_secs: u64, demo_timing: bool, timelock_delay: u64) {
        if !demo_timing && unbond_secs != 604_800 { panic_with_error!(&e, VaultError::BadTiming); }
        gov::init(&e, &admin, timelock_delay);
        set(&e, &Key::Qfx, &qfx); set(&e, &Key::Xlm, &xlm); set(&e, &Key::Peg, &peg);
        set(&e, &Key::Unbond, &unbond_secs); set(&e, &Key::Demo, &demo_timing);
        set(&e, &Key::Cycle, &0u32); set(&e, &Key::Acc, &0i128);
        for k in [Key::TotalWeight, Key::Principal, Key::Owed, Key::Paid, Key::ExitBase, Key::Exited, Key::QAmount] { set(&e, &k, &0i128); }
        set(&e, &Key::Stakers, &0u32); set(&e, &Key::QHead, &0u32); set(&e, &Key::QTail, &0u32);
    }

    /// Admin, once: connect the Blend adapter and reward reserve.
    pub fn wire(e: Env, adapter: Address, reserve: Address) {
        gov::require_admin(&e);
        if e.storage().instance().has(&Key::Reserve) { panic_with_error!(&e, VaultError::AlreadyWired); }
        set(&e, &Key::Adapter, &adapter); set(&e, &Key::Reserve, &reserve);
    }

    pub fn stake(e: Env, user: Address, amount: i128, lock_days: u32) -> Position {
        user.require_auth();
        gov::when_not_paused(&e);
        if !e.storage().instance().has(&Key::Reserve) { panic_with_error!(&e, VaultError::NotWired); }
        if !PegClient::new(&e, &i(&e, &Key::Peg)).healthy() { panic_with_error!(&e, VaultError::PegTripped); }
        if amount < 100 * UNIT { panic_with_error!(&e, VaultError::BelowMinimum); }
        let boost = boost_bps(lock_days).unwrap_or_else(|| panic_with_error!(&e, VaultError::BadLock));
        let pk = Key::Pos(user.clone());
        if let Some(old) = p_get::<Position>(&e, &pk) { if old.status != Status::Closed || old.claimable > 0 { panic_with_error!(&e, VaultError::HasPosition); } }
        let me = e.current_contract_address();
        let qfx: Address = i(&e, &Key::Qfx);
        qfx_tok(&e).transfer(&user, &me, &amount);
        QfxClient::new(&e, &qfx).redeem(&me, &amount); // QFX -> XLM 1:1
        let lend = amount * LEND_BPS / 10_000;
        if lend > 0 {
            let ad: Address = i(&e, &Key::Adapter);
            xlm(&e).transfer(&me, &ad, &lend);
            AdapterClient::new(&e, &ad).supply(&lend);
        }
        let cycle: u32 = i(&e, &Key::Cycle);
        let weight = amount * boost / 10_000;
        let act = cycle + 1;
        let pw: i128 = p_get(&e, &Key::PendingW(act)).unwrap_or(0);
        p_set(&e, &Key::PendingW(act), &(pw + weight));
        add(&e, &Key::Principal, amount);
        let n: u32 = i(&e, &Key::Stakers); set(&e, &Key::Stakers, &(n + 1));
        let tier = tier_of(amount);
        let pos = Position { amount, weight, lock_days, lock_until: e.ledger().timestamp() + lock_days as u64 * 86_400, act_cycle: act,
            debt: 0, activated: false, claimable: 0, status: Status::Active, ready_at: 0, tier };
        p_set(&e, &pk, &pos);
        gov::bump_instance(&e);
        Staked { user, amount, lock_days, tier, act_cycle: act }.publish(&e);
        pos
    }

    /// Reward reserve only: `amount` XLM was just sent here for the cycle that
    /// is ending. Distributes it, then opens the next cycle.
    pub fn notify_reward(e: Env, amount: i128) -> u32 {
        let r: Address = i(&e, &Key::Reserve);
        r.require_auth();
        let tw: i128 = i(&e, &Key::TotalWeight);
        if amount > 0 && tw > 0 {
            let acc: i128 = i(&e, &Key::Acc);
            set(&e, &Key::Acc, &(acc + amount * SCALE / tw));
            add(&e, &Key::Owed, amount);
        } else if amount > 0 {
            // Nobody eligible: keep as surplus owed to nobody -> treat as owed to future stakers via next cycle.
            add(&e, &Key::Owed, 0);
        }
        let c: u32 = i::<u32>(&e, &Key::Cycle) + 1;
        set(&e, &Key::Cycle, &c);
        p_set(&e, &Key::AccAt(c), &i::<i128>(&e, &Key::Acc));
        let pw: i128 = p_get(&e, &Key::PendingW(c)).unwrap_or(0);
        let tw2 = tw + pw;
        set(&e, &Key::TotalWeight, &tw2);
        set(&e, &Key::ExitBase, &i::<i128>(&e, &Key::Principal));
        set(&e, &Key::Exited, &0i128);
        gov::bump_instance(&e);
        CycleEvt { cycle: c, reward: amount, total_weight: tw2 }.publish(&e);
        c
    }

    pub fn claim(e: Env, user: Address) -> i128 {
        user.require_auth();
        let pk = Key::Pos(user.clone());
        let mut p: Position = p_get(&e, &pk).unwrap_or_else(|| panic_with_error!(&e, VaultError::NoPosition));
        sync(&e, &mut p);
        let amt = p.claimable;
        if amt > 0 {
            p.claimable = 0;
            xlm(&e).transfer(&e.current_contract_address(), &user, &amt);
            add(&e, &Key::Owed, -amt); add(&e, &Key::Paid, amt);
        }
        p_set(&e, &pk, &p);
        amt
    }

    /// Start unbonding (after any lock). Stops earning; joins the FIFO exit queue.
    pub fn unbond(e: Env, user: Address) -> Position {
        user.require_auth();
        let pk = Key::Pos(user.clone());
        let mut p: Position = p_get(&e, &pk).unwrap_or_else(|| panic_with_error!(&e, VaultError::NoPosition));
        if p.status != Status::Active { panic_with_error!(&e, VaultError::NotActive); }
        if e.ledger().timestamp() < p.lock_until { panic_with_error!(&e, VaultError::Locked); }
        sync(&e, &mut p);
        if p.activated { add(&e, &Key::TotalWeight, -p.weight); }
        else { let k = Key::PendingW(p.act_cycle); let pw: i128 = p_get(&e, &k).unwrap_or(0); p_set(&e, &k, &(pw - p.weight)); }
        p.status = Status::Unbonding;
        p.ready_at = e.ledger().timestamp() + i::<u64>(&e, &Key::Unbond);
        let t: u32 = i(&e, &Key::QTail);
        p_set(&e, &Key::Queue(t), &user);
        set(&e, &Key::QTail, &(t + 1));
        add(&e, &Key::QAmount, p.amount);
        p_set(&e, &pk, &p);
        p
    }

    /// Permissionless (keeper): pay matured exits in FIFO order, up to 2% of
    /// principal per cycle. Returns total QFX paid out.
    pub fn process_exits(e: Env) -> i128 {
        let me = e.current_contract_address();
        let mut budget: i128 = i::<i128>(&e, &Key::ExitBase) * EXIT_CAP_BPS / 10_000 - i::<i128>(&e, &Key::Exited);
        let mut head: u32 = i(&e, &Key::QHead);
        let tail: u32 = i(&e, &Key::QTail);
        let now = e.ledger().timestamp();
        let mut paid_total = 0i128;
        let mut n = 0u32;
        while head < tail && budget > 0 && n < MAX_EXITS_PER_CALL {
            n += 1;
            let user: Address = p_get(&e, &Key::Queue(head)).unwrap();
            let pk = Key::Pos(user.clone());
            let mut p: Position = p_get(&e, &pk).unwrap();
            if p.ready_at > now { break; }
            let pay = if p.amount < budget { p.amount } else { budget };
            // Cash buffer first, then pull the rest from Blend.
            let cash = xlm(&e).balance(&me) - i::<i128>(&e, &Key::Owed);
            if cash < pay { AdapterClient::new(&e, &i(&e, &Key::Adapter)).withdraw(&(pay - cash)); }
            let qfx: Address = i(&e, &Key::Qfx);
            let xa: Address = i(&e, &Key::Xlm);
            e.authorize_as_current_contract(vec![&e, InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext { contract: xa, fn_name: Symbol::new(&e, "transfer"), args: (me.clone(), qfx.clone(), pay).into_val(&e) },
                sub_invocations: vec![&e],
            })]);
            QfxClient::new(&e, &qfx).deposit(&me, &pay); // re-mint QFX 1:1
            qfx_tok(&e).transfer(&me, &user, &pay);
            p.amount -= pay; budget -= pay; paid_total += pay;
            add(&e, &Key::Principal, -pay); add(&e, &Key::QAmount, -pay);
            if p.amount == 0 {
                p.status = Status::Closed; head += 1;
                let s: u32 = i(&e, &Key::Stakers); set(&e, &Key::Stakers, &(s - 1));
            }
            ExitPaid { user, amount: pay, remaining: p.amount }.publish(&e);
            p_set(&e, &pk, &p);
        }
        set(&e, &Key::QHead, &head);
        add(&e, &Key::Exited, paid_total);
        gov::bump_instance(&e);
        paid_total
    }

    // ----- views
    pub fn position(e: Env, user: Address) -> Option<Position> {
        let mut p: Position = p_get(&e, &Key::Pos(user))?;
        sync(&e, &mut p);
        Some(p)
    }
    pub fn total_weight(e: Env) -> i128 { i(&e, &Key::TotalWeight) }
    pub fn total_principal(e: Env) -> i128 { i(&e, &Key::Principal) }
    pub fn cycle(e: Env) -> u32 { i(&e, &Key::Cycle) }
    pub fn tier(_e: Env, amount: i128) -> u32 { tier_of(amount) }
    pub fn state(e: Env) -> VaultState {
        let me = e.current_contract_address();
        let owed: i128 = i(&e, &Key::Owed);
        let cycle: u32 = i(&e, &Key::Cycle);
        let lent = if e.storage().instance().has(&Key::Adapter) { AdapterClient::new(&e, &i(&e, &Key::Adapter)).supplied() } else { 0 };
        let qh: u32 = i(&e, &Key::QHead); let qt: u32 = i(&e, &Key::QTail);
        VaultState {
            cycle, total_weight: i(&e, &Key::TotalWeight), pending_weight: p_get(&e, &Key::PendingW(cycle + 1)).unwrap_or(0),
            total_principal: i(&e, &Key::Principal), stakers: i(&e, &Key::Stakers),
            cash: xlm(&e).balance(&me) - owed, lent, rewards_owed: owed, rewards_paid: i(&e, &Key::Paid),
            queue_len: qt - qh, queued_amount: i(&e, &Key::QAmount),
            exit_budget_left: i::<i128>(&e, &Key::ExitBase) * EXIT_CAP_BPS / 10_000 - i::<i128>(&e, &Key::Exited),
            unbond_secs: i(&e, &Key::Unbond), demo_timing: i(&e, &Key::Demo),
        }
    }

    pub fn execute_action(e: Env, action: VaultAction) {
        if let VaultAction::Upgrade(_) = action {
            if !PegClient::new(&e, &i(&e, &Key::Peg)).healthy() { panic_with_error!(&e, VaultError::PegTripped); }
        }
        gov::consume(&e, &action);
        match action {
            VaultAction::SetUnbond(secs, demo) => {
                if !demo && secs != 604_800 { panic_with_error!(&e, VaultError::BadTiming); }
                set(&e, &Key::Unbond, &secs); set(&e, &Key::Demo, &demo);
            }
            VaultAction::Upgrade(h) => gov::upgrade_now(&e, &h),
            VaultAction::SetDelay(d) => gov::set_delay_now(&e, d),
            VaultAction::SetGuardian(g) => gov::set_guardian_now(&e, &g),
        }
    }
}
