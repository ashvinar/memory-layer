# Personal Memory Layer

A local, privacy-first system that captures text (no screenshots), distills it into durable Memories, and injects compact Context Capsules into any AI assistant via Pull (local API), Merge (intercept send), or Prefill (fallback).

## Privacy First

**Reads text. Never screenshots.**

- All data stays on your device (SQLite)
- Optional E2EE sync only
- Redaction of secrets enabled by default
- Visible & undoable injection
- Never auto-submits

## Features

### Auto-Carry Handoff
Use Claude Code in terminal, then open ChatGPT web—the input stays clean but a ribbon says "Context applied · Undo".

### Add Context Pill
In any assistant input, a subtle **Add Context** pill appears if helpful; tapping inserts a short preamble with **Added · Undo** chip.

### Memory Search (⌘⌥K)
Spotlight-like search for "andrew invoice", "renderer lines 45–90". Pick results → tray shows token count → **Insert into chat**.

### Delta Injection
In the same thread, only changes are added or we show **Up to date**.

### Pause & Privacy
Menu-bar switch **Pause 1h**; settings show **Reads text. Never screenshots.** Redaction on by default.

## Architecture

### Components

- **apps/mac-daemon** - Swift menu-bar app with local provider (HTTP + Unix socket)
- **apps/chrome-ext** - MV3 extension for Claude.ai and ChatGPT
- **apps/vscode-ext** - TypeScript extension for VSCode integration
- **core/ingestion** - Rust service for event log and normalizers
- **core/indexing** - Rust IR: embeddings + BM25 + recency; Graph store
- **core/composer** - Rust lib to build Context Capsules

### Data Model

**Turn** - Append-only conversation events
**Memory** - Durable facts, decisions, snippets, tasks
**Context Capsule** - Compact summaries (Short/Standard/Detailed)

## Quick Start

### Prerequisites

- macOS 14.0+ (Apple Silicon or Intel)
- Rust 1.80+ (installed automatically by `make bootstrap`)
- Swift 5.9+ (Xcode)
- Node.js 20+ (for extensions)
- TypeScript 5+ (installed automatically)

### Installation

```bash
# Clone the repository
cd ~/memory-layer

# Bootstrap all toolchains and dependencies
make bootstrap

# Build all components
make build

# Run the system
make run

# Run tests
make test
```

### First Run

1. Launch the menu-bar app
2. Grant Accessibility permissions when prompted
3. Click the brain icon → **Manage Apps…** to enable the assistants you use, or add new applications with **Add Application…**
4. Choose context style (Short / Standard / Detailed)
5. Install browser extension from `dist/chrome-ext.crx`
6. Install VSCode extension from `dist/vscode-ext.vsix`

## Development

### Project Structure

```
memory-layer/
├── apps/
│   ├── mac-daemon/         # Swift menu-bar app
│   ├── chrome-ext/         # Chrome MV3 extension
│   ├── safari-ext/         # Safari extension (stub)
│   └── vscode-ext/         # VSCode extension
├── core/
│   ├── ingestion/          # Event capture & normalization
│   ├── indexing/           # Hybrid search (BM25 + embeddings)
│   ├── composer/           # Context Capsule builder
│   └── schemas/            # JSON schemas & type bindings
├── adapters/
│   ├── injectors/          # Merge/Prefill/AX injection logic
│   └── providers/          # Local HTTP & Unix socket servers
├── tests/
│   ├── e2e/               # Playwright + XCUITest
│   └── unit/              # Rust + Swift + TS unit tests
└── scripts/               # Build and dev tooling
```

### Available Commands

```bash
make bootstrap    # Install all toolchains
make build        # Build all components
make run          # Launch all services with hot reload
make test         # Run full test suite
make lint         # Run all linters
make pkg          # Build signed Mac app + CRX + VSIX
make clean        # Clean all build artifacts
```

### Running Components Individually

```bash
# Rust core services
cd core/ingestion && cargo run
cd core/indexing && cargo run
cd core/composer && cargo run

# macOS app
open apps/mac-daemon/MemoryLayer.xcodeproj

# Chrome extension
cd apps/chrome-ext && npm run dev

# VSCode extension
cd apps/vscode-ext && npm run dev
```

### Managing Connected Apps

- Open the menu-bar icon and choose **Manage Apps…** to see everything Memory Layer can capture.
- Toggle monitoring per application; enabled apps update the accessibility whitelist immediately.
- Use **Add Application…** to connect additional `.app` bundles (custom IDEs, browsers, or tools).
- Remove custom entries or hit **Refresh** to rescan for app icons and updated install paths.
- Launch the **Memory Console…** from the menu bar to browse captured memories, topic heatmaps, a live 3D knowledge graph, and the current connection status.

## Injection Lanes

The system chooses the best injection method automatically:

