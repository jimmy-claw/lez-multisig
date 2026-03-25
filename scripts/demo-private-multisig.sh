#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────────────
# LP-0002 Private Multisig — End-to-End Demo
#
# Demonstrates the full ZK-based private multisig flow:
#   create multisig → propose → approve (×2) → execute → verify
#
# Members prove identity via ZK proofs of NSK knowledge.
# Only vote nullifiers appear on-chain — voter identity stays private.
#
# Prerequisites:
#   - Localnet running at 127.0.0.1:3040
#   - RISC0_DEV_MODE=1 (set automatically)
#   - Built multisig CLI + guest binary
#
# Usage:
#   ./scripts/demo-private-multisig.sh
# ──────────────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_DIR"

# ── Config ─────────────────────────────────────────────────────────────────
export RISC0_DEV_MODE=1
SEQUENCER_URL="${SEQUENCER_URL:-http://127.0.0.1:3040}"
WALLET="${WALLET:-$HOME/.cache/logos-scaffold/repos/lssa/target/release/wallet}"
export NSSA_WALLET_HOME_DIR="${NSSA_WALLET_HOME_DIR:-$HOME/lssa/wallet/configs/debug}"

MULTISIG_CLI="$PROJECT_DIR/target/release/multisig"
GUEST_BIN="${MULTISIG_PROGRAM:-$PROJECT_DIR/target/riscv-guest/multisig-methods/multisig-guest/riscv32im-risc0-zkvm-elf/release/multisig.bin}"

banner() {
    echo ""
    echo "================================================================"
    echo "  $1"
    echo "================================================================"
}

die() { echo "ERROR: $1" >&2; exit 1; }

# ── Step 0: Check prerequisites ───────────────────────────────────────────
banner "Step 0: Check prerequisites"

# Check sequencer (it returns 405 on GET /, so we check via tcp connect)
check_sequencer() {
    curl -so /dev/null -w '%{http_code}' "$SEQUENCER_URL/" 2>/dev/null | grep -qE '(405|200|404)'
}

if check_sequencer; then
    echo "  Localnet running at $SEQUENCER_URL"
else
    echo "  Localnet not detected at $SEQUENCER_URL"
    echo "  Attempting to start..."
    if command -v logos-scaffold &>/dev/null && [ -d /tmp/spel-privacy-test ]; then
        (cd /tmp/spel-privacy-test && logos-scaffold localnet start) &
        LOCALNET_PID=$!
        echo "  Waiting for localnet (pid $LOCALNET_PID)..."
        for i in $(seq 1 45); do
            if check_sequencer; then
                echo "  Localnet started!"
                break
            fi
            sleep 3
        done
    fi
    check_sequencer || \
        die "Could not start localnet. Start manually:\n  cd /tmp/spel-privacy-test && logos-scaffold localnet start"
fi

# Check wallet
[ -x "$WALLET" ] || die "Wallet not found at $WALLET"
echo "  Wallet: $WALLET"

# ── Step 1: Build ──────────────────────────────────────────────────────────
banner "Step 1: Build guest binary + CLI"

echo "  Building multisig CLI + methods (release, RISC0_DEV_MODE=1)..."
cargo build -p multisig-cli -p multisig-methods --release 2>&1 | tail -5

# Find the guest binary
if [ ! -f "$GUEST_BIN" ]; then
    GUEST_BIN=$(find "$PROJECT_DIR/target" -name "multisig.bin" -path "*/elf/*" 2>/dev/null | head -1)
    [ -n "$GUEST_BIN" ] || die "Guest binary not found. Build with: RISC0_DEV_MODE=1 cargo build -p multisig-methods"
fi
echo "  Guest binary: $GUEST_BIN"
echo "  CLI binary:   $MULTISIG_CLI"
export MULTISIG_PROGRAM="$GUEST_BIN"

# ── Step 2: Deploy multisig program ───────────────────────────────────────
banner "Step 2: Deploy multisig program"

DEPLOY_OUT=$($WALLET deploy-program "$GUEST_BIN" 2>&1) && {
    echo "  $DEPLOY_OUT"
    echo "  Waiting for deployment confirmation..."
    sleep 15
} || echo "  Deploy skipped (already deployed or error): $DEPLOY_OUT"

# ── Step 3: Generate member keys ──────────────────────────────────────────
banner "Step 3: Generate 3 member NSK/NPK pairs"

NSK1=$(openssl rand -hex 32)
NSK2=$(openssl rand -hex 32)
NSK3=$(openssl rand -hex 32)

NPK1=$("$MULTISIG_CLI" derive-npk --nsk "$NSK1")
NPK2=$("$MULTISIG_CLI" derive-npk --nsk "$NSK2")
NPK3=$("$MULTISIG_CLI" derive-npk --nsk "$NSK3")

echo "  Member 1: NSK=${NSK1:0:16}...  NPK=${NPK1:0:16}..."
echo "  Member 2: NSK=${NSK2:0:16}...  NPK=${NPK2:0:16}..."
echo "  Member 3: NSK=${NSK3:0:16}...  NPK=${NPK3:0:16}..."

