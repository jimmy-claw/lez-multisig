// vote_prove — thin stdin/stdout wrapper around VoteProver for spel-cli pre_tx hook.
//
// Input (stdin, JSON):
//   { "nsk": "<hex64>", "members": ["<hex64>", ...], "proposal_index": <u64>, "domain": "<str>" }
//
// Output (stdout, JSON):
//   { "receipt": "<hex>", "nullifier": "<hex64>" }
//
// Called by spel-cli when SPEL_PRE_TX_BIN is set and instruction has a pre_tx hook.

use std::io::{self, Read};
use multisig_client::VoteProver;
use nssa_core::{NullifierPublicKey, NullifierSecretKey};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    nsk: String,
    members: Vec<String>,
    proposal_index: u64,
    domain: String,
}

#[derive(Serialize)]
struct Output {
    receipt: String,
    nullifier: String,
}

fn parse_hex32(s: &str) -> [u8; 32] {
    let bytes = hex::decode(s).unwrap_or_else(|e| {
        eprintln!("Error decoding hex '{}': {}", s, e);
        std::process::exit(1);
    });
    if bytes.len() != 32 {
        eprintln!("Expected 32 bytes, got {}", bytes.len());
        std::process::exit(1);
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

fn main() {
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin).expect("failed to read stdin");

    let input: Input = serde_json::from_str(&stdin).unwrap_or_else(|e| {
        eprintln!("Failed to parse input JSON: {}", e);
        std::process::exit(1);
    });

    let nsk: NullifierSecretKey = parse_hex32(&input.nsk);
    let member_npks: Vec<NullifierPublicKey> = input.members.iter()
        .map(|s| NullifierPublicKey(parse_hex32(s)))
        .collect();

    let prover = VoteProver::new(member_npks);
    let (receipt_words, nullifier) = prover
        .generate_proof(&nsk, input.proposal_index, &input.domain)
        .unwrap_or_else(|e| {
            eprintln!("Proof generation failed: {}", e);
            std::process::exit(1);
        });

    // Encode receipt as hex (u32 words → bytes → hex)
    let receipt_bytes: Vec<u8> = receipt_words.iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();

    let output = Output {
        receipt: hex::encode(&receipt_bytes),
        nullifier: hex::encode(&nullifier),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
