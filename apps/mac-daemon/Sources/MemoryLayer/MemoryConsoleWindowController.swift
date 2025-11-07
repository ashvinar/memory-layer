import Cocoa
import WebKit

private struct ConnectionRecord {
    let appName: String
    let bundleId: String
    let installed: Bool
    let monitoring: Bool
}

final class MemoryConsoleWindowController: NSWindowController, NSWindowDelegate, NSTabViewDelegate, NSTableViewDataSource, NSTableViewDelegate {
    private let memoryClient = MemoryServiceClient()
    private weak var accessibilityWatcher: AccessibilityWatcher?

    private var tabView: NSTabView!
    private var memories: [MemoryRecord] = []
    private var topics: [TopicSummaryRecord] = []
    private var connections: [ConnectionRecord] = []
    private var workspaces: [WorkspaceRecord] = []
    private var projects: [ProjectRecord] = []
    private var areas: [AreaRecord] = []
    private var hierarchyTopics: [TopicRecord] = []
    private var memoryStatusLabel: NSTextField!
    private var topicsStatusLabel: NSTextField!
    private var connectionsStatusLabel: NSTextField!
    private var graphStatusLabel: NSTextField!
    private var hierarchyStatusLabel: NSTextField!

    private var memoriesTable: NSTableView!
    private var topicsTable: NSTableView!
    private var connectionsTable: NSTableView!
    private var hierarchyTable: NSTableView!
    private var hierarchyLevelSegment: NSSegmentedControl!
    private var currentHierarchyLevel: HierarchyLevel = .workspace
    private var graphView: KnowledgeGraphWebView!
    private var hasRenderedGraph = false
    private var loadedTabs: Set<String> = []
    private var graphRefreshTimer: Timer?
    private var isGraphRefreshing = false

    enum HierarchyLevel: Int {
        case workspace = 0
        case project = 1
        case area = 2
        case topic = 3
    }

    init(accessibilityWatcher: AccessibilityWatcher?) {
        self.accessibilityWatcher = accessibilityWatcher

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 960, height: 620),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.center()
        window.title = "Memory Layer Console"

        super.init(window: window)
        window.delegate = self

        setupUI()
        tabView.selectTabViewItem(withIdentifier: "graph")
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    // MARK: - UI Setup

    private func setupUI() {
        guard let contentView = window?.contentView else { return }

        tabView = NSTabView()
        tabView.translatesAutoresizingMaskIntoConstraints = false
        tabView.delegate = self
        contentView.addSubview(tabView)

        NSLayoutConstraint.activate([
            tabView.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            tabView.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            tabView.topAnchor.constraint(equalTo: contentView.topAnchor),
            tabView.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])

        let memoriesTab = NSTabViewItem(identifier: "memories")
        memoriesTab.label = "Memories"
        memoriesTab.view = buildMemoriesView()
        tabView.addTabViewItem(memoriesTab)

        let topicsTab = NSTabViewItem(identifier: "topics")
        topicsTab.label = "Memory Map"
        topicsTab.view = buildTopicsView()
        tabView.addTabViewItem(topicsTab)

        let hierarchyTab = NSTabViewItem(identifier: "hierarchy")
        hierarchyTab.label = "Hierarchy"
        hierarchyTab.view = buildHierarchyView()
        tabView.addTabViewItem(hierarchyTab)

        let connectionsTab = NSTabViewItem(identifier: "connections")
        connectionsTab.label = "Connections"
        connectionsTab.view = buildConnectionsView()
        tabView.addTabViewItem(connectionsTab)

        let graphTab = NSTabViewItem(identifier: "graph")
        graphTab.label = "Knowledge Graph"
        graphTab.view = buildGraphView()
        tabView.addTabViewItem(graphTab)
    }

    private func buildMemoriesView() -> NSView {
        let container = NSView()

        let header = makeHeader(statusLabel: &memoryStatusLabel, action: #selector(refreshMemories))
        container.addSubview(header)

        let scrollView = NSScrollView()
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.hasVerticalScroller = true
        scrollView.borderType = .noBorder

        memoriesTable = NSTableView()
        memoriesTable.delegate = self
        memoriesTable.dataSource = self
        memoriesTable.usesAlternatingRowBackgroundColors = false
        memoriesTable.rowHeight = MLTableStyle.rowHeight
        memoriesTable.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        memoriesTable.gridColor = MLTableStyle.gridColor

        let topicColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("mem_topic"))
        topicColumn.title = "Topic"
        topicColumn.width = 180

        let textColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("mem_text"))
        textColumn.title = "Preview"
        textColumn.width = 480

