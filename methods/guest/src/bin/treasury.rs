//! Treasury guest binary - follows exact noop pattern from lssa

use nssa_core::program::{AccountPostState, ProgramInput, read_nssa_inputs, write_nssa_outputs};

type Instruction = Vec<u8>;

fn main() {
    let (
        ProgramInput { pre_states, instruction },
        instruction_words,
    ) = read_nssa_inputs::<Instruction>();

    // For now, just pass through all accounts unchanged
    let post_states: Vec<AccountPostState> = pre_states
        .iter()
        .map(|pre| AccountPostState::new(pre.account.clone()))
        .collect();

    write_nssa_outputs(instruction_words, pre_states, post_states);
}
