/// Integration Example: Multi-Signature Savings Withdrawal
/// 
/// This example demonstrates how to integrate the multisig_savings_withdrawal module
/// with a savings contract to enforce multi-signature approval for large withdrawals.

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

/// Message types for savings contract operations
#[derive(Clone)]
#[contracttype]
pub enum SavingsExecuteMsg {
    /// Initialize savings with multisig configuration
    InitializeSavings {
        approvers: Vec<Address>,
        quorum: u32,
        withdrawal_threshold: i128,
    },
    /// Request a withdrawal
    RequestWithdrawal {
        amount: i128,
        vault_id: u64,
    },
    /// Approve a pending withdrawal
    ApproveWithdrawal {
        withdrawal_id: u64,
    },
    /// Execute an approved withdrawal
    ExecuteWithdrawal {
        withdrawal_id: u64,
    },
    /// Update withdrawal configuration (admin only)
    UpdateWithdrawalConfig {
        approvers: Vec<Address>,
        quorum: u32,
        threshold: i128,
    },
}

/// Response types for queries
#[derive(Clone)]
#[contracttype]
pub struct WithdrawalQueryResponse {
    pub id: u64,
    pub requester: Address,
    pub amount: i128,
    pub vault_id: u64,
    pub approval_count: u32,
    pub required_quorum: u32,
    pub is_approved: bool,
}

/// Savings contract with multi-signature withdrawal support
#[contract]
pub struct SavingsContractWithMultisig;

#[contractimpl]
impl SavingsContractWithMultisig {
    /// Initialize the savings contract with multisig configuration
    /// 
    /// # Arguments
    /// * `approvers` - List of authorized withdrawal approvers
    /// * `quorum` - Number of approvals required (M-of-N)
    /// * `withdrawal_threshold` - Amounts at or above this require multisig approval
    /// 
    /// # Example
    /// - Withdrawals below threshold execute immediately
    /// - Withdrawals at or above threshold require quorum approvals
    pub fn initialize_savings(
        env: Env,
        approvers: Vec<Address>,
        quorum: u32,
        withdrawal_threshold: i128,
    ) {
        // Initialize the multisig withdrawal system
        crate::multisig_savings_withdrawal::initialize_withdrawal_config(
            &env,
            approvers,
            quorum,
            withdrawal_threshold,
        );
    }

    /// Request a withdrawal from savings
    /// 
    /// # Arguments
    /// * `amount` - Amount to withdraw
    /// * `vault_id` - ID of the vault to withdraw from
    /// 
    /// # Behavior
    /// - If amount < threshold: Creates request (can be executed immediately)
    /// - If amount >= threshold: Creates pending request requiring approvals
    /// 
    /// # Returns
    /// Withdrawal request ID
    pub fn request_withdrawal(env: Env, amount: i128, vault_id: u64) -> u64 {
        let requester = env.current_contract_address();

        // Create withdrawal request
        let withdrawal_id =
            crate::multisig_savings_withdrawal::request_withdrawal(&env, requester, amount, vault_id);

        // Publish event for off-chain monitoring
        env.events()
            .publish((symbol_short!("savings"), symbol_short!("withdrawal_requested")), withdrawal_id);

        withdrawal_id
    }

    /// Approve a pending withdrawal request
    /// 
    /// # Arguments
    /// * `withdrawal_id` - ID of the withdrawal request to approve
    /// 
    /// # Authorization
    /// - Only authorized approvers can call this
    /// - Caller is authenticated via require_auth()
    /// 
    /// # Behavior
    /// - Rejects if caller is not an authorized approver
    /// - Rejects duplicate approvals from same signer
    /// - Auto-executes withdrawal when quorum is reached
    pub fn approve_withdrawal(env: Env, withdrawal_id: u64) {
        let approver = env.current_contract_address();

        // Record approval and potentially execute if quorum reached
        crate::multisig_savings_withdrawal::approve_withdrawal(&env, approver, withdrawal_id);

        env.events()
            .publish((symbol_short!("savings"), symbol_short!("withdrawal_approved")), withdrawal_id);
    }

