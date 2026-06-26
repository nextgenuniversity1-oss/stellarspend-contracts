/// Utilities and helpers for multi-signature savings withdrawal operations

use soroban_sdk::{contracterror, contracttype, panic_with_error, Address, Env, Vec};

use crate::multisig_savings_withdrawal::{
    get_withdrawal_approval_count, get_withdrawal_approvers, get_withdrawal_quorum,
    get_withdrawal_request, get_withdrawal_status, get_withdrawal_threshold, WithdrawalStatus,
    MultisigWithdrawalError,
};

/// Validation utilities for withdrawal operations
pub struct WithdrawalValidation;

impl WithdrawalValidation {
    /// Validate that an amount is positive and within acceptable range
    pub fn validate_amount(env: &Env, amount: i128) -> bool {
        amount > 0 && amount <= i128::MAX / 2
    }

    /// Validate that vault ID is valid (non-zero)
    pub fn validate_vault_id(env: &Env, vault_id: u64) -> bool {
        vault_id > 0
    }

    /// Validate that withdrawal ID exists and is valid
    pub fn validate_withdrawal_id(env: &Env, withdrawal_id: u64) -> bool {
        withdrawal_id > 0
    }

    /// Validate that approval count matches expected pattern
    pub fn validate_approval_count(env: &Env, withdrawal_id: u64) -> Result<u32, MultisigWithdrawalError> {
        let count = get_withdrawal_approval_count(env, withdrawal_id);
        let quorum = get_withdrawal_quorum(env);

        if count > quorum {
            Err(MultisigWithdrawalError::InvalidQuorum)
        } else {
            Ok(count)
        }
    }
}

/// Query utilities for withdrawal information
pub struct WithdrawalQuery;

impl WithdrawalQuery {
    /// Get comprehensive withdrawal status information
    pub fn get_status_info(
        env: &Env,
        withdrawal_id: u64,
    ) -> WithdrawalStatusInfo {
        let request = get_withdrawal_request(env, withdrawal_id);
        let status = get_withdrawal_status(env, withdrawal_id);
        let approval_count = get_withdrawal_approval_count(env, withdrawal_id);
        let quorum = get_withdrawal_quorum(env);
        let threshold = get_withdrawal_threshold(env);

        let approvals_remaining = if approval_count >= quorum {
            0
        } else {
            quorum - approval_count
        };

        let requires_approval = request.amount >= threshold;

        WithdrawalStatusInfo {
            withdrawal_id,
            requester: request.requester,
            amount: request.amount,
            vault_id: request.vault_id,
            created_at: request.created_at,
            status: status as u32,
            approval_count,
            required_quorum: quorum,
            approvals_remaining,
            requires_approval,
            is_executed: status == WithdrawalStatus::Executed,
            is_pending: status == WithdrawalStatus::Pending,
            is_cancelled: status == WithdrawalStatus::Cancelled,
        }
    }

    /// Get pending withdrawals for a specific requester
    pub fn get_pending_for_requester(
        env: &Env,
        requester: &Address,
        withdrawal_ids: Vec<u64>,
    ) -> Vec<u64> {
        let mut pending = Vec::new(env);

        for id in withdrawal_ids.iter() {
            let request = get_withdrawal_request(env, id);
            let status = get_withdrawal_status(env, id);

            if request.requester == requester.clone() && status == WithdrawalStatus::Pending {
                pending.push_back(id);
            }
        }

        pending
    }

    /// Get pending withdrawals requiring approval from a specific approver
    pub fn get_pending_for_approver(
        env: &Env,
        approver: &Address,
        withdrawal_ids: Vec<u64>,
    ) -> Vec<u64> {
        let mut pending = Vec::new(env);
        let approvers = get_withdrawal_approvers(env);

        // Check if this address is an authorized approver
        let mut is_approver = false;
        for auth in approvers.iter() {
            if auth == approver.clone() {
                is_approver = true;
                break;
            }
        }

        if !is_approver {
            return pending;
        }

        // Find pending withdrawals where this approver hasn't approved
        for id in withdrawal_ids.iter() {
            let status = get_withdrawal_status(env, id);
            let has_approval = crate::multisig_savings_withdrawal::has_withdrawal_approval(env, id, approver);

            if status == WithdrawalStatus::Pending && !has_approval {
                pending.push_back(id);
            }
        }

        pending
    }
}

/// Comprehensive withdrawal status information
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalStatusInfo {
    pub withdrawal_id: u64,
    pub requester: Address,
    pub amount: i128,
    pub vault_id: u64,
    pub created_at: u64,
    pub status: u32,  // 0=Pending, 1=Approved, 2=Executed, 3=Cancelled
    pub approval_count: u32,
    pub required_quorum: u32,
    pub approvals_remaining: u32,
    pub requires_approval: bool,
    pub is_executed: bool,
    pub is_pending: bool,
    pub is_cancelled: bool,
}

/// Audit and reporting utilities
pub struct WithdrawalAudit;

