use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, Env, String};
use the_gist_contracts::{GistRegistry, GistRegistryClient, LocationVerifier, LocationVerifierClient};

fn setup_verifier(env: &Env) -> (Address, LocationVerifierClient) {
    let admin = Address::generate(env);
    let contract_id = env.register_contract(None, LocationVerifier);
    let client = LocationVerifierClient::new(env, &contract_id);
    client.initialize(&admin);
    (admin, client)
}

#[test]
fn test_set_and_get_registry_address() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, client) = setup_verifier(&env);

    let registry_address = Address::generate(&env);
    client.set_registry_address(&admin, &registry_address);

    assert_eq!(client.get_registry_address(), Some(registry_address));
}

#[test]
fn test_add_allowed_prefix_and_verify_geohash() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup_verifier(&env);

    client.add_allowed_prefix(&String::from_slice(&env, "u4pruy"));

    assert!(client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
    assert!(!client.verify_geohash(&String::from_slice(&env, "dr5ru7k")));
}

#[test]
fn test_update_boundaries_replaces_prefixes() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, client) = setup_verifier(&env);

    client.add_allowed_prefix(&String::from_slice(&env, "u4pruy"));
    client.update_boundaries(&String::from_slice(&env, "dr5ru7"));

    assert!(client.verify_geohash(&String::from_slice(&env, "dr5ru7k")));
    assert!(!client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
}

#[test]
fn test_verify_and_post_valid_geohash() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, verifier) = setup_verifier(&env);

    // Deploy GistRegistry
    let registry_id = env.register_contract(None, GistRegistry);
    let registry = GistRegistryClient::new(&env, &registry_id);
    let reg_admin = Address::generate(&env);
    registry.initialize(&reg_admin);

    // Wire registry into verifier (admin auth mocked)
    verifier.set_registry_address(&_admin, &registry_id);
    verifier.add_allowed_prefix(&String::from_slice(&env, "u4pruy"));

    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCidXXXXXXX");
    let geohash = String::from_slice(&env, "u4pruyd");

    let gist_id = verifier.verify_and_post(&author, &cid, &geohash, &None);
    assert_eq!(gist_id, 1u64);
}

#[test]
#[should_panic(expected = "invalid or disallowed location cell")]
fn test_verify_and_post_invalid_geohash_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, verifier) = setup_verifier(&env);

    let registry_id = env.register_contract(None, GistRegistry);
    verifier.set_registry_address(&_admin, &registry_id);
    verifier.add_allowed_prefix(&String::from_slice(&env, "u4pruy"));

    let author = Address::generate(&env);
    let cid = Bytes::from_slice(&env, b"QmTestCidXXXXXXX");
    let bad_geohash = String::from_slice(&env, "dr5ru7k");

    verifier.verify_and_post(&author, &cid, &bad_geohash, &None);
}
