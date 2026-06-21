use soroban_sdk::{Address, Env, String};
use soroban_sdk::testutils::Address as _;
use the_gist_contracts::{LocationVerifier, LocationVerifierClient};

#[test]
fn test_set_and_get_registry_address() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LocationVerifier);
    let client = LocationVerifierClient::new(&env, &contract_id);

    let registry_address = Address::generate(&env);
    client.set_registry_address(&registry_address);

    assert_eq!(client.get_registry_address(), Some(registry_address));
}

#[test]
fn test_add_allowed_prefix_and_verify_geohash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LocationVerifier);
    let client = LocationVerifierClient::new(&env, &contract_id);

    client.add_allowed_prefix(&String::from_slice(&env, "u4pruy"));

    assert!(client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
    assert!(!client.verify_geohash(&String::from_slice(&env, "dr5ru7k")));
}

#[test]
fn test_update_boundaries_replaces_prefixes() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LocationVerifier);
    let client = LocationVerifierClient::new(&env, &contract_id);

    client.add_allowed_prefix(&String::from_slice(&env, "u4pruy"));
    client.update_boundaries(&String::from_slice(&env, "dr5ru7"));

    assert!(client.verify_geohash(&String::from_slice(&env, "dr5ru7k")));
    assert!(!client.verify_geohash(&String::from_slice(&env, "u4pruyd")));
}
