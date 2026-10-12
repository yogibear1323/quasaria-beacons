//! # Quasaria governance helpers (shared library, not a contract)
//!
//! Linked into every Quasaria contract that has an admin. Provides:
//!
//! * **Two-step admin transfer**: `propose_admin(new)` by the current admin,
//!   then `accept_admin()` signed by the new admin (so the admin can be moved
//!   to a Stellar multisig account without risking a typo'd address).
//! * **Guardian pause**: the guardian (or admin) can `pause`; only the admin
//!   can `unpause`. Each contract decides which entry points are gated; exits
//!   (redeem, withdraw, unstake, close) are never gated.
//! * **Timelock**: every admin action that is not purely risk-reducing
//!   (including `SetGuardian`) is `propose`d, waits out its delay, and is
//!   then `execute`d by the admin within a 14-day grace window. The admin or
//!   guardian can cancel a queued action. Each action has a [`DelayClass`]:
//!   on the Stellar public network (detected on-chain via the network id)
//!   `Standard` (parameter changes) can never be below 48 h and `Critical`
//!   (upgrade, oracle, treasury withdrawal, delay change) never below 72 h.
//!   On testnet both use the configured delay (minimum 60 s, 300 s in the
//!   deploy scripts).
//! * **Upgrade** (`update_current_contract_wasm`) — only reachable through a
//!   timelocked action.
//! * Storage TTL constants/helpers and checked i128 math.
//!
//! Testnet-only scaffold, unaudited.
#![no_std]


use soroban_sdk::{
    contracterror, contractevent, contracttype, ContractExecutable, panic_with_error,
    xdr::ToXdr, Address, Bytes, BytesN, Env, IntoVal, Val, I256,
};

pub const DAY_LEDGERS: u32 = 17_280;
pub const INSTANCE_BUMP_THRESHOLD: u32 = 7 * DAY_LEDGERS;
pub const INSTANCE_BUMP_TO: u32 = 30 * DAY_LEDGERS;
pub const PERSISTENT_BUMP_THRESHOLD: u32 = 30 * DAY_LEDGERS;
pub const PERSISTENT_BUMP_TO: u32 = 120 * DAY_LEDGERS;

/// Minimum timelock delay on test networks (seconds).
pub const MIN_DELAY: u64 = 60;
/// Minimum timelock delay when running on the Stellar public network. Also
/// the floor for [`DelayClass::Standard`] actions (parameter changes).
pub const MAINNET_MIN_DELAY: u64 = 48 * 3_600;
/// Floor for [`DelayClass::Critical`] actions on the Stellar public network
/// (`Upgrade`, `SetOracle`, `WithdrawTreasury`, `SetDelay`, ...).
pub const MAINNET_CRITICAL_MIN_DELAY: u64 = 72 * 3_600;
/// Maximum timelock delay (seconds).
pub const MAX_DELAY: u64 = 30 * 86_400;
/// A queued action must be executed within this window after its ETA.
pub const GRACE_PERIOD: u64 = 14 * 86_400;

pub const MAINNET_PASSPHRASE: &[u8] = b"Public Global Stellar Network ; September 2015";

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GovError {
    Paused = 900,
    NoPendingAdmin = 901,
    NotGuardian = 902,
    NotQueued = 903,
    TimelockNotReady = 904,
    TimelockExpired = 905,
    AlreadyQueued = 906,
    InvalidDelay = 907,
    /// The instant path for this change was removed: queue it with
    /// `propose_action` and `execute_action` after the timelock.
    UseTimelock = 908,
    MathOverflow = 910,
    DivByZero = 911,
}

/// How long an action must wait in the timelock.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DelayClass {
    /// Parameter changes (fees, rates, listings, guardian, ...):
    /// `max(configured delay, 48 h)` on mainnet.
    Standard,
    /// Code, oracle, treasury and timelock changes:
    /// `max(configured delay, 72 h)` on mainnet.
    Critical,
}

/// Implemented by every contract's timelocked action enum.
pub trait TimelockAction {
    /// Delay class of this action (see [`DelayClass`]).
    fn delay_class(&self) -> DelayClass;
    /// Cheap static checks run when the action is *queued*, so a malformed
    /// action fails immediately instead of after the delay. The contract
    /// must still validate fully in `execute_action` (state can change).
    fn validate(&self, _env: &Env) {}
}

