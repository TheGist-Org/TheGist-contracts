use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env,
};

#[contracttype]
enum DataKey {
    TokenAddress,
    PendingBalance(Address),
    GistTotalTips(u64),
}

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

    fn read_gist_total(env: &Env, gist_id: u64) -> i128 {
        env.storage()
            .instance()
            .get(&Self::gist_tips_key(gist_id))
            .unwrap_or(0i128)
    }
}

#[contractimpl]
impl GistVault {
    /// Store the XLM token contract address on first deployment.
    pub fn initialize(env: Env, token: Address) {
        if env.storage().instance().has(&DataKey::TokenAddress) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::TokenAddress, &token);
    }

    /// Tip a gist author. Transfers `amount` tokens from caller to the vault
    /// and records the pending balance for `recipient`.
    pub fn tip_author(env: Env, tipper: Address, recipient: Address, gist_id: u64, amount: i128) {
        tipper.require_auth();
        if amount <= 0 {
            panic!("amount must be positive");
        }

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("vault not initialized");
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&tipper, &env.current_contract_address(), &amount);

        let new_balance = Self::read_pending(&env, &recipient)
            .checked_add(amount)
            .expect("balance overflow");
        env.storage()
            .instance()
            .set(&Self::pending_key(&recipient), &new_balance);

        let new_total = Self::read_gist_total(&env, gist_id)
            .checked_add(amount)
            .expect("total overflow");
        env.storage()
            .instance()
            .set(&Self::gist_tips_key(gist_id), &new_total);

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("tipped")),
            (gist_id, recipient, amount),
        );
    }

    /// Claim all pending tips for the caller. Panics if balance is zero.
    pub fn claim_tips(env: Env, recipient: Address) -> i128 {
        recipient.require_auth();

        let balance = Self::read_pending(&env, &recipient);
        if balance == 0 {
            panic!("no pending tips to claim");
        }

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("vault not initialized");
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&env.current_contract_address(), &recipient, &balance);

        env.storage()
            .instance()
            .set(&Self::pending_key(&recipient), &0i128);

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("claimed")),
            (recipient, balance),
        );

        balance
    }

    /// Returns the unclaimed tip balance for `recipient`. Returns 0 if none.
    pub fn get_pending_balance(env: Env, recipient: Address) -> i128 {
        Self::read_pending(&env, &recipient)
    }

    /// Returns total XLM tipped to a gist across all tippers.
    pub fn get_total_tips_for_gist(env: Env, gist_id: u64) -> i128 {
        Self::read_gist_total(&env, gist_id)
    }
}
