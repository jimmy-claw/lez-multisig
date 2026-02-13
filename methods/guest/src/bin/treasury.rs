//! Treasury guest binary - follows exact noop pattern from lssa

use nssa_core::program::{AccountPostState, ProgramInput, read_nssa_inputs, write_nssa_outputs};

type Instruction = Vec<u8>;

fn main() {
    // Log that we're starting
    println!("TREASURY: Guest starting");
    
    let (
        ProgramInput { pre_states, instruction },
        instruction_words,
    ) = read_nssa_inputs::<Instruction>();

    println!("TREASURY: Read {} pre_states", pre_states.len());
    println!("TREASURY: Instruction length: {} bytes", instruction.len());
    
    // For now, just pass through all accounts unchanged
    let post_states: Vec<AccountPostState> = pre_states
        .iter()
        .map(|pre| AccountPostState::new(pre.account.clone()))
        .collect();
    
    println!("TREASURY: Created {} post_states", post_states.len());
    println!("TREASURY: Calling write_nssa_outputs");

    write_nssa_outputs(instruction_words, pre_states, post_states);
    
    println!("TREASURY: Guest complete!");
}
