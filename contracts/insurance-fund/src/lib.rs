//! Quasaria Beacons — insurance fund (TESTNET, UNAUDITED).
//! Holds XLM from the fee router (10%) and reward-reserve overflow. Payouts
//! only through a Critical timelocked action. Reports its target so the
//! reserve knows when to send overflow to the treasury instead.
#![no_std]
use quasaria_gov as gov;
use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, token, Address, BytesN, Env};

#[contracttype]
#[derive(Clone)]
enum Key { Xlm, Target, Paid }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InsAction { Payout(Address, i128), SetTarget(i128), Upgrade(BytesN<32>), SetDelay(u64), SetGuardian(Address) }

impl gov::TimelockAction for InsAction {
    fn delay_class(&self) -> gov::DelayClass {
        match self { InsAction::SetTarget(_) | InsAction::SetGuardian(_) => gov::DelayClass::Standard, _ => gov::DelayClass::Critical }
    }
}

#[contract]
pub struct InsuranceFund;
gov::governance_entrypoints!(InsuranceFund, InsAction);

#[contractimpl]
impl InsuranceFund {
    pub fn __constructor(e: Env, admin: Address, xlm: Address, target: i128, timelock_delay: u64) {
        gov::init(&e, &admin, timelock_delay);
        e.storage().instance().set(&Key::Xlm, &xlm);
        e.storage().instance().set(&Key::Target, &target);
        e.storage().instance().set(&Key::Paid, &0i128);
    }
    pub fn balance(e: Env) -> i128 {
        let x: Address = e.storage().instance().get(&Key::Xlm).unwrap();
        token::TokenClient::new(&e, &x).balance(&e.current_contract_address())
    }
    pub fn target(e: Env) -> i128 { e.storage().instance().get(&Key::Target).unwrap() }
    pub fn below_target(e: Env) -> i128 { let t = Self::target(e.clone()); let b = Self::balance(e); if t > b { t - b } else { 0 } }
    pub fn total_paid(e: Env) -> i128 { e.storage().instance().get(&Key::Paid).unwrap() }
    pub fn execute_action(e: Env, action: InsAction) {
        gov::consume(&e, &action);
        match action {
            InsAction::Payout(to, amt) => {
                if amt <= 0 { panic_with_error!(&e, gov::GovError::MathOverflow); }
                let x: Address = e.storage().instance().get(&Key::Xlm).unwrap();
                token::TokenClient::new(&e, &x).transfer(&e.current_contract_address(), &to, &amt);
                let p: i128 = e.storage().instance().get(&Key::Paid).unwrap();
                e.storage().instance().set(&Key::Paid, &(p + amt));
            }
            InsAction::SetTarget(t) => e.storage().instance().set(&Key::Target, &t),
            InsAction::Upgrade(h) => gov::upgrade_now(&e, &h),
            InsAction::SetDelay(d) => gov::set_delay_now(&e, d),
            InsAction::SetGuardian(g) => gov::set_guardian_now(&e, &g),
        }
        gov::bump_instance(&e);
    }
}
