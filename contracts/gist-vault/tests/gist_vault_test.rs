use gist_registry::{GistRegistry, GistRegistryClient};
use gist_vault::{GistVault, GistVaultClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{token, Address, Bytes, BytesN, Env, String};

/// Deterministic, distinct idempotency keys for tests — one seed per logical tip attempt.
fn idem_key(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

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

    vault_client.tip_author(&tipper, &author, &gist_id, &500_000i128, &idem_key(&env, 1));
    assert_eq!(vault_client.get_pending_balance(&author), 500_000i128);
}

#[test]
fn test_multiple_tips_accumulate_pending_balance() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(&tipper, &author, &gist_id, &200_000i128, &idem_key(&env, 1));
    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128, &idem_key(&env, 2));
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

    vault_client.tip_author(
        &tipper,
        &author1,
        &gist_id1,
        &100_000i128,
        &idem_key(&env, 1),
    );
    vault_client.tip_author(
        &tipper,
        &author2,
        &gist_id2,
        &200_000i128,
        &idem_key(&env, 2),
    );

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

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &idem_key(&env, 1));
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
    vault_client.tip_author(&tipper, &author, &gist_id, &0i128, &idem_key(&env, 1));
}

#[test]
fn test_claim_tips_transfers_and_clears_balance() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");

    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128, &idem_key(&env, 1));
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

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &idem_key(&env, 1));
    vault_client.claim_tips(&author);
    vault_client.tip_author(&tipper, &author, &gist_id, &50_000i128, &idem_key(&env, 2));

    assert_eq!(vault_client.get_pending_balance(&author), 50_000i128);
}

// ── gist existence verification ──────────────────────────────────────────────

#[test]
#[should_panic(expected = "gist not found in GistRegistry")]
fn test_tip_nonexistent_gist_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let recipient = Address::generate(&env);
    vault_client.tip_author(
        &tipper,
        &recipient,
        &999u64,
        &100_000i128,
        &idem_key(&env, 1),
    );
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
    vault_client.tip_author(
        &tipper,
        &wrong_recipient,
        &gist_id,
        &100_000i128,
        &idem_key(&env, 1),
    );
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

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &idem_key(&env, 1));
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
    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &idem_key(&env, 1));
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
    client.tip_author(&tipper, &recipient, &1u64, &100_000i128, &idem_key(&env, 1));
}

// ── idempotency ───────────────────────────────────────────────────────────────

#[test]
fn test_tip_retry_same_key_same_args_is_noop() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let key = idem_key(&env, 1);

    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128, &key);
    // Simulate a client retry after an ambiguous confirmation: same key, same args.
    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128, &key);
    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128, &key);

    // Only one tip should have actually landed.
    assert_eq!(vault_client.get_pending_balance(&author), 300_000i128);
    assert_eq!(vault_client.get_total_tips_for_gist(&gist_id), 300_000i128);

    // Prove it at the token level too, not just via the vault's own bookkeeping —
    // the tipper should only have been charged once.
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&tipper), 1_000_000i128 - 300_000i128);
}

#[test]
#[should_panic(expected = "idempotency key reused with different parameters")]
fn test_tip_retry_same_key_different_amount_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let key = idem_key(&env, 1);

    vault_client.tip_author(&tipper, &author, &gist_id, &300_000i128, &key);
    // Same key, different amount — a client bug or a collision, not a legitimate retry.
    vault_client.tip_author(&tipper, &author, &gist_id, &999_000i128, &key);
}

#[test]
#[should_panic(expected = "idempotency key reused with different parameters")]
fn test_tip_retry_same_key_different_recipient_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id1 = registry_client.post_gist(&ipfs_cid, &geohash, &author1, &None);
    let gist_id2 = registry_client.post_gist(&ipfs_cid, &geohash, &author2, &None);
    let key = idem_key(&env, 1);

    vault_client.tip_author(&tipper, &author1, &gist_id1, &100_000i128, &key);
    // Same key, different recipient/gist_id entirely.
    vault_client.tip_author(&tipper, &author2, &gist_id2, &100_000i128, &key);
}

