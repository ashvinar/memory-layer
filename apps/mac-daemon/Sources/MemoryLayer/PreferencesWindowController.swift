import Cocoa

class PreferencesWindowController: NSWindowController, NSWindowDelegate {
    init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 500, height: 400),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )

        super.init(window: window)

        window.title = "Memory Layer Preferences"
        window.delegate = self
        window.center()

        setupUI()
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
        titleLabel.stringValue = "Memory Layer"
        titleLabel.font = .systemFont(ofSize: 20, weight: .bold)
        titleLabel.isEditable = false
        titleLabel.isBordered = false
        titleLabel.backgroundColor = .clear
        contentView.addSubview(titleLabel)

        yPosition -= 40

        // Subtitle
        let subtitleLabel = NSTextField(frame: NSRect(
            x: margin,
            y: yPosition,
            width: contentView.bounds.width - (margin * 2),
            height: 20
        ))
        subtitleLabel.stringValue = "Reads text. Never screenshots."
        subtitleLabel.font = .systemFont(ofSize: 12)
        subtitleLabel.textColor = .secondaryLabelColor
        subtitleLabel.isEditable = false
        subtitleLabel.isBordered = false
        subtitleLabel.backgroundColor = .clear
        contentView.addSubview(subtitleLabel)

        yPosition -= 40

        // Settings sections
        addSection(
            title: "Service Status",
            content: """
            Composer: http://127.0.0.1:21955
            Indexing: http://127.0.0.1:21954
            Ingestion: http://127.0.0.1:21953

            Use "Test Context Generation" from the menu bar to verify.
            """,
            yPosition: &yPosition,
            in: contentView
        )

        addSection(
            title: "Privacy",
            content: """
            • Text capture only (no screenshots)
            • All data stored locally in SQLite
            • Database: ~/Library/Application Support/MemoryLayer/
            • No cloud sync (local-only)
            """,
            yPosition: &yPosition,
            in: contentView
        )

        addSection(
            title: "Keyboard Shortcuts",
            content: """
            ⌘⌥K - Search Memories
            ⌘, - Preferences
            """,
            yPosition: &yPosition,
            in: contentView
        )

        // Version info at bottom
        let versionLabel = NSTextField(frame: NSRect(
            x: margin,
            y: 10,
            width: contentView.bounds.width - (margin * 2),
            height: 20
        ))
        versionLabel.stringValue = "Version 0.1.0 - MVP"
        versionLabel.font = .systemFont(ofSize: 10)
        versionLabel.textColor = .tertiaryLabelColor
        versionLabel.alignment = .center
        versionLabel.isEditable = false
        versionLabel.isBordered = false
        versionLabel.backgroundColor = .clear
        contentView.addSubview(versionLabel)
    }

    private func addSection(title: String, content: String, yPosition: inout CGFloat, in view: NSView) {
        let margin: CGFloat = 20

        // Section title
        let titleLabel = NSTextField(frame: NSRect(
            x: margin,
            y: yPosition,
            width: view.bounds.width - (margin * 2),
            height: 20
        ))
        titleLabel.stringValue = title
        titleLabel.font = .systemFont(ofSize: 13, weight: .semibold)
        titleLabel.isEditable = false
        titleLabel.isBordered = false
        titleLabel.backgroundColor = .clear
        view.addSubview(titleLabel)

        yPosition -= 25

        // Section content
        let contentLabel = NSTextField(frame: NSRect(
            x: margin + 10,
            y: yPosition - 60,
            width: view.bounds.width - (margin * 2) - 10,
            height: 80
        ))
        contentLabel.stringValue = content
        contentLabel.font = .systemFont(ofSize: 11)
        contentLabel.textColor = .secondaryLabelColor
        contentLabel.isEditable = false
        contentLabel.isBordered = false
        contentLabel.backgroundColor = .clear
        contentLabel.maximumNumberOfLines = 0
        contentLabel.cell?.wraps = true
        contentLabel.cell?.isScrollable = false
        view.addSubview(contentLabel)

        yPosition -= 90
    }
}
