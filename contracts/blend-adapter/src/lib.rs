//! Quasaria Beacons — Blend adapter (TESTNET, UNAUDITED). Option B.
//! The vault sends the lend slice (40%) of redeemed XLM here; the adapter
//! supplies it to the Blend testnet XLM pool (non-collateral supply, request
//! type 0) and withdraws (type 1) back to the vault on exits. In cash-only
//! mode it simply holds the XLM idle (used if the Blend pool is unavailable).
#![no_std]
use quasaria_gov as gov;
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contractclient, contractimpl, contracttype, token, vec, Address, BytesN, Env, IntoVal, Map, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request { pub request_type: u32, pub address: Address, pub amount: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Positions { pub liabilities: Map<u32, i128>, pub collateral: Map<u32, i128>, pub supply: Map<u32, i128> }

#[contractclient(name = "BlendPoolClient")]
pub trait BlendPool {
    fn submit(env: Env, from: Address, spender: Address, to: Address, requests: Vec<Request>) -> Positions;
    fn get_positions(env: Env, address: Address) -> Positions;
}

#[contracttype]
#[derive(Clone)]
enum Key { Vault, Pool, Xlm, CashOnly, Supplied }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterAction { SetCashOnly(bool), Upgrade(BytesN<32>), SetDelay(u64), SetGuardian(Address) }
impl gov::TimelockAction for AdapterAction {
    fn delay_class(&self) -> gov::DelayClass { match self { AdapterAction::Upgrade(_) | AdapterAction::SetDelay(_) => gov::DelayClass::Critical, _ => gov::DelayClass::Standard } }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterState { pub cash_only: bool, pub pool: Address, pub supplied: i128, pub idle: i128, pub blend_btokens: i128 }

#[contract]
pub struct BlendAdapter;
gov::governance_entrypoints!(BlendAdapter, AdapterAction);

fn s_get<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(e: &Env, k: &Key) -> T { e.storage().instance().get(k).unwrap() }
fn xlm(e: &Env) -> token::TokenClient<'_> { token::TokenClient::new(e, &s_get::<Address>(e, &Key::Xlm)) }

#[contractimpl]
impl BlendAdapter {
    pub fn __constructor(e: Env, admin: Address, vault: Address, pool: Address, xlm: Address, cash_only: bool, timelock_delay: u64) {
        gov::init(&e, &admin, timelock_delay);
        let s = e.storage().instance();
        s.set(&Key::Vault, &vault); s.set(&Key::Pool, &pool); s.set(&Key::Xlm, &xlm); s.set(&Key::CashOnly, &cash_only); s.set(&Key::Supplied, &0i128);
    }

    /// Vault: XLM has already been transferred here; lend `amount` of it.
    pub fn supply(e: Env, amount: i128) {
        s_get::<Address>(&e, &Key::Vault).require_auth();
        let sup: i128 = s_get(&e, &Key::Supplied);
        e.storage().instance().set(&Key::Supplied, &(sup + amount));
        if s_get::<bool>(&e, &Key::CashOnly) { return; }
        let pool: Address = s_get(&e, &Key::Pool);
        let me = e.current_contract_address();
        let xa: Address = s_get(&e, &Key::Xlm);
        e.authorize_as_current_contract(vec![&e, InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext { contract: xa.clone(), fn_name: Symbol::new(&e, "transfer"), args: (me.clone(), pool.clone(), amount).into_val(&e) },
            sub_invocations: vec![&e],
        })]);
        BlendPoolClient::new(&e, &pool).submit(&me, &me, &me, &vec![&e, Request { request_type: 0, address: xa, amount }]);
        gov::bump_instance(&e);
    }

    /// Vault: return `amount` XLM to the vault (idle cash first, then Blend).
    /// Returns the amount actually sent.
    pub fn withdraw(e: Env, amount: i128) -> i128 {
        let vault: Address = s_get(&e, &Key::Vault);
        vault.require_auth();
        let me = e.current_contract_address();
        let x = xlm(&e);
        let idle = x.balance(&me);
        if idle < amount && !s_get::<bool>(&e, &Key::CashOnly) {
            let pool: Address = s_get(&e, &Key::Pool);
            BlendPoolClient::new(&e, &pool).submit(&me, &me, &me, &vec![&e, Request { request_type: 1, address: s_get(&e, &Key::Xlm), amount: amount - idle }]);
        }
        let have = x.balance(&me);
        let out = if have < amount { have } else { amount };
        if out > 0 { x.transfer(&me, &vault, &out); }
        let sup: i128 = s_get(&e, &Key::Supplied);
        e.storage().instance().set(&Key::Supplied, &(if sup > out { sup - out } else { 0 }));
        gov::bump_instance(&e);
        out
    }

    pub fn cash_only(e: Env) -> bool { s_get(&e, &Key::CashOnly) }
    pub fn supplied(e: Env) -> i128 { s_get(&e, &Key::Supplied) }
    pub fn state(e: Env) -> AdapterState {
        let me = e.current_contract_address();
        let pool: Address = s_get(&e, &Key::Pool);
        let cash_only: bool = s_get(&e, &Key::CashOnly);
        let bt = if cash_only { 0 } else { BlendPoolClient::new(&e, &pool).get_positions(&me).supply.get(0).unwrap_or(0) };
        AdapterState { cash_only, pool, supplied: s_get(&e, &Key::Supplied), idle: xlm(&e).balance(&me), blend_btokens: bt }
    }

    pub fn execute_action(e: Env, action: AdapterAction) {
        gov::consume(&e, &action);
        match action {
            AdapterAction::SetCashOnly(c) => e.storage().instance().set(&Key::CashOnly, &c),
            AdapterAction::Upgrade(h) => gov::upgrade_now(&e, &h),
            AdapterAction::SetDelay(d) => gov::set_delay_now(&e, d),
            AdapterAction::SetGuardian(g) => gov::set_guardian_now(&e, &g),
        }
    }
}
