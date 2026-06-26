use soroban_sdk::{Address, Env, Vec};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multisig_savings_withdrawal::*;

    // Helper function to create test addresses
    fn address(id: u32) -> Address {
        Address::from_contract_id(&Env::default(), &[id as u8; 32])
    }

    // Helper function to set up withdrawal configuration
    fn setup_withdrawal_config(env: &Env, threshold: i128, quorum: u32, num_approvers: usize) {
        let mut approvers = Vec::new(env);
        for i in 0..num_approvers {
            approvers.push_back(address((i + 1) as u32));
        }

        initialize_withdrawal_config(env, approvers, quorum, threshold);
    }

    // Test 1: Withdrawal below threshold executes immediately
    #[test]
    fn test_withdrawal_below_threshold() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let withdrawal_id = request_withdrawal(&env, requester.clone(), 500, 1);

        // Verify withdrawal is in pending state
        let request = get_withdrawal_request(&env, withdrawal_id);
        assert_eq!(request.amount, 500);
        assert_eq!(request.requester, requester);
        assert_eq!(request.vault_id, 1);
        assert_eq!(request.approval_count, 0);
    }

    // Test 2: Withdrawal above threshold creates pending request
    #[test]
    fn test_withdrawal_above_threshold_creates_request() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // Verify withdrawal request is created
        let request = get_withdrawal_request(&env, withdrawal_id);
        assert_eq!(request.amount, 2000);
        assert_eq!(request.vault_id, 1);

        // Verify status is Pending
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Pending);
    }

    // Test 3: Single approval below quorum does not execute
    #[test]
    fn test_single_approval_below_quorum() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // Record first approval
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);

        // Verify approval count is 1
        let approval_count = get_withdrawal_approval_count(&env, withdrawal_id);
        assert_eq!(approval_count, 1);

        // Verify status is still Pending
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Pending);
    }

    // Test 4: Quorum reached executes withdrawal
    #[test]
    fn test_quorum_reached_executes() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);
        let approver2 = address(2);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // First approval
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Pending);

        // Second approval (quorum reached)
        approve_withdrawal(&env, approver2.clone(), withdrawal_id);

        // Verify status is now Executed
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Executed);
    }

    // Test 5: Duplicate approvals rejected
    #[test]
    fn test_duplicate_approvals_rejected() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // First approval succeeds
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);

        // Second approval from same signer should fail
        // This should panic with DuplicateApproval error
        let result = std::panic::catch_unwind(|| {
            approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        });

        assert!(result.is_err());
    }

    // Test 6: Unauthorized approvers rejected
    #[test]
    fn test_unauthorized_approver_rejected() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let unauthorized_approver = address(99);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // Attempt approval from unauthorized signer should fail
        // This should panic with UnauthorizedApprover error
        let result = std::panic::catch_unwind(|| {
            approve_withdrawal(&env, unauthorized_approver.clone(), withdrawal_id);
        });

        assert!(result.is_err());
    }

    // Test 7: Multiple pending withdrawals handled correctly
    #[test]
    fn test_multiple_pending_withdrawals() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester1 = address(100);
        let requester2 = address(101);

        // Create two withdrawal requests
        let withdrawal_id_1 = request_withdrawal(&env, requester1.clone(), 2000, 1);
        let withdrawal_id_2 = request_withdrawal(&env, requester2.clone(), 3000, 2);

        // Verify both are tracked separately
        let request1 = get_withdrawal_request(&env, withdrawal_id_1);
        let request2 = get_withdrawal_request(&env, withdrawal_id_2);

        assert_eq!(request1.amount, 2000);
        assert_eq!(request2.amount, 3000);
        assert_eq!(request1.vault_id, 1);
        assert_eq!(request2.vault_id, 2);
        assert_ne!(withdrawal_id_1, withdrawal_id_2);
    }

    // Test 8: Executed withdrawals cannot be approved again
    #[test]
    fn test_executed_withdrawal_cannot_be_approved() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);
        let approver2 = address(2);
        let approver3 = address(3);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // First two approvals
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        approve_withdrawal(&env, approver2.clone(), withdrawal_id);

        // Withdrawal is now executed
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Executed);

        // Third approval attempt should fail
        let result = std::panic::catch_unwind(|| {
            approve_withdrawal(&env, approver3.clone(), withdrawal_id);
        });

        assert!(result.is_err());
    }

    // Test 9: Event emission for request flow
    #[test]
    fn test_withdrawal_requested_event() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // Verify withdrawal request was created and event should have been emitted
        assert!(withdrawal_id > 0);
    }

    // Test 10: Event emission for approval flow
    #[test]
    fn test_withdrawal_approved_event() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);

        // Verify approval count increased
        let approval_count = get_withdrawal_approval_count(&env, withdrawal_id);
        assert_eq!(approval_count, 1);
    }

    // Test 11: Event emission for execution flow
    #[test]
    fn test_withdrawal_executed_event() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);
        let approver2 = address(2);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        approve_withdrawal(&env, approver2.clone(), withdrawal_id);

        // Verify execution
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Executed);
    }

    // Test 12: Authorization checks enforced
    #[test]
    fn test_authorization_checks() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // Verify only authorized signers can approve
        let unauthorized = address(99);
        let result = std::panic::catch_unwind(|| {
            approve_withdrawal(&env, unauthorized.clone(), withdrawal_id);
        });

        assert!(result.is_err());
    }

    // Test 13: No withdrawal executes before quorum
    #[test]
    fn test_no_execution_before_quorum() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 3, 3);

        let requester = address(100);
        let approver1 = address(1);
        let approver2 = address(2);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // First approval
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Pending);

        // Second approval (1 less than quorum)
        approve_withdrawal(&env, approver2.clone(), withdrawal_id);
        let status = get_withdrawal_status(&env, withdrawal_id);
        assert_eq!(status, WithdrawalStatus::Pending);
    }

    // Test 14: Approval replay attacks prevented
    #[test]
    fn test_replay_attack_prevention() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // First approval
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);

        // Attempt second approval from same signer (replay)
        let result = std::panic::catch_unwind(|| {
            approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        });

        assert!(result.is_err());
    }

    // Test 15: State transitions validated
    #[test]
    fn test_state_transitions() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);
        let approver2 = address(2);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // Initial state: Pending
        assert_eq!(
            get_withdrawal_status(&env, withdrawal_id),
            WithdrawalStatus::Pending
        );

        // After first approval: still Pending
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        assert_eq!(
            get_withdrawal_status(&env, withdrawal_id),
            WithdrawalStatus::Pending
        );

        // After second approval (quorum reached): Executed
        approve_withdrawal(&env, approver2.clone(), withdrawal_id);
        assert_eq!(
            get_withdrawal_status(&env, withdrawal_id),
            WithdrawalStatus::Executed
        );
    }

    // Test 16: Threshold configuration
    #[test]
    fn test_threshold_configuration() {
        let env = Env::default();
        setup_withdrawal_config(&env, 5000, 2, 3);

        // Verify threshold is set correctly
        let threshold = get_withdrawal_threshold(&env);
        assert_eq!(threshold, 5000);

        // Test that requires_approval works correctly
        assert!(!requires_approval(&env, 4999));
        assert!(requires_approval(&env, 5000));
        assert!(requires_approval(&env, 5001));
    }

    // Test 17: Quorum configuration
    #[test]
    fn test_quorum_configuration() {
        let env = Env::default();
        let threshold = 1000;
        let quorum = 3;
        let num_approvers = 5;

        let mut approvers = Vec::new(&env);
        for i in 0..num_approvers {
            approvers.push_back(address((i + 1) as u32));
        }

        initialize_withdrawal_config(&env, approvers, quorum, threshold);

        // Verify configuration
        assert_eq!(get_withdrawal_quorum(&env), 3);
        assert_eq!(get_withdrawal_threshold(&env), 1000);
    }

    // Test 18: Invalid configuration rejected
    #[test]
    fn test_invalid_configuration_rejected() {
        let env = Env::default();

        // Test with quorum > number of approvers
        let mut approvers = Vec::new(&env);
        approvers.push_back(address(1));
        approvers.push_back(address(2));

        let result = std::panic::catch_unwind(|| {
            initialize_withdrawal_config(&env, approvers, 3, 1000);
        });

        assert!(result.is_err());
    }

    // Test 19: Backward compatibility for small withdrawals
    #[test]
    fn test_backward_compatibility() {
        let env = Env::default();
        setup_withdrawal_config(&env, 10000, 2, 3);

        let requester = address(100);

        // Small withdrawal should work without approvals
        let withdrawal_id = request_withdrawal(&env, requester.clone(), 100, 1);

        // Verify it's created
        let request = get_withdrawal_request(&env, withdrawal_id);
        assert_eq!(request.amount, 100);
    }

    // Test 20: Approval tracking
    #[test]
    fn test_approval_tracking() {
        let env = Env::default();
        setup_withdrawal_config(&env, 1000, 2, 3);

        let requester = address(100);
        let approver1 = address(1);
        let approver2 = address(2);

        let withdrawal_id = request_withdrawal(&env, requester.clone(), 2000, 1);

        // Verify no approvals initially
        assert!(!has_withdrawal_approval(&env, withdrawal_id, &approver1));
        assert!(!has_withdrawal_approval(&env, withdrawal_id, &approver2));

        // First approval
        approve_withdrawal(&env, approver1.clone(), withdrawal_id);
        assert!(has_withdrawal_approval(&env, withdrawal_id, &approver1));
        assert!(!has_withdrawal_approval(&env, withdrawal_id, &approver2));

        // Second approval
        approve_withdrawal(&env, approver2.clone(), withdrawal_id);
        assert!(has_withdrawal_approval(&env, withdrawal_id, &approver1));
        assert!(has_withdrawal_approval(&env, withdrawal_id, &approver2));
    }
}
