use gist_registry::{GistRegistry, GistRegistryClient};
use gist_vault::{GistVault, GistVaultClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, Address, Bytes, Env, String};

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

    // Deploy token (native XLM SAC)
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    // Deploy GistRegistry
    let registry_id = env.register_contract(None, GistRegistry);
    let registry_client = GistRegistryClient::new(&env, &registry_id);
    let registry_admin = Address::generate(&env);
    registry_client.initialize(&registry_admin);

    // Deploy GistVault, wired to the registry
    let vault_id = env.register_contract(None, GistVault);
    let vault_client = GistVaultClient::new(&env, &vault_id);
    let vault_admin = Address::generate(&env);
    vault_client.initialize(&token_id, &vault_admin, &registry_id);

    // Fund a tipper
    let tipper = Address::generate(&env);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
    token_admin_client.mint(&tipper, &1_000_000);

    (
        env,
        tipper,
        token_id,
        vault_admin,
        vault_client,
        registry_client,
    )
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let (env, _tipper, token_id, vault_admin, vault_client, _registry_client) = setup();
    let registry_id = env.register_contract(None, GistRegistry);
    vault_client.initialize(&token_id, &vault_admin, &registry_id);
}

#[test]
fn test_get_pending_balance_starts_at_zero() {
    let (env, _tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let author = Address::generate(&env);
    assert_eq!(vault_client.get_pending_balance(&author), 0i128);
}

#[test]
fn test_tip_and_get_pending_balance() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    // Post a gist so the vault can verify it
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(&tipper, &author, &gist_id, &500_000i128);
    assert_eq!(vault_client.get_pending_balance(&author), 500_000i128);
}

#[test]
fn test_multiple_tips_accumulate_pending_balance() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(&tipper, &author, &gist_id, &200_000i128);
    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128);
    assert_eq!(vault_client.get_pending_balance(&author), 500_000i128);
}

#[test]
fn test_tips_to_different_authors_are_independent() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id1 = registry_client.post_gist(&ipfs_cid, &geohash, &author1, &None);
    let gist_id2 = registry_client.post_gist(&ipfs_cid, &geohash, &author2, &None);

    vault_client.tip_author(&tipper, &author1, &gist_id1, &100_000i128);
    vault_client.tip_author(&tipper, &author2, &gist_id2, &200_000i128);

    assert_eq!(vault_client.get_pending_balance(&author1), 100_000i128);
    assert_eq!(vault_client.get_pending_balance(&author2), 200_000i128);
}

#[test]
fn test_get_total_tips_for_gist() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128);
    assert_eq!(vault_client.get_total_tips_for_gist(&gist_id), 100_000i128);
}

#[test]
#[should_panic(expected = "tip amount must be positive")]
fn test_tip_zero_amount_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    vault_client.tip_author(&tipper, &author, &gist_id, &0i128);
}

#[test]
fn test_claim_tips_transfers_and_clears_balance() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128);
    let claimed = vault_client.claim_tips(&author);
    assert_eq!(claimed, 300_000i128);
    assert_eq!(vault_client.get_pending_balance(&author), 0i128);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&author), 300_000i128);
}

#[test]
#[should_panic(expected = "no pending tips to claim")]
fn test_claim_tips_zero_balance_panics() {
    let (env, _tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let author = Address::generate(&env);
    vault_client.claim_tips(&author);
}

#[test]
fn test_tip_after_claim_starts_fresh() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128);
    vault_client.claim_tips(&author);
    vault_client.tip_author(&tipper, &author, &gist_id, &50_000i128);

    assert_eq!(vault_client.get_pending_balance(&author), 50_000i128);
}

// ── gist existence verification ──────────────────────────────────────────────

#[test]
#[should_panic(expected = "gist not found in GistRegistry")]
fn test_tip_nonexistent_gist_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let recipient = Address::generate(&env);
    vault_client.tip_author(&tipper, &recipient, &999u64, &100_000i128);
}

#[test]
#[should_panic(expected = "recipient does not match gist author")]
fn test_tip_wrong_recipient_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let wrong_recipient = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    // Try to tip with a different recipient than the gist author
    vault_client.tip_author(&tipper, &wrong_recipient, &gist_id, &100_000i128);
}

#[test]
#[should_panic(expected = "gist is not active")]
fn test_tip_inactive_gist_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    registry_client.expire_gist(&author, &gist_id);

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128);
}

// ── admin: set_registry_address ──────────────────────────────────────────────

#[test]
fn test_set_registry_address() {
    let (env, tipper, _token_id, vault_admin, vault_client, _registry_client) = setup();
    let new_registry_id = env.register_contract(None, GistRegistry);
    let new_registry_client = GistRegistryClient::new(&env, &new_registry_id);
    let registry_admin = Address::generate(&env);
    new_registry_client.initialize(&registry_admin);

    // Admin updates the registry address
    vault_client.set_registry_address(&vault_admin, &new_registry_id);

    // Post a gist in the new registry
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = new_registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    // Tipping against the new registry works
    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128);
    assert_eq!(vault_client.get_pending_balance(&author), 100_000i128);
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_set_registry_address_wrong_admin_panics() {
    let (env, _tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let impostor = Address::generate(&env);
    let new_registry = Address::generate(&env);
    vault_client.set_registry_address(&impostor, &new_registry);
}

#[test]
#[should_panic(expected = "registry address not set")]
fn test_tip_before_registry_set_panics() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy a vault without initializing — no token or registry set
    let vault_id = env.register_contract(None, GistVault);
    let client = GistVaultClient::new(&env, &vault_id);

    let tipper = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.tip_author(&tipper, &recipient, &1u64, &100_000i128);
}
