// approve_circuit — pure ZK membership proof + vote nullifier derivation.
//
// This function runs inside the RISC0 guest to prove:
// 1. The voter knows an NSK corresponding to one of the multisig's member NPKs
// 2. The derived vote nullifier hasn't been used before (no double-voting)
//
// The NSK is a ZK witness only — it never appears on-chain.

use nssa_core::{Nullifier, NullifierPublicKey, NullifierSecretKey};

/// Verify membership and compute a vote nullifier.
///
/// # Arguments
/// * `nsk` — voter's nullifier secret key (ZK witness, never revealed)
/// * `member_npks` — list of member NPKs from MultisigState
/// * `proposal_index` — the proposal being voted on
/// * `existing_nullifiers` — nullifiers already recorded in the proposal (approved + rejected)
///
/// # Returns
/// The vote nullifier bytes `[u8; 32]` to be stored in the proposal.
///
/// # Panics
/// * If the NSK doesn't correspond to any member NPK
/// * If the derived nullifier already exists (double-vote)
pub fn approve_circuit(
    nsk: &NullifierSecretKey,
    member_npks: &[NullifierPublicKey],
    proposal_index: u64,
    existing_nullifiers: &[[u8; 32]],
) -> [u8; 32] {
    // 1. Derive NPK from the provided NSK
    let npk = NullifierPublicKey::from(nsk);

    // 2. Verify the derived NPK is in the member list
    assert!(
        member_npks.iter().any(|m| m == &npk),
        "NSK does not correspond to any multisig member"
    );

    // 3. Compute the vote nullifier (ties identity to this specific proposal)
    let nullifier = Nullifier::for_vote(nsk, proposal_index);
    let nullifier_bytes = nullifier.to_byte_array();

    // 4. Check the nullifier hasn't been used (prevents double-voting)
    assert!(
        !existing_nullifiers.contains(&nullifier_bytes),
        "Nullifier already used — double vote detected"
    );

    nullifier_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_nsk() -> NullifierSecretKey {
        [42u8; 32]
    }

    fn test_npk() -> NullifierPublicKey {
        NullifierPublicKey::from(&test_nsk())
    }

    #[test]
    fn test_approve_circuit_valid_member() {
        let nsk = test_nsk();
        let npk = test_npk();
        let members = vec![
            NullifierPublicKey([1u8; 32]), // other member
            npk.clone(),                   // our member
            NullifierPublicKey([3u8; 32]), // other member
        ];

        let nullifier = approve_circuit(&nsk, &members, 1, &[]);

        // Verify nullifier is deterministic
        let expected = Nullifier::for_vote(&nsk, 1).to_byte_array();
        assert_eq!(nullifier, expected);
    }

    #[test]
    fn test_approve_circuit_different_proposals_different_nullifiers() {
        let nsk = test_nsk();
        let npk = test_npk();
        let members = vec![npk];

        let null_1 = approve_circuit(&nsk, &members, 1, &[]);
        let null_2 = approve_circuit(&nsk, &members, 2, &[]);

        assert_ne!(null_1, null_2, "Different proposals must produce different nullifiers");
    }

    #[test]
    #[should_panic(expected = "does not correspond to any multisig member")]
    fn test_approve_circuit_non_member_panics() {
        let nsk = test_nsk();
        let members = vec![
            NullifierPublicKey([1u8; 32]),
            NullifierPublicKey([2u8; 32]),
        ]; // our NPK is not in this list

        approve_circuit(&nsk, &members, 1, &[]);
    }

    #[test]
    #[should_panic(expected = "double vote detected")]
    fn test_approve_circuit_double_vote_panics() {
        let nsk = test_nsk();
        let npk = test_npk();
        let members = vec![npk];

        // First vote succeeds
        let nullifier = approve_circuit(&nsk, &members, 1, &[]);

        // Second vote with same nullifier in existing list should panic
        approve_circuit(&nsk, &members, 1, &[nullifier]);
    }
}