        let kindColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("mem_kind"))
        kindColumn.title = "Kind"
        kindColumn.width = 100

        let createdColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("mem_created"))
        createdColumn.title = "Captured"
        createdColumn.width = 160

        memoriesTable.addTableColumn(topicColumn)
        memoriesTable.addTableColumn(textColumn)
        memoriesTable.addTableColumn(kindColumn)
        memoriesTable.addTableColumn(createdColumn)

        scrollView.documentView = memoriesTable
        container.addSubview(scrollView)

        applySectionConstraints(container: container, header: header, scrollView: scrollView)
        memoryStatusLabel.stringValue = "Loading recent memories…"

        return container
    }

    private func buildTopicsView() -> NSView {
        let container = NSView()

        let header = makeHeader(statusLabel: &topicsStatusLabel, action: #selector(refreshTopics))
        container.addSubview(header)

        let scrollView = NSScrollView()
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.hasVerticalScroller = true
        scrollView.borderType = .noBorder

        topicsTable = NSTableView()
        topicsTable.delegate = self
        topicsTable.dataSource = self
        topicsTable.usesAlternatingRowBackgroundColors = false
        topicsTable.rowHeight = MLTableStyle.rowHeight
        topicsTable.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        topicsTable.gridColor = MLTableStyle.gridColor

        let topicColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("map_topic"))
        topicColumn.title = "Topic"
        topicColumn.width = 360

        let countColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("map_count"))
        countColumn.title = "Memories"
        countColumn.width = 120

        let updatedColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("map_recent"))
        updatedColumn.title = "Last Updated"
        updatedColumn.width = 200

        topicsTable.addTableColumn(topicColumn)
        topicsTable.addTableColumn(countColumn)
        topicsTable.addTableColumn(updatedColumn)

        scrollView.documentView = topicsTable
        container.addSubview(scrollView)

        applySectionConstraints(container: container, header: header, scrollView: scrollView)
        topicsStatusLabel.stringValue = "Analyzing topics…"

        return container
    }

    private func buildHierarchyView() -> NSView {
        let container = NSView()

        // Header with segmented control for level selection
        let header = NSStackView()
        header.orientation = .horizontal
        header.alignment = .centerY
        header.spacing = 12
        header.translatesAutoresizingMaskIntoConstraints = false

        let statusLabel = NSTextField(labelWithString: "")
        statusLabel.font = .mlCaption
        statusLabel.textColor = .mlTextSecondary
        statusLabel.setContentHuggingPriority(.defaultHigh, for: .horizontal)

        hierarchyStatusLabel = statusLabel

        let segmentControl = NSSegmentedControl()
        segmentControl.segmentCount = 4
        segmentControl.segmentStyle = .rounded

        // Add icons and labels for hierarchy levels
        if let workspaceIcon = NSImage(systemSymbolName: "building.2", accessibilityDescription: nil) {
            segmentControl.setImage(workspaceIcon, forSegment: 0)
        }
        segmentControl.setLabel("Workspaces", forSegment: 0)
        segmentControl.setImageScaling(.scaleProportionallyDown, forSegment: 0)

        if let projectIcon = NSImage(systemSymbolName: "folder", accessibilityDescription: nil) {
            segmentControl.setImage(projectIcon, forSegment: 1)
        }
        segmentControl.setLabel("Projects", forSegment: 1)
        segmentControl.setImageScaling(.scaleProportionallyDown, forSegment: 1)

        if let areaIcon = NSImage(systemSymbolName: "square.grid.2x2", accessibilityDescription: nil) {
            segmentControl.setImage(areaIcon, forSegment: 2)
        }
        segmentControl.setLabel("Areas", forSegment: 2)
        segmentControl.setImageScaling(.scaleProportionallyDown, forSegment: 2)

        if let topicIcon = NSImage(systemSymbolName: "tag", accessibilityDescription: nil) {
            segmentControl.setImage(topicIcon, forSegment: 3)
        }
        segmentControl.setLabel("Topics", forSegment: 3)
        segmentControl.setImageScaling(.scaleProportionallyDown, forSegment: 3)

        segmentControl.selectedSegment = 0
        segmentControl.target = self
        segmentControl.action = #selector(hierarchyLevelChanged(_:))
        segmentControl.setContentHuggingPriority(.defaultHigh, for: .horizontal)

        hierarchyLevelSegment = segmentControl

        let spacer = NSView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)

        let refreshButton = NSButton(title: "Refresh", target: self, action: #selector(refreshHierarchy))
        refreshButton.bezelStyle = .rounded

        header.addArrangedSubview(statusLabel)
        header.addArrangedSubview(segmentControl)
        header.addArrangedSubview(spacer)
        header.addArrangedSubview(refreshButton)
        container.addSubview(header)

        // Table view for hierarchy items
        let scrollView = NSScrollView()
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.hasVerticalScroller = true
        scrollView.borderType = .noBorder

        hierarchyTable = NSTableView()
        hierarchyTable.delegate = self
        hierarchyTable.dataSource = self
        hierarchyTable.usesAlternatingRowBackgroundColors = false
        hierarchyTable.rowHeight = MLTableStyle.rowHeight
        hierarchyTable.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        hierarchyTable.gridColor = MLTableStyle.gridColor

        let nameColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("hier_name"))
        nameColumn.title = "Name"
        nameColumn.width = 400

        let countColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("hier_count"))
        countColumn.title = "Items"
        countColumn.width = 120

        let idColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("hier_id"))
        idColumn.title = "ID"
        idColumn.width = 200

        hierarchyTable.addTableColumn(nameColumn)
        hierarchyTable.addTableColumn(countColumn)
        hierarchyTable.addTableColumn(idColumn)

        scrollView.documentView = hierarchyTable
        container.addSubview(scrollView)

        // Constraints
        header.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16).isActive = true
        header.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16).isActive = true
        header.topAnchor.constraint(equalTo: container.topAnchor, constant: 16).isActive = true

        NSLayoutConstraint.activate([
            scrollView.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16),
            scrollView.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16),
            scrollView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 12),
            scrollView.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -16)
        ])

        hierarchyStatusLabel.stringValue = "Loading hierarchy…"
        return container
    }

    private func buildConnectionsView() -> NSView {
        let container = NSView()

        let header = makeHeader(statusLabel: &connectionsStatusLabel, action: #selector(refreshConnections))
        container.addSubview(header)

        let scrollView = NSScrollView()
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.hasVerticalScroller = true
        scrollView.borderType = .noBorder

        connectionsTable = NSTableView()
        connectionsTable.delegate = self
        connectionsTable.dataSource = self
        connectionsTable.usesAlternatingRowBackgroundColors = false
        connectionsTable.rowHeight = MLTableStyle.rowHeight
        connectionsTable.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        connectionsTable.gridColor = MLTableStyle.gridColor

        let nameColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("conn_name"))
        nameColumn.title = "Application"
        nameColumn.width = 240

        let bundleColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("conn_bundle"))
        bundleColumn.title = "Bundle ID"
        bundleColumn.width = 320

        let statusColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("conn_status"))
        statusColumn.title = "Status"
        statusColumn.width = 200

        connectionsTable.addTableColumn(nameColumn)
        connectionsTable.addTableColumn(bundleColumn)
        connectionsTable.addTableColumn(statusColumn)

        scrollView.documentView = connectionsTable
        container.addSubview(scrollView)

        applySectionConstraints(container: container, header: header, scrollView: scrollView)
        connectionsStatusLabel.stringValue = "Inspecting accessibility scope…"

        return container
    }

    private func buildGraphView() -> NSView {
        let container = NSView()

        let header = makeHeader(statusLabel: &graphStatusLabel, action: #selector(refreshGraph))
        container.addSubview(header)

        graphView = KnowledgeGraphWebView()
        container.addSubview(graphView)

        header.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16).isActive = true
        header.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16).isActive = true
        header.topAnchor.constraint(equalTo: container.topAnchor, constant: 16).isActive = true

        NSLayoutConstraint.activate([
            graphView.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16),
            graphView.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16),
            graphView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 12),
            graphView.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -16)
        ])

        graphStatusLabel.stringValue = "Loading knowledge graph…"
        return container
    }

    private func makeHeader(statusLabel: inout NSTextField!, action: Selector) -> NSView {
        let header = NSStackView()
        header.orientation = .horizontal
        header.alignment = .centerY
        header.spacing = 8
        header.translatesAutoresizingMaskIntoConstraints = false

        let label = NSTextField(labelWithString: "")
        label.font = .mlCaption
        label.textColor = .mlTextSecondary
        label.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        label.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let spacer = NSView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        spacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let button = NSButton(title: "Refresh", target: self, action: action)
        button.bezelStyle = .rounded

        header.addArrangedSubview(label)
        header.addArrangedSubview(spacer)
        header.addArrangedSubview(button)

        statusLabel = label
        return header
    }

    private func applySectionConstraints(container: NSView, header: NSView, scrollView: NSScrollView) {
        header.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16).isActive = true
        header.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16).isActive = true
        header.topAnchor.constraint(equalTo: container.topAnchor, constant: 16).isActive = true

        NSLayoutConstraint.activate([
            scrollView.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16),
            scrollView.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16),
            scrollView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 12),
            scrollView.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -16)
        ])
    }

    // MARK: - Data Loading

    @objc private func refreshMemories() {
        fadeTransition(to: "Loading recent memories…", label: memoryStatusLabel)

        Task {
            do {
                let items = try await memoryClient.fetchRecentMemories(limit: 200)
                await MainActor.run {
                    self.memories = items
                    if items.isEmpty {
                        self.fadeTransition(to: "No memories yet. Start using monitored apps to capture data.", label: self.memoryStatusLabel)
                    } else {
                        self.fadeTransition(to: "Loaded \(items.count) memories", label: self.memoryStatusLabel)
                    }

                    NSAnimationContext.runAnimationGroup { context in
                        context.duration = 0.3
                        self.memoriesTable.animator().reloadData()
                    }
                }
            } catch let error as ProviderError {
                await MainActor.run {
                    self.memories = []
                    self.memoriesTable.reloadData()
                    let message: String
                    switch error {
                    case .serviceUnavailable:
                        message = "Memory service offline. Please ensure services are running."
                    case .decodingError(let decodingError):
                        message = "Failed to parse response: \(decodingError.localizedDescription)"
                    case .networkError(let networkError):
                        message = "Network error: \(networkError.localizedDescription)"
                    case .invalidResponse:
                        message = "Invalid response from service."
                    }
                    self.fadeTransition(to: message, label: self.memoryStatusLabel)
                }
            } catch {
                await MainActor.run {
                    self.memories = []
                    self.memoriesTable.reloadData()
                    self.fadeTransition(to: "Failed to load memories: \(error.localizedDescription)", label: self.memoryStatusLabel)
                }
            }
        }
    }

    @objc private func refreshTopics() {
        fadeTransition(to: "Analyzing topics…", label: topicsStatusLabel)

        Task {
            do {
                let items = try await memoryClient.fetchTopicSummaries(limit: 100)
                await MainActor.run {
                    self.topics = items
                    if items.isEmpty {
                        self.fadeTransition(to: "No topics yet. Memories will be organized as they are captured.", label: self.topicsStatusLabel)
                    } else {
                        self.fadeTransition(to: "Mapped \(items.count) topics", label: self.topicsStatusLabel)
                    }

                    NSAnimationContext.runAnimationGroup { context in
                        context.duration = 0.3
                        self.topicsTable.animator().reloadData()
                    }
                }
            } catch let error as ProviderError {
                await MainActor.run {
                    self.topics = []
                    self.topicsTable.reloadData()
                    let message: String
                    switch error {
                    case .serviceUnavailable:
                        message = "Memory service offline. Please ensure services are running."
                    case .decodingError(let decodingError):
                        message = "Failed to parse response: \(decodingError.localizedDescription)"
                    case .networkError(let networkError):
                        message = "Network error: \(networkError.localizedDescription)"
                    case .invalidResponse:
                        message = "Invalid response from service."
                    }
                    self.fadeTransition(to: message, label: self.topicsStatusLabel)
                }
            } catch {
                await MainActor.run {
                    self.topics = []
                    self.topicsTable.reloadData()
                    self.fadeTransition(to: "Failed to load topics: \(error.localizedDescription)", label: self.topicsStatusLabel)
                }
            }
        }
    }

    @objc private func refreshConnections() {
        let allowed = accessibilityWatcher?.currentWhitelistedApps() ?? []
        let isCapturing = accessibilityWatcher?.isCapturing() ?? false

        let records: [ConnectionRecord] = allowed.sorted().map { bundleId in
            let canonicalId = BundleIdentifierResolver.canonical(for: bundleId)
            let installedURL = BundleIdentifierResolver.locate(bundleId: canonicalId)
            let installed = installedURL != nil
            let appName: String
            if let url = installedURL {
                appName = FileManager.default.displayName(atPath: url.path)
            } else {
                appName = canonicalId.components(separatedBy: ".").last?.capitalized ?? canonicalId
            }

            return ConnectionRecord(
                appName: appName,
                bundleId: canonicalId,
                installed: installed,
                monitoring: (accessibilityWatcher?.isMonitoring(bundleId: canonicalId) ?? false) && isCapturing
            )
        }

        connections = records
        connectionsTable.reloadData()

        if records.isEmpty {
            connectionsStatusLabel.stringValue = "No applications connected. Open Manage Apps to add more."
        } else {
            let activeStatus = isCapturing ? "capturing" : "paused"
            connectionsStatusLabel.stringValue = "Monitoring \(records.count) apps (capture \(activeStatus))."
        }
    }

    @objc private func refreshHierarchy() {
        fadeTransition(to: "Loading hierarchy…", label: hierarchyStatusLabel)

        Task {
            do {
                switch currentHierarchyLevel {
                case .workspace:
                    let items = try await memoryClient.fetchWorkspaces()
                    await MainActor.run {
                        self.workspaces = items
                        self.fadeTransition(to: "Loaded \(items.count) workspaces", label: self.hierarchyStatusLabel)

                        NSAnimationContext.runAnimationGroup { context in
                            context.duration = 0.3
                            self.hierarchyTable.animator().reloadData()
                        }
                    }
                case .project:
                    let items = try await memoryClient.fetchProjects()
                    await MainActor.run {
                        self.projects = items
                        self.fadeTransition(to: "Loaded \(items.count) projects", label: self.hierarchyStatusLabel)

                        NSAnimationContext.runAnimationGroup { context in
                            context.duration = 0.3
                            self.hierarchyTable.animator().reloadData()
                        }
                    }
                case .area:
                    let items = try await memoryClient.fetchAreas()
                    await MainActor.run {
                        self.areas = items
                        self.fadeTransition(to: "Loaded \(items.count) areas", label: self.hierarchyStatusLabel)

                        NSAnimationContext.runAnimationGroup { context in
                            context.duration = 0.3
                            self.hierarchyTable.animator().reloadData()
                        }
                    }
                case .topic:
                    let items = try await memoryClient.fetchTopics()
                    await MainActor.run {
                        self.hierarchyTopics = items
                        self.fadeTransition(to: "Loaded \(items.count) topics", label: self.hierarchyStatusLabel)

                        NSAnimationContext.runAnimationGroup { context in
                            context.duration = 0.3
                            self.hierarchyTable.animator().reloadData()
                        }
                    }
                }
            } catch {
                await MainActor.run {
                    self.fadeTransition(to: "Failed to load: \(error.localizedDescription)", label: self.hierarchyStatusLabel)
                }
            }
        }
    }

    @objc private func hierarchyLevelChanged(_ sender: NSSegmentedControl) {
        if let level = HierarchyLevel(rawValue: sender.selectedSegment) {
            currentHierarchyLevel = level
            refreshHierarchy()
        }
    }

    @objc private func refreshGraph() {
        if isGraphRefreshing { return }
        isGraphRefreshing = true

        graphStatusLabel.stringValue = hasRenderedGraph ? "Updating knowledge graph…" : "Loading knowledge graph…"

        Task { [weak self] in
            guard let self else { return }
            do {
                let graphData = try await memoryClient.fetchAgenticGraph(limit: 150)
                await MainActor.run {
                    self.graphView.render(graph: graphData)
                    self.hasRenderedGraph = true
                    if graphData.nodes.isEmpty {
                        self.graphStatusLabel.stringValue = "No graph data yet. Memories will appear here as they build connections."
                    } else {
                        self.graphStatusLabel.stringValue = "Graph: \(graphData.nodes.count) nodes · \(graphData.edges.count) edges"
                    }
                    self.isGraphRefreshing = false
                }
            } catch let error as ProviderError {
                await MainActor.run {
                    let message: String
                    switch error {
                    case .serviceUnavailable:
                        message = "Indexing service offline. Please ensure indexing service is running on port 21954."
                    case .decodingError(let decodingError):
                        message = "Failed to parse graph: \(decodingError.localizedDescription)"
                    case .networkError(let networkError):
                        message = "Network error: \(networkError.localizedDescription)"
                    case .invalidResponse:
                        message = "Invalid response from indexing service."
                    }
                    self.graphStatusLabel.stringValue = message
                    self.hasRenderedGraph = true
                    self.isGraphRefreshing = false
                }
            } catch {
                await MainActor.run {
                    self.graphStatusLabel.stringValue = "Graph unavailable: \(error.localizedDescription)"
                    self.hasRenderedGraph = true
                    self.isGraphRefreshing = false
                }
            }
        }
    }

    // MARK: - NSTableViewDataSource

    func numberOfRows(in tableView: NSTableView) -> Int {
        switch tableView {
        case memoriesTable:
            return memories.count
        case topicsTable:
            return topics.count
        case connectionsTable:
            return connections.count
        case hierarchyTable:
            switch currentHierarchyLevel {
            case .workspace:
                return workspaces.count
            case .project:
                return projects.count
            case .area:
                return areas.count
            case .topic:
                return hierarchyTopics.count
            }
        default:
            return 0
        }
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard let identifier = tableColumn?.identifier else { return nil }

        if tableView == memoriesTable {
            return memoryCell(for: identifier, row: row)
        }

        if tableView == topicsTable {
            return topicCell(for: identifier, row: row)
        }

        if tableView == connectionsTable {
            return connectionCell(for: identifier, row: row)
        }

        if tableView == hierarchyTable {
            return hierarchyCell(for: identifier, row: row)
        }

        return nil
    }

    private func memoryCell(for identifier: NSUserInterfaceItemIdentifier, row: Int) -> NSView? {
        let memory = memories[row]
        switch identifier.rawValue {
        case "mem_topic":
            return makeTextCell(memory.topic, bold: true)
        case "mem_text":
            return makeTextCell(memory.shortPreview)
        case "mem_kind":
            return makeBadgeCell(memory.kind)
        case "mem_created":
            return makeTextCell(memory.createdAtDisplay)
        default:
            return nil
        }
    }

    private func topicCell(for identifier: NSUserInterfaceItemIdentifier, row: Int) -> NSView? {
        let topic = topics[row]
        switch identifier.rawValue {
        case "map_topic":
            return makeTextCell(topic.topic, bold: true)
        case "map_count":
            return makeTextCell("\(topic.memoryCount)")
        case "map_recent":
            return makeTextCell(topic.lastUpdatedDisplay)
        default:
            return nil
        }
    }

    private func connectionCell(for identifier: NSUserInterfaceItemIdentifier, row: Int) -> NSView? {
        let connection = connections[row]
        switch identifier.rawValue {
        case "conn_name":
            return makeTextCell(connection.appName, bold: true)
        case "conn_bundle":
            return makeTextCell(connection.bundleId)
        case "conn_status":
            let status: String
            if connection.installed {
                status = connection.monitoring ? "Installed · Capturing" : "Installed · Paused"
            } else {
                status = "Not installed"
            }
            return makeTextCell(status)
        default:
            return nil
        }
    }

    private func hierarchyCell(for identifier: NSUserInterfaceItemIdentifier, row: Int) -> NSView? {
        switch identifier.rawValue {
        case "hier_name":
            switch currentHierarchyLevel {
            case .workspace:
                let item = workspaces[row]
                return makeTextCell(item.name, bold: true)
            case .project:
                let item = projects[row]
                return makeTextCell(item.displayName, bold: true)
            case .area:
                let item = areas[row]
                return makeTextCell(item.displayName, bold: true)
            case .topic:
                let item = hierarchyTopics[row]
                return makeTextCell(item.displayName, bold: true)
            }
        case "hier_count":
            switch currentHierarchyLevel {
            case .topic:
                let item = hierarchyTopics[row]
                if let count = item.memoryCount {
                    return makeTextCell("\(count) memories")
                }
                return makeTextCell("—")
            default:
                return makeTextCell("—")
            }
        case "hier_id":
            switch currentHierarchyLevel {
            case .workspace:
                return makeTextCell(workspaces[row].id)
            case .project:
                return makeTextCell(projects[row].id)
            case .area:
                return makeTextCell(areas[row].id)
            case .topic:
                return makeTextCell(hierarchyTopics[row].id)
            }
        default:
            return nil
        }
    }

    private func makeTextCell(_ text: String, bold: Bool = false) -> NSView {
        let cell = NSTableCellView()
        let label = NSTextField(labelWithString: text)
        label.lineBreakMode = .byTruncatingTail
        label.font = bold ? .mlBodySemibold : .mlBody2
        label.textColor = bold ? .mlTextPrimary : .mlTextSecondary
        cell.addSubview(label)
        label.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: MLSpacing.small.rawValue),
            label.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -MLSpacing.small.rawValue),
            label.centerYAnchor.constraint(equalTo: cell.centerYAnchor)
        ])
        return cell
    }

    private func makeBadgeCell(_ text: String) -> NSView {
        let cell = NSTableCellView()

        // Map memory kinds to badge styles and icons
        let (style, icon): (MLBadgeView.BadgeStyle, NSImage?) = {
            let lowercased = text.lowercased()
            switch lowercased {
            case let k where k.contains("code"):
                return (.info, NSImage(systemSymbolName: "chevron.left.forwardslash.chevron.right", accessibilityDescription: nil))
            case let k where k.contains("design"):
                return (.custom(backgroundColor: NSColor.systemPurple.withAlphaComponent(0.15), textColor: .systemPurple),
                        NSImage(systemSymbolName: "paintbrush", accessibilityDescription: nil))
            case let k where k.contains("insight"):
                return (.warning, NSImage(systemSymbolName: "lightbulb", accessibilityDescription: nil))
            case let k where k.contains("decision"):
                return (.success, NSImage(systemSymbolName: "checkmark.circle", accessibilityDescription: nil))
            case let k where k.contains("question"):
                return (.custom(backgroundColor: NSColor.systemYellow.withAlphaComponent(0.15), textColor: .systemOrange),
                        NSImage(systemSymbolName: "questionmark.circle", accessibilityDescription: nil))
            case let k where k.contains("context"):
                return (.neutral, NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil))
            default:
                return (.neutral, nil)
            }
        }()

        let badge = MLBadgeView(text: text.uppercased(), style: style, icon: icon)
        badge.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(badge)

        NSLayoutConstraint.activate([
            badge.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
            badge.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: MLSpacing.small.rawValue),
            badge.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -MLSpacing.small.rawValue)
        ])

        return cell
    }

    // MARK: - NSTabViewDelegate

    func tabView(_ tabView: NSTabView, didSelect tabViewItem: NSTabViewItem?) {
        guard let identifier = tabViewItem?.identifier as? String else { return }

        switch identifier {
        case "graph":
            if !loadedTabs.contains(identifier) {
                loadedTabs.insert(identifier)
                refreshGraph()
            } else if !isGraphRefreshing {
                refreshGraph()
            }
            startGraphAutoRefresh()
        case "memories":
            if !loadedTabs.contains(identifier) {
                loadedTabs.insert(identifier)
                memoryStatusLabel.stringValue = "Loading recent memories…"
                refreshMemories()
            }
            stopGraphAutoRefresh()
        case "topics":
            if !loadedTabs.contains(identifier) {
                loadedTabs.insert(identifier)
                topicsStatusLabel.stringValue = "Analyzing topics…"
                refreshTopics()
            }
            stopGraphAutoRefresh()
        case "hierarchy":
            if !loadedTabs.contains(identifier) {
                loadedTabs.insert(identifier)
                hierarchyStatusLabel.stringValue = "Loading hierarchy…"
                refreshHierarchy()
            }
            stopGraphAutoRefresh()
        case "connections":
            if !loadedTabs.contains(identifier) {
                loadedTabs.insert(identifier)
            }
            refreshConnections()
            stopGraphAutoRefresh()
        default:
            stopGraphAutoRefresh()
        }
    }

    private func startGraphAutoRefresh() {
        graphRefreshTimer?.invalidate()
        graphRefreshTimer = Timer.scheduledTimer(withTimeInterval: 10.0, repeats: true) { [weak self] _ in
            self?.refreshGraph()
        }
        if let timer = graphRefreshTimer {
            RunLoop.main.add(timer, forMode: .common)
        }
    }

    private func stopGraphAutoRefresh() {
        graphRefreshTimer?.invalidate()
        graphRefreshTimer = nil
    }

    deinit {
        graphRefreshTimer?.invalidate()
    }

    // MARK: - Animation Helpers

    private func fadeTransition(to newValue: String, label: NSTextField, duration: TimeInterval = 0.2) {
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = duration / 2
            label.animator().alphaValue = 0.0
        }, completionHandler: {
            label.stringValue = newValue
            NSAnimationContext.runAnimationGroup { context in
                context.duration = duration / 2
                label.animator().alphaValue = 1.0
            }
        })
    }

    // MARK: - NSWindowDelegate

    func windowWillClose(_ notification: Notification) {
        stopGraphAutoRefresh()
    }
}