#[contracttype]
#[derive(Clone)]
enum GovKey {
    Admin,
    PendingAdmin,
    Guardian,
    Paused,
    TlDelay,
    Queued(BytesN<32>),
}

#[contractevent(topics = ["admin_proposed"])]
pub struct AdminProposed {
    #[topic]
    pub current: Address,
    #[topic]
    pub proposed: Address,
}

#[contractevent(topics = ["admin_accepted"])]
pub struct AdminAccepted {
    #[topic]
    pub previous: Address,
    #[topic]
    pub admin: Address,
}

#[contractevent(topics = ["guardian_set"], data_format = "single-value")]
pub struct GuardianSet {
    #[topic]
    pub admin: Address,
    pub guardian: Address,
}

#[contractevent(topics = ["paused"], data_format = "single-value")]
pub struct PausedEvt {
    #[topic]
    pub by: Address,
    pub paused: bool,
}

#[contractevent(topics = ["tl_queued"])]
pub struct Queued {
    #[topic]
    pub op: BytesN<32>,
    pub eta: u64,
    pub action: Val,
}

#[contractevent(topics = ["tl_executed"], data_format = "single-value")]
pub struct Executed {
    #[topic]
    pub op: BytesN<32>,
    pub action: Val,
}

#[contractevent(topics = ["tl_cancelled"], data_format = "single-value")]
pub struct Cancelled {
    #[topic]
    pub op: BytesN<32>,
    pub by: Address,
}

#[contractevent(topics = ["upgraded"], data_format = "single-value")]
pub struct Upgraded {
    pub wasm_hash: BytesN<32>,
}

#[contractevent(topics = ["delay_set"], data_format = "single-value")]
pub struct DelaySet {
    pub delay: u64,
}

// ------------------------------------------------------------------ TTL

pub fn bump_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_BUMP_THRESHOLD, INSTANCE_BUMP_TO);
}

/// Extend a persistent entry's TTL if it exists.
pub fn bump_persistent<K: IntoVal<Env, Val>>(env: &Env, key: &K) {
    if env.storage().persistent().has(key) {
        env.storage()
            .persistent()
            .extend_ttl(key, PERSISTENT_BUMP_THRESHOLD, PERSISTENT_BUMP_TO);
    }
}

// ------------------------------------------------------------------ admin

/// Initialise governance state. The admin is also the initial guardian.
pub fn init(env: &Env, admin: &Address, delay: u64) {
    check_delay(env, delay);
    let st = env.storage().instance();
    st.set(&GovKey::Admin, admin);
    st.set(&GovKey::Guardian, admin);
    st.set(&GovKey::Paused, &false);
    st.set(&GovKey::TlDelay, &delay);
    bump_instance(env);
}

pub fn admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&GovKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, GovError::NoPendingAdmin))
}

pub fn require_admin(env: &Env) -> Address {
    let a = admin(env);
    a.require_auth();
    bump_instance(env);
    a
}

pub fn pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&GovKey::PendingAdmin)
}

/// Step 1: the current admin nominates `new_admin`.
pub fn propose_admin(env: &Env, new_admin: &Address) {
    let current = require_admin(env);
    env.storage()
        .instance()
        .set(&GovKey::PendingAdmin, new_admin);
    AdminProposed {
        current,
        proposed: new_admin.clone(),
    }
    .publish(env);
}

/// Step 2: the nominee accepts (must sign).
pub fn accept_admin(env: &Env) -> Address {
    let pending = pending_admin(env).unwrap_or_else(|| panic_with_error!(env, GovError::NoPendingAdmin));
    pending.require_auth();
    let previous = admin(env);
    let st = env.storage().instance();
    st.set(&GovKey::Admin, &pending);
    st.remove(&GovKey::PendingAdmin);
    bump_instance(env);
    AdminAccepted {
        previous,
        admin: pending.clone(),
    }
    .publish(env);
    pending
}

pub fn guardian(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&GovKey::Guardian)
        .unwrap_or_else(|| admin(env))
}

