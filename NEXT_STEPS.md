# Personal Memory Layer - Next Steps Guide

This document provides a clear roadmap for completing the remaining 40% of the project.

---

## 🎯 Current Status: 60% Complete

### ✅ What's Done (Backend Infrastructure - 60%)

1. **Core Services** ✓
   - Ingestion service (port 21953)
   - Indexing service (port 21954)
   - Composer service (port 21955)
   - 18 passing tests

2. **Type System** ✓
   - JSON schemas
   - Rust/TypeScript/Swift types
   - ULID ID generation

3. **Build System** ✓
   - Makefile with 30+ targets
   - Development scripts
   - Git repository

### ⏳ What's Next (Client Applications - 40%)

1. **macOS App** (Highest Priority)
2. **Chrome Extension**
3. **VSCode Extension**
4. **E2E Tests & Demo**

---

## 📱 Task 1: macOS Menu-Bar App (Priority 1)

**Estimated Time:** 3-4 hours
**Complexity:** High (requires Accessibility API, AppKit, and Swift)

### Why Start Here?
- Ties the whole system together
- Provides Accessibility text capture (core feature)
- Memory Search panel is the killer feature
- Once working, demonstrates full system capability

### Implementation Plan

#### Step 1.1: Create Xcode Project (30 minutes)
```bash
cd ~/memory-layer/apps/mac-daemon

# Option A: Use Xcode GUI
open -a Xcode
# File > New > Project > macOS > App
# Name: MemoryLayer
# Interface: AppKit (not SwiftUI for menu bar app)
# Language: Swift

# Option B: Use swift package init
swift package init --type executable --name MemoryLayer
```

**Key files to create:**
- `AppDelegate.swift` - Main app controller
- `StatusItemController.swift` - Menu bar icon management
- `ProviderClient.swift` - HTTP client for composer service
- `AccessibilityWatcher.swift` - Text capture from other apps
- `SearchPanelController.swift` - Memory search UI
- `PreferencesWindowController.swift` - Settings UI

#### Step 1.2: Menu Bar Status Item (1 hour)

**StatusItemController.swift:**
```swift
import Cocoa

class StatusItemController {
    private var statusItem: NSStatusItem!
    private var menu: NSMenu!

    init() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem.button {
            button.image = NSImage(systemSymbolName: "brain", accessibilityDescription: "Memory Layer")
            button.action = #selector(statusItemClicked)
            button.target = self
        }

        setupMenu()
    }

    private func setupMenu() {
        menu = NSMenu()

        menu.addItem(NSMenuItem(title: "Search Memories... (⌘⌥K)",
                                action: #selector(showSearch),
                                keyEquivalent: "k"))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Preferences...",
                                action: #selector(showPreferences),
                                keyEquivalent: ","))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Pause for 1 Hour",
                                action: #selector(pauseCapture),
                                keyEquivalent: ""))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit Memory Layer",
                                action: #selector(NSApplication.terminate(_:)),
                                keyEquivalent: "q"))

        statusItem.menu = menu
    }

    @objc func statusItemClicked() {
        // Show quick stats or status
    }

    @objc func showSearch() {
        // Open search panel
    }

    @objc func showPreferences() {
        // Open preferences window
    }

    @objc func pauseCapture() {
        // Pause for 1 hour
    }
}
```

#### Step 1.3: Provider Client (1 hour)

