import Cocoa

class StatusItemController: NSObject {
    private var statusItem: NSStatusItem!
    private var menu: NSMenu!
    private let providerClient: ProviderClient
    private let accessibilityWatcher: AccessibilityWatcher
    private var searchPanelController: SearchPanelController?
    private var preferencesController: PreferencesWindowController?
    private var appManagerController: AppManagerWindowController?
    private var memoryConsoleController: MemoryConsoleWindowController?
    private var statusMenuItem: NSMenuItem?
    private var isPaused: Bool = false

    private var isHealthy: Bool = false {
        didSet {
            updateStatusIcon()
        }
    }

    init(providerClient: ProviderClient, accessibilityWatcher: AccessibilityWatcher) {
        self.providerClient = providerClient
        self.accessibilityWatcher = accessibilityWatcher
        super.init()

        setupStatusItem()
        setupMenu()
        startHealthCheck()
        registerGlobalHotkeys()
    }

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem.button {
            // Try brain icon first, fallback to "ML" text
            if let image = NSImage(systemSymbolName: "brain.head.profile", accessibilityDescription: "Memory Layer") {
                image.isTemplate = true
                button.image = image
                print("Menu bar icon set: brain.head.profile")
            } else {
                // Fallback to text if icon doesn't load
                button.title = "ML"
                button.font = NSFont.boldSystemFont(ofSize: 12)
                print("Menu bar icon set: ML text")
            }

            button.action = #selector(statusItemClicked)
            button.target = self
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

        updateStatusIcon()
    }

    private func setupMenu() {
        menu = NSMenu()

        // Test Context Generation
        let testItem = NSMenuItem(
            title: "Test Context Generation",
            action: #selector(testContextGeneration),
            keyEquivalent: ""
        )
        testItem.target = self
        menu.addItem(testItem)

        menu.addItem(NSMenuItem.separator())

        // App Manager
        let appManagerItem = NSMenuItem(
            title: "Manage Apps...",
            action: #selector(showAppManager),
            keyEquivalent: "a"
        )
        appManagerItem.keyEquivalentModifierMask = [.command, .option]
        appManagerItem.target = self
        menu.addItem(appManagerItem)

        let consoleItem = NSMenuItem(
            title: "Open Memory Console…",
            action: #selector(showMemoryConsole),
            keyEquivalent: "m"
        )
        consoleItem.keyEquivalentModifierMask = [.command, .option]
        consoleItem.target = self
        menu.addItem(consoleItem)

        // Search Memories
        let searchItem = NSMenuItem(
            title: "Search Memories... ⌘⌥K",
            action: #selector(showSearchPanel),
            keyEquivalent: "k"
        )
        searchItem.keyEquivalentModifierMask = [.command, .option]
        searchItem.target = self
        menu.addItem(searchItem)

        menu.addItem(NSMenuItem.separator())

        // Pause/Resume
        let pauseItem = NSMenuItem(
            title: isPaused ? "Resume Capture" : "Pause for 1 Hour",
            action: #selector(togglePause),
            keyEquivalent: "p"
        )
        pauseItem.keyEquivalentModifierMask = [.command, .option]
        pauseItem.target = self
        menu.addItem(pauseItem)

        menu.addItem(NSMenuItem.separator())

        // Preferences
        let prefsItem = NSMenuItem(
            title: "Preferences...",
            action: #selector(showPreferences),
            keyEquivalent: ","
        )
        prefsItem.target = self
        menu.addItem(prefsItem)

        menu.addItem(NSMenuItem.separator())

        // Status
        let statusItem = NSMenuItem(
            title: isHealthy ? "Service: ✓ Running" : "Service: ✗ Offline",
            action: nil,
            keyEquivalent: ""
        )
        statusItem.isEnabled = false
        menu.addItem(statusItem)
        statusMenuItem = statusItem

        menu.addItem(NSMenuItem.separator())

        // Quit
        let quitItem = NSMenuItem(
            title: "Quit Memory Layer",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        )
        menu.addItem(quitItem)

        statusItem.menu = menu
    }