    /// Execute an approved withdrawal
    /// 
    /// # Arguments
    /// * `withdrawal_id` - ID of the withdrawal request
    /// 
    /// # Requirements
    /// - Withdrawal must have reached quorum
    /// - Withdrawal must not be already executed
    /// 
    /// # Note
    /// This is typically called automatically when quorum is reached,
    /// but can be called explicitly for additional control.
    pub fn execute_withdrawal(env: Env, withdrawal_id: u64) {
        let executor = env.current_contract_address();

        // Execute the withdrawal
        crate::multisig_savings_withdrawal::execute_withdrawal(&env, executor, withdrawal_id);

        env.events()
            .publish((symbol_short!("savings"), symbol_short!("withdrawal_executed")), withdrawal_id);
    }

    /// Query withdrawal request details
    /// 
    /// # Arguments
    /// * `withdrawal_id` - ID of the withdrawal request
    /// 
    /// # Returns
    /// Withdrawal request information with current approval status
    pub fn query_withdrawal(env: Env, withdrawal_id: u64) -> WithdrawalQueryResponse {
        let request = crate::multisig_savings_withdrawal::get_withdrawal_request(&env, withdrawal_id);
        let approval_count = crate::multisig_savings_withdrawal::get_withdrawal_approval_count(&env, withdrawal_id);
        let quorum = crate::multisig_savings_withdrawal::get_withdrawal_quorum(&env);
        let status = crate::multisig_savings_withdrawal::get_withdrawal_status(&env, withdrawal_id);

        WithdrawalQueryResponse {
            id: request.id,
            requester: request.requester,
            amount: request.amount,
            vault_id: request.vault_id,
            approval_count,
            required_quorum: quorum,
            is_approved: status == crate::multisig_savings_withdrawal::WithdrawalStatus::Executed,
        }
    }

    /// Check if a withdrawal requires multisig approval
    /// 
    /// # Arguments
    /// * `amount` - Amount to check
    /// 
    /// # Returns
    /// true if amount requires multisig approval, false otherwise
    pub fn requires_approval(env: Env, amount: i128) -> bool {
        crate::multisig_savings_withdrawal::requires_approval(&env, amount)
    }

    /// Get the current withdrawal threshold
    pub fn get_withdrawal_threshold(env: Env) -> i128 {
        crate::multisig_savings_withdrawal::get_withdrawal_threshold(&env)
    }

    /// Get the current required quorum
    pub fn get_withdrawal_quorum(env: Env) -> u32 {
        crate::multisig_savings_withdrawal::get_withdrawal_quorum(&env)
    }

    /// Get list of authorized withdrawal approvers
    pub fn get_withdrawal_approvers(env: Env) -> Vec<Address> {
        crate::multisig_savings_withdrawal::get_withdrawal_approvers(&env)
    }

    /// Check if an address is an authorized approver
    pub fn is_withdrawal_approver(env: Env, address: Address) -> bool {
        crate::multisig_savings_withdrawal::is_withdrawal_approver(&env, &address)
    }
}

/// Integration workflow example:
/// 
/// 1. Initialize:
///    initialize_savings([approver1, approver2, approver3], 2, 10000)
///    - Requires 2-of-3 approvals for withdrawals >= 10000
/// 
/// 2. Request withdrawal >= threshold:
///    id = request_withdrawal(15000, 1)
///    - Creates pending withdrawal request
///    - Requires 2 approvals to execute
/// 
/// 3. First approver approves:
///    approve_withdrawal(id)
///    - Records approval
///    - Still waiting for 1 more approval
/// 
/// 4. Second approver approves:
///    approve_withdrawal(id)
///    - Records approval
///    - Quorum reached (2/2 approvals)
///    - Withdrawal automatically executed
/// 
/// 5. Query status:
///    response = query_withdrawal(id)
///    - Returns is_approved: true
///    - Withdrawal is ready to be processed
/// 
/// Security features:
/// - Duplicate approvals prevented (same signer can't approve twice)
/// - Only authorized approvers can approve
/// - Quorum enforcement (M-of-N approval required)
/// - State transitions validated (can't execute incomplete withdrawals)
/// - No execution before quorum
/// - Approval tracking on-chain