1. **Pull** - Partner calls `/v1/context` (best, cleanest)
2. **Merge** - Intercept outgoing request and splice context (transparent)
3. **Prefill** - Insert visible preamble into input (fallback)

All lanes show indicators:
- Pull/Merge: "Context applied · Undo" ribbon (5s)
- Prefill: "Added · Undo" chip inline

## API

### Local Provider

**HTTP:** `http://127.0.0.1:21955/v1/context`
**Unix Socket:** `~/Library/Application Support/MemoryLayer/context.sock`

#### POST /v1/context

Request:
```json
{
  "topic_hint": "optional string",
  "intent": "optional string",
  "budget_tokens": 220,
  "scopes": ["assistant", "file", "page"],
  "thread_key": "optional string",
  "last_capsule_id": "optional cap_..."
}
```

Response:
```json
{
  "capsule_id": "cap_01HQXY...",
  "preamble_text": "Context: ...",
  "messages": [{"role": "system", "content": "..."}],
  "provenance": [...],
  "delta_of": "optional cap_...",
  "ttl_sec": 600
}
```

#### POST /v1/undo

Request:
```json
{
  "capsule_id": "cap_01HQXY...",
  "thread_key": "thr_..."
}
```

### Indexing Service – Agentic Memory Base

The indexing service (port `21954`) now maintains an agentic memory base inspired by [A-mem](https://github.com/agiresearch/A-mem). In addition to standard hybrid search, it exposes:

| Endpoint | Description |
|----------|-------------|
| `GET /agentic/recent?limit=12` | Latest agentic memories with tags, keywords, and link counts |
| `GET /agentic/search?q=query&limit=8` | Hybrid search across agentic metadata and content |
| `GET /agentic/{mem_id}` | Full agentic record with links, evolution history, and retrieval stats |
| `GET /agentic/graph?limit=200` | Knowledge graph export (nodes + weighted edges) compatible with A-mem protocol |

Each ingested memory is automatically enriched with:
- Locally derived keywords, tags, and context
- Topic-based bidirectional links between related memories
- Retrieval counts plus last-accessed timestamps
- Evolution history entries seeded from ingestion events
- Knowledge graph projection (nodes + edges) for visualization or downstream agents

> The **Knowledge Graph** tab in the Mac Console renders this export with a bundled WebKit view using three.js (loaded from the unpkg CDN). If the indexing service or network is unavailable, a local sample graph is displayed so the UI stays responsive.

These endpoints give clients a richer knowledge graph to drive advanced context selection, dashboards, or visualizations.

## Privacy & Security

- **Local-only by default** - All data on device
- **Text capture only** - No screenshots, ever
- **Redaction** - Emails, API keys, secrets filtered
- **Per-app scopes** - Control what each app can see
- **Rate limiting** - Prevent abuse
- **Audit log** - Track all context requests
- **Code-sign verification** - Pull callers must be verified

### Data Storage

- **Database:** SQLite with FTS5 for full-text search
- **Location:** `~/Library/Application Support/MemoryLayer/memory.db`
- **Encryption:** Optional E2EE for sync (stub in MVP)
- **Retention:** Configurable TTL per memory type

## Performance Targets

- **Compose p95:** < 150ms for 220 tokens
- **Search p95:** < 100ms for top 10 results
- **Memory extraction:** < 2s for average turn
- **UI responsiveness:** < 16ms frame time

## Testing

### Unit Tests

```bash
# Rust
cd core/composer && cargo test
cd core/indexing && cargo test
cd core/ingestion && cargo test

# Swift
xcodebuild test -project apps/mac-daemon/MemoryLayer.xcodeproj -scheme MemoryLayer

# TypeScript
cd apps/chrome-ext && npm test
cd apps/vscode-ext && npm test
```

### E2E Tests

```bash
# Browser extensions (Playwright)
cd tests/e2e && npm test

# macOS app (XCUITest)
xcodebuild test -project apps/mac-daemon/MemoryLayer.xcodeproj -scheme MemoryLayerUITests
```

## Roadmap

- [x] Core Rust services (ingestion, indexing, composer)
- [x] macOS menu-bar app with local provider
- [x] Chrome MV3 extension (all three lanes)
- [x] VSCode extension
- [x] Memory Search panel (⌘⌥K)
- [x] Comprehensive test suite
- [ ] Safari extension
- [ ] Firefox extension
- [ ] E2EE sync between devices
- [ ] Advanced memory types (project, person, codebase)
- [ ] Timeline visualization
- [ ] iOS companion app

## Contributing

This is a personal project, but suggestions and bug reports are welcome via GitHub Issues.

## License

MIT License - see LICENSE file for details

## Acknowledgments

Built with:
- Rust (tokio, serde, rusqlite, tantivy)
- Swift/SwiftUI/AppKit
- TypeScript
- sentence-transformers (for embeddings)

---

**Remember: Reads text. Never screenshots.**
