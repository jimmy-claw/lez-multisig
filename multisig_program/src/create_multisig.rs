// CreateMultisig handler — initializes a new M-of-N multisig
//
// In the private multisig design, members register their NPK (nullifier public key).
// No member accounts are claimed — membership is proven via ZK proofs of NSK knowledge.

use nssa_core::account::{Account, AccountWithMetadata};
use nssa_core::program::{AccountPostState, ChainedCall};
use nssa_core::NullifierPublicKey;
use multisig_core::MultisigState;

/// Handle CreateMultisig instruction
///
/// Expected accounts:
/// - accounts[0]: multisig_state (PDA, uninitialized) — derived from (program_id, create_key)
///
/// Members are identified by their NPKs, not by on-chain accounts.
/// Authorization: anyone can create a new multisig (create_key makes PDA unique)
pub fn handle(
    accounts: &[AccountWithMetadata],
    create_key: &[u8; 32],
    threshold: u8,
    members: &[NullifierPublicKey],
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    // Validate inputs
    assert!(!members.is_empty(), "Multisig must have at least one member");
    assert!(threshold >= 1, "Threshold must be at least 1");
    assert!((threshold as usize) <= members.len(), "Threshold cannot exceed member count");
    assert!(members.len() <= 10, "Maximum 10 members for PoC");

    assert!(
        !accounts.is_empty(),
        "CreateMultisig requires at least the multisig_state account"
    );

    // Verify multisig state account is uninitialized
    assert!(
        accounts[0].account == Account::default(),
        "Multisig state account must be uninitialized"
    );

    // Create multisig state with NPK-based member list
    let state = MultisigState::new(*create_key, threshold, members.to_vec());

    let mut multisig_account = Account::default();
    let state_bytes = borsh::to_vec(&state).unwrap();
    multisig_account.data = state_bytes.try_into().unwrap();

    // Claim the multisig state PDA
    let mut post_states = vec![AccountPostState::new_claimed(multisig_account)];

    // Pass through any additional accounts unchanged
    for acc in &accounts[1..] {
        post_states.push(AccountPostState::new(acc.account.clone()));
    }

    (post_states, vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use nssa_core::account::{Account, AccountId};

    fn make_account(id: &[u8; 32], authorized: bool) -> AccountWithMetadata {
        AccountWithMetadata {
            account_id: AccountId::new(*id),
            account: Account::default(),
            is_authorized: authorized,
        }
    }

    #[test]
    fn test_create_multisig_2_of_3() {
        let create_key = [1u8; 32];
        let members = vec![
            NullifierPublicKey([10u8; 32]),
            NullifierPublicKey([11u8; 32]),
            NullifierPublicKey([12u8; 32]),
        ];

        let accounts = vec![make_account(&[99u8; 32], false)]; // state PDA

        let (post_states, chained) = handle(&accounts, &create_key, 2, &members);

        assert!(chained.is_empty());
        assert_eq!(post_states.len(), 1); // only multisig state

        // Verify multisig state was written correctly
        let state: MultisigState = borsh::from_slice(
            &Vec::from(post_states[0].account().data.clone())
        ).unwrap();
        assert_eq!(state.threshold, 2);
        assert_eq!(state.member_count, 3);
        assert_eq!(state.members, members);
        assert_eq!(state.create_key, create_key);
        assert_eq!(state.transaction_index, 0);
    }

    #[test]
    #[should_panic(expected = "Threshold must be at least 1")]
    fn test_create_multisig_zero_threshold_fails() {
        let create_key = [1u8; 32];
        let members = vec![NullifierPublicKey([10u8; 32])];
        let accounts = vec![make_account(&[99u8; 32], false)];
        handle(&accounts, &create_key, 0, &members);
    }

    #[test]
    #[should_panic(expected = "Threshold cannot exceed member count")]
    fn test_create_multisig_threshold_exceeds_members_fails() {
        let create_key = [1u8; 32];
        let members = vec![NullifierPublicKey([10u8; 32]), NullifierPublicKey([11u8; 32])];
        let accounts = vec![make_account(&[99u8; 32], false)];
        handle(&accounts, &create_key, 3, &members);
    }

    #[test]
    #[should_panic(expected = "Maximum 10 members")]
    fn test_create_multisig_too_many_members_fails() {
        let create_key = [1u8; 32];
        let members: Vec<NullifierPublicKey> = (0u8..11).map(|i| NullifierPublicKey([i; 32])).collect();
        let accounts = vec![make_account(&[99u8; 32], false)];
        handle(&accounts, &create_key, 1, &members);
    }

    #[test]
    #[should_panic(expected = "must be uninitialized")]
    fn test_create_multisig_already_initialized_fails() {
        let create_key = [1u8; 32];
        let members = vec![NullifierPublicKey([10u8; 32])];

        // State account already has data
        let mut state_account = Account::default();
        state_account.data = vec![1u8; 10].try_into().unwrap();
        let accounts = vec![
            AccountWithMetadata {
                account_id: AccountId::new([99u8; 32]),
                account: state_account,
                is_authorized: false,
            },
        ];
        handle(&accounts, &create_key, 1, &members);
    }
}
