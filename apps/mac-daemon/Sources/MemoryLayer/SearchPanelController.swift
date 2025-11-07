import Cocoa

class SearchPanelController: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate {
    private var searchField: NSSearchField!
    private var tableView: NSTableView!
    private var scrollView: NSScrollView!
    private var statusLabel: NSTextField!
    private let providerClient: ProviderClient
    private var searchDebounceTimer: Timer?

    private var results: [SearchResult] = [] {
        didSet {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.2
                tableView?.animator().reloadData()
            }
            updateStatusLabel()
        }
    }

    private var isSearching: Bool = false {
        didSet {
            updateStatusLabel()
        }
    }

    init(providerClient: ProviderClient) {
        self.providerClient = providerClient

        // Create the panel window
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 600, height: 400),
            styleMask: [.titled, .closable, .resizable, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        super.init(window: panel)

        panel.title = "Memory Search"
        panel.level = .floating
        panel.isMovableByWindowBackground = true
        panel.delegate = self
        panel.center()

        setupUI()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupUI() {
        guard let contentView = window?.contentView else { return }

        // Search field at top
        searchField = NSSearchField()
        searchField.translatesAutoresizingMaskIntoConstraints = false
        searchField.placeholderString = "Search memories, code snippets, conversations..."
        searchField.font = .mlBody2
        searchField.delegate = self
        contentView.addSubview(searchField)

        // Status label below search field
        statusLabel = NSTextField(labelWithString: "Type to search...")
        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.font = .mlCaption
        statusLabel.textColor = .mlTextSecondary
        contentView.addSubview(statusLabel)

        // Table view for results
        scrollView = NSScrollView()
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.hasVerticalScroller = true
        scrollView.borderType = .noBorder

        tableView = NSTableView()
        tableView.headerView = nil
        tableView.usesAlternatingRowBackgroundColors = false
        tableView.rowHeight = MLTableStyle.rowHeight
        tableView.gridColor = MLTableStyle.gridColor

        // Create columns
        let textColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("text"))
        textColumn.title = "Result"
        textColumn.width = 400

        let scoreColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("score"))
        scoreColumn.title = "Score"
        scoreColumn.width = 120

        tableView.addTableColumn(textColumn)
        tableView.addTableColumn(scoreColumn)

        tableView.dataSource = self
        tableView.delegate = self
        tableView.target = self
        tableView.doubleAction = #selector(tableViewDoubleClick)

        scrollView.documentView = tableView
        contentView.addSubview(scrollView)

        // Apply Auto Layout constraints
        NSLayoutConstraint.activate([
            searchField.topAnchor.constraint(equalTo: contentView.topAnchor, constant: MLSpacing.medium.rawValue),
            searchField.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: MLSpacing.medium.rawValue),
            searchField.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -MLSpacing.medium.rawValue),
            searchField.heightAnchor.constraint(equalToConstant: 32),

            statusLabel.topAnchor.constraint(equalTo: searchField.bottomAnchor, constant: MLSpacing.small.rawValue),
            statusLabel.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: MLSpacing.medium.rawValue),
            statusLabel.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -MLSpacing.medium.rawValue),

            scrollView.topAnchor.constraint(equalTo: statusLabel.bottomAnchor, constant: MLSpacing.medium.rawValue),
            scrollView.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: MLSpacing.medium.rawValue),
            scrollView.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -MLSpacing.medium.rawValue),
            scrollView.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -MLSpacing.medium.rawValue)
        ])
    }

    private func updateStatusLabel() {
        if isSearching {
            statusLabel.stringValue = "Searching..."
        } else if results.isEmpty {
            statusLabel.stringValue = searchField.stringValue.isEmpty ? "Type to search..." : "No results found"
        } else {
            statusLabel.stringValue = "\(results.count) result\(results.count == 1 ? "" : "s") found"
        }
    }

    // MARK: - NSSearchFieldDelegate

    func controlTextDidChange(_ obj: Notification) {
        guard let searchField = obj.object as? NSSearchField else { return }
        let query = searchField.stringValue

        // Cancel any pending search
        searchDebounceTimer?.invalidate()

        if query.isEmpty {
            results = []
            return
        }

        // Debounce search by 300ms to prevent excessive API calls
        searchDebounceTimer = Timer.scheduledTimer(withTimeInterval: 0.3, repeats: false) { [weak self] _ in
            self?.performSearch(query: query)
        }
    }

    private func performSearch(query: String) {
        isSearching = true

        Task {
            do {
                let searchResults = try await providerClient.search(query: query, limit: 20)
                await MainActor.run {
                    self.results = searchResults
                    self.isSearching = false
                }
            } catch {
                await MainActor.run {
                    self.results = []
                    self.isSearching = false
                    self.statusLabel.stringValue = "Search failed: \(error.localizedDescription)"
                }
            }
        }
    }

    // MARK: - NSTableViewDataSource

    func numberOfRows(in tableView: NSTableView) -> Int {
        return results.count
    }

    // MARK: - NSTableViewDelegate

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let result = results[row]

        if tableColumn?.identifier.rawValue == "text" {
            // Display memory text with topic badge
            let cell = NSTableCellView()
            cell.identifier = NSUserInterfaceItemIdentifier("textCell")

            let memory = result.memory
            var displayText = memory.text

            // Truncate if too long
            if displayText.count > 100 {
                displayText = String(displayText.prefix(97)) + "..."
            }

            // Topic badge
            if !memory.topic.isEmpty {
                let topicBadge = MLBadgeView(text: memory.topic, style: .neutral, icon: nil)
                topicBadge.translatesAutoresizingMaskIntoConstraints = false
                cell.addSubview(topicBadge)

                NSLayoutConstraint.activate([
                    topicBadge.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: MLSpacing.small.rawValue),
                    topicBadge.topAnchor.constraint(equalTo: cell.topAnchor, constant: MLSpacing.small.rawValue)
                ])
            }

            // Memory text
            let textLabel = NSTextField(labelWithString: displayText)
            textLabel.translatesAutoresizingMaskIntoConstraints = false
            textLabel.lineBreakMode = .byTruncatingTail
            textLabel.font = .mlBody2
            textLabel.textColor = .mlTextPrimary
            cell.addSubview(textLabel)

            let topOffset = memory.topic.isEmpty ? MLSpacing.small.rawValue : 28
            NSLayoutConstraint.activate([
                textLabel.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: MLSpacing.small.rawValue),
                textLabel.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -MLSpacing.small.rawValue),
                textLabel.topAnchor.constraint(equalTo: cell.topAnchor, constant: topOffset)
            ])

            return cell

        } else if tableColumn?.identifier.rawValue == "score" {
            // Display score as progress bar
            let cell = NSTableCellView()
            cell.identifier = NSUserInterfaceItemIdentifier("scoreCell")

            // Normalize score to 0-1 range (assuming scores are 0-1)
            let normalizedScore = min(max(result.score, 0), 1)

            // Container for progress bar
            let progressContainer = NSView()
            progressContainer.translatesAutoresizingMaskIntoConstraints = false
            progressContainer.wantsLayer = true
            progressContainer.layer?.backgroundColor = NSColor.mlBorder.cgColor
            progressContainer.layer?.cornerRadius = MLCornerRadius.small.rawValue
            cell.addSubview(progressContainer)

            // Progress fill
            let progressFill = NSView()
            progressFill.translatesAutoresizingMaskIntoConstraints = false
            progressFill.wantsLayer = true

            // Color based on score
            let fillColor: NSColor = {
                if normalizedScore >= 0.8 { return .mlSuccess }
                else if normalizedScore >= 0.5 { return .mlPrimary }
                else if normalizedScore >= 0.3 { return .mlWarning }
                else { return .mlError }
            }()
            progressFill.layer?.backgroundColor = fillColor.cgColor
            progressFill.layer?.cornerRadius = MLCornerRadius.small.rawValue
            progressContainer.addSubview(progressFill)

            // Score label
            let scoreLabel = NSTextField(labelWithString: String(format: "%.0f%%", normalizedScore * 100))
            scoreLabel.translatesAutoresizingMaskIntoConstraints = false
            scoreLabel.font = .mlCaption
            scoreLabel.textColor = .mlTextSecondary
            scoreLabel.alignment = .right
            cell.addSubview(scoreLabel)

            NSLayoutConstraint.activate([
                progressContainer.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: MLSpacing.small.rawValue),
                progressContainer.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
                progressContainer.widthAnchor.constraint(equalToConstant: 60),
                progressContainer.heightAnchor.constraint(equalToConstant: 8),

                progressFill.leadingAnchor.constraint(equalTo: progressContainer.leadingAnchor),
                progressFill.topAnchor.constraint(equalTo: progressContainer.topAnchor),
                progressFill.bottomAnchor.constraint(equalTo: progressContainer.bottomAnchor),
                progressFill.widthAnchor.constraint(equalTo: progressContainer.widthAnchor, multiplier: normalizedScore),

                scoreLabel.leadingAnchor.constraint(equalTo: progressContainer.trailingAnchor, constant: MLSpacing.small.rawValue),
                scoreLabel.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -MLSpacing.small.rawValue),
                scoreLabel.centerYAnchor.constraint(equalTo: cell.centerYAnchor)
            ])

            return cell
        }

        return nil
    }

    @objc private func tableViewDoubleClick() {
        let row = tableView.clickedRow
        guard row >= 0 && row < results.count else { return }

        let result = results[row]
        showMemoryDetail(result.memory)
    }

    private func showMemoryDetail(_ memory: Memory) {
        let alert = NSAlert()
        alert.messageText = memory.topic
        alert.informativeText = """
        Type: \(memory.kind)
        Created: \(memory.createdAt)

        \(memory.text)

        Entities: \(memory.entities.joined(separator: ", "))
        """
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Copy Text")

        let response = alert.runModal()
        if response == .alertSecondButtonReturn {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(memory.text, forType: .string)
        }
    }

    // MARK: - NSWindowDelegate

    func windowWillClose(_ notification: Notification) {
        // Clear search when window closes
        searchField.stringValue = ""
        results = []
    }

    override func showWindow(_ sender: Any?) {
        super.showWindow(sender)
        window?.makeKeyAndOrderFront(nil)
        searchField.becomeFirstResponder()
    }
}