/// Replace the guardian. Call **only** from an executed, timelocked action
/// (`SetGuardian`): an instant guardian swap would let a compromised admin
/// cut off the cancellation path before acting.
pub fn set_guardian_now(env: &Env, guardian: &Address) {
    let a = admin(env);
    env.storage().instance().set(&GovKey::Guardian, guardian);
    bump_instance(env);
    GuardianSet {
        admin: a,
        guardian: guardian.clone(),
    }
    .publish(env);
}

fn require_admin_or_guardian(env: &Env, caller: &Address) {
    caller.require_auth();
    if *caller != admin(env) && *caller != guardian(env) {
        panic_with_error!(env, GovError::NotGuardian);
    }
}

// ------------------------------------------------------------------ pause

/// Guardian or admin halts the gated entry points.
pub fn pause(env: &Env, caller: &Address) {
    require_admin_or_guardian(env, caller);
    env.storage().instance().set(&GovKey::Paused, &true);
    bump_instance(env);
    PausedEvt {
        by: caller.clone(),
        paused: true,
    }
    .publish(env);
}

/// Only the admin (multisig) can resume.
pub fn unpause(env: &Env) {
    let a = require_admin(env);
    env.storage().instance().set(&GovKey::Paused, &false);
    PausedEvt { by: a, paused: false }.publish(env);
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&GovKey::Paused)
        .unwrap_or(false)
}

pub fn when_not_paused(env: &Env) {
    if is_paused(env) {
        panic_with_error!(env, GovError::Paused);
    }
}

// ------------------------------------------------------------------ timelock

pub fn is_mainnet(env: &Env) -> bool {
    let h = env
        .crypto()
        .sha256(&Bytes::from_slice(env, MAINNET_PASSPHRASE))
        .to_bytes();
    env.ledger().network_id() == h
}

pub fn check_delay(env: &Env, delay: u64) {
    let min = if is_mainnet(env) {
        MAINNET_MIN_DELAY
    } else {
        MIN_DELAY
    };
    if delay < min || delay > MAX_DELAY {
        panic_with_error!(env, GovError::InvalidDelay);
    }
}

/// Configured base delay (seconds).
pub fn delay(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&GovKey::TlDelay)
        .unwrap_or(MAINNET_MIN_DELAY)
}

/// Floor for a delay class on the current network.
pub fn class_floor(env: &Env, class: DelayClass) -> u64 {
    if !is_mainnet(env) {
        return MIN_DELAY;
    }
    match class {
        DelayClass::Standard => MAINNET_MIN_DELAY,
        DelayClass::Critical => MAINNET_CRITICAL_MIN_DELAY,
    }
}

/// Delay applied to an action of `class`: the configured delay, raised to
/// the class floor on mainnet (48 h standard / 72 h critical). On testnet
/// this is just the configured delay.
pub fn effective_delay(env: &Env, class: DelayClass) -> u64 {
    let d = delay(env);
    let floor = class_floor(env, class);
    if d < floor {
        floor
    } else {
        d
    }
}

/// Apply a new delay (call only from an executed, timelocked action).
pub fn set_delay_now(env: &Env, new_delay: u64) {
    check_delay(env, new_delay);
    env.storage().instance().set(&GovKey::TlDelay, &new_delay);
    DelaySet { delay: new_delay }.publish(env);
}

/// Identifier of an action: sha256 of its XDR encoding.
pub fn op_id<T: IntoVal<Env, Val> + Clone>(env: &Env, action: &T) -> BytesN<32> {
    let v: Val = action.clone().into_val(env);
    env.crypto().sha256(&v.to_xdr(env)).to_bytes()
}

/// Admin queues `action`; returns its ETA (ledger timestamp). The wait is
/// [`effective_delay`] for the action's [`DelayClass`].
pub fn queue<T: IntoVal<Env, Val> + Clone + TimelockAction>(env: &Env, action: &T) -> u64 {
    require_admin(env);
    action.validate(env);
    let op = op_id(env, action);
    let key = GovKey::Queued(op.clone());
    if env.storage().persistent().has(&key) {
        panic_with_error!(env, GovError::AlreadyQueued);
    }
    let eta = checked_add_u64(
        env,
        env.ledger().timestamp(),
        effective_delay(env, action.delay_class()),
    );
    env.storage().persistent().set(&key, &eta);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_BUMP_THRESHOLD, PERSISTENT_BUMP_TO);
    Queued {
        op,
        eta,
        action: action.clone().into_val(env),
    }
    .publish(env);
    eta
}

