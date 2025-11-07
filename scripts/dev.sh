#!/usr/bin/env bash

set -e

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
RESET='\033[0m'

echo -e "${CYAN}Starting Personal Memory Layer in development mode...${RESET}\n"

# Ensure port is free before starting a service
ensure_port_free() {
    local service="$1"
    local port="$2"

    if ! command -v lsof >/dev/null 2>&1; then
        echo -e "${YELLOW}lsof not available; cannot preflight port ${port}.${RESET}"
        return
    fi

    local pids
    pids=$(lsof -ti tcp:${port} 2>/dev/null | tr '\n' ' ')

    if [ -n "$pids" ]; then
        echo -e "${YELLOW}Port ${port} already in use. Stopping existing ${service} instance(s)...${RESET}"
        for pid in $pids; do
            echo -e "  ${YELLOW}Sending TERM to PID ${pid}${RESET}"
            kill "$pid" 2>/dev/null || true
        done
        sleep 1

        pids=$(lsof -ti tcp:${port} 2>/dev/null | tr '\n' ' ')
        if [ -n "$pids" ]; then
            for pid in $pids; do
                echo -e "  ${YELLOW}Force killing PID ${pid}${RESET}"
                kill -9 "$pid" 2>/dev/null || true
            done
            sleep 1
        fi
    fi

    if lsof -ti tcp:${port} >/dev/null 2>&1; then
        echo -e "${RED}Port ${port} is still busy. Please free it manually and retry.${RESET}"
        exit 1
    fi
}

# Function to cleanup background processes on exit
cleanup() {
    echo -e "\n${YELLOW}Shutting down services...${RESET}"
    kill $(jobs -p) 2>/dev/null || true
    pkill -f memory-layer-ingestion 2>/dev/null || true
    pkill -f memory-layer-indexing 2>/dev/null || true
    pkill -f memory-layer-composer 2>/dev/null || true
    echo -e "${GREEN}✓ Services stopped${RESET}"
}

trap cleanup EXIT INT TERM

# Start ingestion service
echo -e "${CYAN}Starting ingestion service...${RESET}"
ensure_port_free "ingestion" 21953
cd core/ingestion
cargo run --bin memory-layer-ingestion &
INGESTION_PID=$!
cd ../..

# Wait a moment for ingestion to start
sleep 2

# Start indexing service
echo -e "${CYAN}Starting indexing service...${RESET}"
ensure_port_free "indexing" 21954
cd core/indexing
cargo run --bin memory-layer-indexing &
INDEXING_PID=$!
cd ../..

# Wait a moment for indexing to start
sleep 2

# Start composer service
echo -e "${CYAN}Starting composer service...${RESET}"
ensure_port_free "composer" 21955
cd core/composer
cargo run &
COMPOSER_PID=$!
cd ../..

# Wait a moment for composer to start
sleep 2

# Start Chrome extension dev server (if exists)
if [ -f "./apps/chrome-ext/package.json" ]; then
    echo -e "${CYAN}Starting Chrome extension dev server...${RESET}"
    cd apps/chrome-ext
    npm run dev &
    CHROME_PID=$!
    cd ../..
fi

# Start VSCode extension dev server (if exists)
if [ -f "./apps/vscode-ext/package.json" ]; then
    echo -e "${CYAN}Starting VSCode extension dev server...${RESET}"
    cd apps/vscode-ext
    npm run dev &
    VSCODE_PID=$!
    cd ../..
fi

echo -e "\n${GREEN}========================================${RESET}"
echo -e "${GREEN}✓ All services started!${RESET}"
echo -e "${GREEN}========================================${RESET}"
echo -e "\n${CYAN}Services running:${RESET}"
echo -e "  - Ingestion service (PID: $INGESTION_PID)"
echo -e "  - Indexing service (PID: $INDEXING_PID)"
echo -e "  - Composer service (PID: $COMPOSER_PID)"
[ ! -z "$CHROME_PID" ] && echo -e "  - Chrome extension dev server (PID: $CHROME_PID)"
[ ! -z "$VSCODE_PID" ] && echo -e "  - VSCode extension dev server (PID: $VSCODE_PID)"
echo -e "\n${CYAN}Provider endpoints:${RESET}"
echo -e "  - HTTP: ${YELLOW}http://127.0.0.1:21955/v1/context${RESET}"
echo -e "  - Unix Socket: ${YELLOW}~/Library/Application Support/MemoryLayer/context.sock${RESET}"
echo -e "\n${YELLOW}Press Ctrl+C to stop all services${RESET}\n"

# Wait for all background processes
wait
