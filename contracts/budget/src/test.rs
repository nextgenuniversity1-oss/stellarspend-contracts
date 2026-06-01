#![cfg(test)]

use super::{Beneficiary, BudgetContract, BudgetContractClient};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol};

fn setup() -> (Env, Address, Address, BudgetContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(BudgetContract, ());
    let client = BudgetContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    (env, admin, user, client)
}

fn setup_categories(
    client: &BudgetContractClient,
    admin: &Address,
    user: &Address,
) -> (Symbol, Symbol) {
    let food = symbol_short!("food");
    let travel = symbol_short!("travel");
    client.set_category_budget(admin, user, &food, &1_000);
    client.set_category_budget(admin, user, &travel, &500);
    (food, travel)
}

#[test]
fn test_initialize() {
    let (_, admin, _, client) = setup();
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_suspicious_activity_count(), 0);
}

#[test]
fn test_transfer_between_categories() {
    let (_, admin, user, client) = setup();
    let (food, travel) = setup_categories(&client, &admin, &user);

    let transfer_id = client.transfer_between_categories(&user, &food, &travel, &200);

    assert_eq!(transfer_id, 1);
    assert_eq!(client.get_category_balance(&user, &food), 800);
    assert_eq!(client.get_category_balance(&user, &travel), 700);

    let history = client.get_transfer_history(&user);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().amount, 200);
    assert_eq!(history.get(0).unwrap().from_category, food);
    assert_eq!(history.get(0).unwrap().to_category, travel);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_transfer_insufficient_balance() {
    let (_, admin, user, client) = setup();
    let (food, travel) = setup_categories(&client, &admin, &user);
    client.transfer_between_categories(&user, &food, &travel, &2_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_transfer_same_category() {
    let (_, admin, user, client) = setup();
    let (food, _) = setup_categories(&client, &admin, &user);
    client.transfer_between_categories(&user, &food, &food, &100);
}

#[test]
fn test_spend_and_remaining_balance() {
    let (_, admin, user, client) = setup();
    let (food, _) = setup_categories(&client, &admin, &user);

    let remaining = client.spend_from_category(&user, &food, &300);
    assert_eq!(remaining, 700);
    assert_eq!(client.get_category_balance(&user, &food), 700);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_rapid_spending_triggers_freeze() {
    let (_, admin, user, client) = setup();
    let (food, _) = setup_categories(&client, &admin, &user);

    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_frozen_budget_blocks_transfer() {
    let (_, admin, user, client) = setup();
    let (food, travel) = setup_categories(&client, &admin, &user);

    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);

    assert!(client.is_frozen(&user));
    client.transfer_between_categories(&user, &food, &travel, &50);
}

#[test]
fn test_manual_unfreeze() {
    let (_, admin, user, client) = setup();
    let (food, _) = setup_categories(&client, &admin, &user);

    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);

    assert!(client.is_frozen(&user));
    client.unfreeze_budget(&admin, &user);
    assert!(!client.is_frozen(&user));

    let remaining = client.spend_from_category(&user, &food, &10);
    assert_eq!(remaining, 960);
}

#[test]
fn test_user_can_unfreeze_own_budget() {
    let (_, _, user, client) = setup();
    let admin = client.get_admin();
    let (food, _) = setup_categories(&client, &admin, &user);

    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);
    client.spend_from_category(&user, &food, &10);

    client.unfreeze_budget(&user, &user);
    assert!(!client.is_frozen(&user));
}

#[test]
fn test_transfer_history_preserved() {
    let (_, admin, user, client) = setup();
    let (food, travel) = setup_categories(&client, &admin, &user);

    client.transfer_between_categories(&user, &food, &travel, &100);
    client.transfer_between_categories(&user, &travel, &food, &50);

    let history = client.get_transfer_history(&user);
    assert_eq!(history.len(), 2);

    let first = client.get_transfer(&1);
    assert_eq!(first.amount, 100);
    let second = client.get_transfer(&2);
    assert_eq!(second.amount, 50);
}

#[test]
fn test_inactivity_timeout_and_ownership_transfer() {
    let (env, admin, owner, client) = setup();
    let beneficiary = Address::generate(&env);

    client.update_budget(&admin, &owner, &1_000, &None);
    client.set_category_budget(&admin, &owner, &symbol_short!("food"), &1_000);

    let inheritance = soroban_sdk::vec![&env, beneficiary.clone()];
    client.register_inheritance_beneficiaries(&owner, &inheritance);

    client.set_inactivity_timeout(&owner, &10);

    // Let's advance ledger timestamp by 15 seconds
    env.ledger().set_timestamp(15);

    client.claim_ownership(&beneficiary, &owner);

    // Ownership should be transferred
    assert_eq!(client.get_budget(&beneficiary).unwrap().amount, 1_000);
    assert!(client.get_budget(&owner).is_none());

    // Category should be transferred
    assert_eq!(
        client.get_category_balance(&beneficiary, &symbol_short!("food")),
        1_000
    );
}

#[test]
fn test_distribute_remaining_funds() {
    let (env, admin, owner, client) = setup();
    let beneficiary1 = Address::generate(&env);
    let beneficiary2 = Address::generate(&env);

    client.update_budget(&admin, &owner, &1_000, &None);

    let beneficiaries = soroban_sdk::vec![
        &env,
        Beneficiary {
            address: beneficiary1.clone(),
            percentage: 60,
        },
        Beneficiary {
            address: beneficiary2.clone(),
            percentage: 40,
        }
    ];

    client.register_beneficiaries(&owner, &beneficiaries);
    client.set_inactivity_timeout(&owner, &10);

    // Advance timestamp
    env.ledger().set_timestamp(15);

    client.distribute_remaining_funds(&beneficiary1, &owner);

    // Check balances/budgets
    assert_eq!(client.get_budget(&beneficiary1).unwrap().amount, 600);
    assert_eq!(client.get_budget(&beneficiary2).unwrap().amount, 400);
    assert!(client.get_budget(&owner).is_none());
}

#[test]
#[should_panic(expected = "Error(Contract, #15)")]
fn test_register_beneficiaries_invalid_percentages() {
    let (env, _, owner, client) = setup();
    let beneficiary1 = Address::generate(&env);
    let beneficiary2 = Address::generate(&env);

    let beneficiaries = soroban_sdk::vec![
        &env,
        Beneficiary {
            address: beneficiary1,
            percentage: 50,
        },
        Beneficiary {
            address: beneficiary2,
            percentage: 40, // 50 + 40 = 90 (not 100)
        }
    ];

    client.register_beneficiaries(&owner, &beneficiaries);
}