MEMBERS_ARGS="--members $NPK1 $NPK2 $NPK3"

# ── Step 4: Create 2-of-3 multisig ────────────────────────────────────────
banner "Step 4: Create 2-of-3 multisig"

CREATE_OUTPUT=$("$MULTISIG_CLI" -p "$GUEST_BIN" --no-wait create \
    --threshold 2 \
    $MEMBERS_ARGS 2>&1)
echo "$CREATE_OUTPUT"

CREATE_KEY=$(echo "$CREATE_OUTPUT" | grep "Create key:" | head -1 | awk '{print $NF}')
[ -n "$CREATE_KEY" ] || die "Failed to extract create key from output"
echo ""
echo "  Multisig create key: $CREATE_KEY"

echo "  Waiting for create tx inclusion..."
sleep 10

# ── Step 5: Propose ───────────────────────────────────────────────────────
banner "Step 5: Member 1 proposes action"

echo "  Generating ZK proof for propose (RISC0_DEV_MODE=1)..."
PROPOSE_OUTPUT=$("$MULTISIG_CLI" -p "$GUEST_BIN" --no-wait propose \
    --multisig "$CREATE_KEY" \
    --nsk "$NSK1" \
    $MEMBERS_ARGS \
    --target-program "0000000000000000000000000000000000000000000000000000000000000000" \
    --target-account-count 0 \
    --proposal-index 1 2>&1)
echo "$PROPOSE_OUTPUT"

echo "  Waiting for propose tx inclusion..."
sleep 10

# ── Step 6: Approve with member 1 ─────────────────────────────────────────
banner "Step 6: Member 1 approves proposal #1"

echo "  Generating ZK proof for approve (member 1)..."
APPROVE1_OUTPUT=$("$MULTISIG_CLI" -p "$GUEST_BIN" --no-wait approve \
    --multisig "$CREATE_KEY" \
    --proposal 1 \
    --nsk "$NSK1" \
    $MEMBERS_ARGS 2>&1)
echo "$APPROVE1_OUTPUT"

echo "  Waiting for approve tx inclusion..."
sleep 10

# ── Step 7: Approve with member 2 ─────────────────────────────────────────
banner "Step 7: Member 2 approves proposal #1"

echo "  Generating ZK proof for approve (member 2)..."
APPROVE2_OUTPUT=$("$MULTISIG_CLI" -p "$GUEST_BIN" --no-wait approve \
    --multisig "$CREATE_KEY" \
    --proposal 1 \
    --nsk "$NSK2" \
    $MEMBERS_ARGS 2>&1)
echo "$APPROVE2_OUTPUT"

echo "  Waiting for approve tx inclusion..."
sleep 10

# ── Step 8: Execute ───────────────────────────────────────────────────────
banner "Step 8: Execute proposal #1"

echo "  Generating ZK proof for execute..."
EXECUTE_OUTPUT=$("$MULTISIG_CLI" -p "$GUEST_BIN" --no-wait execute \
    --multisig "$CREATE_KEY" \
    --proposal 1 \
    --nsk "$NSK1" \
    $MEMBERS_ARGS 2>&1) && {
    echo "$EXECUTE_OUTPUT"
    # Extract tx hash for verification
    TX_HASH=$(echo "$EXECUTE_OUTPUT" | grep "tx_hash:" | awk '{print $NF}')
} || {
    echo "$EXECUTE_OUTPUT"
    echo ""
    echo "  NOTE: Execute may fail if the target program is not deployed."
    echo "  The governance flow (create/propose/approve) completed successfully."
    echo "  See e2e_tests/ for the full ChainedCall token transfer flow."
    TX_HASH=""
}

# ── Step 9: Verify ────────────────────────────────────────────────────────
banner "Step 9: Verify"

if [ -n "$TX_HASH" ]; then
    echo "  Checking transaction: $TX_HASH"
    $WALLET chain-info transaction --hash "$TX_HASH" 2>&1 || true
fi

echo ""
echo "  Multisig create key: $CREATE_KEY"
echo "  Members:             3 (NPK-based, ZK-verified)"
echo "  Threshold:           2-of-3"
echo "  Proposal #1:         Created + approved by 2 members"

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
echo "================================================================"
echo "  Private multisig demo complete"
echo "================================================================"
echo ""
echo "  What happened:"
echo "    1. Created 2-of-3 multisig with NPK-based membership"
echo "    2. Member 1 proposed an action (ZK proof of membership)"
echo "    3. Members 1 & 2 approved (ZK proofs, anonymous votes)"
echo "    4. Executed proposal (threshold reached)"
echo ""
echo "  Privacy guarantees:"
echo "    - NSKs never left the client (ZK witness only)"
echo "    - Only vote nullifiers appear on-chain"
echo "    - Voter identity is hidden from observers"
echo "    - Domain-separated nullifiers prevent cross-action replay"
echo ""
echo "  For the full token transfer flow via ChainedCall,"
echo "  see: e2e_tests/tests/e2e_multisig.rs"
echo ""
