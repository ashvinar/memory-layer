#!/usr/bin/env bash

set -e

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
RESET='\033[0m'

echo -e "${CYAN}Bootstrapping Personal Memory Layer development environment...${RESET}\n"

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Rust not found. Installing...${RESET}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo -e "${GREEN}✓ Rust installed${RESET}"
else
    echo -e "${GREEN}✓ Rust already installed ($(cargo --version))${RESET}"
fi

# Check for Node.js
if ! command -v node &> /dev/null; then
    echo -e "${RED}❌ Node.js not found. Please install Node.js 20+ and try again.${RESET}"
    exit 1
else
    echo -e "${GREEN}✓ Node.js installed ($(node --version))${RESET}"
fi

# Check for npm
if ! command -v npm &> /dev/null; then
    echo -e "${RED}❌ npm not found. Please install npm and try again.${RESET}"
    exit 1
else
    echo -e "${GREEN}✓ npm installed ($(npm --version))${RESET}"
fi

# Install global TypeScript if not present
if ! command -v tsc &> /dev/null; then
    echo -e "${YELLOW}TypeScript not found. Installing globally...${RESET}"
    npm install -g typescript
    echo -e "${GREEN}✓ TypeScript installed${RESET}"
else
    echo -e "${GREEN}✓ TypeScript already installed ($(tsc --version))${RESET}"
fi

# Install Rust tools
echo -e "\n${CYAN}Installing Rust development tools...${RESET}"
rustup component add clippy rustfmt
echo -e "${GREEN}✓ Rust tools installed${RESET}"

# Install Python for embeddings (if not present)
if ! command -v python3 &> /dev/null; then
    echo -e "${YELLOW}Python3 not found. Please install Python 3.11+ for embeddings support.${RESET}"
else
    echo -e "${GREEN}✓ Python3 installed ($(python3 --version))${RESET}"

    # Check for sentence-transformers
    if ! python3 -c "import sentence_transformers" &> /dev/null; then
        echo -e "${YELLOW}Installing sentence-transformers for embeddings...${RESET}"
        pip3 install sentence-transformers torch
        echo -e "${GREEN}✓ Embeddings dependencies installed${RESET}"
    else
        echo -e "${GREEN}✓ Embeddings dependencies already installed${RESET}"
    fi
fi

# Install Chrome extension dependencies
if [ -f "./apps/chrome-ext/package.json" ]; then
    echo -e "\n${CYAN}Installing Chrome extension dependencies...${RESET}"
    cd ./apps/chrome-ext
    npm install
    cd ../..
    echo -e "${GREEN}✓ Chrome extension dependencies installed${RESET}"
fi

# Install VSCode extension dependencies
if [ -f "./apps/vscode-ext/package.json" ]; then
    echo -e "\n${CYAN}Installing VSCode extension dependencies...${RESET}"
    cd ./apps/vscode-ext
    npm install
    cd ../..
    echo -e "${GREEN}✓ VSCode extension dependencies installed${RESET}"
fi

# Install E2E test dependencies
if [ -f "./tests/e2e/package.json" ]; then
    echo -e "\n${CYAN}Installing E2E test dependencies...${RESET}"
    cd ./tests/e2e
    npm install
    cd ../..
    echo -e "${GREEN}✓ E2E test dependencies installed${RESET}"
fi

# Build Rust workspace
echo -e "\n${CYAN}Building Rust workspace...${RESET}"
cargo build
echo -e "${GREEN}✓ Rust workspace built${RESET}"

# Check for Xcode
if ! command -v xcodebuild &> /dev/null; then
    echo -e "${YELLOW}⚠ Xcode not found. macOS app cannot be built.${RESET}"
else
    echo -e "${GREEN}✓ Xcode installed${RESET}"
fi

# Optional: Install swiftlint and swiftformat
if ! command -v swiftlint &> /dev/null; then
    echo -e "${YELLOW}swiftlint not found. Install with: brew install swiftlint${RESET}"
fi

if ! command -v swiftformat &> /dev/null; then
    echo -e "${YELLOW}swiftformat not found. Install with: brew install swiftformat${RESET}"
fi

# Create application support directory
mkdir -p ~/Library/Application\ Support/MemoryLayer
echo -e "${GREEN}✓ Application support directory created${RESET}"

echo -e "\n${GREEN}========================================${RESET}"
echo -e "${GREEN}✓ Bootstrap complete!${RESET}"
echo -e "${GREEN}========================================${RESET}"
echo -e "\n${CYAN}Next steps:${RESET}"
echo -e "  1. Run ${YELLOW}make build${RESET} to build all components"
echo -e "  2. Run ${YELLOW}make test${RESET} to run tests"
echo -e "  3. Run ${YELLOW}make run${RESET} to start the development server"
echo -e "  4. Run ${YELLOW}make help${RESET} for more commands\n"