    private func updateStatusIcon() {
        guard let button = statusItem?.button else { return }

        // Update color based on health
        if isHealthy {
            button.contentTintColor = nil // Default color
        } else {
            button.contentTintColor = .systemRed
        }

        // Update menu status item if menu is initialized
        statusMenuItem?.title = isHealthy ? "Service: ✓ Running" : "Service: ✗ Offline"
    }

    private func startHealthCheck() {
        // Check health every 30 seconds
        Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true) { [weak self] _ in
            Task {
                await self?.checkHealth()
            }
        }

        // Initial check
        Task {
            await checkHealth()
        }
    }

    private func checkHealth() async {
        isHealthy = await providerClient.checkHealth()
    }

    @objc private func statusItemClicked() {
        // Show menu on any click
    }

    @objc private func testContextGeneration() {
        Task {
            do {
                let request = ContextRequest(
                    topicHint: "Memory Layer Testing",
                    budgetTokens: 220,
                    scopes: ["assistant"]
                )

                let capsule = try await providerClient.getContext(request: request)

                // Show alert with result
                DispatchQueue.main.async {
                    let alert = NSAlert()
                    alert.messageText = "Context Generated Successfully"
                    alert.informativeText = """
                    Capsule ID: \(capsule.capsuleId)
                    Style: \(capsule.styleDisplay)
                    Tokens: \(capsule.tokenCountDisplay)

                    Preamble:
                    \(capsule.preambleText.prefix(200))...
                    """
                    alert.alertStyle = .informational
                    alert.addButton(withTitle: "OK")
                    alert.runModal()
                }
            } catch {
                DispatchQueue.main.async {
                    let alert = NSAlert()
                    alert.messageText = "Failed to Generate Context"
                    alert.informativeText = error.localizedDescription
                    alert.alertStyle = .warning
                    alert.addButton(withTitle: "OK")
                    alert.runModal()
                }
            }
        }
    }

    @objc private func showAppManager() {
        if appManagerController == nil {
            appManagerController = AppManagerWindowController(accessibilityWatcher: accessibilityWatcher)
        }
        appManagerController?.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc private func showMemoryConsole() {
        if memoryConsoleController == nil {
            memoryConsoleController = MemoryConsoleWindowController(accessibilityWatcher: accessibilityWatcher)
        }
        memoryConsoleController?.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc private func showSearchPanel() {
        if searchPanelController == nil {
            searchPanelController = SearchPanelController(providerClient: providerClient)
        }
        searchPanelController?.showWindow(nil)
    }

    @objc private func showPreferences() {
        if preferencesController == nil {
            preferencesController = PreferencesWindowController()
        }
        preferencesController?.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc private func togglePause() {
        if isPaused {
            // Resume
            accessibilityWatcher.start()
            isPaused = false
        } else {
            // Pause for 1 hour
            accessibilityWatcher.pause(for: 3600)  // 1 hour in seconds
            isPaused = true

            // Schedule to reset pause state after 1 hour
            Timer.scheduledTimer(withTimeInterval: 3600, repeats: false) { [weak self] _ in
                self?.isPaused = false
                self?.setupMenu()  // Refresh menu to update pause item title
            }
        }

        setupMenu()  // Refresh menu to update pause item title
    }

    private func registerGlobalHotkeys() {
        // Register global keyboard shortcuts using NSEvent
        NSEvent.addGlobalMonitorForEvents(matching: .keyDown) { [weak self] event in
            // Check for ⌘⌥K (Command+Option+K)
            if event.modifierFlags.contains([.command, .option]) && event.keyCode == 40 { // 40 is K
                self?.showSearchPanel()
            }
        }

        // Also monitor local events (when app has focus)
        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.modifierFlags.contains([.command, .option]) && event.keyCode == 40 {
                self?.showSearchPanel()
                return nil // Consume the event
            }
            return event
        }

        print("Global hotkeys registered: ⌘⌥K for search")
    }
}
