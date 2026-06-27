use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Env};

use crate::gist_vault::{GistVault, GistVaultClient};

fn setup() -> (Env, Address, Address, GistVaultClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy a minimal SAC-compatible token for testing
    let admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(admin.clone()).address();

    let vault_id = env.register_contract(None, GistVault);
    let client = GistVaultClient::new(&env, &vault_id);
    client.initialize(&token_id);

    // Mint tokens to a tipper
    let tipper = Address::generate(&env);
    let token_admin = token::StellarAssetClient::new(&env, &token_id);
    token_admin.mint(&tipper, &1_000_000);

    (env, tipper, token_id, client)
}

#[test]
fn test_get_pending_balance_starts_at_zero() {
    let (env, _tipper, _token_id, client) = setup();
    let author = Address::generate(&env);
    assert_eq!(client.get_pending_balance(&author), 0i128);
}

#[test]
fn test_tip_and_get_pending_balance() {
    let (env, tipper, _token_id, client) = setup();
    let author = Address::generate(&env);

    client.tip_author(&tipper, &author, &1u64, &500_000i128);
    assert_eq!(client.get_pending_balance(&author), 500_000i128);
}

#[test]
fn test_get_total_tips_for_gist() {
    let (env, tipper, _token_id, client) = setup();
    let author = Address::generate(&env);

    client.tip_author(&tipper, &author, &42u64, &100_000i128);
    assert_eq!(client.get_total_tips_for_gist(&42u64), 100_000i128);
}

#[test]
fn test_claim_tips_transfers_and_clears_balance() {
    let (env, tipper, token_id, client) = setup();
    let author = Address::generate(&env);

    client.tip_author(&tipper, &author, &1u64, &300_000i128);
    let claimed = client.claim_tips(&author);
    assert_eq!(claimed, 300_000i128);
    assert_eq!(client.get_pending_balance(&author), 0i128);

    // Verify token balance transferred
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&author), 300_000i128);
}

#[test]
#[should_panic(expected = "no pending tips to claim")]
fn test_claim_tips_zero_balance_panics() {
    let (env, _tipper, _token_id, client) = setup();
    let author = Address::generate(&env);
    client.claim_tips(&author);
}
