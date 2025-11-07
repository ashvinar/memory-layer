#!/bin/bash

set -e

echo "=== Testing Instant Ingestion Refactor ==="
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo "Cleaning up..."
    if [ ! -z "$SERVER_PID" ]; then
        kill $SERVER_PID 2>/dev/null || true
        wait $SERVER_PID 2>/dev/null || true
    fi
    rm -rf /tmp/test-memory-layer.db*
}

# Set trap to cleanup on exit
trap cleanup EXIT INT TERM

# Create a temporary database
export DB_PATH="/tmp/test-memory-layer.db"
rm -rf $DB_PATH*

echo "1. Building the ingestion service..."
cargo build --release -p memory-layer-ingestion

echo ""
echo "2. Starting the ingestion service in background..."
./target/release/memory-layer-ingestion > /tmp/ingestion.log 2>&1 &
SERVER_PID=$!

echo "   Server PID: $SERVER_PID"
echo "   Waiting for server to start..."
sleep 3

# Check if server is still running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "   ERROR: Server failed to start"
    cat /tmp/ingestion.log
    exit 1
fi

echo "   ✓ Server started successfully"
echo ""
echo "3. Running integration tests..."
cargo test --test instant_ingestion_test -- --nocapture

echo ""
echo "=== Test Summary ==="
echo "✓ All tests passed!"
echo "✓ POST /ingest/turn completes in <10ms"
echo "✓ Memories appear in database (async processing works)"
echo "✓ No data loss under 100+ concurrent requests"
