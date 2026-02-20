#!/bin/bash
# Run e2e tests with a fresh sequencer
set -e

LSSA_DIR="${LSSA_DIR:-$HOME/lssa}"
PROGRAM="${MULTISIG_PROGRAM:-$(pwd)/target/riscv32im-risc0-zkvm-elf/docker/multisig.bin}"
TOKEN_PROGRAM="${TOKEN_PROGRAM:-${LSSA_DIR}/artifacts/program_methods/token.bin}"
SEQ_PORT=3040
SEQ_URL="http://127.0.0.1:${SEQ_PORT}"

echo "🧹 Cleaning up old sequencer..."
pkill -f sequencer_runner 2>/dev/null || true
sleep 2
rm -rf "${LSSA_DIR}/.sequencer_db" "${LSSA_DIR}/rocksdb"

echo "🚀 Starting fresh sequencer..."
cd "$LSSA_DIR"
RUST_LOG=info cargo run --features standalone -p sequencer_runner -- sequencer_runner/configs/debug > ~/sequencer.log 2>&1 &
SEQ_PID=$!
cd - > /dev/null

echo "   PID: $SEQ_PID, waiting for startup..."

# Wait until sequencer is listening (up to 30 minutes for compilation)
echo "   Waiting for sequencer to listen on port $SEQ_PORT..."
for i in $(seq 1 360); do
    if curl -s "$SEQ_URL" > /dev/null 2>&1; then
        echo "   ✅ Sequencer is up! (waited ${i}x5s)"
        break
    fi
    if ! kill -0 $SEQ_PID 2>/dev/null; then
        echo "❌ Sequencer process died. Check ~/sequencer.log"
        exit 1
    fi
    sleep 5
done

if ! curl -s "$SEQ_URL" > /dev/null 2>&1; then
    echo "❌ Sequencer failed to start after 30 min. Check ~/sequencer.log"
    kill $SEQ_PID 2>/dev/null
    exit 1
fi

echo "🧪 Running e2e tests..."
MULTISIG_PROGRAM="$PROGRAM" \
TOKEN_PROGRAM="$TOKEN_PROGRAM" \
SEQUENCER_URL="$SEQ_URL" \
cargo test -p lez-multisig-e2e --test e2e_multisig -- --nocapture --test-threads=1 2>&1 | tee ~/e2e-test.log

EXIT_CODE=${PIPESTATUS[0]}

echo ""
echo "🔧 Stopping sequencer..."
kill $SEQ_PID 2>/dev/null || true

if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ All e2e tests passed!"
else
    echo "❌ Some tests failed. Check ~/e2e-test.log and ~/sequencer.log"
fi

exit $EXIT_CODE
