use gist_registry::{GistRegistry, GistRegistryClient};
use gist_vault::{BatchTipItem, GistVault, GistVaultClient};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::{token, Address, Bytes, BytesN, Env, FromVal, String};

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

// ── protocol fee: set_fee_bps / set_treasury ────────────────────────────────

#[test]
fn test_set_fee_bps_by_admin() {
    let (_env, _tipper, _token_id, vault_admin, vault_client, _registry_client) = setup();
    vault_client.set_fee_bps(&vault_admin, &500u32);
    // No panic means success — we verify indirectly via tipping below.
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_set_fee_bps_wrong_admin_panics() {
    let (env, _tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let impostor = Address::generate(&env);
    vault_client.set_fee_bps(&impostor, &500u32);
}

#[test]
#[should_panic(expected = "fee exceeds maximum of 1000 bps (10%)")]
fn test_set_fee_bps_over_max_panics() {
    let (_env, _tipper, _token_id, vault_admin, vault_client, _registry_client) = setup();
    vault_client.set_fee_bps(&vault_admin, &1001u32);
}

#[test]
fn test_set_fee_bps_at_max_is_allowed() {
    let (_env, _tipper, _token_id, vault_admin, vault_client, _registry_client) = setup();
    vault_client.set_fee_bps(&vault_admin, &1000u32);
}

#[test]
fn test_set_treasury_by_admin() {
    let (env, _tipper, _token_id, vault_admin, vault_client, _registry_client) = setup();
    let treasury = Address::generate(&env);
    vault_client.set_treasury(&vault_admin, &treasury);
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_set_treasury_wrong_admin_panics() {
    let (env, _tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let impostor = Address::generate(&env);
    let treasury = Address::generate(&env);
    vault_client.set_treasury(&impostor, &treasury);
}

// ── protocol fee: 0% fee (default) ─────────────────────────────────────────

#[test]
fn test_tip_with_zero_fee_configured_no_split() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    // Explicitly set fee to 0
    vault_client.set_fee_bps(&vault_admin, &0u32);

    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );

    // Author gets the full amount
    assert_eq!(vault_client.get_pending_balance(&author), 1_000_000i128);
    assert_eq!(
        vault_client.get_total_tips_for_gist(&gist_id),
        1_000_000i128
    );

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&tipper), 0i128);
}

// ── protocol fee: normal 5% rate ────────────────────────────────────────────

#[test]
fn test_tip_with_fee_splits_correctly() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32); // 5%

    // tip = 1_000_000, fee = 1_000_000 * 500 / 10_000 = 50_000, net = 950_000
    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );

    assert_eq!(vault_client.get_pending_balance(&author), 950_000i128);
    assert_eq!(
        vault_client.get_total_tips_for_gist(&gist_id),
        1_000_000i128
    );

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&treasury), 50_000i128);
    assert_eq!(token_client.balance(&author), 0i128);
}

// ── protocol fee: maximum 10% rate ──────────────────────────────────────────

#[test]
fn test_tip_with_max_fee_10_percent() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &1000u32); // 10%

    // tip = 1_000_000, fee = 1_000_000 * 1000 / 10_000 = 100_000, net = 900_000
    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );

    assert_eq!(vault_client.get_pending_balance(&author), 900_000i128);
    assert_eq!(
        vault_client.get_total_tips_for_gist(&gist_id),
        1_000_000i128
    );

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&treasury), 100_000i128);
}

// ── protocol fee: rounding → fee = 0 ────────────────────────────────────────

#[test]
fn test_tip_small_amount_fee_rounds_to_zero() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32); // 5%

    // tip = 1, fee = 1 * 500 / 10_000 = 0 (floor rounding), net = 1
    vault_client.tip_author(&tipper, &author, &gist_id, &1i128, &idem_key(&env, 1));

    assert_eq!(vault_client.get_pending_balance(&author), 1i128);
    assert_eq!(vault_client.get_total_tips_for_gist(&gist_id), 1i128);

    let token_client = token::Client::new(&env, &token_id);
    // Treasury gets 0 — no transfer attempted (zero-value transfer skipped)
    assert_eq!(token_client.balance(&treasury), 0i128);
}

// ── protocol fee: floor rounding behavior ───────────────────────────────────

