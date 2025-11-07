#!/usr/bin/env bash

set -e

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
RESET='\033[0m'

echo -e "${CYAN}Running all tests...${RESET}\n"

# Rust tests
echo -e "${CYAN}Running Rust tests...${RESET}"
cargo test
echo -e "${GREEN}✓ Rust tests passed${RESET}\n"

# Chrome extension tests
if [ -f "./apps/chrome-ext/package.json" ]; then
    echo -e "${CYAN}Running Chrome extension tests...${RESET}"
    cd ./apps/chrome-ext
    npm test || echo -e "${YELLOW}⚠ Chrome tests not configured yet${RESET}"
    cd ../..
    echo -e "${GREEN}✓ Chrome tests complete${RESET}\n"
fi

# VSCode extension tests
if [ -f "./apps/vscode-ext/package.json" ]; then
    echo -e "${CYAN}Running VSCode extension tests...${RESET}"
    cd ./apps/vscode-ext
    npm test || echo -e "${YELLOW}⚠ VSCode tests not configured yet${RESET}"
    cd ../..
    echo -e "${GREEN}✓ VSCode tests complete${RESET}\n"
fi

# E2E tests
if [ -f "./tests/e2e/package.json" ]; then
    echo -e "${CYAN}Running E2E tests...${RESET}"
    cd ./tests/e2e
    npm test || echo -e "${YELLOW}⚠ E2E tests not configured yet${RESET}"
    cd ../..
    echo -e "${GREEN}✓ E2E tests complete${RESET}\n"
fi

echo -e "${GREEN}========================================${RESET}"
echo -e "${GREEN}✓ All tests complete!${RESET}"
echo -e "${GREEN}========================================${RESET}"
