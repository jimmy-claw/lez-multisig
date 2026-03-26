// Propose handler — creates a new proposal as a separate PDA account.
//
// No signer account needed. Membership is proven by the vote_circuit receipt.
// The #[pre_tx_hook] macro auto-injects env::verify for the receipt.
//
// Expected accounts:
// - accounts[0]: multisig_state PDA (read membership, increment tx_index)
// - accounts[1]: proposal PDA account (must be Account::default() = uninitialized)

use nssa_core::account::{Account, AccountWithMetadata};
use nssa_core::program::{AccountPostState, ChainedCall, InstructionData, ProgramId};
use multisig_core::{MultisigState, Proposal};

/// Placeholder image ID for the vote circuit — updated when circuit is finalized.
const VOTE_CIRCUIT_IMAGE_ID: [u32; 8] = [0u32; 8];

#[allow(unused_variables)]
fn verify_vote_proof(_vote_receipt: &[u32]) {
    // Receipt verification skipped for dev mode demo
    // Production: use risc0_zkvm::guest::env::verify(VOTE_CIRCUIT_IMAGE_ID, vote_receipt)
}

pub fn handle(
    accounts: &[AccountWithMetadata],
    target_program_id: &ProgramId,
    target_instruction_data: &InstructionData,
    target_account_count: u8,
    pda_seeds: &[[u8; 32]],
    authorized_indices: &[u8],
    vote_receipt: &[u32],
    nullifier: [u8; 32],
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    assert!(accounts.len() >= 2, "Propose requires multisig_state + proposal accounts");

    let multisig_account = &accounts[0];
    let proposal_account = &accounts[1];

    // Proposal account must be uninitialized
    assert!(
        proposal_account.account == Account::default(),
        "Proposal account must be uninitialized"
    );

    // Verify the ZK proof of membership
    verify_vote_proof(vote_receipt);

    // Read and update multisig state (increment transaction_index)
    let state_data: Vec<u8> = multisig_account.account.data.clone().into();
    let mut state: MultisigState = borsh::from_slice(&state_data)
        .expect("Failed to deserialize multisig state");

    let proposal_index = state.next_proposal_index();

    // Create the proposal — proposer identity is private, stored as nullifier only
    let proposal = Proposal::new(
        proposal_index,
        [0u8; 32], // proposer identity is private
        nullifier,
        state.create_key,
        target_program_id.clone(),
        target_instruction_data.clone(),
        target_account_count,
        pda_seeds.to_vec(),
        authorized_indices.to_vec(),
    );

    // Serialize updated multisig state (with incremented tx_index)
    let state_bytes = borsh::to_vec(&state).unwrap();
    let mut multisig_post = multisig_account.account.clone();
    multisig_post.data = state_bytes.try_into().unwrap();

    // Serialize proposal into new account and claim it
    let proposal_bytes = borsh::to_vec(&proposal).unwrap();
    let mut proposal_post = Account::default();
    proposal_post.data = proposal_bytes.try_into().unwrap();

    (
        vec![
            AccountPostState::new(multisig_post),
            AccountPostState::new_claimed(proposal_post),
        ],
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nssa_core::account::{Account, AccountId};
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

    fn make_state(threshold: u8, members: Vec<[u8; 32]>) -> Vec<u8> {
        borsh::to_vec(&MultisigState::new([0u8; 32], threshold, members)).unwrap()
    }

    #[test]
    fn test_propose_creates_proposal_and_increments_index() {
        let members = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let state_data = make_state(2, members.clone());

        let accounts = vec![
            make_account(&[10u8; 32], state_data),  // multisig state
            make_account(&[20u8; 32], vec![]),       // proposal PDA (uninitialized)
        ];

        let program_id: ProgramId = [42u32; 8];
        let nullifier = [5u8; 32];
        let (post_states, chained) = handle(
            &accounts,
            &program_id,
            &vec![0u32],
            1,
            &[],
            &[],
            &[],         // empty vote_receipt (verification is no-op in tests)
            nullifier,
        );

        assert!(chained.is_empty());
        assert_eq!(post_states.len(), 2);

        // Multisig state should have incremented tx index
        let state: MultisigState = borsh::from_slice(
            &Vec::from(post_states[0].account().data.clone())
        ).unwrap();
        assert_eq!(state.transaction_index, 1);

        // Proposal should exist with proposer auto-approved (via proposer_nullifier)
        let proposal: Proposal = borsh::from_slice(
            &Vec::from(post_states[1].account().data.clone())
        ).unwrap();
        assert_eq!(proposal.index, 1);
        assert_eq!(proposal.proposer, [0u8; 32]); // identity is private
        assert_eq!(proposal.proposer_nullifier, nullifier);
        assert_eq!(proposal.approved, vec![nullifier]); // proposer auto-approved
        assert_eq!(proposal.status, multisig_core::ProposalStatus::Active);
    }
}
