// multisig_client — client-side ZK proof generation for private multisig voting.
//
// Uses RISC0 to prove NSK membership and compute vote nullifiers.
// In RISC0_DEV_MODE, proofs are fast/fake — suitable for development.

use nssa_core::{NullifierPublicKey, NullifierSecretKey};
use vote_circuit::{VoteCircuitInput, VoteCircuitOutput, compute_vote_nullifier};
use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ProverError {
    Prover(String),
    Serialization(String),
}

impl fmt::Display for ProverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProverError::Prover(msg) => write!(f, "prover error: {msg}"),
            ProverError::Serialization(msg) => write!(f, "serialization error: {msg}"),
        }
    }
}

impl std::error::Error for ProverError {}

// ---------------------------------------------------------------------------
// VoteProver
// ---------------------------------------------------------------------------

pub struct VoteProver {
    pub member_npks: Vec<NullifierPublicKey>,
}

impl VoteProver {
    pub fn new(member_npks: Vec<NullifierPublicKey>) -> Self {
        Self { member_npks }
    }

    /// Generate a vote proof. Returns (receipt_words: Vec<u32>, nullifier: [u8;32]).
    ///
    /// Uses RISC0 to prove NSK membership and compute a domain-separated nullifier.
    /// The NSK is a ZK witness — it never leaves the prover.
    pub fn generate_proof(
        &self,
        nsk: &NullifierSecretKey,
        proposal_index: u64,
        domain: &str,
    ) -> Result<(Vec<u32>, [u8; 32]), ProverError> {
        let input = VoteCircuitInput {
            nsk: *nsk,
            proposal_index,
            member_npks: self.member_npks.clone(),
            domain: domain.to_string(),
        };

        // Compute expected nullifier locally for verification
        let expected_nullifier = compute_vote_nullifier(nsk, proposal_index, domain);

        // Build RISC0 executor environment
        // Write fields individually — matches #[pre_tx_hook] write order
        // and guest env::read() calls
        let env = risc0_zkvm::ExecutorEnv::builder()
            .write(&input.nsk)
            .map_err(|e| ProverError::Serialization(e.to_string()))?
            .write(&input.member_npks.iter().map(|npk| npk.0).collect::<Vec<[u8;32]>>())
            .map_err(|e| ProverError::Serialization(e.to_string()))?
            .write(&input.proposal_index)
            .map_err(|e| ProverError::Serialization(e.to_string()))?
            .write(&input.domain)
            .map_err(|e| ProverError::Serialization(e.to_string()))?
            .build()
            .map_err(|e| ProverError::Prover(e.to_string()))?;

        // Prove (in RISC0_DEV_MODE this is fast/fake)
        let prove_info = risc0_zkvm::default_prover()
            .prove(env, multisig_methods::VOTE_CIRCUIT_ELF)
            .map_err(|e| ProverError::Prover(e.to_string()))?;

        let receipt = prove_info.receipt;

        // Decode journal to verify circuit output matches expectation
        let output: VoteCircuitOutput = receipt.journal.decode()
            .map_err(|e| ProverError::Serialization(e.to_string()))?;

        assert_eq!(
            output.nullifier, expected_nullifier,
            "nullifier mismatch between local computation and ZK circuit"
        );

        // Convert journal bytes to u32 words for on-chain submission
        let journal_bytes = &receipt.journal.bytes;
        let vote_receipt: Vec<u32> = journal_bytes
            .chunks(4)
            .map(|chunk| {
                let mut arr = [0u8; 4];
                arr[..chunk.len()].copy_from_slice(chunk);
                u32::from_le_bytes(arr)
            })
            .collect();

        Ok((vote_receipt, expected_nullifier))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vote_prover_new() {
        let members = vec![
            NullifierPublicKey([1u8; 32]),
            NullifierPublicKey([2u8; 32]),
        ];
        let prover = VoteProver::new(members.clone());
        assert_eq!(prover.member_npks, members);
    }

    // Full proof generation test — requires RISC0 toolchain (run with RISC0_DEV_MODE=1)
    #[test]
    #[ignore]
    fn test_vote_prover_generates_proof() {
        let nsk: NullifierSecretKey = [42u8; 32];
        let npk = NullifierPublicKey::from(&nsk);
        let members = vec![
            NullifierPublicKey([1u8; 32]),
            npk,
            NullifierPublicKey([3u8; 32]),
        ];

        let prover = VoteProver::new(members);
        let (vote_receipt, nullifier) = prover
            .generate_proof(&nsk, 1, "approve")
            .expect("proof generation failed");

        assert!(!vote_receipt.is_empty());
        let expected = compute_vote_nullifier(&nsk, 1, "approve");
        assert_eq!(nullifier, expected);
    }
}
