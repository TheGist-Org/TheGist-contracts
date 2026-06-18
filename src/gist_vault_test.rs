use soroban_sdk::{Address, Env, U256};
use soroban_sdk::testutils::Address as _;
use crate::gist_vault::GistVault;

#[test]
fn test_get_tip_balance_starts_at_zero() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = crate::gist_vault::GistVaultClient::new(&env, &contract_id);

    let author = Address::generate(&env);
    assert_eq!(client.get_tip_balance(&author), U256::from_u128(&env, 0));
}
