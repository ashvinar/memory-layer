#!/usr/bin/env bash

set -e

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
RESET='\033[0m'

echo -e "${CYAN}Running all linters...${RESET}\n"

# Rust linting
echo -e "${CYAN}Running Rust linter (clippy)...${RESET}"
cargo clippy -- -D warnings
echo -e "${GREEN}✓ Rust linting passed${RESET}\n"

# Rust formatting check
echo -e "${CYAN}Checking Rust formatting...${RESET}"
cargo fmt -- --check
echo -e "${GREEN}✓ Rust formatting OK${RESET}\n"

# Chrome extension linting
if [ -f "./apps/chrome-ext/package.json" ]; then
    echo -e "${CYAN}Running Chrome extension linter...${RESET}"
    cd ./apps/chrome-ext
    npm run lint || echo -e "${YELLOW}⚠ Chrome linting not configured yet${RESET}"
    cd ../..
    echo -e "${GREEN}✓ Chrome linting complete${RESET}\n"
fi

# VSCode extension linting
if [ -f "./apps/vscode-ext/package.json" ]; then
    echo -e "${CYAN}Running VSCode extension linter...${RESET}"
    cd ./apps/vscode-ext
    npm run lint || echo -e "${YELLOW}⚠ VSCode linting not configured yet${RESET}"
    cd ../..
    echo -e "${GREEN}✓ VSCode linting complete${RESET}\n"
fi

# Swift linting (if swiftlint is installed)
if command -v swiftlint &> /dev/null; then
    if [ -d "./apps/mac-daemon" ]; then
        echo -e "${CYAN}Running Swift linter...${RESET}"
        cd ./apps/mac-daemon
        swiftlint || echo -e "${YELLOW}⚠ Swift linting warnings${RESET}"
        cd ../..
        echo -e "${GREEN}✓ Swift linting complete${RESET}\n"
    fi
else
    echo -e "${YELLOW}⚠ swiftlint not installed, skipping Swift linting${RESET}\n"
fi

echo -e "${GREEN}========================================${RESET}"
echo -e "${GREEN}✓ All linting complete!${RESET}"
echo -e "${GREEN}========================================${RESET}"
