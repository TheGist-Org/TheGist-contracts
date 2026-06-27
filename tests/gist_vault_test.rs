use soroban_sdk::{Address, Env};
use the_gist_contracts::{GistVault, TipBalance};

#[test]
fn test_tip_author() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test addresses
    let recipient = Address::generate(&env);
    let amount = 100_00000; // 1 XLM in stroops

    // Send a tip
    client.tip_author(&recipient, &amount);

    // Verify balance
    let balance = client.get_tip_balance(&recipient);
    assert_eq!(balance.total_tips, amount);
    assert_eq!(balance.tip_count, 1);
}

#[test]
fn test_multiple_tips_to_same_author() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test addresses
    let recipient = Address::generate(&env);
    let amount1 = 50_00000; // 0.5 XLM
    let amount2 = 75_00000; // 0.75 XLM
    let amount3 = 25_00000; // 0.25 XLM

    // Send multiple tips
    client.tip_author(&recipient, &amount1);
    client.tip_author(&recipient, &amount2);
    client.tip_author(&recipient, &amount3);

    // Verify accumulated balance
    let balance = client.get_tip_balance(&recipient);
    assert_eq!(balance.total_tips, amount1 + amount2 + amount3);
    assert_eq!(balance.tip_count, 3);
}

#[test]
fn test_tip_multiple_authors() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test addresses
    let author1 = Address::generate(&env);
    let author2 = Address::generate(&env);
    let amount = 100_00000; // 1 XLM

    // Tip both authors
    client.tip_author(&author1, &amount);
    client.tip_author(&author2, &amount);

    // Verify balances are separate
    let balance1 = client.get_tip_balance(&author1);
    let balance2 = client.get_tip_balance(&author2);
    
    assert_eq!(balance1.total_tips, amount);
    assert_eq!(balance1.tip_count, 1);
    assert_eq!(balance2.total_tips, amount);
    assert_eq!(balance2.tip_count, 1);
}

#[test]
fn test_withdraw_tips() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test addresses
    let recipient = Address::generate(&env);
    let amount = 100_00000; // 1 XLM

    // Send a tip
    client.tip_author(&recipient, &amount);

    // Withdraw tips
    let withdrawn = client.withdraw_tips(&recipient);

    // Verify withdrawal amount
    assert_eq!(withdrawn, amount);

    // Verify balance is cleared
    let balance = client.get_tip_balance(&recipient);
    assert_eq!(balance.total_tips, 0);
    assert_eq!(balance.tip_count, 0);
}

#[test]
fn test_withdraw_partial_tips() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test addresses
    let recipient = Address::generate(&env);
    let amount1 = 50_00000; // 0.5 XLM
    let amount2 = 75_00000; // 0.75 XLM

    // Send multiple tips
    client.tip_author(&recipient, &amount1);
    client.tip_author(&recipient, &amount2);

    // Withdraw all tips
    let withdrawn = client.withdraw_tips(&recipient);

    // Verify withdrawal amount (total of both tips)
    assert_eq!(withdrawn, amount1 + amount2);

    // Verify balance is cleared
    let balance = client.get_tip_balance(&recipient);
    assert_eq!(balance.total_tips, 0);
}

#[test]
fn test_get_tip_balance_zero() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test address
    let recipient = Address::generate(&env);

    // Verify balance is zero for new address
    let balance = client.get_tip_balance(&recipient);
    assert_eq!(balance.total_tips, 0);
    assert_eq!(balance.tip_count, 0);
}

#[test]
#[should_panic(expected = "Tip amount must be positive")]
fn test_tip_zero_amount() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test addresses
    let recipient = Address::generate(&env);
    let amount = 0; // Invalid amount

    // Attempt to send zero tip (should panic)
    client.tip_author(&recipient, &amount);
}

#[test]
#[should_panic(expected = "No tips to withdraw")]
fn test_withdraw_no_tips() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test address
    let recipient = Address::generate(&env);

    // Attempt to withdraw with no tips (should panic)
    client.withdraw_tips(&recipient);
}

#[test]
fn test_tip_after_withdrawal() {
    let env = Env::default();
    let contract_id = env.register_contract(None, GistVault);
    let client = the_gist_contracts::GistVaultClient::new(&env, &contract_id);

    // Create test addresses
    let recipient = Address::generate(&env);
    let amount1 = 100_00000; // 1 XLM
    let amount2 = 50_00000; // 0.5 XLM

    // Send first tip
    client.tip_author(&recipient, &amount1);

    // Withdraw
    client.withdraw_tips(&recipient);

    // Send another tip
    client.tip_author(&recipient, &amount2);

    // Verify new balance
    let balance = client.get_tip_balance(&recipient);
    assert_eq!(balance.total_tips, amount2);
    assert_eq!(balance.tip_count, 1);
}
