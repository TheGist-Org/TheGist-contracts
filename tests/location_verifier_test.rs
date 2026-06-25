use soroban_sdk::{testutils::Address as _, Address, Env, String};
use the_gist_contracts::{LocationVerifier, LocationVerifierClient};

fn setup() -> (Env, LocationVerifierClient<'static>, Address) {
    let env = Env::default();

    env.mock_all_auths();

    let contract_id = env.register_contract(None, LocationVerifier);
    let client = LocationVerifierClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_set_and_get_registry_address() {
    let (env, client, _) = setup();

    let registry_address = Address::generate(&env);

    client.set_registry_address(&registry_address);

    assert_eq!(
        client.get_registry_address(),
        Some(registry_address)
    );
}

#[test]
fn test_valid_geohash() {
    let (env, client, admin) = setup();

    client.add_allowed_prefix(
        &admin,
        &String::from_str(&env, "s17"),
    );

    assert!(
        client.is_valid_geohash(
            &String::from_str(&env, "s17bcde")
        )
    );
}

#[test]
fn test_invalid_length() {
    let (env, client, admin) = setup();

    client.add_allowed_prefix(
        &admin,
        &String::from_str(&env, "s17"),
    );

    assert!(
        !client.is_valid_geohash(
            &String::from_str(&env, "s17ab")
        )
    );
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