impl WithdrawalAudit {
    /// Calculate withdrawal statistics
    pub fn calculate_stats(
        env: &Env,
        withdrawal_ids: Vec<u64>,
    ) -> WithdrawalStats {
        let mut total_withdrawals = 0;
        let mut pending_count = 0;
        let mut executed_count = 0;
        let mut cancelled_count = 0;
        let mut total_amount = 0i128;
        let mut pending_amount = 0i128;
        let mut executed_amount = 0i128;

        for id in withdrawal_ids.iter() {
            total_withdrawals += 1;
            let request = get_withdrawal_request(env, id);
            let status = get_withdrawal_status(env, id);

            total_amount = total_amount
                .checked_add(request.amount)
                .unwrap_or(total_amount);

            match status {
                WithdrawalStatus::Pending => {
                    pending_count += 1;
                    pending_amount = pending_amount
                        .checked_add(request.amount)
                        .unwrap_or(pending_amount);
                }
                WithdrawalStatus::Executed => {
                    executed_count += 1;
                    executed_amount = executed_amount
                        .checked_add(request.amount)
                        .unwrap_or(executed_amount);
                }
                WithdrawalStatus::Cancelled => {
                    cancelled_count += 1;
                }
                _ => {}
            }
        }

        WithdrawalStats {
            total_withdrawals,
            pending_count,
            executed_count,
            cancelled_count,
            total_amount,
            pending_amount,
            executed_amount,
            success_rate: if total_withdrawals > 0 {
                (executed_count as u32 * 100) / total_withdrawals as u32
            } else {
                0
            },
        }
    }

    /// Check if all approvals from required set have been obtained
    pub fn check_required_approvals(
        env: &Env,
        withdrawal_id: u64,
        required_approvers: &Vec<Address>,
    ) -> bool {
        for approver in required_approvers.iter() {
            if !crate::multisig_savings_withdrawal::has_withdrawal_approval(env, withdrawal_id, &approver) {
                return false;
            }
        }
        true
    }

    /// Get approval signatures from authorized approvers
    pub fn get_approval_list(
        env: &Env,
        withdrawal_id: u64,
    ) -> Vec<Address> {
        let mut approvals = Vec::new(env);
        let approvers = get_withdrawal_approvers(env);

        for approver in approvers.iter() {
            if crate::multisig_savings_withdrawal::has_withdrawal_approval(env, withdrawal_id, &approver) {
                approvals.push_back(approver);
            }
        }

        approvals
    }
}

/// Withdrawal statistics
#[contracttype]
#[derive(Clone)]
pub struct WithdrawalStats {
    pub total_withdrawals: u64,
    pub pending_count: u64,
    pub executed_count: u64,
    pub cancelled_count: u64,
    pub total_amount: i128,
    pub pending_amount: i128,
    pub executed_amount: i128,
    pub success_rate: u32,  // Percentage 0-100
}

/// Helper for managing withdrawal lifecycle
pub struct WithdrawalLifecycle;

impl WithdrawalLifecycle {
    /// Check if a withdrawal can be approved
    pub fn can_approve(env: &Env, withdrawal_id: u64, approver: &Address) -> bool {
        let status = get_withdrawal_status(env, withdrawal_id);

        // Must be pending
        if status != WithdrawalStatus::Pending {
            return false;
        }

        // Approver must be authorized
        if !crate::multisig_savings_withdrawal::is_withdrawal_approver(env, approver) {
            return false;
        }

        // Approver must not have already approved
        if crate::multisig_savings_withdrawal::has_withdrawal_approval(env, withdrawal_id, approver) {
            return false;
        }

        true
    }

    /// Check if a withdrawal can be executed
    pub fn can_execute(env: &Env, withdrawal_id: u64) -> bool {
        let status = get_withdrawal_status(env, withdrawal_id);

        // Must not already be executed
        if status == WithdrawalStatus::Executed {
            return false;
        }

        // Must not be cancelled
        if status == WithdrawalStatus::Cancelled {
            return false;
        }

        // Must have reached quorum
        let approval_count = get_withdrawal_approval_count(env, withdrawal_id);
        let quorum = get_withdrawal_quorum(env);

        approval_count >= quorum
    }

    /// Get the reason why approval/execution might fail
    pub fn get_failure_reason(env: &Env, withdrawal_id: u64, approver: &Address) -> Option<&'static str> {
        let status = get_withdrawal_status(env, withdrawal_id);

        if status == WithdrawalStatus::Executed {
            return Some("withdrawal_already_executed");
        }

        if status == WithdrawalStatus::Cancelled {
            return Some("withdrawal_cancelled");
        }

        if status != WithdrawalStatus::Pending {
            return Some("withdrawal_invalid_status");
        }

        if !crate::multisig_savings_withdrawal::is_withdrawal_approver(env, approver) {
            return Some("approver_not_authorized");
        }

        if crate::multisig_savings_withdrawal::has_withdrawal_approval(env, withdrawal_id, approver) {
            return Some("duplicate_approval");
        }

        None
    }
}

/// Testing utilities (enabled with #[cfg(test)])
#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// Helper to validate withdrawal configuration for tests
    pub fn validate_test_config(
        env: &Env,
        expected_quorum: u32,
        expected_threshold: i128,
    ) -> bool {
        let quorum = get_withdrawal_quorum(env);
        let threshold = get_withdrawal_threshold(env);

        quorum == expected_quorum && threshold == expected_threshold
    }
}
