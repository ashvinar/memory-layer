.PHONY: all bootstrap build run test lint clean pkg help
.DEFAULT_GOAL := help

# Colors for output
CYAN := \033[0;36m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
RESET := \033[0m

# Project paths
PROJECT_ROOT := $(shell pwd)
CORE_DIR := $(PROJECT_ROOT)/core
APPS_DIR := $(PROJECT_ROOT)/apps
SCRIPTS_DIR := $(PROJECT_ROOT)/scripts
TESTS_DIR := $(PROJECT_ROOT)/tests

# Binary paths
CARGO := $(shell which cargo)
NPM := $(shell which npm)
XCODEBUILD := $(shell which xcodebuild)
TSC := $(shell which tsc)

##@ General

help: ## Display this help
	@echo "$(CYAN)Personal Memory Layer - Build System$(RESET)"
	@echo ""
	@awk 'BEGIN {FS = ":.*##"; printf "Usage: make $(CYAN)<target>$(RESET)\n"} /^[a-zA-Z_-]+:.*?##/ { printf "  $(CYAN)%-15s$(RESET) %s\n", $$1, $$2 } /^##@/ { printf "\n$(YELLOW)%s$(RESET)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

##@ Setup

bootstrap: ## Install all toolchains and dependencies
	@echo "$(CYAN)Bootstrapping development environment...$(RESET)"
	@$(SCRIPTS_DIR)/bootstrap.sh

check-tools: ## Check if required tools are installed
	@echo "$(CYAN)Checking required tools...$(RESET)"
	@command -v cargo >/dev/null 2>&1 || { echo "$(RED)❌ Rust/Cargo not found$(RESET)"; exit 1; }
	@echo "$(GREEN)✓ Rust $(shell cargo --version)$(RESET)"
	@command -v npm >/dev/null 2>&1 || { echo "$(RED)❌ npm not found$(RESET)"; exit 1; }
	@echo "$(GREEN)✓ npm $(shell npm --version)$(RESET)"
	@command -v tsc >/dev/null 2>&1 || { echo "$(RED)❌ TypeScript not found$(RESET)"; exit 1; }
	@echo "$(GREEN)✓ TypeScript $(shell tsc --version)$(RESET)"
	@command -v xcodebuild >/dev/null 2>&1 || { echo "$(RED)❌ Xcode not found$(RESET)"; exit 1; }
	@echo "$(GREEN)✓ Xcode installed$(RESET)"

##@ Build

build: build-rust build-mac build-chrome build-vscode ## Build all components
	@echo "$(GREEN)✓ All components built successfully$(RESET)"

build-rust: ## Build Rust core services
	@echo "$(CYAN)Building Rust core services...$(RESET)"
	@cd $(CORE_DIR) && cargo build --release
	@echo "$(GREEN)✓ Rust services built$(RESET)"

build-mac: ## Build macOS app
	@echo "$(CYAN)Building macOS app...$(RESET)"
	@if [ -f "$(APPS_DIR)/mac-daemon/MemoryLayer.xcodeproj/project.pbxproj" ]; then \
		xcodebuild -project $(APPS_DIR)/mac-daemon/MemoryLayer.xcodeproj \
			-scheme MemoryLayer \
			-configuration Release \
			build; \
		echo "$(GREEN)✓ macOS app built$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ macOS app project not found, skipping$(RESET)"; \
	fi

build-chrome: ## Build Chrome extension
	@echo "$(CYAN)Building Chrome extension...$(RESET)"
	@if [ -f "$(APPS_DIR)/chrome-ext/package.json" ]; then \
		cd $(APPS_DIR)/chrome-ext && npm install && npm run build; \
		echo "$(GREEN)✓ Chrome extension built$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ Chrome extension not found, skipping$(RESET)"; \
	fi

build-vscode: ## Build VSCode extension
	@echo "$(CYAN)Building VSCode extension...$(RESET)"
	@if [ -f "$(APPS_DIR)/vscode-ext/package.json" ]; then \
		cd $(APPS_DIR)/vscode-ext && npm install && npm run build; \
		echo "$(GREEN)✓ VSCode extension built$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ VSCode extension not found, skipping$(RESET)"; \
	fi

##@ Development

run: ## Launch all services with hot reload
	@echo "$(CYAN)Starting Memory Layer services...$(RESET)"
	@$(SCRIPTS_DIR)/dev.sh

dev-rust: ## Run Rust services in development mode
	@echo "$(CYAN)Starting Rust services...$(RESET)"
	@cd $(CORE_DIR)/ingestion && cargo run & \
	cd $(CORE_DIR)/indexing && cargo run & \
	cd $(CORE_DIR)/composer && cargo run & \
	wait

dev-chrome: ## Run Chrome extension in development mode
	@echo "$(CYAN)Starting Chrome extension dev server...$(RESET)"
	@cd $(APPS_DIR)/chrome-ext && npm run dev

dev-vscode: ## Run VSCode extension in development mode
	@echo "$(CYAN)Starting VSCode extension dev mode...$(RESET)"
	@cd $(APPS_DIR)/vscode-ext && npm run dev

##@ Testing

test: test-rust test-chrome test-vscode test-e2e ## Run all tests
	@echo "$(GREEN)✓ All tests passed$(RESET)"

test-rust: ## Run Rust unit tests
	@echo "$(CYAN)Running Rust tests...$(RESET)"
	@cd $(CORE_DIR) && cargo test
	@echo "$(GREEN)✓ Rust tests passed$(RESET)"

test-chrome: ## Run Chrome extension tests
	@echo "$(CYAN)Running Chrome extension tests...$(RESET)"
	@if [ -f "$(APPS_DIR)/chrome-ext/package.json" ]; then \
		cd $(APPS_DIR)/chrome-ext && npm test; \
		echo "$(GREEN)✓ Chrome tests passed$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ Chrome extension not found, skipping$(RESET)"; \
	fi

test-vscode: ## Run VSCode extension tests
	@echo "$(CYAN)Running VSCode extension tests...$(RESET)"
	@if [ -f "$(APPS_DIR)/vscode-ext/package.json" ]; then \
		cd $(APPS_DIR)/vscode-ext && npm test; \
		echo "$(GREEN)✓ VSCode tests passed$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ VSCode extension not found, skipping$(RESET)"; \
	fi

test-e2e: ## Run end-to-end tests
	@echo "$(CYAN)Running E2E tests...$(RESET)"
	@if [ -f "$(TESTS_DIR)/e2e/package.json" ]; then \
		cd $(TESTS_DIR)/e2e && npm install && npm test; \
		echo "$(GREEN)✓ E2E tests passed$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ E2E tests not found, skipping$(RESET)"; \
	fi

test-mac: ## Run macOS app tests (XCUITest)
	@echo "$(CYAN)Running macOS app tests...$(RESET)"
	@if [ -f "$(APPS_DIR)/mac-daemon/MemoryLayer.xcodeproj/project.pbxproj" ]; then \
		xcodebuild test -project $(APPS_DIR)/mac-daemon/MemoryLayer.xcodeproj \
			-scheme MemoryLayer; \
		echo "$(GREEN)✓ macOS tests passed$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ macOS app project not found, skipping$(RESET)"; \
	fi

##@ Quality

lint: lint-rust lint-ts lint-swift ## Run all linters
	@echo "$(GREEN)✓ All linting passed$(RESET)"

lint-rust: ## Run Rust linter (clippy)
	@echo "$(CYAN)Running Rust linter...$(RESET)"
	@cd $(CORE_DIR) && cargo clippy -- -D warnings
	@echo "$(GREEN)✓ Rust linting passed$(RESET)"

lint-ts: ## Run TypeScript linter (eslint)
	@echo "$(CYAN)Running TypeScript linter...$(RESET)"
	@if [ -f "$(APPS_DIR)/chrome-ext/package.json" ]; then \
		cd $(APPS_DIR)/chrome-ext && npm run lint; \
	fi
	@if [ -f "$(APPS_DIR)/vscode-ext/package.json" ]; then \
		cd $(APPS_DIR)/vscode-ext && npm run lint; \
	fi
	@echo "$(GREEN)✓ TypeScript linting passed$(RESET)"

lint-swift: ## Run Swift linter (swiftlint)
	@echo "$(CYAN)Running Swift linter...$(RESET)"
	@if command -v swiftlint >/dev/null 2>&1; then \
		cd $(APPS_DIR)/mac-daemon && swiftlint; \
		echo "$(GREEN)✓ Swift linting passed$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ swiftlint not installed, skipping$(RESET)"; \
	fi

format: format-rust format-ts format-swift ## Format all code

format-rust: ## Format Rust code
	@echo "$(CYAN)Formatting Rust code...$(RESET)"
	@cd $(CORE_DIR) && cargo fmt
	@echo "$(GREEN)✓ Rust code formatted$(RESET)"

format-ts: ## Format TypeScript code
	@echo "$(CYAN)Formatting TypeScript code...$(RESET)"
	@if [ -f "$(APPS_DIR)/chrome-ext/package.json" ]; then \
		cd $(APPS_DIR)/chrome-ext && npm run format; \
	fi
	@if [ -f "$(APPS_DIR)/vscode-ext/package.json" ]; then \
		cd $(APPS_DIR)/vscode-ext && npm run format; \
	fi
	@echo "$(GREEN)✓ TypeScript code formatted$(RESET)"

format-swift: ## Format Swift code
	@echo "$(CYAN)Formatting Swift code...$(RESET)"
	@if command -v swiftformat >/dev/null 2>&1; then \
		cd $(APPS_DIR)/mac-daemon && swiftformat .; \
		echo "$(GREEN)✓ Swift code formatted$(RESET)"; \
	else \
		echo "$(YELLOW)⚠ swiftformat not installed, skipping$(RESET)"; \
	fi

##@ Packaging

pkg: pkg-mac pkg-chrome pkg-vscode ## Build all distributable packages
	@echo "$(GREEN)✓ All packages created$(RESET)"

pkg-mac: ## Build signed macOS app
	@echo "$(CYAN)Building signed macOS app...$(RESET)"
	@$(SCRIPTS_DIR)/pkg.sh mac

pkg-chrome: ## Build Chrome extension CRX
	@echo "$(CYAN)Building Chrome extension CRX...$(RESET)"
	@$(SCRIPTS_DIR)/pkg.sh chrome

pkg-vscode: ## Build VSCode extension VSIX
	@echo "$(CYAN)Building VSCode extension VSIX...$(RESET)"
	@$(SCRIPTS_DIR)/pkg.sh vscode

##@ Cleanup

clean: ## Clean all build artifacts
	@echo "$(CYAN)Cleaning build artifacts...$(RESET)"
	@cd $(CORE_DIR) && cargo clean
	@rm -rf $(APPS_DIR)/chrome-ext/dist
	@rm -rf $(APPS_DIR)/chrome-ext/node_modules
	@rm -rf $(APPS_DIR)/vscode-ext/dist
	@rm -rf $(APPS_DIR)/vscode-ext/node_modules
	@rm -rf $(APPS_DIR)/vscode-ext/out
	@rm -rf $(TESTS_DIR)/e2e/node_modules
	@rm -f $(PROJECT_ROOT)/dist/*.crx
	@rm -f $(PROJECT_ROOT)/dist/*.vsix
	@rm -f $(PROJECT_ROOT)/dist/*.app.zip
	@echo "$(GREEN)✓ Clean complete$(RESET)"

clean-all: clean ## Clean everything including dependencies
	@echo "$(CYAN)Deep cleaning...$(RESET)"
	@find . -name "node_modules" -type d -exec rm -rf {} + 2>/dev/null || true
	@find . -name "target" -type d -exec rm -rf {} + 2>/dev/null || true
	@echo "$(GREEN)✓ Deep clean complete$(RESET)"

##@ Database

db-migrate: ## Run database migrations
	@echo "$(CYAN)Running database migrations...$(RESET)"
	@$(SCRIPTS_DIR)/migrate.sh

db-reset: ## Reset database (DESTRUCTIVE)
	@echo "$(RED)⚠ This will delete all data!$(RESET)"
	@read -p "Are you sure? [y/N] " -n 1 -r; \
	echo; \
	if [[ $$REPLY =~ ^[Yy]$$ ]]; then \
		rm -f ~/Library/Application\ Support/MemoryLayer/memory.db*; \
		echo "$(GREEN)✓ Database reset$(RESET)"; \
	fi
