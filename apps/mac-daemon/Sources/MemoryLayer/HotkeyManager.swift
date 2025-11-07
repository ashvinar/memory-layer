import Cocoa
import Carbon

final class HotkeyManager {
    private weak var accessibilityWatcher: AccessibilityWatcher?
    private var eventHandler: EventHandlerRef?
    private var hotKeyRef: EventHotKeyRef?

    private let hotkeyID = EventHotKeyID(signature: OSType(0x4D4C4D4C), id: 1) // 'MLML'

    init(accessibilityWatcher: AccessibilityWatcher) {
        self.accessibilityWatcher = accessibilityWatcher
    }

    func register() {
        // Register global hotkey: Cmd+Shift+M
        var hotKeyID = hotkeyID
        var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))

        // Install event handler
        InstallEventHandler(GetApplicationEventTarget(), { (nextHandler, theEvent, userData) -> OSStatus in
            guard let userData = userData else { return OSStatus(eventNotHandledErr) }
            let manager = Unmanaged<HotkeyManager>.fromOpaque(userData).takeUnretainedValue()
            manager.handleHotkey()
            return noErr
        }, 1, &eventType, Unmanaged.passUnretained(self).toOpaque(), &eventHandler)

        // Register hotkey: Cmd (cmdKey) + Shift (shiftKey) + M (keyCode 46)
        let modifiers = UInt32(cmdKey | shiftKey)
        let keyCode = UInt32(46) // M key

        RegisterEventHotKey(keyCode, modifiers, hotKeyID, GetApplicationEventTarget(), 0, &hotKeyRef)

        print("HotkeyManager: Registered Cmd+Shift+M to add frontmost app to monitoring")
    }

    func unregister() {
        if let hotKeyRef = hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
            self.hotKeyRef = nil
        }

        if let eventHandler = eventHandler {
            RemoveEventHandler(eventHandler)
            self.eventHandler = nil
        }

        print("HotkeyManager: Unregistered hotkey")
    }

    private func handleHotkey() {
        DispatchQueue.main.async { [weak self] in
            self?.showAddAppDialog()
        }
    }

    private func showAddAppDialog() {
        guard let frontmostApp = NSWorkspace.shared.frontmostApplication,
              let bundleID = frontmostApp.bundleIdentifier else {
            showAlert(title: "No Application", message: "Could not detect the frontmost application.")
            return
        }

        let appName = frontmostApp.localizedName ?? bundleID

        // Check if already monitoring
        if accessibilityWatcher?.isMonitoring(bundleId: bundleID) == true {
            showAlert(
                title: "Already Monitoring",
                message: "'\(appName)' is already being monitored by Memory Layer."
            )
            return
        }

        // Show confirmation dialog
        let alert = NSAlert()
        alert.messageText = "Add '\(appName)' to Memory Layer?"
        alert.informativeText = """
        This will start capturing text from '\(appName)' and sending it to Memory Layer.

        Bundle ID: \(bundleID)

        You can remove it later from the Manage Apps menu.
        """
        alert.alertStyle = .informational
        alert.addButton(withTitle: "Add to Monitoring")
        alert.addButton(withTitle: "Cancel")

        let response = alert.runModal()

        if response == .alertFirstButtonReturn {
            addAppToMonitoring(bundleId: bundleID, appName: appName)
        }
    }

    private func addAppToMonitoring(bundleId: String, appName: String) {
        guard let watcher = accessibilityWatcher else { return }

        var currentApps = watcher.currentWhitelistedApps()

        // Add the new bundle ID if not already present
        if !currentApps.contains(bundleId) {
            currentApps.append(bundleId)
            watcher.updateWhitelistedApps(bundleIds: currentApps)

            showAlert(
                title: "App Added",
                message: "'\(appName)' has been added to Memory Layer monitoring.",
                style: .informational
            )

            print("HotkeyManager: Added '\(appName)' (\(bundleId)) to monitoring")
        }
    }

    private func showAlert(title: String, message: String, style: NSAlert.Style = .warning) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = style
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    deinit {
        unregister()
    }
}
