// Guest binary entry point for the Treasury program.
//
// This runs inside the Risc0 zkVM. It reads the program inputs from the
// host, dispatches to the treasury_program logic, and writes outputs
// (updated accounts + optional chained call) back to the host.

#![no_main]

use borsh::BorshDeserialize;
use nssa_core::program::{read_nssa_inputs, write_nssa_outputs, write_nssa_outputs_with_chained_call};

risc0_zkvm::guest::entry!(main);

fn main() {
    // Read standardized program inputs from the zkVM host.
    let mut program_input = read_nssa_inputs();

    // Dispatch to treasury program logic.
    let (updated_accounts, chained_call) = treasury_program::process(
        &program_input.program_id,
        &mut program_input.accounts,
        &program_input.input_data,
    );

    // Write outputs back to the host.
    match chained_call {
        Some(call) => write_nssa_outputs_with_chained_call(updated_accounts, call),
        None => write_nssa_outputs(updated_accounts),
    }
}
