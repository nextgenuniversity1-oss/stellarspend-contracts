use soroban_sdk::{
    contracterror, contracttype, panic_with_error, symbol_short, Address, Env, Vec,
};

/// Storage keys for multi-signature savings withdrawal operations
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    // Withdrawal configuration
    WithdrawalThreshold,
    WithdrawalApprovers,
    WithdrawalQuorum,

    // Withdrawal request tracking
    WithdrawalRequest(u64),
    NextWithdrawalId,
    WithdrawalApproval(u64, Address),
    WithdrawalApprovalCount(u64),

    // Status tracking
    WithdrawalStatus(u64),
}

/// Status of a withdrawal request
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum WithdrawalStatus {
    Pending = 0,
    Approved = 1,
    Executed = 2,
    Cancelled = 3,
}

/// Represents a pending withdrawal request
#[derive(Clone)]
#[contracttype]
pub struct WithdrawalRequest {
    pub id: u64,
    pub requester: Address,
    pub amount: i128,
    pub vault_id: u64,
    pub created_at: u64,
    pub approval_count: u32,
}

/// Error types for multi-signature withdrawals
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MultisigWithdrawalError {
    NotInitialized = 1,
    Unauthorized = 2,
    InvalidThreshold = 3,
    InvalidQuorum = 4,
    WithdrawalNotFound = 5,
    DuplicateApproval = 6,
    InsufficientApprovals = 7,
    AlreadyExecuted = 8,
    InvalidAmount = 9,
    WithdrawalCancelled = 10,
    QuorumNotReached = 11,
    UnauthorizedApprover = 12,
    ThresholdNotInitialized = 13,
    ApprovalsNotInitialized = 14,
}

/// Event publishing for withdrawal operations
pub struct WithdrawalEvents;

impl WithdrawalEvents {
    /// Emitted when a withdrawal request is created
    pub fn withdrawal_requested(
        env: &Env,
        withdrawal_id: u64,
        requester: &Address,
        amount: i128,
        vault_id: u64,
    ) {
        let topics = (symbol_short!("withdraw"), symbol_short!("request"), withdrawal_id);
        env.events()
            .publish(topics, (requester.clone(), amount, vault_id));
    }

    /// Emitted when a withdrawal request is approved
    pub fn withdrawal_approved(
        env: &Env,
        withdrawal_id: u64,
        approver: &Address,
        approval_count: u32,
        required_quorum: u32,
    ) {
        let topics = (symbol_short!("withdraw"), symbol_short!("approve"), withdrawal_id);
        env.events()
            .publish(topics, (approver.clone(), approval_count, required_quorum));
    }

    /// Emitted when a withdrawal is executed
    pub fn withdrawal_executed(
        env: &Env,
        withdrawal_id: u64,
        executor: &Address,
        amount: i128,
        vault_id: u64,
    ) {
        let topics = (symbol_short!("withdraw"), symbol_short!("executed"), withdrawal_id);
        env.events()
            .publish(topics, (executor.clone(), amount, vault_id));
    }
}

/// Initialize withdrawal configuration
/// Requires admin to set up approvers, quorum, and threshold
pub fn initialize_withdrawal_config(
    env: &Env,
    approvers: Vec<Address>,
    quorum: u32,
    threshold: i128,
) {
    validate_approver_config(env, &approvers, quorum);

    if threshold < 0 {
        panic_with_error!(env, MultisigWithdrawalError::InvalidThreshold);
    }

    env.storage()
        .instance()
        .set(&DataKey::WithdrawalApprovers, &approvers);
    env.storage()
        .instance()
        .set(&DataKey::WithdrawalQuorum, &quorum);
    env.storage()
        .instance()
        .set(&DataKey::WithdrawalThreshold, &threshold);
    env.storage().instance().set(&DataKey::NextWithdrawalId, &0u64);
}

/// Update the withdrawal threshold (admin only)
pub fn set_withdrawal_threshold(env: &Env, threshold: i128) {
    if threshold < 0 {
        panic_with_error!(env, MultisigWithdrawalError::InvalidThreshold);
    }

    env.storage()
        .instance()
        .set(&DataKey::WithdrawalThreshold, &threshold);
}

