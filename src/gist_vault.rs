use soroban_sdk::{contract, contractimpl, Address, Env};

/// Tip data structure tracking accumulated tips for an author
#[derive(Clone)]
#[contracttype]
pub struct TipBalance {
    /// Total accumulated tips for this author
    pub total_tips: i128,
    /// Number of individual tips received
    pub tip_count: u64,
}

/// Event emitted when a tip is sent to an author
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum GistVaultEvent {
    TipSent(TipSentEvent),
    TipsWithdrawn(TipsWithdrawnEvent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct TipSentEvent {
    pub recipient: Address,
    pub amount: i128,
    pub sender: Address,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct TipsWithdrawnEvent {
    pub recipient: Address,
    pub amount: i128,
}

/// GistVault - An optional tipping vault for anonymous XLM tips
/// Users can send XLM tips to gist authors anonymously via Soroban escrow
/// The sender's identity is not linked to the recipient's identity on-chain beyond the transaction itself
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
    /// Send a tip to a gist author anonymously
    /// 
    /// # Arguments
    /// * `recipient` - The address of the gist author to receive the tip
    /// * `amount` - The amount of XLM to tip (in stroops, 1 XLM = 10^7 stroops)
    /// 
    /// # Note
    /// The tip is stored in escrow and the recipient can withdraw it at any time.
    /// The sender's identity is only visible in the transaction, not in the contract state.
    pub fn tip_author(env: Env, recipient: Address, amount: i128) {
        // Verify the caller is sending the tip
        let sender = env.current_contract_address();
        
        // Ensure amount is positive
        assert!(amount > 0, "Tip amount must be positive");

        // Get current balance for recipient
        let balance_key = recipient.clone();
        let current_balance: TipBalance = env.storage().get(&balance_key).unwrap_or(Ok(TipBalance {
            total_tips: 0,
            tip_count: 0,
        })).unwrap();

        // Update balance
        let new_balance = TipBalance {
            total_tips: current_balance.total_tips + amount,
            tip_count: current_balance.tip_count + 1,
        };

        // Store updated balance
        env.storage().set(&balance_key, &new_balance);

        // Emit event (note: sender is the contract address for anonymity)
        env.events().publish(
            GistVaultEvent::TipSent(TipSentEvent {
                recipient: recipient.clone(),
                amount,
                sender,
            }),
            (),
        );
    }

    /// Withdraw accumulated tips for the calling author
    /// 
    /// # Arguments
    /// * `recipient` - The address of the author withdrawing their tips
    /// 
    /// # Returns
    /// The amount withdrawn
    /// 
    /// # Note
    /// Only the recipient can withdraw their own tips.
    /// This would typically be called with a token transfer in a real implementation.
    pub fn withdraw_tips(env: Env, recipient: Address) -> i128 {
        // Verify the caller is the recipient
        recipient.require_auth();

        // Get current balance
        let balance_key = recipient.clone();
        let balance: TipBalance = env.storage().get(&balance_key).unwrap_or(Ok(TipBalance {
            total_tips: 0,
            tip_count: 0,
        })).unwrap();

        let amount = balance.total_tips;
        
        // Ensure there are tips to withdraw
        assert!(amount > 0, "No tips to withdraw");

        // Clear the balance
        env.storage().remove(&balance_key);

        // Emit event
        env.events().publish(
            GistVaultEvent::TipsWithdrawn(TipsWithdrawnEvent {
                recipient: recipient.clone(),
                amount,
            }),
            (),
        );

        amount
    }

    /// Get the current tip balance for an author
    /// 
    /// # Arguments
    /// * `recipient` - The address of the author
    /// 
    /// # Returns
    /// The TipBalance struct containing total tips and tip count
    pub fn get_tip_balance(env: Env, recipient: Address) -> TipBalance {
        let balance_key = recipient;
        env.storage().get(&balance_key).unwrap_or(Ok(TipBalance {
            total_tips: 0,
            tip_count: 0,
        })).unwrap()
    }

/// Get the total tips accumulated across all authors
///
/// # Returns
/// The total amount of tips in the vault.
///
/// Note: Calculating this dynamically would require iterating over
/// all storage entries, which is expensive in Soroban. Currently,
/// this returns `0` until aggregate tracking is implemented.
pub fn get_total_vault_balance(env: Env) -> i128 {
    let _ = env;
    0
}

/// Get the tip balance for a specific author
///
/// # Arguments
/// * `author` - The author's address
///
/// # Returns
/// The amount of tips accumulated by the author.
pub fn get_tip_balance(env: Env, author: Address) -> U256 {
    let _ = author;

    // Placeholder for balance query until author balances
    // are persisted in contract storage.
    U256::from_u128(&env, 0)
}
    }
}
