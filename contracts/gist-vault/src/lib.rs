#![no_std]
// soroban-sdk's #[contract]/#[contractimpl]/#[contractclient] macros expand to code
// using .unwrap() internally; this repo's clippy.toml denies it for app code, but that
// can't be scoped around macro-generated spans, so it's allowed crate-wide here.
#![allow(clippy::disallowed_methods)]

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, symbol_short, token, Address, Bytes, Env,
    String,
};

#[contracttype]
enum DataKey {
    Admin,
    TokenAddress,
    RegistryAddress,
    PendingBalance(Address),
    GistTotalTips(u64),
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

    fn read_pending(env: &Env, recipient: &Address) -> i128 {
        env.storage()
            .instance()
            .get(&Self::pending_key(recipient))
            .unwrap_or(0i128)
    }

    fn write_pending(env: &Env, recipient: &Address, balance: i128) {
        env.storage()
            .instance()
            .set(&Self::pending_key(recipient), &balance);
    }

    fn read_gist_total(env: &Env, gist_id: u64) -> i128 {
        env.storage()
            .instance()
            .get(&Self::gist_tips_key(gist_id))
            .unwrap_or(0i128)
    }

    fn write_gist_total(env: &Env, gist_id: u64, total: i128) {
        env.storage()
            .instance()
            .set(&Self::gist_tips_key(gist_id), &total);
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

    /// Send a tip to a gist's author. Verifies the gist exists in GistRegistry and that
    /// `recipient` matches the gist's author. Transfers `amount` of the vault's token from
    /// `tipper` into escrow, credited against `recipient`'s pending balance and `gist_id`'s
    /// running tip total.
    pub fn tip_author(env: Env, tipper: Address, recipient: Address, gist_id: u64, amount: i128) {
        tipper.require_auth();
        assert!(amount > 0, "tip amount must be positive");

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

        let token_client = token::Client::new(&env, &Self::read_token(&env));
        token_client.transfer(&tipper, &env.current_contract_address(), &amount);

        let new_pending = Self::read_pending(&env, &recipient)
            .checked_add(amount)
            .expect("pending balance overflow");
        Self::write_pending(&env, &recipient, new_pending);

        let new_total = Self::read_gist_total(&env, gist_id)
            .checked_add(amount)
            .expect("gist total overflow");
        Self::write_gist_total(&env, gist_id, new_total);

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("tipped")),
            GistTippedEvent {
                gist_id,
                recipient,
                amount,
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
