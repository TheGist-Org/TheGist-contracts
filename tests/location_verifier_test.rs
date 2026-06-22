#![cfg(test)]

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, Env, String};
use the_gist_contracts::{
    GistRegistry, GistRegistryClient, LocationVerifier, LocationVerifierClient,
};

fn setup(env: &Env) -> (LocationVerifierClient, Address) {
    let contract_id = env.register_contract(None, LocationVerifier);
    let client = LocationVerifierClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (client, admin)
}

// ── is_valid_geohash ──────────────────────────────────────────────────────────

#[test]
fn test_valid_geohash_7_chars() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(client.is_valid_geohash(&String::from_slice(&env, "u4pruyd")));
}

#[test]
fn test_valid_geohash_all_alphabet_chars() {
    let env = Env::default();
    let (client, _) = setup(&env);
    // uses valid geohash chars only
    assert!(client.is_valid_geohash(&String::from_slice(&env, "dr5ru7k")));
}

#[test]
fn test_invalid_geohash_length_6() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruy")));
}

#[test]
fn test_invalid_geohash_length_8() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyde")));
}

#[test]
fn test_invalid_geohash_char_a() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4prayd")));
}

#[test]
fn test_invalid_geohash_char_i() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4priyd")));
}

#[test]
fn test_invalid_geohash_char_l() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyl")));
}

#[test]
fn test_invalid_geohash_char_o() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "u4pruyo")));
}

#[test]
fn test_invalid_geohash_empty() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.is_valid_geohash(&String::from_slice(&env, "")));
}

// ── add_allowed_prefix ────────────────────────────────────────────────────────

#[test]
fn test_admin_adds_prefix_successfully() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
    assert!(client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
}

#[test]
#[should_panic(expected = "prefix length must not exceed 6")]
fn test_add_prefix_too_long_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "u4pruyd")); // 7 chars
}

#[test]
#[should_panic(expected = "prefix cannot be empty")]
fn test_add_empty_prefix_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, ""));
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_add_prefix_non_admin_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let non_admin = Address::generate(&env);
    client.add_allowed_prefix(&non_admin, &String::from_slice(&env, "u4pruy"));
}

// ── remove_allowed_prefix ─────────────────────────────────────────────────────

#[test]
fn test_admin_removes_existing_prefix() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
    assert!(client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
    client.remove_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
    assert!(!client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
}

#[test]
#[should_panic(expected = "prefix not found")]
fn test_remove_nonexistent_prefix_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    client.remove_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
}

#[test]
#[should_panic(expected = "caller is not the admin")]
fn test_remove_prefix_non_admin_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
    let non_admin = Address::generate(&env);
    client.remove_allowed_prefix(&non_admin, &String::from_slice(&env, "u4pruy"));
}

// ── verify_geohash ────────────────────────────────────────────────────────────

#[test]
fn test_geohash_valid_chars_but_not_in_allowed_prefix_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    client.add_allowed_prefix(&admin, &String::from_slice(&env, "u4pruy"));
    // valid geohash-7 but different prefix
    assert!(!client.verify_geohash(&String::from_slice(&env, "dr5ru7k")));
}

#[test]
fn test_verify_geohash_no_prefixes_returns_false() {
    let env = Env::default();
    let (client, _) = setup(&env);
    assert!(!client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
}

// ── verify_and_post integration ───────────────────────────────────────────────

fn setup_with_registry(env: &Env) -> (LocationVerifierClient, Address, Address) {
    // Deploy GistRegistry
    let registry_id = env.register_contract(None, GistRegistry);
    let registry_client = GistRegistryClient::new(env, &registry_id);
    let registry_admin = Address::generate(env);
    registry_client.initialize(&registry_admin);

    // Deploy LocationVerifier
    let verifier_id = env.register_contract(None, LocationVerifier);
    let verifier_client = LocationVerifierClient::new(env, &verifier_id);
    let verifier_admin = Address::generate(env);
    verifier_client.initialize(&verifier_admin);
    verifier_client.set_registry_address(&verifier_admin, &registry_id);
    verifier_client.add_allowed_prefix(&verifier_admin, &String::from_slice(env, "u4pruy"));

    (verifier_client, verifier_admin, registry_id)
}

#[test]
fn test_verify_and_post_valid_call_returns_gist_id() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let (client, _, _) = setup_with_registry(&env);

    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = client.verify_and_post(&cid, &geohash, &author, &None);
    assert_eq!(gist_id, 1);
}

#[test]
#[should_panic(expected = "invalid geohash")]
fn test_verify_and_post_invalid_geohash_reverts() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let (client, _, _) = setup_with_registry(&env);

    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890");
    let geohash = String::from_slice(&env, "u4pruy"); // only 6 chars

    client.verify_and_post(&cid, &geohash, &author, &None);
}

#[test]
#[should_panic(expected = "geohash not in allowed boundaries")]
fn test_verify_and_post_out_of_boundary_reverts() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let (client, _, _) = setup_with_registry(&env);

    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890");
    let geohash = String::from_slice(&env, "dr5ru7k"); // valid geohash but wrong prefix

    client.verify_and_post(&cid, &geohash, &author, &None);
}

#[test]
fn test_verify_and_post_increments_gist_id() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let (client, _, _) = setup_with_registry(&env);

    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCid1234567890");
    let geohash = String::from_slice(&env, "u4pruyd");

    let id1 = client.verify_and_post(&cid, &geohash, &author, &None);
    let id2 = client.verify_and_post(&cid, &geohash, &author, &None);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_set_and_get_registry_address() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);
    let registry = Address::generate(&env);
    client.set_registry_address(&admin, &registry);
    assert_eq!(client.get_registry_address(), Some(registry));
}
