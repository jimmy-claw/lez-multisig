// Execute handler — executes a fully-approved proposal by emitting a ChainedCall.
//
// The multisig doesn't execute actions directly. It builds a ChainedCall
// to the target program specified in the proposal, delegating actual execution.
//
// For each authorized_index in the proposal, the corresponding target account
// gets `is_authorized = true` in the ChainedCall. The NSSA runtime verifies
// that these accounts are valid PDAs of the calling program using the PDA seeds.
//
// Expected accounts:
// - accounts[0]: multisig_state (PDA, owned by multisig program)
// - accounts[1]: executor (must be authorized signer, must be member)
// - accounts[2..]: target accounts to pass to the ChainedCall

use nssa_core::account::AccountWithMetadata;
use nssa_core::program::{AccountPostState, ChainedCall, PdaSeed};
use multisig_core::{MultisigState, ProposalStatus};

pub fn handle(
    accounts: &[AccountWithMetadata],
    proposal_index: u64,
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    assert!(accounts.len() >= 2, "Execute requires at least multisig_state + executor");

    let multisig_account = &accounts[0];
    let executor_account = &accounts[1];
    let target_accounts = &accounts[2..];

    assert!(executor_account.is_authorized, "Executor must sign the transaction");

    let state_data: Vec<u8> = multisig_account.account.data.clone().into();
    let mut state: MultisigState = borsh::from_slice(&state_data)
        .expect("Failed to deserialize multisig state");

    let executor_id = *executor_account.account_id.value();
    assert!(state.is_member(&executor_id), "Executor is not a multisig member");

    // Find proposal and verify it's ready
    let threshold = state.threshold;
    let proposal = state.get_proposal_mut(proposal_index)
        .expect("Proposal not found");

    assert_eq!(proposal.status, ProposalStatus::Active, "Proposal is not active");
    assert!(
        proposal.has_threshold(threshold),
        "Proposal does not have enough approvals: need {}, have {}",
        threshold,
        proposal.approved.len()
    );

    assert_eq!(
        target_accounts.len(),
        proposal.target_account_count as usize,
        "Expected {} target accounts, got {}",
        proposal.target_account_count,
        target_accounts.len()
    );

    // Extract ChainedCall parameters from proposal
    let target_program_id = proposal.target_program_id.clone();
    let target_instruction_data = proposal.target_instruction_data.clone();
    let pda_seeds: Vec<PdaSeed> = proposal.pda_seeds.iter().map(|s| PdaSeed::new(*s)).collect();
    let authorized_indices = proposal.authorized_indices.clone();

    // Mark as executed and clean up
    proposal.status = ProposalStatus::Executed;
    state.cleanup_proposals();

    // Serialize updated state
    let state_bytes = borsh::to_vec(&state).unwrap();
    let mut multisig_post = multisig_account.account.clone();
    multisig_post.data = state_bytes.try_into().unwrap();

    // Build target account pre_states with authorization based on proposal
    let chained_pre_states: Vec<AccountWithMetadata> = target_accounts
        .iter()
        .enumerate()
        .map(|(i, acc)| {
            let mut acc = acc.clone();
            if authorized_indices.contains(&(i as u8)) {
                acc.is_authorized = true;
            }
            acc
        })
        .collect();

    let chained_call = ChainedCall {
        program_id: target_program_id,
        instruction_data: target_instruction_data,
        pre_states: chained_pre_states,
        pda_seeds,
    };

    let executor_post = executor_account.account.clone();

    (
        vec![AccountPostState::new(multisig_post), AccountPostState::new(executor_post)],
        vec![chained_call],
    )
}
