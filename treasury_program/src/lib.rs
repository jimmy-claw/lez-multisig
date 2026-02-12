// treasury_program — on-chain logic for the Treasury program.
//
//! Each instruction handler lives in its own module. The top-level `process`
//! function dispatches based on the deserialized [`Instruction`].

pub mod create_vault;
pub mod receive;
pub mod send;

use borsh::BorshDeserialize;
use nssa_core::program::{AccountState, ChainedCall, ProgramId};
use treasury_core::Instruction;

/// Main entry point called from the guest binary.
///
/// Reads the instruction from `input_data`, then delegates to the appropriate
/// handler.  Returns a list of updated account states and an optional chained
/// call.
pub fn process(
    program_id: &ProgramId,
    accounts: &mut [AccountState],
    input_data: &[u8],
) -> (Vec<AccountState>, Option<ChainedCall>) {
    let instruction =
        Instruction::try_from_slice(input_data).expect("failed to deserialize instruction");

    match instruction {
        Instruction::CreateVault {
            token_name,
            initial_supply,
            treasury_program_id,
            token_program_id,
        } => create_vault::handle(
            program_id,
            accounts,
            token_name,
            initial_supply,
            &treasury_program_id,
            &token_program_id,
        ),
        Instruction::Send {
            amount,
            token_program_id,
        } => send::handle(program_id, accounts, amount, &token_program_id),
        Instruction::Deposit {
            amount,
            token_program_id,
        } => receive::handle(program_id, accounts, amount, &token_program_id),
    }
}
