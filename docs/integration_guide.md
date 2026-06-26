# Multi-Signature Savings Withdrawals - Integration Guide

## Quick Start

### Step 1: Initialize Configuration

```rust
use stellarspend_contracts::multisig_savings_withdrawal::*;

// Define authorized approvers
let approvers = vec![
    Address::from_string(&env, &String::from_str(&env, "GAPPROVER1")),
    Address::from_string(&env, &String::from_str(&env, "GAPPROVER2")),
    Address::from_string(&env, &String::from_str(&env, "GAPPROVER3")),
];

// Initialize with 2-of-3 approvals for withdrawals >= 10,000 stroops
initialize_withdrawal_config(&env, approvers, 2, 10_000);
```

### Step 2: Request Withdrawal

```rust
// Small withdrawal (< 10,000) - executes immediately
let id1 = request_withdrawal(&env, user1.clone(), 5_000, vault_id);

// Large withdrawal (>= 10,000) - requires approvals
let id2 = request_withdrawal(&env, user2.clone(), 50_000, vault_id);
```

### Step 3: Approve Withdrawal

```rust
// Approver 1 approves
approve_withdrawal(&env, approver1.clone(), id2);

// Approver 2 approves (quorum reached, auto-executes)
approve_withdrawal(&env, approver2.clone(), id2);

// Withdrawal now executed
assert_eq!(get_withdrawal_status(&env, id2), WithdrawalStatus::Executed);
```

### Step 4: Query Status

```rust
let request = get_withdrawal_request(&env, id2);
let status = get_withdrawal_status(&env, id2);
let approvals = get_withdrawal_approval_count(&env, id2);

println!("Amount: {}", request.amount);
println!("Status: {:?}", status);
println!("Approvals: {}/{}", approvals, get_withdrawal_quorum(&env));
```

## Integration Patterns

### Pattern 1: Direct Integration

Integrate directly with your savings contract:

```rust
#[contract]
pub struct SavingsContract;

#[contractimpl]
impl SavingsContract {
    pub fn initialize(
        env: Env,
        approvers: Vec<Address>,
        quorum: u32,
        threshold: i128,
    ) {
        initialize_withdrawal_config(&env, approvers, quorum, threshold);
    }

    pub fn withdraw(env: Env, amount: i128, vault_id: u64) -> u64 {
        let requester = env.current_contract_address();
        request_withdrawal(&env, requester, amount, vault_id)
    }

    pub fn approve(env: Env, withdrawal_id: u64) {
        let approver = env.current_contract_address();
        approve_withdrawal(&env, approver, withdrawal_id);
    }
}
```

### Pattern 2: Decorator Pattern

Wrap existing withdrawal logic:

```rust
pub fn withdraw_with_multisig(
    env: Env,
    amount: i128,
    vault_id: u64,
) -> Result<u64, WithdrawalError> {
    // Check if multisig is required
    if requires_approval(&env, amount) {
        // Create pending withdrawal
        let id = request_withdrawal(&env, env.current_contract_address(), amount, vault_id);
        Ok(id)
    } else {
        // Execute immediately
        execute_small_withdrawal(&env, amount, vault_id)
    }
}
```

### Pattern 3: Middleware Approach

Intercept withdrawals and apply multisig:

```rust
pub fn execute_withdrawal(env: Env, withdrawal_id: u64) -> Result<(), Error> {
    let request = get_withdrawal_request(&env, withdrawal_id);

    // Apply multisig if needed
    if requires_approval(&env, request.amount) {
        let status = get_withdrawal_status(&env, withdrawal_id);
        if status != WithdrawalStatus::Executed {
            return Err(Error::PendingApproval);
        }
    }

    // Proceed with withdrawal
    process_withdrawal(&env, request)
}
```

## Migration Guide

### For Existing Contracts

#### Phase 1: Preparation (Non-Breaking)

1. Add multisig module dependency
2. Deploy new contract version alongside existing
3. No changes to existing withdrawal logic
4. New withdrawals use multisig framework

```toml
# Cargo.toml
[dependencies]
stellarspend-contracts = { path = "../.." }
```

#### Phase 2: Gradual Rollout

1. Set high threshold initially (e.g., i128::MAX)
2. All withdrawals bypass multisig initially
3. Slowly lower threshold as adoption increases
4. Users adapt to new approval flow

```rust
// Start with high threshold
initialize_withdrawal_config(&env, approvers, 2, i128::MAX);

// After 2 weeks, lower to 100,000
set_withdrawal_threshold(&env, 100_000);

// After 4 weeks, lower to 10,000
set_withdrawal_threshold(&env, 10_000);
```

