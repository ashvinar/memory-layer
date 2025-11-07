import Cocoa

// MARK: - Colors

extension NSColor {
    // Brand
    static let mlPrimary = NSColor(calibratedRed: 0.24, green: 0.52, blue: 0.96, alpha: 1.0) // #3D85F5
    static let mlPrimaryLight = NSColor(calibratedRed: 0.4, green: 0.6, blue: 1.0, alpha: 1.0)
    static let mlPrimaryDark = NSColor(calibratedRed: 0.18, green: 0.42, blue: 0.86, alpha: 1.0)

    // Semantic
    static let mlSuccess = NSColor.systemGreen
    static let mlWarning = NSColor.systemOrange
    static let mlError = NSColor.systemRed
    static let mlInfo = NSColor.systemBlue

    // Neutrals (dynamic for dark mode)
    static let mlBackground = NSColor.controlBackgroundColor
    static let mlSurface = NSColor.textBackgroundColor
    static let mlBorder = NSColor.separatorColor
    static let mlDivider = NSColor.separatorColor.withAlphaComponent(0.5)

    // Text (dynamic)
    static let mlTextPrimary = NSColor.labelColor
    static let mlTextSecondary = NSColor.secondaryLabelColor
    static let mlTextTertiary = NSColor.tertiaryLabelColor
    static let mlTextDisabled = NSColor.quaternaryLabelColor

    // Status Colors
    static let mlStatusRunning = NSColor.systemGreen
    static let mlStatusStopped = NSColor.systemRed
    static let mlStatusIdle = NSColor.systemOrange
    static let mlStatusUnknown = NSColor.systemGray

    // Memory Lifecycle Colors
    static let mlFleetingColor = NSColor.systemYellow
    static let mlPermanentColor = NSColor.systemGreen
    static let mlArchivedColor = NSColor.systemGray
    static let mlDeprecatedColor = NSColor.systemRed
}

// MARK: - Typography

extension NSFont {
    // Display (titles, headers)
    static let mlDisplay1 = NSFont.systemFont(ofSize: 28, weight: .bold)
    static let mlDisplay2 = NSFont.systemFont(ofSize: 24, weight: .semibold)
    static let mlDisplay3 = NSFont.systemFont(ofSize: 20, weight: .semibold)

    // Headings
    static let mlHeading1 = NSFont.systemFont(ofSize: 17, weight: .semibold)
    static let mlHeading2 = NSFont.systemFont(ofSize: 15, weight: .semibold)
    static let mlHeading3 = NSFont.systemFont(ofSize: 13, weight: .semibold)

    // Body Text
    static let mlBody1 = NSFont.systemFont(ofSize: 15, weight: .regular)
    static let mlBody2 = NSFont.systemFont(ofSize: 13, weight: .regular)
    static let mlBody3 = NSFont.systemFont(ofSize: 11, weight: .regular)

    // Emphasis
    static let mlBodyMedium = NSFont.systemFont(ofSize: 13, weight: .medium)
    static let mlBodySemibold = NSFont.systemFont(ofSize: 13, weight: .semibold)
    static let mlBodyBold = NSFont.systemFont(ofSize: 13, weight: .bold)

    // Code & Monospace
    static let mlMono = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    static let mlMonoSmall = NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)

    // Captions & Labels
    static let mlCaption = NSFont.systemFont(ofSize: 11, weight: .regular)
    static let mlLabel = NSFont.systemFont(ofSize: 12, weight: .medium)
}

// MARK: - Spacing

enum MLSpacing: CGFloat {
    case tiny = 4
    case small = 8
    case medium = 16
    case large = 24
    case xlarge = 32
    case xxlarge = 48
    case xxxlarge = 64
}

// MARK: - Corner Radius

enum MLCornerRadius: CGFloat {
    case small = 4
    case medium = 8
    case large = 12
    case xlarge = 16
    case circle = 9999
}

// MARK: - Shadows

enum MLShadowStyle {
    case none
    case small
    case medium
    case large
}

extension CALayer {
    func applyMLShadow(style: MLShadowStyle) {
        switch style {
        case .none:
            shadowOpacity = 0
        case .small:
            shadowColor = NSColor.black.cgColor
            shadowOpacity = 0.1
            shadowOffset = CGSize(width: 0, height: 1)
            shadowRadius = 2
        case .medium:
            shadowColor = NSColor.black.cgColor
            shadowOpacity = 0.15
            shadowOffset = CGSize(width: 0, height: 2)
            shadowRadius = 4
        case .large:
            shadowColor = NSColor.black.cgColor
            shadowOpacity = 0.2
            shadowOffset = CGSize(width: 0, height: 4)
            shadowRadius = 8
        }
    }
}

extension NSView {
    func applyMLShadow(style: MLShadowStyle) {
        wantsLayer = true
        layer?.applyMLShadow(style: style)
    }
}

// MARK: - Table Styling

struct MLTableStyle {
    static let rowHeight: CGFloat = 56
    static let headerHeight: CGFloat = 28
    static let gridColor = NSColor.mlBorder
    static let selectionColor = NSColor.selectedContentBackgroundColor
    static let alternatingRowColors = false
}

// MARK: - Badge Styling

class MLBadgeView: NSView {
    enum BadgeStyle {
        case success
        case warning
        case error
        case info
        case neutral
        case custom(backgroundColor: NSColor, textColor: NSColor)
    }

    private let label = NSTextField()
    private let iconView = NSImageView()
    private var style: BadgeStyle = .neutral

    var text: String = "" {
        didSet {
            label.stringValue = text
        }
    }

