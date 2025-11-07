import Cocoa
import UniformTypeIdentifiers

// Represents a monitored application
struct MonitoredApp: Codable {
    let bundleId: String
    var name: String
    var icon: NSImage?
    var isEnabled: Bool
    var captureCount: Int
    var addedManually: Bool
    var bundlePath: String?

    enum CodingKeys: String, CodingKey {
        case bundleId
        case name
        case isEnabled
        case captureCount
        case addedManually
        case bundlePath
    }

    init(bundleId: String,
         name: String,
         icon: NSImage? = nil,
         isEnabled: Bool,
         captureCount: Int,
         addedManually: Bool,
         bundlePath: String? = nil) {
        self.bundleId = BundleIdentifierResolver.canonical(for: bundleId)
        self.name = name
        self.icon = icon
        self.isEnabled = isEnabled
        self.captureCount = captureCount
        self.addedManually = addedManually
        self.bundlePath = bundlePath
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let decodedBundleId = try container.decode(String.self, forKey: .bundleId)
        bundleId = BundleIdentifierResolver.canonical(for: decodedBundleId)
        name = try container.decode(String.self, forKey: .name)
        isEnabled = try container.decode(Bool.self, forKey: .isEnabled)
        captureCount = try container.decodeIfPresent(Int.self, forKey: .captureCount) ?? 0
        addedManually = try container.decodeIfPresent(Bool.self, forKey: .addedManually) ?? false
        bundlePath = try container.decodeIfPresent(String.self, forKey: .bundlePath)
        icon = nil
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(bundleId, forKey: .bundleId)
        try container.encode(name, forKey: .name)
        try container.encode(isEnabled, forKey: .isEnabled)
        try container.encode(captureCount, forKey: .captureCount)
        try container.encode(addedManually, forKey: .addedManually)
        try container.encodeIfPresent(bundlePath, forKey: .bundlePath)
    }
}

class AppManagerWindowController: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate {
    private var tableView: NSTableView!
    private var statusLabel: NSTextField!
    private var memoryCountLabel: NSTextField!
    private var captureToggle: NSSwitch!
    private var addButton: NSButton!
    private var removeButton: NSButton!
    private var accessibilityWatcher: AccessibilityWatcher?

    private var monitoredApps: [MonitoredApp] = []
    private let defaultsKey = "MemoryLayerMonitoredApps"