#### Phase 3: Full Integration

1. Threshold set to desired level
2. All large withdrawals require approvals
3. Monitor approval success rates
4. Adjust quorum if needed

```rust
// Optimal configuration
initialize_withdrawal_config(&env, approvers, 2, 10_000);
```

### Handling Existing Withdrawals

- **In-flight withdrawals**: Unaffected by new multisig system
- **Pending requests**: Can be migrated to new system
- **Historical data**: Remains accessible via events
- **Backward compatibility**: Maintained for small withdrawals

## Advanced Usage

### Custom Validation

```rust
use stellarspend_contracts::multisig_savings_withdrawal_utils::*;

pub fn custom_withdrawal_request(
    env: Env,
    user: Address,
    amount: i128,
    vault_id: u64,
) -> Result<u64, String> {
    // Validate amount
    if !WithdrawalValidation::validate_amount(&env, amount) {
        return Err("invalid_amount".to_string());
    }

    // Validate vault
    if !WithdrawalValidation::validate_vault_id(&env, vault_id) {
        return Err("invalid_vault".to_string());
    }

    // Create request
    let id = request_withdrawal(&env, user, amount, vault_id);
    Ok(id)
}
```

### Status Queries

```rust
use stellarspend_contracts::multisig_savings_withdrawal_utils::*;

pub fn get_withdrawal_info(env: Env, withdrawal_id: u64) -> WithdrawalStatusInfo {
    WithdrawalQuery::get_status_info(&env, withdrawal_id)
}

pub fn get_pending_approvals(env: Env, approver: Address) -> Vec<u64> {
    let all_ids = get_all_withdrawal_ids(&env);
    WithdrawalQuery::get_pending_for_approver(&env, &approver, all_ids)
}
```

### Audit Functions

```rust
use stellarspend_contracts::multisig_savings_withdrawal_utils::*;

pub fn get_withdrawal_statistics(env: Env) -> WithdrawalStats {
    let all_ids = get_all_withdrawal_ids(&env);
    WithdrawalAudit::calculate_stats(&env, all_ids)
}

pub fn get_approvers_for_withdrawal(env: Env, withdrawal_id: u64) -> Vec<Address> {
    WithdrawalAudit::get_approval_list(&env, withdrawal_id)
}
```

### Lifecycle Helpers

```rust
use stellarspend_contracts::multisig_savings_withdrawal_utils::*;

pub fn can_user_approve(env: Env, withdrawal_id: u64, user: Address) -> bool {
    WithdrawalLifecycle::can_approve(&env, withdrawal_id, &user)
}

pub fn can_execute_now(env: Env, withdrawal_id: u64) -> bool {
    WithdrawalLifecycle::can_execute(&env, withdrawal_id)
}

pub fn why_cant_approve(env: Env, withdrawal_id: u64, user: Address) -> Option<&'static str> {
    WithdrawalLifecycle::get_failure_reason(&env, withdrawal_id, &user)
}
```

## Event Handling

### Off-Chain Monitoring

```rust
// Listen for withdrawal events
env.events()
    .subscribe("withdraw", "request", |event| {
        println!("New withdrawal: {} for vault {}", event.amount, event.vault_id);
        notify_approvers(&event);
    });

env.events()
    .subscribe("withdraw", "approve", |event| {
        println!("Approval #{}/{}", event.approval_count, event.required_quorum);
    });

env.events()
    .subscribe("withdraw", "executed", |event| {
        println!("Withdrawal executed: {}", event.withdrawal_id);
    });
```

### Event Topics

```
WithdrawalRequested:    ("withdraw", "request", withdrawal_id)
WithdrawalApproved:     ("withdraw", "approve", withdrawal_id)
WithdrawalExecuted:     ("withdraw", "executed", withdrawal_id)
```

## Configuration Management

### Update Approvers

```rust
let new_approvers = vec![
    Address::from_string(&env, &String::from_str(&env, "GNEWAPPROVER1")),
    Address::from_string(&env, &String::from_str(&env, "GNEWAPPROVER2")),
    Address::from_string(&env, &String::from_str(&env, "GNEWAPPROVER3")),
    Address::from_string(&env, &String::from_str(&env, "GNEWAPPROVER4")),
];

set_withdrawal_approvers(&env, new_approvers, 3);  // Now 3-of-4
```

### Update Threshold

```rust
// Increase threshold to 100,000
set_withdrawal_threshold(&env, 100_000);

// Future withdrawals >= 100,000 require approval
// Existing pending withdrawals unaffected
```

## Error Handling

### Handle Common Errors