pub fn eta<T: IntoVal<Env, Val> + Clone>(env: &Env, action: &T) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&GovKey::Queued(op_id(env, action)))
}

/// Admin executes a queued action whose delay has elapsed. Removes it from
/// the queue; the caller then applies the effect.
pub fn consume<T: IntoVal<Env, Val> + Clone>(env: &Env, action: &T) {
    require_admin(env);
    let op = op_id(env, action);
    let key = GovKey::Queued(op.clone());
    let eta: u64 = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| panic_with_error!(env, GovError::NotQueued));
    let now = env.ledger().timestamp();
    if now < eta {
        panic_with_error!(env, GovError::TimelockNotReady);
    }
    if now > checked_add_u64(env, eta, GRACE_PERIOD) {
        panic_with_error!(env, GovError::TimelockExpired);
    }
    env.storage().persistent().remove(&key);
    Executed {
        op,
        action: action.clone().into_val(env),
    }
    .publish(env);
}

/// Admin or guardian cancels a queued action.
pub fn cancel<T: IntoVal<Env, Val> + Clone>(env: &Env, caller: &Address, action: &T) {
    require_admin_or_guardian(env, caller);
    let op = op_id(env, action);
    let key = GovKey::Queued(op.clone());
    if !env.storage().persistent().has(&key) {
        panic_with_error!(env, GovError::NotQueued);
    }
    env.storage().persistent().remove(&key);
    Cancelled {
        op,
        by: caller.clone(),
    }
    .publish(env);
}

/// Swap the contract's code (call only from an executed, timelocked action).
pub fn upgrade_now(env: &Env, wasm_hash: &BytesN<32>) {
    env.deployer()
        .update_current_contract(ContractExecutable::Wasm(wasm_hash.clone()));
    Upgraded {
        wasm_hash: wasm_hash.clone(),
    }
    .publish(env);
}

// ------------------------------------------------------------------ checked math

pub fn checked_add_u64(env: &Env, a: u64, b: u64) -> u64 {
    a.checked_add(b)
        .unwrap_or_else(|| panic_with_error!(env, GovError::MathOverflow))
}

pub fn add(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_add(b)
        .unwrap_or_else(|| panic_with_error!(env, GovError::MathOverflow))
}

pub fn sub(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_sub(b)
        .unwrap_or_else(|| panic_with_error!(env, GovError::MathOverflow))
}

pub fn mul(env: &Env, a: i128, b: i128) -> i128 {
    a.checked_mul(b)
        .unwrap_or_else(|| panic_with_error!(env, GovError::MathOverflow))
}

pub fn div(env: &Env, a: i128, b: i128) -> i128 {
    if b == 0 {
        panic_with_error!(env, GovError::DivByZero);
    }
    a.checked_div(b)
        .unwrap_or_else(|| panic_with_error!(env, GovError::MathOverflow))
}

/// Euclidean floor(a * b / c) (floor for c > 0), computed in 256-bit when the product overflows i128.
/// Panics with `MathOverflow` only if the *result* does not fit in i128.
pub fn mul_div_floor(env: &Env, a: i128, b: i128, c: i128) -> i128 {
    if c == 0 {
        panic_with_error!(env, GovError::DivByZero);
    }
    match a.checked_mul(b) {
        Some(p) => p.div_euclid(c),
        None => {
            // Euclidean division, same semantics as the fast path.
            let p = I256::from_i128(env, a).mul(&I256::from_i128(env, b));
            let wc = I256::from_i128(env, c);
            let r = p.rem_euclid(&wc);
            let wide = p.sub(&r).div(&wc);
            wide.to_i128()
                .unwrap_or_else(|| panic_with_error!(env, GovError::MathOverflow))
        }
    }
}