final class KnowledgeGraphWebView: WKWebView, WKNavigationDelegate {
    private var pendingBase64Graph: String?
    private var isContentLoaded = false

    init() {
        let configuration = WKWebViewConfiguration()
        configuration.preferences.javaScriptEnabled = true
        configuration.preferences.setValue(true, forKey: "developerExtrasEnabled")
        super.init(frame: .zero, configuration: configuration)

        translatesAutoresizingMaskIntoConstraints = false
        navigationDelegate = self

        // WKWebView doesn't support isOpaque/backgroundColor directly
        // The background will be transparent through the web content
        setValue(false, forKey: "drawsBackground")

        loadGraphShell()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func render(graph: AgenticGraph) {
        guard let data = try? JSONEncoder().encode(graph) else { return }
        let base64 = data.base64EncodedString()
        let script = "window.renderGraph(JSON.parse(atob('\(base64)')));"

        if isContentLoaded {
            evaluateJavaScript(script, completionHandler: nil)
        } else {
            pendingBase64Graph = base64
        }
    }

    private func loadGraphShell() {
        // Use Bundle.module for Swift Package resources
        guard let url = Bundle.module.url(forResource: "Resources/KnowledgeGraph/index", withExtension: "html") else {
            // Load a fallback HTML if the resource is missing
            loadFallbackGraph()
            return
        }
        let directory = url.deletingLastPathComponent()
        loadFileURL(url, allowingReadAccessTo: directory)
    }

    private func loadFallbackGraph() {
        // Minimal fallback HTML with inline graph rendering
        let html = """
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <title>Knowledge Graph</title>
            <style>
                body { margin: 0; padding: 20px; font-family: system-ui; background: transparent; }
                #graph { width: 100%; height: 600px; border: 1px solid #ccc; border-radius: 8px; }
            </style>
        </head>
        <body>
            <div id="graph">
                <p>Knowledge Graph visualization will appear here.</p>
                <p>Install graph resources or check console for details.</p>
            </div>
            <script>
                window.renderGraph = function(data) {
                    console.log('Graph data:', data);
                    document.getElementById('graph').innerHTML = '<pre>' + JSON.stringify(data, null, 2) + '</pre>';
                };
            </script>
        </body>
        </html>
        """
        loadHTMLString(html, baseURL: nil)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        isContentLoaded = true

        if let base64 = pendingBase64Graph {
            let script = "window.renderGraph(JSON.parse(atob('\(base64)')));"
            evaluateJavaScript(script, completionHandler: nil)
            pendingBase64Graph = nil
        }
    }
}
