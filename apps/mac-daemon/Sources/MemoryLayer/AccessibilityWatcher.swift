import Cocoa
import ApplicationServices
import CoreFoundation

class AccessibilityWatcher: NSObject {
    private var timer: Timer?
    private var lastTextByApp: [String: String] = [:]
    private let providerClient: ProviderClient
    private let ingestionClient = IngestionClient()
    private lazy var claudeAdapter = ClaudeDesktopAdapter(ingestionClient: ingestionClient)
    private var isRunning = false
    private var allowedBundleIDs: Set<String>
    private var expandedBundleIDs: Set<String> {
        BundleIdentifierResolver.expand(allowedBundleIDs)
    }
    private let defaults: UserDefaults
    private static let allowedAppsKey = "MemoryLayerAllowedApps"

    static let defaultBundleIdentifiers: [String] = [
        "com.anthropic.claude-desktop",
        "com.openai.ChatGPT",
        "com.microsoft.VSCode",
        "com.apple.Safari",
        "com.google.Chrome",
        "com.apple.mail",
        "com.apple.Notes",
        "com.apple.dt.Xcode",
        "com.jetbrains.intellij",
        "com.sublimetext.4",
        "org.vim.MacVim",
        "com.cursor.IDE",
        "com.apple.Terminal"
    ]

    init(providerClient: ProviderClient) {
        self.providerClient = providerClient
        self.defaults = UserDefaults.standard
        let stored = defaults.array(forKey: Self.allowedAppsKey) as? [String]
        self.allowedBundleIDs = Set(stored ?? AccessibilityWatcher.defaultBundleIdentifiers)
        super.init()
        persistAllowedBundleIDsIfNeeded()
    }

    func start() {
        guard !isRunning else { return }

        // Check for Accessibility permissions
        let options: NSDictionary = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true]
        let accessEnabled = AXIsProcessTrustedWithOptions(options)

        if !accessEnabled {
            print("Accessibility access not granted - will prompt user")
            showAccessibilityAlert()
            return
        }

        print("AccessibilityWatcher started")
        isRunning = true

        // Poll every 3 seconds for text changes
        timer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { [weak self] _ in
            self?.captureText()
        }