    var icon: NSImage? {
        didSet {
            iconView.image = icon
            iconView.isHidden = (icon == nil)
        }
    }

    init(text: String, style: BadgeStyle = .neutral, icon: NSImage? = nil) {
        super.init(frame: .zero)
        self.text = text
        self.style = style
        self.icon = icon
        setupView()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }

    private func setupView() {
        wantsLayer = true
        layer?.cornerRadius = MLCornerRadius.small.rawValue

        // Configure label
        label.isBordered = false
        label.isEditable = false
        label.drawsBackground = false
        label.font = .mlCaption
        label.alignment = .center
        label.stringValue = text

        // Configure icon
        iconView.imageScaling = .scaleProportionallyDown
        iconView.image = icon
        iconView.isHidden = (icon == nil)

        addSubview(iconView)
        addSubview(label)

        applyStyle()
        setupConstraints()
    }

    private func applyStyle() {
        let (bgColor, textColor): (NSColor, NSColor) = {
            switch style {
            case .success:
                return (NSColor.mlSuccess.withAlphaComponent(0.15), NSColor.mlSuccess)
            case .warning:
                return (NSColor.mlWarning.withAlphaComponent(0.15), NSColor.mlWarning)
            case .error:
                return (NSColor.mlError.withAlphaComponent(0.15), NSColor.mlError)
            case .info:
                return (NSColor.mlInfo.withAlphaComponent(0.15), NSColor.mlInfo)
            case .neutral:
                return (NSColor.mlBorder, NSColor.mlTextSecondary)
            case .custom(let bg, let text):
                return (bg, text)
            }
        }()

        layer?.backgroundColor = bgColor.cgColor
        label.textColor = textColor
        iconView.contentTintColor = textColor
    }

    private func setupConstraints() {
        label.translatesAutoresizingMaskIntoConstraints = false
        iconView.translatesAutoresizingMaskIntoConstraints = false

        if icon != nil {
            NSLayoutConstraint.activate([
                iconView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: MLSpacing.small.rawValue),
                iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
                iconView.widthAnchor.constraint(equalToConstant: 12),
                iconView.heightAnchor.constraint(equalToConstant: 12),

                label.leadingAnchor.constraint(equalTo: iconView.trailingAnchor, constant: MLSpacing.tiny.rawValue),
                label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -MLSpacing.small.rawValue),
                label.centerYAnchor.constraint(equalTo: centerYAnchor),

                heightAnchor.constraint(equalToConstant: 20)
            ])
        } else {
            NSLayoutConstraint.activate([
                label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: MLSpacing.small.rawValue),
                label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -MLSpacing.small.rawValue),
                label.centerYAnchor.constraint(equalTo: centerYAnchor),

                heightAnchor.constraint(equalToConstant: 20)
            ])
        }
    }
}

// MARK: - Animation Helpers

extension NSView {
    func fadeIn(duration: TimeInterval = 0.3) {
        NSAnimationContext.runAnimationGroup { context in
            context.duration = duration
            self.animator().alphaValue = 1.0
        }
    }

    func fadeOut(duration: TimeInterval = 0.3) {
        NSAnimationContext.runAnimationGroup { context in
            context.duration = duration
            self.animator().alphaValue = 0.0
        }
    }

    func fadeTransition(to newValue: String, label: NSTextField, duration: TimeInterval = 0.2) {
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
}

// MARK: - Progress Indicator Styling

extension NSProgressIndicator {
    static func mlSpinner() -> NSProgressIndicator {
        let spinner = NSProgressIndicator()
        spinner.style = .spinning
        spinner.controlSize = .small
        spinner.isIndeterminate = true
        return spinner
    }
}

// MARK: - Empty State View

class MLEmptyStateView: NSView {
    private let iconView = NSImageView()
    private let titleLabel = NSTextField()
    private let messageLabel = NSTextField()

    init(icon: NSImage, title: String, message: String) {
        super.init(frame: .zero)

        // Icon
        iconView.image = icon
        iconView.contentTintColor = .mlTextTertiary
        iconView.imageScaling = .scaleProportionallyDown

        // Title
        titleLabel.stringValue = title
        titleLabel.font = .mlHeading2
        titleLabel.textColor = .mlTextPrimary
        titleLabel.alignment = .center
        titleLabel.isBordered = false
        titleLabel.isEditable = false
        titleLabel.drawsBackground = false

        // Message
        messageLabel.stringValue = message
        messageLabel.font = .mlBody2
        messageLabel.textColor = .mlTextSecondary
        messageLabel.alignment = .center
        messageLabel.isBordered = false
        messageLabel.isEditable = false
        messageLabel.drawsBackground = false
        messageLabel.maximumNumberOfLines = 2
        messageLabel.lineBreakMode = .byWordWrapping

        addSubview(iconView)
        addSubview(titleLabel)
        addSubview(messageLabel)

        setupConstraints()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupConstraints() {
        iconView.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        messageLabel.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            iconView.centerXAnchor.constraint(equalTo: centerXAnchor),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor, constant: -40),
            iconView.widthAnchor.constraint(equalToConstant: 64),
            iconView.heightAnchor.constraint(equalToConstant: 64),

            titleLabel.topAnchor.constraint(equalTo: iconView.bottomAnchor, constant: MLSpacing.medium.rawValue),
            titleLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            titleLabel.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: MLSpacing.large.rawValue),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -MLSpacing.large.rawValue),

            messageLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: MLSpacing.small.rawValue),
            messageLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            messageLabel.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: MLSpacing.large.rawValue),
            messageLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -MLSpacing.large.rawValue),
        ])
    }
}