**ProviderClient.swift:**
```swift
import Foundation

struct ContextRequest: Codable {
    let topicHint: String?
    let intent: String?
    let budgetTokens: Int
    let scopes: [String]
    let threadKey: String?
    let lastCapsuleId: String?

    enum CodingKeys: String, CodingKey {
        case topicHint = "topic_hint"
        case intent
        case budgetTokens = "budget_tokens"
        case scopes
        case threadKey = "thread_key"
        case lastCapsuleId = "last_capsule_id"
    }
}

struct ContextCapsule: Codable {
    let capsuleId: String
    let preambleText: String
    let messages: [Message]
    let provenance: [ProvenanceItem]
    let deltaOf: String?
    let ttlSec: Int
    let tokenCount: Int?
    let style: String?

    // ... CodingKeys
}

class ProviderClient {
    private let baseURL = URL(string: "http://127.0.0.1:21955")!
    private let session = URLSession.shared

    func getContext(request: ContextRequest) async throws -> ContextCapsule {
        let url = baseURL.appendingPathComponent("/v1/context")

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = "POST"
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        urlRequest.httpBody = try JSONEncoder().encode(request)

        let (data, response) = try await session.data(for: urlRequest)

        guard let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200 else {
            throw NSError(domain: "ProviderClient", code: -1, userInfo: nil)
        }

        return try JSONDecoder().decode(ContextCapsule.self, from: data)
    }

    func undo(capsuleId: String, threadKey: String) async throws {
        let url = baseURL.appendingPathComponent("/v1/undo")

        let request = ["capsule_id": capsuleId, "thread_key": threadKey]

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = "POST"
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        urlRequest.httpBody = try JSONSerialization.data(withJSONObject: request)

        let (_, response) = try await session.data(for: urlRequest)

        guard let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200 else {
            throw NSError(domain: "ProviderClient", code: -1, userInfo: nil)
        }
    }
}
```

#### Step 1.4: Accessibility Watcher (1-2 hours)

**AccessibilityWatcher.swift:**
```swift
import Cocoa
import ApplicationServices

class AccessibilityWatcher {
    private var timer: Timer?
    private var lastText: String = ""
    private let whitelistedBundleIDs = [
        "com.anthropic.claude",
        "com.openai.chat",
        "com.microsoft.VSCode"
    ]

    func start() {
        // Check for Accessibility permissions
        let options: NSDictionary = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true]
        let accessEnabled = AXIsProcessTrustedWithOptions(options)

        if !accessEnabled {
            print("Accessibility access not granted")
            return
        }

        // Poll every 2 seconds
        timer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            self?.captureText()
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    private func captureText() {
        guard let app = NSWorkspace.shared.frontmostApplication,
              let bundleID = app.bundleIdentifier,
              whitelistedBundleIDs.contains(bundleID) else {
            return
        }

        // Get AX element for focused window
        let axApp = AXUIElementCreateApplication(app.processIdentifier)

        var focusedElement: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(axApp,
                                                    kAXFocusedUIElementAttribute as CFString,
                                                    &focusedElement)

        guard result == .success,
              let element = focusedElement else {
            return
        }

        // Try to get text value
        var value: CFTypeRef?
        AXUIElementCopyAttributeValue(element as! AXUIElement,
                                     kAXValueAttribute as CFString,
                                     &value)

        if let text = value as? String, text != lastText {
            print("Text changed: \(text.prefix(50))...")
            lastText = text

            // TODO: Send to ingestion service
            sendToIngestion(text: text, bundleID: bundleID)
        }
    }

    private func sendToIngestion(text: String, bundleID: String) {
        // Send turn to http://127.0.0.1:21953/ingest/turn
        // Implementation here
    }
}
```

#### Step 1.5: Search Panel (1-2 hours)

