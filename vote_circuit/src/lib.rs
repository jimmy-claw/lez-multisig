// vote_circuit — ZK membership proof + domain-separated vote nullifier.
//
// Proves that the voter knows an NSK corresponding to one of the multisig's
// member NPKs, and computes a domain-separated nullifier that prevents:
// - Double-voting on the same proposal
// - Cross-action replay (approve nullifier != reject nullifier)
//
// The NSK is a ZK witness — it never appears on-chain.
// Day 2 wraps this in a RISC0 guest binary (env::read / env::commit).

use nssa_core::{NullifierPublicKey, NullifierSecretKey};
use risc0_zkvm::sha::{Impl, Sha256};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Private + public inputs fed to the vote circuit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteCircuitInput {
    /// Voter's nullifier secret key (ZK witness — never revealed)
    pub nsk: NullifierSecretKey,
    /// The proposal being voted on
    pub proposal_index: u64,
    /// Member NPKs from MultisigState (public input)
    pub member_npks: Vec<NullifierPublicKey>,
    /// Action domain: "approve" | "reject" | "propose" | "execute"
    pub domain: String,
}

/// Public outputs committed by the circuit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoteCircuitOutput {
    /// Domain-separated vote nullifier
    pub nullifier: [u8; 32],
    /// The proposal index (echoed for on-chain verification)
    pub proposal_index: u64,
}

// ---------------------------------------------------------------------------
// Circuit logic
// ---------------------------------------------------------------------------

/// Run the vote circuit: verify membership and produce a domain-separated nullifier.
///
/// # Panics
/// * If the NSK doesn't correspond to any member NPK
pub fn vote_circuit(input: &VoteCircuitInput) -> VoteCircuitOutput {
    // 1. Derive NPK from NSK
    let npk = NullifierPublicKey::from(&input.nsk);

    // 2. Set membership check
    assert!(
        input.member_npks.contains(&npk),
        "NSK does not correspond to any registered member"
    );

    // 3. Compute domain-separated vote nullifier
    let nullifier = compute_vote_nullifier(&input.nsk, input.proposal_index, &input.domain);

    VoteCircuitOutput {
        nullifier,
        proposal_index: input.proposal_index,
    }
}

/// Domain-separated vote nullifier derivation.
///
/// ```text
/// SHA256("/LEZ/v1/multisig/vote/" || domain || 0x00 || nsk || proposal_index_le)
/// ```
///
/// The domain tag prevents cross-action replay: an approve nullifier for
/// proposal 7 is different from a reject nullifier for proposal 7, even
/// with the same NSK.
pub fn compute_vote_nullifier(nsk: &NullifierSecretKey, proposal_index: u64, domain: &str) -> [u8; 32] {
    let mut bytes = b"/LEZ/v1/multisig/vote/".to_vec();
    bytes.extend_from_slice(domain.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(nsk);
    bytes.extend_from_slice(&proposal_index.to_le_bytes());
    Impl::hash_bytes(&bytes).as_bytes().try_into().unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_nsk() -> NullifierSecretKey {
        [42u8; 32]
    }

    fn test_npk() -> NullifierPublicKey {
        NullifierPublicKey::from(&test_nsk())
    }

    fn members_with_test_npk() -> Vec<NullifierPublicKey> {
        vec![
            NullifierPublicKey([1u8; 32]),
            test_npk(),
            NullifierPublicKey([3u8; 32]),
        ]
    }

    #[test]
    fn test_valid_member_approve() {
        let input = VoteCircuitInput {
            nsk: test_nsk(),
            proposal_index: 1,
            member_npks: members_with_test_npk(),
            domain: "approve".into(),
        };

        let output = vote_circuit(&input);

        assert_eq!(output.proposal_index, 1);
        let expected = compute_vote_nullifier(&test_nsk(), 1, "approve");
        assert_eq!(output.nullifier, expected);
    }

    #[test]
    fn test_different_proposals_different_nullifiers() {
        let null_1 = compute_vote_nullifier(&test_nsk(), 1, "approve");
        let null_2 = compute_vote_nullifier(&test_nsk(), 2, "approve");
        assert_ne!(null_1, null_2, "different proposals must produce different nullifiers");
    }

    #[test]
    fn test_different_domains_different_nullifiers() {
        let approve = compute_vote_nullifier(&test_nsk(), 1, "approve");
        let reject = compute_vote_nullifier(&test_nsk(), 1, "reject");
        let propose = compute_vote_nullifier(&test_nsk(), 1, "propose");
        let execute = compute_vote_nullifier(&test_nsk(), 1, "execute");

        // All four must be distinct
        let all = [approve, reject, propose, execute];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j], "domain separation failed for pair ({i}, {j})");
            }
        }
    }

    #[test]
    fn test_deterministic_output() {
        let null_a = compute_vote_nullifier(&test_nsk(), 5, "approve");
        let null_b = compute_vote_nullifier(&test_nsk(), 5, "approve");
        assert_eq!(null_a, null_b, "same inputs must produce identical nullifier");
    }

    #[test]
    #[should_panic(expected = "does not correspond to any registered member")]
    fn test_non_member_panics() {
        let input = VoteCircuitInput {
            nsk: test_nsk(),
            proposal_index: 1,
            member_npks: vec![
                NullifierPublicKey([1u8; 32]),
                NullifierPublicKey([2u8; 32]),
            ],
            domain: "approve".into(),
        };
        vote_circuit(&input);
    }

    #[test]
    fn test_different_nsks_different_nullifiers() {
        let nsk_a = [42u8; 32];
        let nsk_b = [99u8; 32];
        let null_a = compute_vote_nullifier(&nsk_a, 1, "approve");
        let null_b = compute_vote_nullifier(&nsk_b, 1, "approve");
        assert_ne!(null_a, null_b, "different NSKs must produce different nullifiers");
    }
}
