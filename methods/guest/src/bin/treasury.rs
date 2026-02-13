//! Treasury guest binary — entry point for the Risc0 zkVM.

#![no_main]

use nssa_core::program::{read_nssa_inputs, write_nssa_outputs_with_chained_call};
use treasury_program;

fn main() {
    // Read inputs from the zkVM environment
    let (program_input, instruction_words) = read_nssa_inputs::<treasury_program::Instruction>();
    
    // Process the instruction (pre_states is now called pre_states)
    let mut accounts = program_input.pre_states.clone();
    
    let output = treasury_program::process(
        &mut accounts,
        &program_input.instruction,
    );
    
    // Write outputs back to the zkVM
    write_nssa_outputs_with_chained_call(
        instruction_words,
        program_input.pre_states,
        output.post_states,
        output.chained_calls,
    );
}
