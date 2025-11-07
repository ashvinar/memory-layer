#!/usr/bin/env bash

set -e

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
RESET='\033[0m'

COMPONENT=$1
DIST_DIR="./dist"

# Create dist directory
mkdir -p "$DIST_DIR"

case "$COMPONENT" in
    mac)
        echo -e "${CYAN}Packaging macOS app...${RESET}"

        if [ ! -f "./apps/mac-daemon/MemoryLayer.xcodeproj/project.pbxproj" ]; then
            echo -e "${RED}❌ macOS app project not found${RESET}"
            exit 1
        fi

        # Build for release
        xcodebuild \
            -project ./apps/mac-daemon/MemoryLayer.xcodeproj \
            -scheme MemoryLayer \
            -configuration Release \
            -archivePath "$DIST_DIR/MemoryLayer.xcarchive" \
            archive

        # Export app
        xcodebuild \
            -exportArchive \
            -archivePath "$DIST_DIR/MemoryLayer.xcarchive" \
            -exportPath "$DIST_DIR" \
            -exportOptionsPlist ./apps/mac-daemon/ExportOptions.plist

        # Create zip
        cd "$DIST_DIR"
        zip -r MemoryLayer.app.zip MemoryLayer.app
        cd ..

        echo -e "${GREEN}✓ macOS app packaged: $DIST_DIR/MemoryLayer.app.zip${RESET}"
        ;;

    chrome)
        echo -e "${CYAN}Packaging Chrome extension...${RESET}"

        if [ ! -f "./apps/chrome-ext/package.json" ]; then
            echo -e "${RED}❌ Chrome extension not found${RESET}"
            exit 1
        fi

        # Build extension
        cd ./apps/chrome-ext
        npm run build

        # Create ZIP for Chrome Web Store
        cd dist
        zip -r "../../../dist/chrome-ext.zip" ./*
        cd ../../..

        echo -e "${GREEN}✓ Chrome extension packaged: $DIST_DIR/chrome-ext.zip${RESET}"
        echo -e "${YELLOW}Note: For CRX, use Chrome's built-in packager${RESET}"
        ;;

    vscode)
        echo -e "${CYAN}Packaging VSCode extension...${RESET}"

        if [ ! -f "./apps/vscode-ext/package.json" ]; then
            echo -e "${RED}❌ VSCode extension not found${RESET}"
            exit 1
        fi

        # Install vsce if not present
        if ! command -v vsce &> /dev/null; then
            echo -e "${YELLOW}Installing vsce...${RESET}"
            npm install -g @vscode/vsce
        fi

        # Build and package
        cd ./apps/vscode-ext
        npm run build
        vsce package --out "../../dist/memory-layer-vscode.vsix"
        cd ../..

        echo -e "${GREEN}✓ VSCode extension packaged: $DIST_DIR/memory-layer-vscode.vsix${RESET}"
        ;;

    *)
        echo -e "${RED}Usage: $0 {mac|chrome|vscode}${RESET}"
        exit 1
        ;;
esac

echo -e "${GREEN}✓ Packaging complete${RESET}"
