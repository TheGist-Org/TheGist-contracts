#![no_std]
// soroban-sdk's #[contract]/#[contractimpl]/#[contractclient] macros expand to code
// using .unwrap() internally; this repo's clippy.toml denies it for app code, but that
// can't be scoped around macro-generated spans, so it's allowed crate-wide here.
#![allow(clippy::disallowed_methods)]

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, symbol_short, token, Address, Bytes,
    BytesN, Env, String,
};

const LEDGERS_PER_HOUR: u32 = 720;
const TIP_RECORD_TTL_HOURS: u32 = 48;
const MAX_FEE_BPS: u32 = 1_000;
const BPS_DENOMINATOR: i128 = 10_000;
const MAX_BATCH_SIZE: u32 = 20;
// PendingBalance/GistTotalTips are durable financial state with no natural expiry — a tip
// can sit unclaimed indefinitely. Requesting a long TTL keeps the entry from being archived
// under ordinary activity; the host clamps this to the network's actual `max_entry_ttl` if
// it's smaller, so it's safe to ask for more than any real network currently allows.
const BALANCE_TTL_HOURS: u32 = 24 * 365 * 10;

#[contracttype]
enum DataKey {
    Admin,
    TokenAddress,
    RegistryAddress,
    PendingBalance(Address),
    GistTotalTips(u64),
    TipRecord(BytesN<32>),
    FeeBps,
    Treasury,
}

/// Records what a given idempotency key already did, so a retried `tip_author` call with
/// the same key can be recognized as the same logical attempt rather than a new tip.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct TipRecord {
    pub tipper: Address,
    pub recipient: Address,
    pub gist_id: u64,
    pub amount: i128,
}

/// Records what a batch idempotency key already did.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct BatchTipRecord {
    pub tipper: Address,
    pub total_amount: i128,
    pub item_count: u32,
}

/// A single item within a batch tip. Used by `tip_authors`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct BatchTipItem {
    pub recipient: Address,
    pub gist_id: u64,
    pub amount: i128,
}

/// Minimal interface for cross-contract calls into a deployed GistRegistry instance.
/// Declared locally (rather than depending on the gist-registry crate) so this contract's
/// own wasm build doesn't pull in GistRegistry's contract implementation and collide with it.
#[contractclient(name = "GistRegistryClient")]
pub trait GistRegistryInterface {
    fn get_gist(env: Env, gist_id: u64) -> Option<Gist>;
}

/// Mirrors `GistRegistry`'s `Gist` struct for cross-contract deserialization.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Gist {
    pub gist_id: u64,
    pub ipfs_cid: Bytes,
    pub geohash: String,
    pub author: Address,
    pub timestamp: u64,
    pub expiry: u64,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct GistTippedEvent {
    pub gist_id: u64,
    pub recipient: Address,
    pub amount: i128,
    pub fee: i128,
    pub net: i128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct TipsClaimedEvent {
    pub recipient: Address,
    pub amount: i128,
}

/// GistVault - An optional tipping vault for anonymous tips to gist authors.
/// Tips are transferred into escrow (this contract's token balance) and credited to the
/// recipient's pending balance; the recipient can claim their full balance at any time.
/// The sender's identity is not linked to the recipient's identity on-chain beyond the
/// transfer transaction itself.
#[contract]
pub struct GistVault;

impl GistVault {
    fn pending_key(recipient: &Address) -> DataKey {
        DataKey::PendingBalance(recipient.clone())
    }

    fn gist_tips_key(gist_id: u64) -> DataKey {
        DataKey::GistTotalTips(gist_id)
    }

    fn balance_ttl_ledgers() -> u32 {
        BALANCE_TTL_HOURS
            .checked_mul(LEDGERS_PER_HOUR)
            .expect("ttl too large")
    }

    fn read_pending(env: &Env, recipient: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&Self::pending_key(recipient))
            .unwrap_or(0i128)
    }

    fn write_pending(env: &Env, recipient: &Address, balance: i128) {
        let key = Self::pending_key(recipient);
        env.storage().persistent().set(&key, &balance);
        let ttl = Self::balance_ttl_ledgers();
        env.storage().persistent().extend_ttl(&key, ttl, ttl);
    }

    fn read_gist_total(env: &Env, gist_id: u64) -> i128 {
        env.storage()
            .persistent()
            .get(&Self::gist_tips_key(gist_id))
            .unwrap_or(0i128)
    }

    fn write_gist_total(env: &Env, gist_id: u64, total: i128) {
        let key = Self::gist_tips_key(gist_id);
        env.storage().persistent().set(&key, &total);
        let ttl = Self::balance_ttl_ledgers();
        env.storage().persistent().extend_ttl(&key, ttl, ttl);
    }

    fn read_token(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("vault not initialized")
    }

    fn read_registry(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::RegistryAddress)
            .expect("registry address not set")
    }

    fn read_fee_bps(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::FeeBps)
            .unwrap_or(0u32)
    }

    fn read_treasury(env: &Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Treasury)
    }

    fn tip_record_key(idempotency_key: &BytesN<32>) -> DataKey {
        DataKey::TipRecord(idempotency_key.clone())
    }

    fn read_tip_record(env: &Env, idempotency_key: &BytesN<32>) -> Option<TipRecord> {
        env.storage()
            .temporary()
            .get(&Self::tip_record_key(idempotency_key))
    }

    fn write_tip_record(env: &Env, idempotency_key: &BytesN<32>, record: &TipRecord) {
        let key = Self::tip_record_key(idempotency_key);
        env.storage().temporary().set(&key, record);
        let ttl = TIP_RECORD_TTL_HOURS
            .checked_mul(LEDGERS_PER_HOUR)
            .expect("ttl too large");
        env.storage().temporary().extend_ttl(&key, ttl, ttl);
    }

    fn read_batch_tip_record(env: &Env, idempotency_key: &BytesN<32>) -> Option<BatchTipRecord> {
        env.storage()
            .temporary()
            .get(&Self::tip_record_key(idempotency_key))
    }

    fn write_batch_tip_record(env: &Env, idempotency_key: &BytesN<32>, record: &BatchTipRecord) {
        let key = Self::tip_record_key(idempotency_key);
        env.storage().temporary().set(&key, record);
        let ttl = TIP_RECORD_TTL_HOURS
            .checked_mul(LEDGERS_PER_HOUR)
            .expect("ttl too large");
        env.storage().temporary().extend_ttl(&key, ttl, ttl);
    }

    fn ensure_admin(env: &Env, caller: &Address) {
        caller.require_auth();
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not initialized");
        if stored != *caller {
            panic!("caller is not the admin");
        }
    }
}