#[test]
fn test_tip_floor_rounding_favors_recipient() {
    let (env, tipper, _token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &333u32); // 3.33%

    // tip = 100, fee = 100 * 333 / 10_000 = 3 (floor), net = 97
    vault_client.tip_author(&tipper, &author, &gist_id, &100i128, &idem_key(&env, 1));
    assert_eq!(vault_client.get_pending_balance(&author), 97i128);

    // tip = 101, fee = 101 * 333 / 10_000 = 3 (floor), net = 98
    vault_client.tip_author(&tipper, &author, &gist_id, &101i128, &idem_key(&env, 2));
    assert_eq!(vault_client.get_pending_balance(&author), 97 + 98);
}

// ── protocol fee: gross vs net accounting ───────────────────────────────────

#[test]
fn test_gist_total_records_gross_not_net() {
    let (env, tipper, _token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32); // 5%

    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );

    // Gist total = gross (1_000_000), pending balance = net (950_000)
    assert_eq!(
        vault_client.get_total_tips_for_gist(&gist_id),
        1_000_000i128
    );
    assert_eq!(vault_client.get_pending_balance(&author), 950_000i128);
}

// ── protocol fee: missing treasury when fees enabled ────────────────────────

#[test]
#[should_panic(expected = "fee_bps > 0 but no treasury configured")]
fn test_tip_with_fee_no_treasury_panics() {
    let (env, tipper, _token_id, vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    // Set fee but no treasury
    vault_client.set_fee_bps(&vault_admin, &500u32);

    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );
}

// ── protocol fee: correct treasury transfer ─────────────────────────────────

#[test]
fn test_fee_transferred_to_treasury_not_tipper() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &100u32); // 1%

    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );

    // Fee goes to treasury, not to tipper or author directly
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&treasury), 10_000i128); // 1% of 1M
    assert_eq!(token_client.balance(&tipper), 0i128); // tipper paid 1M
    assert_eq!(token_client.balance(&author), 0i128); // author pending, not claimed
}

// ── protocol fee: claim works with net balance ──────────────────────────────

#[test]
fn test_claim_after_tip_with_fee() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32); // 5%

    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );
    let claimed = vault_client.claim_tips(&author);
    assert_eq!(claimed, 950_000i128);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&author), 950_000i128);
}

// ── protocol fee: multiple tips accumulate correctly ────────────────────────

#[test]
fn test_multiple_tips_with_fee_accumulate() {
    let (env, tipper, _token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32); // 5%

    // Fund the tipper with enough for two tips
    let token_admin_client = token::StellarAssetClient::new(&env, &_token_id);
    token_admin_client.mint(&tipper, &5_000_000);

    // Tip 1: fee=50_000, net=950_000; Tip 2: fee=100_000, net=1_900_000
    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );
    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &2_000_000i128,
        &idem_key(&env, 2),
    );

    assert_eq!(vault_client.get_pending_balance(&author), 2_850_000i128); // 950K + 1.9M
    assert_eq!(
        vault_client.get_total_tips_for_gist(&gist_id),
        3_000_000i128
    ); // gross
}

// ── protocol fee: idempotent retry respects fee ────────────────────────────

#[test]
fn test_idempotent_retry_no_second_fee_transfer() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let key = idem_key(&env, 1);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32);

    vault_client.tip_author(&tipper, &author, &gist_id, &1_000_000i128, &key);
    // Retry with same key — should be a no-op
    vault_client.tip_author(&tipper, &author, &gist_id, &1_000_000i128, &key);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&treasury), 50_000i128); // fee transferred once only
    assert_eq!(vault_client.get_pending_balance(&author), 950_000i128); // net credited once
}

// ── protocol fee: event data ────────────────────────────────────────────────

#[test]
fn test_tip_event_emitted_with_fee_fields() {
    let (env, tipper, _token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32);

    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );

    // Verify GistTipped event was emitted — topics = [vault, tipped]
    let events = env.events().all();
    let tipped_count = events
        .iter()
        .filter(|e| {
            e.1.len() >= 2
                && soroban_sdk::Symbol::from_val(&env, &e.1.get_unchecked(0))
                    == soroban_sdk::symbol_short!("vault")
                && soroban_sdk::Symbol::from_val(&env, &e.1.get_unchecked(1))
                    == soroban_sdk::symbol_short!("tipped")
        })
        .count();
    assert_eq!(tipped_count, 1, "expected exactly one GistTipped event");
}

