#![no_main]

risc0_zkvm::guest::entry!(main);

use vote_circuit::{VoteCircuitInput, vote_circuit};

fn main() {
    let input: VoteCircuitInput = risc0_zkvm::guest::env::read();
    let output = vote_circuit(&input);
    risc0_zkvm::guest::env::commit(&output);
}