        let claudeEnabled = BundleIdentifierResolver.contains(
            allowedBundleIDs,
            candidate: "com.anthropic.claude-desktop"
        )
        claudeAdapter.setEnabled(claudeEnabled)
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        isRunning = false
        claudeAdapter.stop()
        print("AccessibilityWatcher stopped")
    }

    func pause(for duration: TimeInterval) {
        stop()
        Timer.scheduledTimer(withTimeInterval: duration, repeats: false) { [weak self] _ in
            self?.start()
        }
    }

    private func captureText() {
        guard let app = NSWorkspace.shared.frontmostApplication,
              let bundleID = app.bundleIdentifier,
              expandedBundleIDs.contains(bundleID) else {
            return
        }

        if BundleIdentifierResolver.contains(
            Set(arrayLiteral: "com.anthropic.claude-desktop"),
            candidate: bundleID
        ) {
            return
        }

        // Get AX element for focused window
        let axApp = AXUIElementCreateApplication(app.processIdentifier)

        var focusedElement: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(
            axApp,
            kAXFocusedUIElementAttribute as CFString,
            &focusedElement
        )

        guard result == .success,
              let element = focusedElement else {
            return
        }

        // Try to get text value from different attributes
        let text = extractText(from: element as! AXUIElement)

        if let text = text, !text.isEmpty {
            // Check if text has changed since last capture
            let lastText = lastTextByApp[bundleID] ?? ""

            if text != lastText && text.count > 10 {  // Only capture meaningful changes
                lastTextByApp[bundleID] = text

                ingestionClient.ingestUserTurn(
                    text: text,
                    bundleId: bundleID,
                    appName: app.localizedName ?? bundleID
                )
            }
        }
    }

    private func extractText(from element: AXUIElement) -> String? {
        var visited: Set<Int> = []
        let segments = collectText(from: element, visited: &visited, depth: 0)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        guard !segments.isEmpty else {
            return nil
        }

        var deduped: [String] = []
        var seen = Set<String>()
        for segment in segments {
            if seen.insert(segment).inserted {
                deduped.append(segment)
            }
        }

        return deduped.joined(separator: "\n")
    }

    private func collectText(from element: AXUIElement, visited: inout Set<Int>, depth: Int) -> [String] {
        guard depth < 32 else { return [] }

        let elementHash = Int(CFHash(element))
        if visited.contains(elementHash) {
            return []
        }
        visited.insert(elementHash)

        var segments: [String] = []
        let attributes: [CFString] = [
            kAXValueAttribute as CFString,
            kAXTitleAttribute as CFString,
            kAXDescriptionAttribute as CFString,
            kAXSelectedTextAttribute as CFString,
            kAXPlaceholderValueAttribute as CFString
        ]

        for attribute in attributes {
            if let text = resolveTextAttribute(on: element, attribute: attribute), !text.isEmpty {
                segments.append(text)
            }
        }

        if let children = copyChildren(of: element) {
            for child in children {
                segments.append(contentsOf: collectText(from: child, visited: &visited, depth: depth + 1))
            }
        }

        return segments
    }

    private func resolveTextAttribute(on element: AXUIElement, attribute: CFString) -> String? {
        var value: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(element, attribute, &value)
        guard result == .success, let value else { return nil }

        if let stringValue = value as? String {
            return stringValue
        }

        if let attrString = value as? NSAttributedString {
            return attrString.string
        }

        if CFGetTypeID(value) == CFAttributedStringGetTypeID() {
            let cfAttrString = unsafeBitCast(value, to: CFAttributedString.self)
            return (cfAttrString as NSAttributedString).string
        }

        if let array = value as? [String] {
            return array.joined(separator: "\n")
        }

        return nil
    }

    private func copyChildren(of element: AXUIElement) -> [AXUIElement]? {
        var children: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &children)

        if result == .success, let childArray = children as? [AXUIElement] {
            return childArray
        }

        return nil
    }


    private func showAccessibilityAlert() {
        DispatchQueue.main.async {
            let alert = NSAlert()
            alert.messageText = "Accessibility Access Required"
            alert.informativeText = """
            Memory Layer needs accessibility permissions to read text from other applications.

            Please grant access in System Settings > Privacy & Security > Accessibility.

            Note: Memory Layer reads text only, never takes screenshots.
            """
            alert.alertStyle = .warning
            alert.addButton(withTitle: "Open System Settings")
            alert.addButton(withTitle: "Cancel")

            let response = alert.runModal()
            if response == .alertFirstButtonReturn {
                // Open System Preferences to Accessibility
                NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!)
            }
        }
    }

    func currentWhitelistedApps() -> [String] {
        return Array(allowedBundleIDs).sorted()
    }

    func updateWhitelistedApps(bundleIds: [String]) {
        let sanitized = bundleIds
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .map { BundleIdentifierResolver.canonical(for: $0) }

        allowedBundleIDs = Set(sanitized)
        let expanded = expandedBundleIDs
        lastTextByApp = lastTextByApp.filter { expanded.contains($0.key) }
        persistAllowedBundleIDs()

        let claudeEnabled = BundleIdentifierResolver.contains(
            allowedBundleIDs,
            candidate: "com.anthropic.claude-desktop"
        )
        claudeAdapter.setEnabled(claudeEnabled && isRunning)

        if allowedBundleIDs.isEmpty {
            stop()
        } else if !isRunning {
            start()
        }

        print("Updated monitored apps: \(allowedBundleIDs)")
    }

    func isMonitoring(bundleId: String) -> Bool {
        BundleIdentifierResolver.contains(allowedBundleIDs, candidate: bundleId)
    }

    func isCapturing() -> Bool {
        return isRunning
    }

    private func persistAllowedBundleIDs() {
        let sorted = Array(allowedBundleIDs).sorted()
        defaults.set(sorted, forKey: Self.allowedAppsKey)
    }

    private func persistAllowedBundleIDsIfNeeded() {
        if defaults.array(forKey: Self.allowedAppsKey) == nil {
            persistAllowedBundleIDs()
        } else {
            let stored = defaults.array(forKey: Self.allowedAppsKey) as? [String]
            let fallback = AccessibilityWatcher.defaultBundleIdentifiers
            let canonicalized = (stored ?? fallback)
                .map { BundleIdentifierResolver.canonical(for: $0) }
            allowedBundleIDs = Set(canonicalized)
        }
    }
}