    init(accessibilityWatcher: AccessibilityWatcher?) {
        self.accessibilityWatcher = accessibilityWatcher

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        super.init(window: window)

        monitoredApps = loadMonitoredApps()

        window.title = "Memory Layer - App Manager"
        window.delegate = self
        window.center()
        window.setFrameAutosaveName("AppManagerWindow")

        setupUI()
        loadAppIcons()
        saveMonitoredApps()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupUI() {
        guard let contentView = window?.contentView else { return }

        let margin: CGFloat = 20
        var yPosition = contentView.bounds.height - 60

        // Title
        let titleLabel = NSTextField(frame: NSRect(
            x: margin,
            y: yPosition,
            width: contentView.bounds.width - (margin * 2),
            height: 30
        ))
        titleLabel.stringValue = "Connected Applications"
        titleLabel.font = .mlDisplay2
        titleLabel.textColor = .mlTextPrimary
        titleLabel.isEditable = false
        titleLabel.isBordered = false
        titleLabel.backgroundColor = .clear
        titleLabel.autoresizingMask = [.width, .minYMargin]
        contentView.addSubview(titleLabel)

        yPosition -= 40

        // Subtitle
        let subtitleLabel = NSTextField(frame: NSRect(
            x: margin,
            y: yPosition,
            width: contentView.bounds.width - (margin * 2),
            height: 20
        ))
        subtitleLabel.stringValue = "Choose which applications Memory Layer monitors for text capture"
        subtitleLabel.font = .mlBody2
        subtitleLabel.textColor = .mlTextSecondary
        subtitleLabel.isEditable = false
        subtitleLabel.isBordered = false
        subtitleLabel.backgroundColor = .clear
        subtitleLabel.autoresizingMask = [.width, .minYMargin]
        contentView.addSubview(subtitleLabel)

        yPosition -= 50

        // Status bar
        let statusContainer = NSView(frame: NSRect(
            x: margin,
            y: contentView.bounds.height - 140,
            width: contentView.bounds.width - (margin * 2),
            height: 60
        ))
        statusContainer.wantsLayer = true
        if let layer = statusContainer.layer {
            // Create gradient background
            let gradientLayer = CAGradientLayer()
            gradientLayer.frame = statusContainer.bounds
            gradientLayer.colors = [
                NSColor.mlPrimary.withAlphaComponent(0.08).cgColor,
                NSColor.mlPrimaryLight.withAlphaComponent(0.04).cgColor
            ]
            gradientLayer.startPoint = CGPoint(x: 0, y: 0)
            gradientLayer.endPoint = CGPoint(x: 1, y: 1)
            gradientLayer.cornerRadius = MLCornerRadius.medium.rawValue
            layer.addSublayer(gradientLayer)

            layer.borderColor = NSColor.mlBorder.cgColor
            layer.borderWidth = 1
            layer.cornerRadius = MLCornerRadius.medium.rawValue
            layer.masksToBounds = true
        }
        statusContainer.autoresizingMask = [.width, .minYMargin]
        contentView.addSubview(statusContainer)

        // Status info inside container
        statusLabel = NSTextField(frame: NSRect(x: 15, y: 30, width: 300, height: 20))
        statusLabel.stringValue = "Status: ✓ Capturing"
        statusLabel.font = .mlBodySemibold
        statusLabel.isEditable = false
        statusLabel.isBordered = false
        statusLabel.backgroundColor = .clear
        statusLabel.textColor = .mlSuccess
        statusContainer.addSubview(statusLabel)

        memoryCountLabel = NSTextField(frame: NSRect(x: 15, y: 10, width: 300, height: 20))
        memoryCountLabel.stringValue = "Total memories captured: 0"
        memoryCountLabel.font = .mlCaption
        memoryCountLabel.isEditable = false
        memoryCountLabel.isBordered = false
        memoryCountLabel.backgroundColor = .clear
        memoryCountLabel.textColor = .mlTextSecondary
        statusContainer.addSubview(memoryCountLabel)

        // Master toggle
        captureToggle = NSSwitch(frame: NSRect(
            x: statusContainer.bounds.width - 80,
            y: 20,
            width: 60,
            height: 30
        ))
        captureToggle.autoresizingMask = [.minXMargin]
        captureToggle.state = .on
        captureToggle.target = self
        captureToggle.action = #selector(toggleAllCapture)
        statusContainer.addSubview(captureToggle)

        let toggleLabel = NSTextField(frame: NSRect(
            x: statusContainer.bounds.width - 150,
            y: 20,
            width: 60,
            height: 20
        ))
        toggleLabel.stringValue = "Capture:"
        toggleLabel.font = .mlLabel
        toggleLabel.textColor = .mlTextSecondary
        toggleLabel.isEditable = false
        toggleLabel.isBordered = false
        toggleLabel.backgroundColor = .clear
        toggleLabel.alignment = .right
        toggleLabel.autoresizingMask = [.minXMargin]
        statusContainer.addSubview(toggleLabel)

        // Table view for apps
        let scrollView = NSScrollView(frame: NSRect(
            x: margin,
            y: 90,
            width: contentView.bounds.width - (margin * 2),
            height: contentView.bounds.height - 300
        ))
        scrollView.autoresizingMask = [.width, .height]
        scrollView.hasVerticalScroller = true
        scrollView.borderType = .noBorder

        tableView = NSTableView(frame: scrollView.bounds)
        tableView.autoresizingMask = [.width, .height]
        tableView.rowHeight = MLTableStyle.rowHeight
        tableView.intercellSpacing = NSSize(width: 0, height: 4)
        tableView.backgroundColor = .mlBackground
        tableView.usesAlternatingRowBackgroundColors = false
        tableView.gridColor = .mlBorder

        // Create columns
        let iconColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("icon"))
        iconColumn.title = ""
        iconColumn.width = 50

        let nameColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("name"))
        nameColumn.title = "Application"
        nameColumn.width = 300