#[test]
fn test_tip_different_keys_are_independent_even_with_identical_args() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    // Two genuinely separate tips that happen to share the same amount/recipient/gist —
    // distinct keys mean both should go through, not be mistaken for a retry.
    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &idem_key(&env, 1));
    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &idem_key(&env, 2));

    assert_eq!(vault_client.get_pending_balance(&author), 200_000i128);
}

#[test]
#[should_panic(expected = "idempotency key reused with different parameters")]
fn test_tip_retry_same_key_different_tipper_panics() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let key = idem_key(&env, 1);

    // A second, unrelated tipper happens to fund and reuse the exact same key —
    // whether that's a broken key-generation scheme on the client or a deliberate
    // collision attempt, it must not be treated as author's own retry.
    let other_tipper = Address::generate(&env);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
    token_admin_client.mint(&other_tipper, &1_000_000);

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &key);
    vault_client.tip_author(&other_tipper, &author, &gist_id, &100_000i128, &key);
}

#[test]
fn test_tip_retry_after_ttl_expiry_is_treated_as_fresh() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    // Post with the max allowed TTL (168h) so the gist itself outlives the ledger
    // jump below — this test isolates the idempotency record's own 48h TTL, not the
    // gist's separate (and shorter, by default) expiry.
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &Some(168));
    let key = idem_key(&env, 1);

    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &key);
    assert_eq!(vault_client.get_pending_balance(&author), 100_000i128);

    // Advancing the ledger far enough to expire the TipRecord would also archive
    // every contract's own instance storage (vault, registry, token) in the test
    // sandbox — a real long-lived deployment needs routine keep-alive TTL bumps
    // regardless of this feature, so extend those *before* the jump to isolate what
    // we're actually testing: the *temporary* TipRecord's shorter TTL. An entry that
    // has already gone archived can't simply have its TTL bumped afterward — on a
    // real network that requires an explicit restore, and the test sandbox enforces
    // the same rule.
    for contract_id in [&vault_client.address, &registry_client.address, &token_id] {
        env.as_contract(contract_id, || {
            env.storage().instance().extend_ttl(100_000, 100_000);
        });
    }

    // Advance well past the 48h / 34,560-ledger TTL window documented in
    // docs/GIST_VAULT.md — the idempotency record has expired by now.
    let current = env.ledger().sequence();
    env.ledger().set_sequence_number(current + 48 * 720 + 1);

    // This is the accepted residual risk, made concrete: a key reused after the TTL
    // window is treated as a brand new attempt, not protected forever. Same key,
    // same args — but it's now a second, real tip, not a no-op.
    vault_client.tip_author(&tipper, &author, &gist_id, &100_000i128, &key);
    assert_eq!(vault_client.get_pending_balance(&author), 200_000i128);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&tipper), 1_000_000i128 - 200_000i128);
}

#[test]
#[should_panic(expected = "balance overflow")]
fn test_tip_amounts_summing_past_i128_max_panic() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    // A single token balance (and therefore a single tip) can never itself exceed
    // i128::MAX — the token contract's own balance guard would reject that. So we use
    // two separate tippers, each funding an amount comfortably under i128::MAX, whose
    // *sum* overflows once both have been transferred into the vault's own holdings.
    //
    // Note this panics from the *token contract's* own balance-overflow guard
    // ("balance overflow in receive_balance" on the vault's own account), not from
    // GistVault's `checked_add` on `pending_balance`/`gist_total`. That guard is real
    // and correct, but realistically unreachable via this public API: an author's
    // pending balance (or a gist's tip total) can never exceed the vault's own
    // aggregate token holdings, since every credited stroop corresponds to a stroop
    // already actually transferred in — so the token's own i128 ceiling on the
    // vault's account is always hit first. It remains as defense-in-depth in case
    // that invariant is ever broken by a future change.
    let big: i128 = i128::MAX / 2 + 1;
    let other_tipper = Address::generate(&env);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
    token_admin_client.mint(&tipper, &big);
    token_admin_client.mint(&other_tipper, &big);

    vault_client.tip_author(&tipper, &author, &gist_id, &big, &idem_key(&env, 1));
    vault_client.tip_author(&other_tipper, &author, &gist_id, &big, &idem_key(&env, 2));
}
