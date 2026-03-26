#![no_main]

risc0_zkvm::guest::entry!(main);

use nssa_core::NullifierPublicKey;
use vote_circuit::{VoteCircuitInput, vote_circuit};

fn main() {
    // Read inputs individually — matches spel-cli pre_tx_hook write order
    let nsk: [u8; 32] = risc0_zkvm::guest::env::read();
    let member_npks_raw: Vec<[u8; 32]> = risc0_zkvm::guest::env::read();
    let proposal_index: u64 = risc0_zkvm::guest::env::read();
    let domain: String = risc0_zkvm::guest::env::read();

    let input = VoteCircuitInput {
        nsk,
        member_npks: member_npks_raw.into_iter().map(NullifierPublicKey).collect(),
        proposal_index,
        domain,
    };

    let output = vote_circuit(&input);
    risc0_zkvm::guest::env::commit(&output);
}
