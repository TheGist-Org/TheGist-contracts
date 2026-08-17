use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Bytes, Env, String};

use crate::{GistVault, GistVaultClient};

use gist_registry::{GistRegistry, GistRegistryClient};

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    GistVaultClient<'static>,
    GistRegistryClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let registry_id = env.register_contract(None, GistRegistry);
    let registry_client = GistRegistryClient::new(&env, &registry_id);
    let registry_admin = Address::generate(&env);
    registry_client.initialize(&registry_admin);

    let vault_id = env.register_contract(None, GistVault);
    let client = GistVaultClient::new(&env, &vault_id);
    let vault_admin = Address::generate(&env);
    client.initialize(&token_id, &vault_admin, &registry_id);

    let tipper = Address::generate(&env);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
    token_admin_client.mint(&tipper, &1_000_000);

    (
        env,
        tipper,
        token_id,
        vault_admin,
        client,
        registry_client,
    )
}

#[test]
fn test_get_pending_balance_starts_at_zero() {
    let (env, _tipper, _token_id, _vault_admin, client, _registry_client) = setup();
    let author = Address::generate(&env);
    assert_eq!(client.get_pending_balance(&author), 0i128);
}

#[test]
fn test_tip_and_get_pending_balance() {
    let (env, tipper, _token_id, _vault_admin, client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.tip_author(&tipper, &author, &gist_id, &500_000i128);
    assert_eq!(client.get_pending_balance(&author), 500_000i128);
}

#[test]
fn test_get_total_tips_for_gist() {
    let (env, tipper, _token_id, _vault_admin, client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.tip_author(&tipper, &author, &gist_id, &100_000i128);
    assert_eq!(client.get_total_tips_for_gist(&gist_id), 100_000i128);
}

#[test]
fn test_claim_tips_transfers_and_clears_balance() {
    let (env, tipper, token_id, _vault_admin, client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    client.tip_author(&tipper, &author, &gist_id, &300_000i128);
    let claimed = client.claim_tips(&author);
    assert_eq!(claimed, 300_000i128);
    assert_eq!(client.get_pending_balance(&author), 0i128);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&author), 300_000i128);
}

#[test]
#[should_panic(expected = "no pending tips to claim")]
fn test_claim_tips_zero_balance_panics() {
    let (env, _tipper, _token_id, _vault_admin, client, _registry_client) = setup();
    let author = Address::generate(&env);
    client.claim_tips(&author);
}
