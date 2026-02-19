pub mod create_multisig;
pub mod execute;
pub mod add_member;
pub mod remove_member;
pub mod change_threshold;

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
            threshold,
            members,
        } => create_multisig::handle(accounts, *threshold, members),

        Instruction::Execute { recipient, amount } => {
            execute::handle(accounts, recipient, *amount)
        }

        Instruction::AddMember { new_member } => {
            add_member::handle(accounts, new_member)
        }

        Instruction::RemoveMember { member_to_remove } => {
            remove_member::handle(accounts, member_to_remove)
        }

        Instruction::ChangeThreshold { new_threshold } => {
            change_threshold::handle(accounts, *new_threshold)
        }
    }
}
