pub mod create_multisig;
pub mod propose;
pub mod approve;
pub mod reject;
pub mod execute;
pub mod approve_circuit;

use nssa_core::account::AccountWithMetadata;
use nssa_core::program::{AccountPostState, ChainedCall};
use multisig_core::Instruction;

/// Main entry point called from the guest binary.
pub fn process(
    accounts: &[AccountWithMetadata],
    instruction: &Instruction,
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    match instruction {
        Instruction::CreateMultisig {
            create_key,
            threshold,
            members,
        } => create_multisig::handle(accounts, create_key, *threshold, members),

        Instruction::Propose {
            target_program_id,
            target_instruction_data,
            target_account_count,
            pda_seeds,
            authorized_indices,
            vote_receipt,
            nullifier,
        } => propose::handle(
            accounts,
            target_program_id,
            target_instruction_data,
            *target_account_count,
            pda_seeds,
            authorized_indices,
            vote_receipt,
            *nullifier,
        ),

        Instruction::Approve { proposal_index, vote_receipt, nullifier } => {
            approve::handle(accounts, *proposal_index, vote_receipt, *nullifier)
        }

        Instruction::Reject { proposal_index, vote_receipt, nullifier } => {
            reject::handle(accounts, *proposal_index, vote_receipt, *nullifier)
        }

        Instruction::Execute { proposal_index, vote_receipt, nullifier } => {
            execute::handle(accounts, *proposal_index, vote_receipt, *nullifier)
        }
    }
}
