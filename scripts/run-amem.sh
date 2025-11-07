#!/bin/bash
# Script to run the A-mem enhanced indexing service

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🧠 Memory Layer A-mem Service Launcher"
echo "======================================="

# Check for required environment variables
if [ -z "$OPENAI_API_KEY" ] && [ -z "$ANTHROPIC_API_KEY" ] && [ -z "$OLLAMA_HOST" ]; then
    echo "⚠️  No LLM provider configured!"
    echo ""
    echo "For best results, set one of:"
    echo "  • OPENAI_API_KEY for OpenAI GPT"
    echo "  • ANTHROPIC_API_KEY for Claude"
    echo "  • OLLAMA_HOST for local Ollama"
    echo ""
    echo "Continuing with basic keyword extraction..."
    echo ""
fi

# Build the enhanced service
echo "📦 Building A-mem enhanced service..."
cd "$PROJECT_ROOT/core/indexing"

# Create a binary specifically for A-mem
cargo build --release --bin amem-service 2>/dev/null || {
    echo "Creating amem-service binary target..."

    # Create bin directory if it doesn't exist
    mkdir -p src/bin

    # Copy main_amem.rs to bin/amem-service.rs
    cp src/main_amem.rs src/bin/amem-service.rs

    # Build again
    cargo build --release --bin amem-service
}

# Ensure database directory exists
DB_DIR="$HOME/Library/Application Support/MemoryLayer"
mkdir -p "$DB_DIR"

echo ""
echo "🚀 Starting A-mem enhanced indexing service..."
echo "   Port: 21956"
echo "   Database: $DB_DIR/memory.db"
echo ""

# Set database path
export DB_PATH="$DB_DIR/memory.db"

# Run the service
if [ -f "target/release/amem-service" ]; then
    exec "$PROJECT_ROOT/core/indexing/target/release/amem-service"
else
    # Fallback: run with cargo
    exec cargo run --release --bin amem-service
fi