```rust
use stellarspend_contracts::multisig_savings_withdrawal::MultisigWithdrawalError;

pub fn approve_with_error_handling(env: Env, withdrawal_id: u64) -> Result<(), String> {
    let approver = env.current_contract_address();

    match approve_withdrawal_result(&env, &approver, withdrawal_id) {
        Ok(()) => Ok(()),
        Err(MultisigWithdrawalError::DuplicateApproval) => {
            Err("You've already approved this withdrawal".to_string())
        }
        Err(MultisigWithdrawalError::UnauthorizedApprover) => {
            Err("You're not an authorized approver".to_string())
        }
        Err(MultisigWithdrawalError::WithdrawalCancelled) => {
            Err("This withdrawal has been cancelled".to_string())
        }
        Err(_) => Err("Approval failed".to_string()),
    }
}
```

## Testing Integration

### Unit Tests

```rust
#[test]
fn test_integration() {
    let env = Env::default();
    
    // Initialize
    let approvers = vec![address(1), address(2), address(3)];
    initialize_withdrawal_config(&env, approvers, 2, 10_000);
    
    // Request large withdrawal
    let id = request_withdrawal(&env, address(100), 50_000, 1);
    
    // Verify pending
    assert_eq!(get_withdrawal_status(&env, id), WithdrawalStatus::Pending);
    
    // Approve
    approve_withdrawal(&env, address(1), id);
    assert_eq!(get_withdrawal_approval_count(&env, id), 1);
    
    approve_withdrawal(&env, address(2), id);
    assert_eq!(get_withdrawal_approval_count(&env, id), 2);
    
    // Verify executed
    assert_eq!(get_withdrawal_status(&env, id), WithdrawalStatus::Executed);
}
```

### Integration Tests

```rust
#[test]
fn test_savings_contract_integration() {
    let env = Env::default();
    let contract = SavingsContract {};
    
    // Initialize
    contract.initialize(&env, vec![address(1), address(2)], 2, 10_000);
    
    // Request withdrawal
    let id = contract.withdraw(&env, 50_000, 1);
    
    // Approve
    contract.approve(&env, address(1), id);
    contract.approve(&env, address(2), id);
    
    // Verify executed
    let request = get_withdrawal_request(&env, id);
    assert_eq!(request.amount, 50_000);
}
```

## Performance Optimization

### Batch Operations

```rust
pub fn batch_approve(
    env: Env,
    approver: Address,
    withdrawal_ids: Vec<u64>,
) -> u32 {
    let mut count = 0;
    
    for id in withdrawal_ids.iter() {
        if WithdrawalLifecycle::can_approve(&env, id, &approver) {
            approve_withdrawal(&env, approver.clone(), id);
            count += 1;
        }
    }
    
    count
}
```

### Query Optimization

```rust
pub fn get_pending_for_user(env: Env, user: Address) -> Vec<WithdrawalStatusInfo> {
    let all_ids = get_all_withdrawal_ids(&env);
    let mut pending = Vec::new(&env);
    
    for id in all_ids.iter() {
        let info = WithdrawalQuery::get_status_info(&env, id);
        if info.requester == user && info.is_pending {
            pending.push_back(info);
        }
    }
    
    pending
}
```

## Support & Debugging

### Logging

```rust
pub fn debug_withdrawal(env: Env, withdrawal_id: u64) {
    let request = get_withdrawal_request(&env, withdrawal_id);
    let status = get_withdrawal_status(&env, withdrawal_id);
    let approvals = get_withdrawal_approval_count(&env, withdrawal_id);
    
    println!("Withdrawal ID: {}", withdrawal_id);
    println!("Amount: {}", request.amount);
    println!("Status: {:?}", status);
    println!("Approvals: {}", approvals);
}
```

### Validation Checks

```rust
pub fn validate_withdrawal_state(env: Env, withdrawal_id: u64) -> Result<(), String> {
    let request = get_withdrawal_request(&env, withdrawal_id)?;
    
    if !WithdrawalValidation::validate_amount(&env, request.amount) {
        return Err("Invalid amount".to_string());
    }
    
    if !WithdrawalValidation::validate_vault_id(&env, request.vault_id) {
        return Err("Invalid vault".to_string());
    }
    
    let count = WithdrawalValidation::validate_approval_count(&env, withdrawal_id)?;
    
    Ok(())
}
```

## References

- Module: `contracts/multisig_savings_withdrawal.rs`
- Utilities: `contracts/multisig_savings_withdrawal_utils.rs`
- Integration: `contracts/multisig_savings_integration.rs`
- Documentation: `docs/multisig_savings_withdrawals.md`
- Tests: `tests/multisig_savings_withdrawal_tests.rs`
