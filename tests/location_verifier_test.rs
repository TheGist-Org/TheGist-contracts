#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Bytes, Env, String,
};
use the_gist_contracts::{
    GistRegistry, GistRegistryClient,
    LocationVerifier, LocationVerifierClient,
};

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────

fn setup() -> (Env, LocationVerifierClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, LocationVerifier);
    let client = LocationVerifierClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    // add a known good prefix
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
    (env, client)
}

fn admin_of(env: &Env, client: &LocationVerifierClient) -> Address {
    client.get_admin().unwrap()
}

// ──────────────────────────────────────────────
// is_valid_geohash tests
// ──────────────────────────────────────────────

#[test]
fn test_valid_geohash7_in_prefix_passes() {
    let (env, client) = setup();
    // "u4pruyd" starts with "u4pruy" — length 7, all valid chars
    assert!(client.is_valid_geohash(&String::from_slice(&env, "u4pruyd")));
}

#[test]
fn test_length_6_fails() {
    let (env, client) = setup();
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruy")));
}

#[test]
fn test_length_8_fails() {
    let (env, client) = setup();
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyd0")));
}

#[test]
fn test_invalid_char_a_fails() {
    let (env, client) = setup();
    // replace last char with 'a' (excluded from geohash alphabet)
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruya")));
}

#[test]
fn test_invalid_char_i_fails() {
    let (env, client) = setup();
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyi")));
}

#[test]
fn test_invalid_char_l_fails() {
    let (env, client) = setup();
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyl")));
}

#[test]
fn test_invalid_char_o_fails() {
    let (env, client) = setup();
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyo")));
}

#[test]
fn test_valid_chars_but_not_in_allowed_prefix_fails() {
    let (env, client) = setup();
    // "dr5ru7k" is a valid geohash-7 but prefix "dr5ru7" is not allowed
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "dr5ru7k")));
}

#[test]
fn test_empty_string_fails() {
    let (env, client) = setup();
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "")));
}

// ──────────────────────────────────────────────
// add_allowed_prefix tests
// ──────────────────────────────────────────────

#[test]
fn test_admin_adds_prefix_successfully() {
    let (env, client) = setup();
    let admin = admin_of(&env, &client);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "dr5ru7"));
    assert!(client.is_valid_geohash(&String::from_slice(&env, "dr5ru7k")));
}

#[test]
#[should_panic(expected = "prefix length cannot exceed 6")]
fn test_prefix_longer_than_6_rejected() {
    let (env, client) = setup();
    let admin = admin_of(&env, &client);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy0"));
}

#[test]
#[should_panic(expected = "prefix cannot be empty")]
fn test_empty_prefix_rejected() {
    let (env, client) = setup();
    let admin = admin_of(&env, &client);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, ""));
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_non_admin_add_rejected() {
    let (env, client) = setup();
    let non_admin = Address::generate(&env);
    client.add_allowed_prefix(&non_admin, &String::from_slice(&env, "dr5ru7"));
}

// ──────────────────────────────────────────────
// remove_allowed_prefix tests
// ──────────────────────────────────────────────

#[test]
fn test_admin_removes_existing_prefix() {
    let (env, client) = setup();
    let admin = admin_of(&env, &client);
    // prefix "u4pruy" was added in setup
    client.remove_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
    // now the geohash should no longer be valid
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyd")));
}

#[test]
#[should_panic(expected = "prefix not found")]
fn test_remove_nonexistent_prefix_panics() {
    let (env, client) = setup();
    let admin = admin_of(&env, &client);
    client.remove_allowed_prefix(&admin, &String::from_slice(&env, "xxxxxx"));
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_non_admin_remove_rejected() {
    let (env, client) = setup();
    let non_admin = Address::generate(&env);
    client.remove_allowed_prefix(&non_admin, &String::from_slice(&env, "u4pruy"));
}

// ──────────────────────────────────────────────
// verify_and_post integration tests
// ──────────────────────────────────────────────

fn setup_with_registry() -> (Env, LocationVerifierClient<'static>, GistRegistryClient<'static>) {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    // Deploy GistRegistry
    let reg_id = env.register_contract(None, GistRegistry);
    let reg_client = GistRegistryClient::new(&env, &reg_id);
    let reg_admin = Address::generate(&env);
    reg_client.initialize(&reg_admin);

    // Deploy LocationVerifier
    let lv_id = env.register_contract(None, LocationVerifier);
    let lv_client = LocationVerifierClient::new(&env, &lv_id);
    let lv_admin = Address::generate(&env);
    lv_client.initialize(&lv_admin);
    lv_client.add_allowed_prefix(&lv_admin, &String::from_slice(&env, "u4pruy"));
    lv_client.set_registry_address(&reg_id);

    (env, lv_client, reg_client)
}

#[test]
fn test_verify_and_post_valid_returns_gist_id() {
    let (env, lv_client, _) = setup_with_registry();
    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890123456789012345678901234");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = lv_client.verify_and_post(&geohash, &cid, &author, &None);
    assert_eq!(gist_id, 1u64);
}

#[test]
#[should_panic(expected = "geohash is not valid or not in an allowed region")]
fn test_verify_and_post_invalid_geohash_reverts() {
    let (env, lv_client, _) = setup_with_registry();
    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890123456789012345678901234");
    let geohash = String::from_slice(&env, "dr5ru7k"); // not in allowed prefix

    lv_client.verify_and_post(&geohash, &cid, &author, &None);
}

#[test]
fn test_verify_and_post_passes_correct_params() {
    let (env, lv_client, reg_client) = setup_with_registry();
    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890123456789012345678901234");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = lv_client.verify_and_post(&geohash, &cid, &author, &None);

    // Confirm the gist was written with correct params in the registry
    let gist = reg_client.get_gist(&gist_id).expect("gist should exist");
    assert_eq!(gist.geohash, geohash);
    assert_eq!(gist.ipfs_cid, cid);
    assert_eq!(gist.author, author);
}

#[test]
fn test_verify_and_post_increments_gist_id() {
    let (env, lv_client, _) = setup_with_registry();
    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890123456789012345678901234");
    let geohash = String::from_slice(&env, "u4pruyd");

    let id1 = lv_client.verify_and_post(&geohash, &cid, &author, &None);
    let id2 = lv_client.verify_and_post(&geohash, &cid, &author, &None);
    assert_eq!(id2, id1 + 1);
}

#[test]
fn test_invalid_character() {
    let (env, client, admin) = setup();

    client.add_allowed_prefix(
        &admin,
        &String::from_str(&env, "s17"),
    );

    assert!(
        !client.is_valid_geohash(
            &String::from_str(&env, "s17abi")
        )
    );
}

#[test]
fn test_outside_allowed_prefix() {
    let (env, client, admin) = setup();

    client.add_allowed_prefix(
        &admin,
        &String::from_str(&env, "s17"),
    );

    assert!(
        !client.is_valid_geohash(
            &String::from_str(&env, "u44abcd")
        )
    );
}

#[test]
fn test_add_and_remove_prefix() {
    let (env, client, admin) = setup();

    client.add_allowed_prefix(
        &admin,
        &String::from_str(&env, "s17"),
    );

    assert_eq!(
        client.get_allowed_prefixes().len(),
        1
    );

    client.remove_allowed_prefix(
        &admin,
        &String::from_str(&env, "s17"),
    );

    assert_eq!(
        client.get_allowed_prefixes().len(),
        0
    );
}