/// Update withdrawal approvers and quorum (admin only)
pub fn set_withdrawal_approvers(env: &Env, approvers: Vec<Address>, quorum: u32) {
    validate_approver_config(env, &approvers, quorum);

    env.storage()
        .instance()
        .set(&DataKey::WithdrawalApprovers, &approvers);
    env.storage()
        .instance()
        .set(&DataKey::WithdrawalQuorum, &quorum);
}

/// Request a withdrawal
/// - If amount < threshold: can be executed immediately
/// - If amount >= threshold: creates a pending request requiring approvals
pub fn request_withdrawal(
    env: &Env,
    requester: Address,
    amount: i128,
    vault_id: u64,
) -> u64 {
    requester.require_auth();

    if amount <= 0 {
        panic_with_error!(env, MultisigWithdrawalError::InvalidAmount);
    }

    let threshold = get_withdrawal_threshold(env);
    let withdrawal_id = next_withdrawal_id(env);

    // Create withdrawal request
    let request = WithdrawalRequest {
        id: withdrawal_id,
        requester: requester.clone(),
        amount,
        vault_id,
        created_at: env.ledger().timestamp(),
        approval_count: 0,
    };

    env.storage()
        .persistent()
        .set(&DataKey::WithdrawalRequest(withdrawal_id), &request);
    env.storage()
        .persistent()
        .set(&DataKey::WithdrawalStatus(withdrawal_id), &WithdrawalStatus::Pending);

    WithdrawalEvents::withdrawal_requested(env, withdrawal_id, &requester, amount, vault_id);

    withdrawal_id
}

/// Approve a pending withdrawal request
/// - Only authorized approvers can approve
/// - Prevents duplicate approvals
/// - Automatically executes when quorum is reached
pub fn approve_withdrawal(env: &Env, approver: Address, withdrawal_id: u64) {
    approver.require_auth();

    // Verify approver is authorized
    require_withdrawal_approver(env, &approver);

    // Get withdrawal request
    let mut request = get_withdrawal_request(env, withdrawal_id);

    // Check withdrawal status
    let status = get_withdrawal_status(env, withdrawal_id);
    if status != WithdrawalStatus::Pending {
        panic_with_error!(env, MultisigWithdrawalError::WithdrawalCancelled);
    }

    // Check for duplicate approvals
    if has_withdrawal_approval(env, withdrawal_id, &approver) {
        panic_with_error!(env, MultisigWithdrawalError::DuplicateApproval);
    }

    // Record approval
    env.storage()
        .persistent()
        .set(&DataKey::WithdrawalApproval(withdrawal_id, approver.clone()), &true);

    let approval_count = get_withdrawal_approval_count(env, withdrawal_id);
    let next_count = approval_count
        .checked_add(1)
        .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::InvalidQuorum));

    env.storage()
        .persistent()
        .set(&DataKey::WithdrawalApprovalCount(withdrawal_id), &next_count);

    request.approval_count = next_count;

    let quorum = get_withdrawal_quorum(env);

    WithdrawalEvents::withdrawal_approved(
        env,
        withdrawal_id,
        &approver,
        next_count,
        quorum,
    );

    // Auto-execute if quorum reached
    if next_count >= quorum {
        execute_withdrawal_internal(env, withdrawal_id, &request);
    }
}

/// Execute an approved withdrawal
/// - Only executes if quorum is reached
/// - Transitions from Pending → Approved → Executed
pub fn execute_withdrawal(env: &Env, executor: Address, withdrawal_id: u64) {
    executor.require_auth();

    let request = get_withdrawal_request(env, withdrawal_id);
    let status = get_withdrawal_status(env, withdrawal_id);

    if status == WithdrawalStatus::Executed {
        panic_with_error!(env, MultisigWithdrawalError::AlreadyExecuted);
    }

    if status == WithdrawalStatus::Cancelled {
        panic_with_error!(env, MultisigWithdrawalError::WithdrawalCancelled);
    }

    let approval_count = get_withdrawal_approval_count(env, withdrawal_id);
    let quorum = get_withdrawal_quorum(env);

    if approval_count < quorum {
        panic_with_error!(env, MultisigWithdrawalError::QuorumNotReached);
    }

    execute_withdrawal_internal(env, withdrawal_id, &request);
}

