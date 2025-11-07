# Repository Guidelines

## Project Structure & Module Organization
- `core/` contains the Rust workspace; `ingestion`, `indexing`, `composer`, and shared schemas live here. Update schemas before touching adapters.
- `adapters/` carries injector/provider bridges that expose the Rust services to clients; keep protocol changes synchronized with both ends.
- `apps/` hosts user interfaces: `mac-daemon` (Swift menu bar), `chrome-ext` (MV3 TypeScript), `safari-ext` placeholder, `vscode-ext` stub awaiting wiring.
- Automation and utilities live in `scripts/`; integration and regression suites live in `tests/unit` and `tests/e2e`.

## Build, Test, and Development Commands
- `make bootstrap` installs Rust, npm, and Swift dependencies via `scripts/bootstrap.sh`.
- `make build` compiles Rust services in release mode, builds the Mac app, and bundles the extensions.
- `make run` launches the orchestrated dev stack with hot reload, calling `scripts/dev.sh`.
- `make test` executes Rust unit, extension, and e2e suites; use `make test-rust` or `make test-e2e` when iterating.
- Component loops: `cd core/ingestion && cargo run` to exercise the log pipeline; `cd apps/chrome-ext && npm run dev` for extension live reload.

## Coding Style & Naming Conventions
- Format Rust code with `cargo fmt` (4-space indent, snake_case modules) and keep imports ordered; enforce `cargo clippy -- -D warnings` before review.
- Swift code follows `swiftlint` defaults; prefer 4-space indents and PascalCase types under `apps/mac-daemon`.
- TypeScript modules stick to 2-space indents, camelCase exports, and live under `apps/chrome-ext/src`; add or update `npm run lint` and `npm run format` scripts to match Makefile targets when extending.

## Testing Guidelines
- Write Rust unit tests alongside modules in each `core/*/src` directory and name test helpers `_tests` modules.
- Browser and VSCode extensions should expose Jest or Playwright specs named `*.spec.ts` within their respective `tests` directories and run via `npm test`.
- End-to-end scenarios live in `tests/e2e`; prefer Playwright for browser flows and XCUITest harnesses for macOS automation.
- Target ≥80% relevant branch coverage on core services; capture new fixtures under `tests/unit/data`.

## Commit & Pull Request Guidelines
- Git history is empty; use imperative, component-scoped messages going forward (for example, `core: tighten memory pruning`).
- Reference issue IDs when available, include rationale plus testing notes in the PR description, and attach screenshots or logs for UI changes.
- Keep PRs focused; update docs and schema files in the same change when API surfaces evolve.