**SearchPanelController.swift:**
```swift
import Cocoa

class SearchPanelController: NSWindowController {
    private var searchField: NSSearchField!
    private var tableView: NSTableView!
    private var results: [SearchResult] = []

    override func windowDidLoad() {
        super.windowDidLoad()

        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400),
                           styleMask: [.titled, .closable, .nonactivatingPanel],
                           backing: .buffered,
                           defer: false)

        panel.level = .floating
        panel.isMovableByWindowBackground = true
        panel.title = "Memory Search"

        setupUI(in: panel.contentView!)

        self.window = panel
    }

    private func setupUI(in view: NSView) {
        // Search field at top
        searchField = NSSearchField(frame: NSRect(x: 20, y: 360, width: 560, height: 30))
        searchField.placeholderString = "Search memories, files, conversations..."
        searchField.target = self
        searchField.action = #selector(searchChanged)
        view.addSubview(searchField)

        // Table view for results
        let scrollView = NSScrollView(frame: NSRect(x: 20, y: 20, width: 560, height: 330))
        tableView = NSTableView(frame: scrollView.bounds)

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("result"))
        column.title = "Results"
        column.width = 540
        tableView.addTableColumn(column)

        tableView.dataSource = self
        tableView.delegate = self

        scrollView.documentView = tableView
        view.addSubview(scrollView)
    }

    @objc func searchChanged() {
        let query = searchField.stringValue
        guard !query.isEmpty else { return }

        // Query http://127.0.0.1:21954/search?q=...
        Task {
            do {
                results = try await search(query: query)
                tableView.reloadData()
            } catch {
                print("Search failed: \(error)")
            }
        }
    }

    func search(query: String) async throws -> [SearchResult] {
        // Implementation here
        return []
    }
}

extension SearchPanelController: NSTableViewDataSource, NSTableViewDelegate {
    func numberOfRows(in tableView: NSTableView) -> Int {
        return results.count
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let result = results[row]
        let cell = NSTableCellView()

        let textField = NSTextField(frame: cell.bounds)
        textField.isEditable = false
        textField.isBordered = false
        textField.backgroundColor = .clear
        textField.stringValue = result.text

        cell.addSubview(textField)
        return cell
    }
}

struct SearchResult {
    let text: String
    let score: Double
}
```

### Required Entitlements

**MemoryLayer.entitlements:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <false/>
    <key>com.apple.security.network.client</key>
    <true/>
</dict>
</plist>
```

**Info.plist additions:**
```xml
<key>NSAppleEventsUsageDescription</key>
<string>Memory Layer needs to read text from other applications to provide contextual assistance.</string>
<key>NSAccessibilityUsageDescription</key>
<string>Memory Layer reads text from applications to build your personal context. No screenshots are ever taken.</string>
<key>LSUIElement</key>
<true/>
```

---

## 🌐 Task 2: Chrome MV3 Extension (Priority 2)

**Estimated Time:** 2-3 hours
**Complexity:** Medium

### Quick Implementation Guide

#### manifest.json
```json
{
  "manifest_version": 3,
  "name": "Memory Layer",
  "version": "0.1.0",
  "description": "Reads text. Never screenshots.",

  "permissions": [
    "storage",
    "nativeMessaging"
  ],

  "host_permissions": [
    "http://127.0.0.1:21955/*"
  ],

  "background": {
    "service_worker": "background.js"
  },

  "content_scripts": [
    {
      "matches": ["https://claude.ai/*"],
      "js": ["content-claude.js"],
      "run_at": "document_end"
    },
    {
      "matches": ["https://chatgpt.com/*"],
      "js": ["content-chatgpt.js"],
      "run_at": "document_end"
    }
  ],

  "icons": {
    "16": "icons/icon-16.png",
    "48": "icons/icon-48.png",
    "128": "icons/icon-128.png"
  }
}
```

#### Key Implementation Files

**src/content/injector.ts** - Core injection logic:
```typescript
// Prefill lane: Insert visible text into input
async function prefillLane(preamble: string, inputElement: HTMLElement) {
  // Insert text + show "Added · Undo" chip
}