#[test]
fn test_tip_event_zero_fee_emitted() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.tip_author(
        &tipper,
        &author,
        &gist_id,
        &1_000_000i128,
        &idem_key(&env, 1),
    );

    let events = env.events().all();
    let tipped_count = events
        .iter()
        .filter(|e| {
            e.1.len() >= 2
                && soroban_sdk::Symbol::from_val(&env, &e.1.get_unchecked(0))
                    == soroban_sdk::symbol_short!("vault")
                && soroban_sdk::Symbol::from_val(&env, &e.1.get_unchecked(1))
                    == soroban_sdk::symbol_short!("tipped")
        })
        .count();
    assert_eq!(tipped_count, 1, "expected exactly one GistTipped event");
}

// ── protocol fee: odd tip amounts with various rates ────────────────────────

#[test]
fn test_tip_various_amounts_with_fee() {
    let (env, tipper, _token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &333u32); // 3.33%

    // 7 stroops: 7 * 333 / 10_000 = 0 (floor), net = 7
    vault_client.tip_author(&tipper, &author, &gist_id, &7i128, &idem_key(&env, 1));
    assert_eq!(vault_client.get_pending_balance(&author), 7i128);

    // 300: 300 * 333 / 10_000 = 9 (floor), net = 291
    vault_client.tip_author(&tipper, &author, &gist_id, &300i128, &idem_key(&env, 2));
    assert_eq!(vault_client.get_pending_balance(&author), 298i128);

    // 3001: 3001 * 333 / 10_000 = 99 (floor), net = 2902
    vault_client.tip_author(&tipper, &author, &gist_id, &3001i128, &idem_key(&env, 3));
    assert_eq!(vault_client.get_pending_balance(&author), 298 + 2902);
}

// ── batch tipping: tip_authors ────────────────────────────────────────────

fn batch_item(recipient: &Address, gist_id: u64, amount: i128) -> BatchTipItem {
    BatchTipItem {
        recipient: recipient.clone(),
        gist_id,
        amount,
    }
}

#[test]
fn test_tip_authors_basic_batch() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id1 = registry_client.post_gist(&ipfs_cid, &geohash, &author1, &None);
    let gist_id2 = registry_client.post_gist(&ipfs_cid, &geohash, &author2, &None);

    let tips = soroban_sdk::vec![
        &env,
        batch_item(&author1, gist_id1, 100_000),
        batch_item(&author2, gist_id2, 200_000),
    ];

    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));

    assert_eq!(vault_client.get_pending_balance(&author1), 100_000);
    assert_eq!(vault_client.get_pending_balance(&author2), 200_000);
    assert_eq!(vault_client.get_total_tips_for_gist(&gist_id1), 100_000);
    assert_eq!(vault_client.get_total_tips_for_gist(&gist_id2), 200_000);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&tipper), 1_000_000 - 300_000);
}

#[test]
fn test_tip_authors_single_item_batch() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let tips = soroban_sdk::vec![&env, batch_item(&author, gist_id, 500_000)];
    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));

    assert_eq!(vault_client.get_pending_balance(&author), 500_000);
}

#[test]
#[should_panic(expected = "batch must not be empty")]
fn test_tip_authors_empty_batch_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, _registry_client) = setup();
    let tips: soroban_sdk::Vec<BatchTipItem> = soroban_sdk::vec![&env];
    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));
}

#[test]
#[should_panic(expected = "batch size exceeds maximum of 20")]
fn test_tip_authors_over_cap_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let tips: soroban_sdk::Vec<BatchTipItem> = soroban_sdk::vec![
        &env,
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000),
        batch_item(&author, gist_id, 1_000), // 21st item
    ];
    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));
}

#[test]
#[should_panic(expected = "tip amount must be positive")]
fn test_tip_authors_zero_amount_reverts_all() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let tips = soroban_sdk::vec![
        &env,
        batch_item(&author, gist_id, 100_000),
        batch_item(&author, gist_id, 0), // invalid
    ];
    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));
}

