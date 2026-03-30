// Approve handler — any member approves an existing proposal via ZK proof.
//
// caller is a private signer account — its identity is hidden by the privacy circuit.
// Membership is proven by the vote_circuit receipt.
//
// Expected accounts:
// - accounts[0]: caller (private signer, returned unchanged)
// - accounts[1]: multisig_state PDA (read membership)
// - accounts[2]: proposal PDA account (owned by multisig program)

use risc0_zkvm;
use nssa_core::account::AccountWithMetadata;
use nssa_core::program::{AccountPostState, ChainedCall};
use multisig_core::{MultisigState, Proposal, ProposalStatus};

include!(concat!(env!("OUT_DIR"), "/image_id.rs"));

fn verify_vote_proof(vote_receipt: &[u8]) {
    risc0_zkvm::guest::env::verify(VOTE_CIRCUIT_IMAGE_ID, vote_receipt)
        .expect("vote proof verification failed: invalid or mismatched circuit receipt");
}

pub fn handle(
    accounts: &[AccountWithMetadata],
    _proposal_index: u64,
    vote_receipt: &[u8],
    nullifier: [u8; 32],
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    assert!(accounts.len() >= 3, "Approve requires caller + multisig_state + proposal accounts");

    let caller_account = &accounts[0];
    let multisig_account = &accounts[1];
    let proposal_account = &accounts[2];

    // Verify the ZK proof of membership
    verify_vote_proof(vote_receipt);

    // Read multisig state
    let state_data: Vec<u8> = multisig_account.account.data.clone().into();
    let state: MultisigState = borsh::from_slice(&state_data)
        .expect("Failed to deserialize multisig state");

    // Read and update proposal
    let proposal_data: Vec<u8> = proposal_account.account.data.clone().into();
    let mut proposal: Proposal = borsh::from_slice(&proposal_data)
        .expect("Failed to deserialize proposal");

    assert_eq!(proposal.multisig_create_key, state.create_key, "Proposal does not belong to this multisig");
    assert_eq!(proposal.status, ProposalStatus::Active, "Proposal is not active");

    // Check nullifier not already used, then record it
    let is_new = proposal.approve(nullifier);
    assert!(is_new, "Member has already approved this proposal");

    // Write back proposal
    let proposal_bytes = borsh::to_vec(&proposal).unwrap();
    let mut proposal_post = proposal_account.account.clone();
    proposal_post.data = proposal_bytes.try_into().unwrap();

    // Multisig state unchanged
    let multisig_post = multisig_account.account.clone();

    // Return caller unchanged (private account, no data modification)
    let caller_post = caller_account.account.clone();

    (
        vec![
            AccountPostState::new(caller_post),
            AccountPostState::new(multisig_post),
            AccountPostState::new(proposal_post),
        ],
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nssa_core::account::{Account, AccountId};
    use nssa_core::program::ProgramId;
    use multisig_core::MultisigState;

    fn make_account(id: &[u8; 32], data: Vec<u8>) -> AccountWithMetadata {
        let mut account = Account::default();
        account.data = data.try_into().unwrap();
        AccountWithMetadata {
            account_id: AccountId::new(*id),
            account,
            is_authorized: false,
        }
    }

    fn make_multisig_state(threshold: u8, members: Vec<[u8; 32]>) -> Vec<u8> {
        let mut state = MultisigState::new([0u8; 32], threshold, members);
        state.transaction_index = 1;
        borsh::to_vec(&state).unwrap()
    }

    fn make_proposal(proposer_nullifier: [u8; 32]) -> Vec<u8> {
        let fake_program_id: ProgramId = [42u32; 8];
        let proposal = Proposal::new(
            1,
            [0u8; 32], // proposer identity is private
            proposer_nullifier,
            [0u8; 32], // create_key matches multisig
            fake_program_id,
            vec![0u32],
            1,
            vec![],
            vec![],
        );
        borsh::to_vec(&proposal).unwrap()
    }

    #[test]
    fn test_approve_adds_approval() {
        let members = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let state_data = make_multisig_state(2, members);
        let proposal_data = make_proposal([1u8; 32]);

        let accounts = vec![
            make_account(&[99u8; 32], vec![]),       // caller (private signer)
            make_account(&[10u8; 32], state_data),
            make_account(&[20u8; 32], proposal_data),
        ];

        let nullifier = [5u8; 32];
        let (post_states, _) = handle(&accounts, 1, &[], nullifier);

        assert_eq!(post_states.len(), 3);
        let proposal: Proposal = borsh::from_slice(&Vec::from(post_states[2].account().data.clone())).unwrap();
        assert_eq!(proposal.approved.len(), 2);
        assert!(proposal.approved.contains(&[1u8; 32])); // proposer_nullifier
        assert!(proposal.approved.contains(&nullifier));  // new approval
    }

    #[test]
    #[should_panic(expected = "already approved")]
    fn test_approve_duplicate_fails() {
        let members = vec![[1u8; 32], [2u8; 32]];
        let state_data = make_multisig_state(2, members);
        let proposal_data = make_proposal([1u8; 32]);

        let accounts = vec![
            make_account(&[99u8; 32], vec![]),       // caller (private signer)
            make_account(&[10u8; 32], state_data),
            make_account(&[20u8; 32], proposal_data),
        ];

        // Try to approve with the same nullifier as the proposer
        handle(&accounts, 1, &[], [1u8; 32]);
    }
}
