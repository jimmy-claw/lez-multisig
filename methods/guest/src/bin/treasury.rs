//! Treasury guest binary — entry point for the Risc0 zkVM.

#![no_main]

use nssa_core::program::read_nssa_inputs;
use treasury_program;

fn main() {
    // Read inputs from the zkVM environment
    let (program_input, instruction_words) = read_nssa_inputs::<treasury_program::Instruction>();
    
    // Clone for output since process consumes
    let accounts = program_input.accounts.clone();
    
    // Process the instruction
    let output = treasury_program::process(
        &program_input.program_id,
        &mut accounts.clone(),
        &program_input.input_data,
    );
    
    // Write outputs back to the zkVM
    output.write();
}