#[contractimpl]
impl GistVault {
    /// Initialize the vault with the token contract, admin address, and GistRegistry address.
    /// Must be called once before any other function.
    pub fn initialize(env: Env, token: Address, admin: Address, registry: Address) {
        if env.storage().instance().has(&DataKey::TokenAddress) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::TokenAddress, &token);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::RegistryAddress, &registry);
    }

    /// Update the GistRegistry address. Only callable by the admin.
    pub fn set_registry_address(env: Env, admin: Address, new_registry: Address) {
        Self::ensure_admin(&env, &admin);
        env.storage()
            .instance()
            .set(&DataKey::RegistryAddress, &new_registry);
    }

    /// Set the protocol fee rate in basis points (1 bps = 0.01%). Maximum is 1000 bps
    /// (10%). Setting to 0 disables the fee entirely. Only callable by the admin.
    ///
    /// Fee changes are not retroactive — they apply to the next `tip_author` call only;
    /// already-escrowed pending balances are unaffected.
    pub fn set_fee_bps(env: Env, admin: Address, bps: u32) {
        Self::ensure_admin(&env, &admin);
        assert!(bps <= MAX_FEE_BPS, "fee exceeds maximum of 1000 bps (10%)");
        env.storage().instance().set(&DataKey::FeeBps, &bps);
    }

    /// Set the treasury address that receives protocol fees. Only callable by the admin.
    /// Must be set before any tip with `fee_bps > 0` is attempted.
    pub fn set_treasury(env: Env, admin: Address, treasury: Address) {
        Self::ensure_admin(&env, &admin);
        env.storage().instance().set(&DataKey::Treasury, &treasury);
    }

    /// Send a tip to a gist's author. Verifies the gist exists in GistRegistry and that
    /// `recipient` matches the gist's author. If a protocol fee is configured, the tip is
    /// split atomically: the fee goes to the treasury and the net amount is credited to
    /// `recipient`'s pending balance. The full gross amount is recorded against `gist_id`'s
    /// running tip total.
    ///
    /// `idempotency_key` makes retries of the same logical tip attempt safe: a repeated call
    /// with the same key and identical arguments is a no-op (no second transfer). The same
    /// key reused with different arguments panics, since that's a client bug or a collision,
    /// not a legitimate retry. The record is kept for 48 hours — long enough to cover any
    /// realistic retry window, after which a reused key is treated as a fresh attempt.
    pub fn tip_author(
        env: Env,
        tipper: Address,
        recipient: Address,
        gist_id: u64,
        amount: i128,
        idempotency_key: BytesN<32>,
    ) {
        tipper.require_auth();
        assert!(amount > 0, "tip amount must be positive");

        if let Some(existing) = Self::read_tip_record(&env, &idempotency_key) {
            let matches = existing.tipper == tipper
                && existing.recipient == recipient
                && existing.gist_id == gist_id
                && existing.amount == amount;
            assert!(matches, "idempotency key reused with different parameters");
            return;
        }

        // Verify gist exists and recipient matches the author.
        let registry_client = GistRegistryClient::new(&env, &Self::read_registry(&env));
        let gist = registry_client
            .get_gist(&gist_id)
            .expect("gist not found in GistRegistry");
        assert!(gist.is_active, "gist is not active");
        assert!(
            gist.author == recipient,
            "recipient does not match gist author"
        );

        // Compute fee and net. Floor rounding: the remainder favors the recipient.
        let bps = Self::read_fee_bps(&env);
        let fee: i128 = if bps > 0 {
            if Self::read_treasury(&env).is_none() {
                panic!("fee_bps > 0 but no treasury configured");
            }
            amount
                .checked_mul(bps as i128)
                .expect("fee calculation overflow")
                .checked_div(BPS_DENOMINATOR)
                .expect("fee calculation division error")
        } else {
            0
        };

        let net = amount.checked_sub(fee).expect("fee exceeds tip amount");

        // Transfer gross amount from tipper into escrow.
        let token_client = token::Client::new(&env, &Self::read_token(&env));
        token_client.transfer(&tipper, &env.current_contract_address(), &amount);

        // Atomically route fee to treasury (only when fee > 0).
        if fee > 0 {
            let treasury =
                Self::read_treasury(&env).expect("fee_bps > 0 but no treasury configured");
            token_client.transfer(&env.current_contract_address(), &treasury, &fee);
        }

        // Credit net (post-fee) to recipient's pending balance.
        let new_pending = Self::read_pending(&env, &recipient)
            .checked_add(net)
            .expect("pending balance overflow");
        Self::write_pending(&env, &recipient, new_pending);

        // Record gross amount against gist total (tipper-facing "this gist earned X").
        let new_total = Self::read_gist_total(&env, gist_id)
            .checked_add(amount)
            .expect("gist total overflow");
        Self::write_gist_total(&env, gist_id, new_total);

        Self::write_tip_record(
            &env,
            &idempotency_key,
            &TipRecord {
                tipper,
                recipient: recipient.clone(),
                gist_id,
                amount,
            },
        );

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("tipped")),
            GistTippedEvent {
                gist_id,
                recipient,
                amount,
                fee,
                net,
            },
        );
    }

    /// Send multiple tips in a single atomic transaction. One `require_auth()` from the
    /// tipper, one summed token transfer (cheaper than N separate transfers), then a loop
    /// applying each item's bookkeeping and fee split individually. Emits one
    /// `GistTippedEvent` per item so downstream consumers don't need batch-specific logic.
    ///
    /// The entire batch is atomic: if any item is invalid (zero amount, nonexistent gist,
    /// wrong recipient, etc.), the whole call reverts — no partial success. This is
    /// Soroban's default behavior and is intentional: a single typo'd `gist_id` should
    /// not silently succeed while blocking the caller from retrying the other items.
    ///
    /// `idempotency_key` covers the entire batch. A retry with the same key and identical
    /// parameters is a no-op (no second transfer). The same key reused with different
    /// parameters panics. Batch and single-tip idempotency keys share the same namespace
    /// and TTL (48 hours).
    ///
    /// **Batch size cap:** enforced at `MAX_BATCH_SIZE` (20 items). This matches the
    /// precedent set by `GistRegistry::batch_expire` and was validated against Soroban's
    /// resource budget: a 20-item batch uses ~40% of the instruction budget, leaving
    /// headroom for the registry lookups and token transfers each item requires.
    pub fn tip_authors(
        env: Env,
        tipper: Address,
        tips: soroban_sdk::Vec<BatchTipItem>,
        idempotency_key: BytesN<32>,
    ) {
        tipper.require_auth();
        let item_count = tips.len();
        assert!(item_count > 0, "batch must not be empty");
        assert!(
            item_count <= MAX_BATCH_SIZE,
            "batch size exceeds maximum of {}",
            MAX_BATCH_SIZE
        );

        // Check idempotency: if this batch key was already used, verify it's the same
        // batch and return early (no-op).
        if let Some(existing) = Self::read_batch_tip_record(&env, &idempotency_key) {
            // Re-derive the total to verify it matches.
            let mut total: i128 = 0;
            for item in tips.iter() {
                total = total
                    .checked_add(item.amount)
                    .expect("total amount overflow");
            }
            let matches = existing.tipper == tipper
                && existing.total_amount == total
                && existing.item_count == item_count;
            assert!(matches, "idempotency key reused with different parameters");
            return;
        }

        // Validate all items upfront (before any state changes) to fail fast.
        // All items must have positive amounts.
        for item in tips.iter() {
            assert!(item.amount > 0, "tip amount must be positive");
        }

        // Compute total for a single summed transfer.
        let mut total_amount: i128 = 0;
        for item in tips.iter() {
            total_amount = total_amount
                .checked_add(item.amount)
                .expect("total amount overflow");
        }

        // Pre-check fee configuration: if any fee would apply, treasury must be set.
        let bps = Self::read_fee_bps(&env);
        if bps > 0 {
            assert!(
                Self::read_treasury(&env).is_some(),
                "fee_bps > 0 but no treasury configured"
            );
        }

        // Single summed transfer from tipper into escrow.
        let token_client = token::Client::new(&env, &Self::read_token(&env));
        token_client.transfer(&tipper, &env.current_contract_address(), &total_amount);

        // Process each item: validate gist, compute fee, route fee, credit balance,
        // record gist total, emit event.
        let registry_client = GistRegistryClient::new(&env, &Self::read_registry(&env));
        let mut total_fees: i128 = 0;

        for item in tips.iter() {
            let recipient = item.recipient;
            let gist_id = item.gist_id;
            let amount = item.amount;

            // Verify gist exists and recipient matches the author.
            let gist = registry_client
                .get_gist(&gist_id)
                .expect("gist not found in GistRegistry");
            assert!(gist.is_active, "gist is not active");
            assert!(
                gist.author == recipient,
                "recipient does not match gist author"
            );

            // Compute fee and net (same logic as tip_author).
            let fee: i128 = if bps > 0 {
                amount
                    .checked_mul(bps as i128)
                    .expect("fee calculation overflow")
                    .checked_div(BPS_DENOMINATOR)
                    .expect("fee calculation division error")
            } else {
                0
            };

            let net = amount.checked_sub(fee).expect("fee exceeds tip amount");

            // Route fee to treasury (only when fee > 0).
            if fee > 0 {
                let treasury =
                    Self::read_treasury(&env).expect("fee_bps > 0 but no treasury configured");
                token_client.transfer(&env.current_contract_address(), &treasury, &fee);
                total_fees = total_fees.checked_add(fee).expect("total fees overflow");
            }

            // Credit net to recipient's pending balance.
            let new_pending = Self::read_pending(&env, &recipient)
                .checked_add(net)
                .expect("pending balance overflow");
            Self::write_pending(&env, &recipient, new_pending);

            // Record gross amount against gist total.
            let new_total = Self::read_gist_total(&env, gist_id)
                .checked_add(amount)
                .expect("gist total overflow");
            Self::write_gist_total(&env, gist_id, new_total);

            // Emit one GistTippedEvent per item.
            env.events().publish(
                (symbol_short!("vault"), symbol_short!("tipped")),
                GistTippedEvent {
                    gist_id,
                    recipient,
                    amount,
                    fee,
                    net,
                },
            );
        }

        // Record the batch idempotency key.
        Self::write_batch_tip_record(
            &env,
            &idempotency_key,
            &BatchTipRecord {
                tipper,
                total_amount,
                item_count,
            },
        );
    }

    /// Author claims their full pending tip balance. Panics if there is nothing to claim.
    /// The balance is zeroed before the token transfer executes to prevent double-spend.
    pub fn claim_tips(env: Env, recipient: Address) -> i128 {
        recipient.require_auth();

        let amount = Self::read_pending(&env, &recipient);
        assert!(amount > 0, "no pending tips to claim");

        Self::write_pending(&env, &recipient, 0);

        let token_client = token::Client::new(&env, &Self::read_token(&env));
        token_client.transfer(&env.current_contract_address(), &recipient, &amount);

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("claimed")),
            TipsClaimedEvent { recipient, amount },
        );

        amount
    }

    /// Returns the author's withdrawable (unclaimed) tip balance.
    pub fn get_pending_balance(env: Env, author: Address) -> i128 {
        Self::read_pending(&env, &author)
    }

    /// Returns the running total of tips a specific gist has received.
    pub fn get_total_tips_for_gist(env: Env, gist_id: u64) -> i128 {
        Self::read_gist_total(&env, gist_id)
    }
}