/// Internal execution logic for withdrawals
fn execute_withdrawal_internal(env: &Env, withdrawal_id: u64, request: &WithdrawalRequest) {
    env.storage()
        .persistent()
        .set(&DataKey::WithdrawalStatus(withdrawal_id), &WithdrawalStatus::Executed);

    WithdrawalEvents::withdrawal_executed(
        env,
        withdrawal_id,
        &request.requester,
        request.amount,
        request.vault_id,
    );
}

/// Get a withdrawal request
pub fn get_withdrawal_request(env: &Env, withdrawal_id: u64) -> WithdrawalRequest {
    env.storage()
        .persistent()
        .get(&DataKey::WithdrawalRequest(withdrawal_id))
        .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::WithdrawalNotFound))
}

/// Get the status of a withdrawal request
pub fn get_withdrawal_status(env: &Env, withdrawal_id: u64) -> WithdrawalStatus {
    env.storage()
        .persistent()
        .get(&DataKey::WithdrawalStatus(withdrawal_id))
        .unwrap_or(WithdrawalStatus::Pending)
}

/// Get the current approval count for a withdrawal
pub fn get_withdrawal_approval_count(env: &Env, withdrawal_id: u64) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::WithdrawalApprovalCount(withdrawal_id))
        .unwrap_or(0)
}

/// Check if an address has already approved a withdrawal
pub fn has_withdrawal_approval(env: &Env, withdrawal_id: u64, approver: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::WithdrawalApproval(withdrawal_id, approver.clone()))
}

/// Get the withdrawal threshold
pub fn get_withdrawal_threshold(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::WithdrawalThreshold)
        .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::ThresholdNotInitialized))
}

/// Get the list of authorized withdrawal approvers
pub fn get_withdrawal_approvers(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::WithdrawalApprovers)
        .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::ApprovalsNotInitialized))
}

/// Get the required quorum for withdrawal approvals
pub fn get_withdrawal_quorum(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::WithdrawalQuorum)
        .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::InvalidQuorum))
}

/// Check if an address is an authorized withdrawal approver
pub fn is_withdrawal_approver(env: &Env, approver: &Address) -> bool {
    let approvers = get_withdrawal_approvers(env);
    for auth in approvers.iter() {
        if auth == approver.clone() {
            return true;
        }
    }
    false
}

/// Require that the caller is an authorized withdrawal approver
pub fn require_withdrawal_approver(env: &Env, approver: &Address) {
    if !is_withdrawal_approver(env, approver) {
        panic_with_error!(env, MultisigWithdrawalError::UnauthorizedApprover);
    }
}

/// Get the next withdrawal request ID and increment counter
pub fn next_withdrawal_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&DataKey::NextWithdrawalId)
        .unwrap_or(0);

    let next = current
        .checked_add(1)
        .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::InvalidQuorum));

    env.storage()
        .instance()
        .set(&DataKey::NextWithdrawalId, &next);

    next
}

/// Validate approver configuration
fn validate_approver_config(env: &Env, approvers: &Vec<Address>, quorum: u32) {
    let approver_count = approvers.len();

    if approver_count == 0 || quorum == 0 || quorum > approver_count {
        panic_with_error!(env, MultisigWithdrawalError::InvalidQuorum);
    }

    // Check for duplicates
    for i in 0..approver_count {
        let approver_i = approvers
            .get(i)
            .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::InvalidQuorum));

        for j in (i + 1)..approver_count {
            let approver_j = approvers
                .get(j)
                .unwrap_or_else(|| panic_with_error!(env, MultisigWithdrawalError::InvalidQuorum));

            if approver_i == approver_j {
                panic_with_error!(env, MultisigWithdrawalError::InvalidThreshold);
            }
        }
    }
}

/// Check if withdrawal requires multisig approval based on threshold
pub fn requires_approval(env: &Env, amount: i128) -> bool {
    let threshold = get_withdrawal_threshold(env);
    amount >= threshold
}