/// Euclidean ceil(a * b / c) (ceil for c > 0), computed in 256-bit when needed.
pub fn mul_div_ceil(env: &Env, a: i128, b: i128, c: i128) -> i128 {
    if c == 0 {
        panic_with_error!(env, GovError::DivByZero);
    }
    let (q, exact) = match a.checked_mul(b) {
        Some(p) => (p.div_euclid(c), p.rem_euclid(c) == 0),
        None => {
            let p = I256::from_i128(env, a).mul(&I256::from_i128(env, b));
            let wc = I256::from_i128(env, c);
            let r = p.rem_euclid(&wc);
            let exact = r.to_i128() == Some(0);
            let q = p
                .sub(&r)
                .div(&wc)
                .to_i128()
                .unwrap_or_else(|| panic_with_error!(env, GovError::MathOverflow));
            (q, exact)
        }
    };
    if exact {
        q
    } else {
        add(env, q, 1)
    }
}

// ------------------------------------------------------------------ entry points

/// Generates the standard governance entry points for a contract type and
/// its timelocked action enum (which must implement [`TimelockAction`] and
/// have a `SetGuardian(Address)` variant applied with [`set_guardian_now`]).
/// The contract must implement `execute_action` itself (it applies the
/// effect of each action after `gov::consume`).
#[macro_export]
macro_rules! governance_entrypoints {
    ($contract:ident, $action:ident) => {
        #[soroban_sdk::contractimpl]
        impl $contract {
            /// Current admin.
            pub fn admin(env: soroban_sdk::Env) -> soroban_sdk::Address {
                $crate::admin(&env)
            }
            /// Nominee awaiting `accept_admin`, if any.
            pub fn pending_admin(env: soroban_sdk::Env) -> Option<soroban_sdk::Address> {
                $crate::pending_admin(&env)
            }
            /// Two-step admin transfer, step 1 (current admin).
            pub fn propose_admin(env: soroban_sdk::Env, new_admin: soroban_sdk::Address) {
                $crate::propose_admin(&env, &new_admin)
            }
            /// Two-step admin transfer, step 2 (signed by the nominee).
            pub fn accept_admin(env: soroban_sdk::Env) -> soroban_sdk::Address {
                $crate::accept_admin(&env)
            }
            pub fn guardian(env: soroban_sdk::Env) -> soroban_sdk::Address {
                $crate::guardian(&env)
            }
            // No instant `set_guardian`: use the timelocked `SetGuardian`
            // action (`propose_action` -> wait -> `execute_action`).
            /// Configured base timelock delay (seconds).
            pub fn timelock_delay(env: soroban_sdk::Env) -> u64 {
                $crate::delay(&env)
            }
            /// Delay that `action` would wait if queued now (seconds):
            /// the base delay raised to its class floor on mainnet.
            pub fn action_delay(env: soroban_sdk::Env, action: $action) -> u64 {
                $crate::effective_delay(&env, $crate::TimelockAction::delay_class(&action))
            }
            /// Queue a timelocked admin action. Returns its ETA.
            pub fn propose_action(env: soroban_sdk::Env, action: $action) -> u64 {
                $crate::queue(&env, &action)
            }
            /// Admin or guardian cancels a queued action.
            pub fn cancel_action(
                env: soroban_sdk::Env,
                caller: soroban_sdk::Address,
                action: $action,
            ) {
                $crate::cancel(&env, &caller, &action)
            }
            /// ETA of a queued action (None if not queued).
            pub fn action_eta(env: soroban_sdk::Env, action: $action) -> Option<u64> {
                $crate::eta(&env, &action)
            }
        }
    };
}

/// Generates `pause` / `unpause` / `paused` for contracts with user-facing
/// state-changing entry points.
#[macro_export]
macro_rules! pause_entrypoints {
    ($contract:ident) => {
        #[soroban_sdk::contractimpl]
        impl $contract {
            /// Guardian or admin: halt new risk (exits stay open).
            pub fn pause(env: soroban_sdk::Env, caller: soroban_sdk::Address) {
                $crate::pause(&env, &caller)
            }
            /// Admin only.
            pub fn unpause(env: soroban_sdk::Env) {
                $crate::unpause(&env)
            }
            pub fn paused(env: soroban_sdk::Env) -> bool {
                $crate::is_paused(&env)
            }
        }
    };
}
