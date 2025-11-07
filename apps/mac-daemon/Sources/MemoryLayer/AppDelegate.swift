import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItemController: StatusItemController?
    var providerClient: ProviderClient?
    var accessibilityWatcher: AccessibilityWatcher?
    var memoryConsoleController: MemoryConsoleWindowController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        print("Memory Layer starting...")

        // Initialize provider client
        providerClient = ProviderClient()

        // Initialize accessibility watcher
        accessibilityWatcher = AccessibilityWatcher(providerClient: providerClient!)
        accessibilityWatcher?.start()

        // Initialize status bar item
        statusItemController = StatusItemController(providerClient: providerClient!, accessibilityWatcher: accessibilityWatcher!)

        // Present the Memory Console on launch so users see the main UI
        memoryConsoleController = MemoryConsoleWindowController(accessibilityWatcher: accessibilityWatcher)
        memoryConsoleController?.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)

        print("Memory Layer ready")
        print("Provider endpoint: http://127.0.0.1:21955/v1/context")
        print("Keyboard shortcut: ⌘⌥K to open search")
        print("Look for the brain icon in your menu bar (top-right of screen)")

        // Ensure the app stays in the foreground briefly so icon appears
    }

    func applicationWillTerminate(_ notification: Notification) {
        print("Memory Layer shutting down...")
    }
}