        let statusColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("status"))
        statusColumn.title = "Status"
        statusColumn.width = 200

        let toggleColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("toggle"))
        toggleColumn.title = "Enabled"
        toggleColumn.width = 100

        tableView.addTableColumn(iconColumn)
        tableView.addTableColumn(nameColumn)
        tableView.addTableColumn(statusColumn)
        tableView.addTableColumn(toggleColumn)

        tableView.dataSource = self
        tableView.delegate = self

        scrollView.documentView = tableView
        contentView.addSubview(scrollView)

        // Bottom instructions
        let instructionsLabel = NSTextField(frame: NSRect(
            x: margin,
            y: 55,
            width: contentView.bounds.width - (margin * 2),
            height: 20
        ))
        instructionsLabel.stringValue = "Memory Layer reads text only, never takes screenshots. All data is stored locally."
        instructionsLabel.font = .mlCaption
        instructionsLabel.textColor = .mlTextTertiary
        instructionsLabel.isEditable = false
        instructionsLabel.isBordered = false
        instructionsLabel.backgroundColor = .clear
        instructionsLabel.alignment = .center
        instructionsLabel.autoresizingMask = [.width, .maxYMargin]
        contentView.addSubview(instructionsLabel)

        addButton = NSButton(frame: NSRect(
            x: margin,
            y: 15,
            width: 140,
            height: 30
        ))
        addButton.title = "Add Application..."
        addButton.bezelStyle = .rounded
        addButton.target = self
        addButton.action = #selector(addApplication)
        addButton.autoresizingMask = [.maxXMargin, .maxYMargin]
        contentView.addSubview(addButton)

        removeButton = NSButton(frame: NSRect(
            x: margin + 150,
            y: 15,
            width: 100,
            height: 30
        ))
        removeButton.title = "Remove"
        removeButton.bezelStyle = .rounded
        removeButton.target = self
        removeButton.action = #selector(removeSelectedApplication)
        removeButton.isEnabled = false
        removeButton.autoresizingMask = [.maxXMargin, .maxYMargin]
        contentView.addSubview(removeButton)

        // Refresh button
        let refreshButton = NSButton(frame: NSRect(
            x: contentView.bounds.width - 120,
            y: 15,
            width: 100,
            height: 30
        ))
        refreshButton.title = "Refresh"
        refreshButton.bezelStyle = .rounded
        refreshButton.target = self
        refreshButton.action = #selector(refreshApps)
        refreshButton.autoresizingMask = [.minXMargin, .maxYMargin]
        contentView.addSubview(refreshButton)
    }

    private func resolveBundleURL(bundleId: String, currentPath: String?) -> URL? {
        if let currentPath,
           FileManager.default.fileExists(atPath: currentPath) {
            return URL(fileURLWithPath: currentPath, isDirectory: true)
        }

        return BundleIdentifierResolver.locate(bundleId: bundleId, currentPath: currentPath)
    }

    private func cacheBundlePath(_ path: String, for bundleId: String) {
        if let index = monitoredApps.firstIndex(where: { $0.bundleId == bundleId }) {
            monitoredApps[index].bundlePath = path
        }
    }

    private func defaultMonitoredApps() -> [MonitoredApp] {
        return [
            MonitoredApp(bundleId: "com.anthropic.claude-desktop", name: "Claude Desktop", isEnabled: true, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.openai.ChatGPT", name: "ChatGPT", isEnabled: true, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.microsoft.VSCode", name: "VS Code", isEnabled: true, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.apple.Safari", name: "Safari", isEnabled: true, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.google.Chrome", name: "Chrome", isEnabled: true, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.apple.mail", name: "Mail", isEnabled: false, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.apple.Notes", name: "Notes", isEnabled: false, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.apple.dt.Xcode", name: "Xcode", isEnabled: false, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.jetbrains.intellij", name: "IntelliJ IDEA", isEnabled: false, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.sublimetext.4", name: "Sublime Text", isEnabled: false, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "org.vim.MacVim", name: "MacVim", isEnabled: false, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.cursor.IDE", name: "Cursor", isEnabled: true, captureCount: 0, addedManually: false),
            MonitoredApp(bundleId: "com.apple.Terminal", name: "Terminal", isEnabled: true, captureCount: 0, addedManually: false)
        ]
    }

    private func loadMonitoredApps() -> [MonitoredApp] {
        guard let data = UserDefaults.standard.data(forKey: defaultsKey),
              let saved = try? JSONDecoder().decode([MonitoredApp].self, from: data) else {
            return defaultMonitoredApps()
        }

        var combined = saved.map { app -> MonitoredApp in
            MonitoredApp(
                bundleId: BundleIdentifierResolver.canonical(for: app.bundleId),
                name: app.name,
                icon: nil,
                isEnabled: app.isEnabled,
                captureCount: app.captureCount,
                addedManually: app.addedManually,
                bundlePath: app.bundlePath
            )
        }
        let defaults = defaultMonitoredApps()

        for defaultApp in defaults {
            if let index = combined.firstIndex(where: { $0.bundleId == defaultApp.bundleId }) {
                combined[index].name = defaultApp.name
                combined[index].addedManually = false
            } else {
                combined.append(defaultApp)
            }
        }

        return combined.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    private func saveMonitoredApps() {
        let encoder = JSONEncoder()
        if let data = try? encoder.encode(monitoredApps) {
            UserDefaults.standard.set(data, forKey: defaultsKey)
        }

        syncAccessibilityWatcher()
        updateStats()
    }

    private func syncAccessibilityWatcher() {
        let enabledBundleIds = monitoredApps
            .filter { $0.isEnabled }
            .map { BundleIdentifierResolver.canonical(for: $0.bundleId) }

        accessibilityWatcher?.updateWhitelistedApps(bundleIds: enabledBundleIds)
    }

    private func isAppInstalled(_ app: MonitoredApp) -> Bool {
        if let path = app.bundlePath,
           FileManager.default.fileExists(atPath: path) {
            return true
        }

        if let url = resolveBundleURL(bundleId: app.bundleId, currentPath: app.bundlePath) {
            cacheBundlePath(url.path, for: app.bundleId)
            return true
        }

        return false
    }

    private func loadAppIcons() {
        for i in 0..<monitoredApps.count {
            let app = monitoredApps[i]
            if let url = resolveBundleURL(bundleId: app.bundleId, currentPath: app.bundlePath) {
                monitoredApps[i].icon = NSWorkspace.shared.icon(forFile: url.path)
                monitoredApps[i].bundlePath = url.path
            } else if let path = app.bundlePath {
                monitoredApps[i].icon = NSWorkspace.shared.icon(forFile: path)
            } else {
                monitoredApps[i].icon = NSImage(systemSymbolName: "app", accessibilityDescription: nil)
            }
        }
    }

    private func updateStats() {
        let enabledCount = monitoredApps.filter { $0.isEnabled }.count
        let installedCount = monitoredApps.filter { isAppInstalled($0) }.count
        memoryCountLabel.stringValue = "Connected apps: \(enabledCount) of \(installedCount) installed (\(monitoredApps.count) total)"

        let isCapturing = accessibilityWatcher?.isCapturing() ?? (captureToggle.state == .on)
        captureToggle.state = isCapturing ? .on : .off

        // Update status
        if enabledCount == 0 {
            statusLabel.stringValue = "Status: ⚠︎ No apps enabled"
            statusLabel.textColor = .systemOrange
        } else if isCapturing {
            statusLabel.stringValue = "Status: ✓ Capturing"
            statusLabel.textColor = .systemGreen
        } else {
            statusLabel.stringValue = "Status: ⏸ Paused"
            statusLabel.textColor = .systemOrange
        }
    }

    // MARK: - Actions

    @objc private func toggleAllCapture() {
        let isOn = captureToggle.state == .on

        if isOn {
            accessibilityWatcher?.start()
        } else {
            accessibilityWatcher?.stop()
        }

        updateStats()
    }

    @objc private func refreshApps() {
        loadAppIcons()
        saveMonitoredApps()
        tableView.reloadData()
        tableView.deselectAll(nil)
        removeButton.isEnabled = false
    }

    @objc private func toggleApp(_ sender: NSSwitch) {
        let row = tableView.row(for: sender)
        if row >= 0 && row < monitoredApps.count {
            monitoredApps[row].isEnabled = sender.state == .on
            saveMonitoredApps()
            let selectedRow = tableView.selectedRow
            tableView.reloadData()
            if selectedRow >= 0 && selectedRow < monitoredApps.count {
                removeButton.isEnabled = monitoredApps[selectedRow].addedManually
            } else {
                removeButton.isEnabled = false
            }
        }
    }

    @objc private func addApplication() {
        let panel = NSOpenPanel()
        panel.prompt = "Connect"
        if #available(macOS 12.0, *) {
            panel.allowedContentTypes = [.applicationBundle]
        } else {
            panel.allowedFileTypes = ["app"]
        }
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.title = "Select an application to connect"

        if panel.runModal() == .OK, let url = panel.url {
            guard let bundle = Bundle(url: url),
                  let bundleId = bundle.bundleIdentifier else {
                showAlert(message: "Unable to identify application bundle", info: "Choose a standard macOS application bundle (with a bundle identifier).")
                return
            }

            let name = bundle.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ??
                bundle.object(forInfoDictionaryKey: "CFBundleName") as? String ??
                url.deletingPathExtension().lastPathComponent

            if let existingIndex = monitoredApps.firstIndex(where: { $0.bundleId == bundleId }) {
                monitoredApps[existingIndex].name = name
                monitoredApps[existingIndex].bundlePath = url.path
                monitoredApps[existingIndex].icon = NSWorkspace.shared.icon(forFile: url.path)
                monitoredApps[existingIndex].isEnabled = true
                monitoredApps[existingIndex].addedManually = true
            } else {
                let icon = NSWorkspace.shared.icon(forFile: url.path)
                let newApp = MonitoredApp(bundleId: bundleId,
                                          name: name,
                                          icon: icon,
                                          isEnabled: true,
                                          captureCount: 0,
                                          addedManually: true,
                                          bundlePath: url.path)
                monitoredApps.append(newApp)
            }

            monitoredApps.sort { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
            saveMonitoredApps()
            tableView.reloadData()
            tableView.deselectAll(nil)
            removeButton.isEnabled = false
        }
    }

    @objc private func removeSelectedApplication() {
        let row = tableView.selectedRow
        guard row >= 0, row < monitoredApps.count else {
            removeButton.isEnabled = false
            return
        }

        let app = monitoredApps[row]
        guard app.addedManually else {
            NSSound.beep()
            removeButton.isEnabled = false
            return
        }

        let alert = NSAlert()
        alert.messageText = "Remove \(app.name)?"
        alert.informativeText = "Memory Layer will stop capturing text from this application."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Remove")
        alert.addButton(withTitle: "Cancel")

        if alert.runModal() == .alertFirstButtonReturn {
            monitoredApps.remove(at: row)
            saveMonitoredApps()
            tableView.reloadData()
            tableView.deselectAll(nil)
            removeButton.isEnabled = false
        }
    }

    private func showAlert(message: String, info: String) {
        let alert = NSAlert()
        alert.messageText = message
        alert.informativeText = info
        alert.alertStyle = .warning
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    // MARK: - NSTableViewDataSource

    func numberOfRows(in tableView: NSTableView) -> Int {
        return monitoredApps.count
    }

    // MARK: - NSTableViewDelegate

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard row < monitoredApps.count else { return nil }

        let app = monitoredApps[row]
        let identifier = tableColumn?.identifier

        let cell = NSView()

        switch identifier?.rawValue {
        case "icon":
            let imageView = NSImageView(frame: NSRect(x: 10, y: 10, width: 40, height: 40))
            imageView.image = app.icon
            imageView.imageScaling = .scaleProportionallyUpOrDown
            cell.addSubview(imageView)

        case "name":
            let nameLabel = NSTextField(frame: NSRect(x: 0, y: 30, width: 300, height: 20))
            nameLabel.stringValue = app.name
            nameLabel.font = .systemFont(ofSize: 13, weight: .medium)
            nameLabel.isEditable = false
            nameLabel.isBordered = false
            nameLabel.backgroundColor = .clear
            cell.addSubview(nameLabel)

            let bundleLabel = NSTextField(frame: NSRect(x: 0, y: 10, width: 300, height: 20))
            bundleLabel.stringValue = app.bundleId
            bundleLabel.font = .systemFont(ofSize: 10)
            bundleLabel.textColor = .secondaryLabelColor
            bundleLabel.isEditable = false
            bundleLabel.isBordered = false
            bundleLabel.backgroundColor = .clear
            cell.addSubview(bundleLabel)

        case "status":
            let installed = isAppInstalled(app)
            let statusLabel = NSTextField(frame: NSRect(x: 0, y: 20, width: 200, height: 20))

            if installed {
                if app.isEnabled {
                    statusLabel.stringValue = "✓ Monitoring"
                    statusLabel.textColor = .systemGreen
                } else {
                    statusLabel.stringValue = "Not monitoring"
                    statusLabel.textColor = .secondaryLabelColor
                }
            } else {
                statusLabel.stringValue = "Not installed"
                statusLabel.textColor = .tertiaryLabelColor
            }

            statusLabel.font = .systemFont(ofSize: 12)
            statusLabel.isEditable = false
            statusLabel.isBordered = false
            statusLabel.backgroundColor = .clear
            cell.addSubview(statusLabel)

        case "toggle":
            let toggle = NSSwitch(frame: NSRect(x: 20, y: 18, width: 60, height: 30))
            toggle.state = app.isEnabled ? .on : .off
            toggle.target = self
            toggle.action = #selector(toggleApp(_:))

            let installed = isAppInstalled(app)
            toggle.isEnabled = installed

            cell.addSubview(toggle)

        default:
            break
        }

        return cell
    }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        return 60
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        let row = tableView.selectedRow
        if row >= 0 && row < monitoredApps.count {
            removeButton.isEnabled = monitoredApps[row].addedManually
        } else {
            removeButton.isEnabled = false
        }
    }
}