#[test]
#[should_panic(expected = "gist not found in GistRegistry")]
fn test_tip_authors_nonexistent_gist_reverts_all() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let tips = soroban_sdk::vec![
        &env,
        batch_item(&author, gist_id, 100_000),
        batch_item(&author, 999, 100_000), // nonexistent
    ];
    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));
}

#[test]
fn test_tip_authors_idempotent_retry_same_args() {
    let (env, tipper, token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let key = idem_key(&env, 1);

    let tips = soroban_sdk::vec![
        &env,
        batch_item(&author, gist_id, 100_000),
        batch_item(&author, gist_id, 200_000),
    ];

    vault_client.tip_authors(&tipper, &tips, &key);
    // Retry with same key and same args — should be a no-op.
    vault_client.tip_authors(&tipper, &tips, &key);

    assert_eq!(vault_client.get_pending_balance(&author), 300_000);
    assert_eq!(vault_client.get_total_tips_for_gist(&gist_id), 300_000);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&tipper), 1_000_000 - 300_000);
}

#[test]
#[should_panic(expected = "idempotency key reused with different parameters")]
fn test_tip_authors_idempotent_different_args_panics() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);
    let key = idem_key(&env, 1);

    let tips1 = soroban_sdk::vec![&env, batch_item(&author, gist_id, 100_000)];
    let tips2 = soroban_sdk::vec![&env, batch_item(&author, gist_id, 200_000)];

    vault_client.tip_authors(&tipper, &tips1, &key);
    vault_client.tip_authors(&tipper, &tips2, &key); // different amount
}

#[test]
fn test_tip_authors_with_fee_split() {
    let (env, tipper, token_id, vault_admin, vault_client, registry_client) = setup();
    let treasury = Address::generate(&env);
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id1 = registry_client.post_gist(&ipfs_cid, &geohash, &author1, &None);
    let gist_id2 = registry_client.post_gist(&ipfs_cid, &geohash, &author2, &None);

    vault_client.set_treasury(&vault_admin, &treasury);
    vault_client.set_fee_bps(&vault_admin, &500u32); // 5%

    let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
    token_admin_client.mint(&tipper, &5_000_000);

    let tips = soroban_sdk::vec![
        &env,
        batch_item(&author1, gist_id1, 1_000_000),
        batch_item(&author2, gist_id2, 2_000_000),
    ];

    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));

    // author1: fee=50_000, net=950_000
    // author2: fee=100_000, net=1_900_000
    assert_eq!(vault_client.get_pending_balance(&author1), 950_000);
    assert_eq!(vault_client.get_pending_balance(&author2), 1_900_000);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&treasury), 150_000); // 50K + 100K
}

#[test]
fn test_tip_authors_emits_individual_events() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let tips = soroban_sdk::vec![
        &env,
        batch_item(&author, gist_id, 100_000),
        batch_item(&author, gist_id, 200_000),
    ];

    vault_client.tip_authors(&tipper, &tips, &idem_key(&env, 1));

    let events = env.events().all();
    let tipped_count = events
        .iter()
        .filter(|e| {
            e.1.len() >= 2
                && soroban_sdk::Symbol::from_val(&env, &e.1.get_unchecked(0))
                    == soroban_sdk::symbol_short!("vault")
                && soroban_sdk::Symbol::from_val(&env, &e.1.get_unchecked(1))
                    == soroban_sdk::symbol_short!("tipped")
        })
        .count();
    assert_eq!(
        tipped_count, 2,
        "expected one GistTipped event per batch item"
    );
}

#[test]
fn test_tip_authors_max_batch_size_allowed() {
    let (env, tipper, _token_id, _vault_admin, vault_client, registry_client) = setup();
    let author = Address::generate(&env);
    let ipfs_cid = Bytes::from_slice(&env, b"QmTest123");
    let geohash = String::from_str(&env, "u4pruyd");
    let gist_id = registry_client.post_gist(&ipfs_cid, &geohash, &author, &None);

    let mut tips_vec = soroban_sdk::vec![&env];
    for i in 0..20u32 {
        tips_vec.push_back(batch_item(&author, gist_id, (i as i128 + 1) * 1_000));
    }

    vault_client.tip_authors(&tipper, &tips_vec, &idem_key(&env, 1));

    // Sum of 1..20 * 1000 = 210_000
    assert_eq!(vault_client.get_pending_balance(&author), 210_000);
}