// Merge lane: Intercept and modify fetch
function mergeLane(originalFetch: typeof fetch) {
  return async (url: RequestInfo, init?: RequestInit) => {
    // Call provider, splice system message
    // Show "Context applied · Undo" ribbon
  }
}
```

**src/background/service-worker.ts** - Background logic:
```typescript
// Handle messages from content scripts
// Manage native messaging
// Rate limiting
```

---

## 💻 Task 3: VSCode Extension (Priority 3)

**Estimated Time:** 1-2 hours
**Complexity:** Low-Medium

### Quick Implementation Guide

**package.json:**
```json
{
  "name": "memory-layer",
  "displayName": "Memory Layer",
  "description": "Reads text. Never screenshots.",
  "version": "0.1.0",
  "engines": {
    "vscode": "^1.90.0"
  },
  "activationEvents": [
    "onStartupFinished"
  ],
  "main": "./out/extension.js",
  "contributes": {
    "commands": [
      {
        "command": "memoryLayer.insertContext",
        "title": "Memory Layer: Insert Context"
      }
    ],
    "keybindings": [
      {
        "command": "memoryLayer.insertContext",
        "key": "cmd+alt+i"
      }
    ]
  }
}
```

**src/extension.ts:**
```typescript
export function activate(context: vscode.ExtensionContext) {
  // Register command
  let disposable = vscode.commands.registerCommand(
    'memoryLayer.insertContext',
    async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;

      // Get context from provider
      const capsule = await getContext({
        budget_tokens: 220,
        scopes: ['file']
      });

      // Insert as comment or in chat
      editor.edit(editBuilder => {
        editBuilder.insert(
          editor.selection.start,
          `// ${capsule.preamble_text}\n`
        );
      });
    }
  );

  context.subscriptions.push(disposable);
}
```

---

## 🧪 Task 4: E2E Testing (Priority 4)

**Estimated Time:** 2-3 hours
**Complexity:** Medium

### Playwright for Browser Extensions

**tests/e2e/chrome-extension.spec.ts:**
```typescript
import { test, expect } from '@playwright/test';

test('context injection on Claude.ai', async ({ page }) => {
  // Load extension
  // Navigate to claude.ai
  // Trigger context request
  // Verify ribbon appears
  // Test undo functionality
});
```

---

## 📊 Progress Tracking

| Component | Status | Time Estimate | Priority |
|-----------|--------|---------------|----------|
| Core Services | ✅ Complete | - | - |
| macOS App | ⏳ 20% | 3-4h | 1 |
| Chrome Extension | ❌ Not Started | 2-3h | 2 |
| VSCode Extension | ❌ Not Started | 1-2h | 3 |
| E2E Tests | ❌ Not Started | 2-3h | 4 |
| Demo Video | ❌ Not Started | 1h | 5 |

**Total Remaining:** ~10-14 hours

---

## 🎯 Completion Checklist

### Must Have (MVP)
- [ ] macOS menu-bar app running
- [ ] Accessibility text capture working
- [ ] Memory Search panel functional (⌘⌥K)
- [ ] Chrome extension with Prefill lane working
- [ ] VSCode command working
- [ ] All services start with `make run`

### Nice to Have
- [ ] All three injection lanes (Pull/Merge/Prefill)
- [ ] Native messaging bridge
- [ ] Undo functionality fully wired
- [ ] E2E test coverage
- [ ] Demo video showing full flow

### Polish
- [ ] App icons for all platforms
- [ ] Error handling and user feedback
- [ ] Logging and debugging tools
- [ ] Performance optimization
- [ ] Documentation updates

---

## 🚀 Recommended Order

1. **Week 1: macOS App** (Focus here first)
   - Day 1-2: Menu bar + provider client
   - Day 3: Accessibility watcher
   - Day 4: Search panel
   - Day 5: Testing and polish

2. **Week 2: Extensions**
   - Day 1-2: Chrome extension (Prefill lane)
   - Day 3: VSCode extension
   - Day 4-5: Testing and integration

3. **Week 3: Polish & Ship**
   - Day 1-2: E2E tests
   - Day 3: Bug fixes
   - Day 4: Demo video
   - Day 5: Documentation and release

---

## 💡 Tips for Success

1. **Start Services First**
   ```bash
   cd ~/memory-layer
   make run  # Keep this running in background
   ```

2. **Test Incrementally**
   - Don't wait until everything is done
   - Test each component as you build it
   - Use curl to test APIs

3. **Use the Types**
   - All types are already defined in `core/schemas/`
   - Copy the Swift types to your macOS app
   - Copy the TS types to extensions

4. **Follow the Patterns**
   - Look at how the Rust services are structured
   - Apply similar patterns to Swift and TS code
   - Use async/await everywhere

5. **Ask for Help**
   - If stuck on Accessibility API, check Apple docs
   - If stuck on Chrome extension, check MV3 migration guide
   - The Rust code is well-tested reference

---

**You've built a solid foundation. Now bring it to life with the client apps!**
